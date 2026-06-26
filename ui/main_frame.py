"""메인 창 — URL 입력, 포맷/화질, 진행률, 히스토리.

다운로드는 별도 스레드에서 돌리고 UI 갱신은 wx.CallAfter로 메인 스레드에 위임한다
(wxPython은 메인 스레드에서만 위젯을 건드릴 수 있다).

테마(라이트/다크)·언어(한국어/영어)는 상단 우측 토글에서 즉시 전환하며 config에
영속화된다. 테마는 위젯을 파괴하지 않고 in-place로 재색칠하고, 언어는 wx.RadioButton
등의 런타임 라벨 변경 제약 때문에 패널을 재생성(상태 보존)해 다시 그린다.
"""

import os
import re
import sys
import threading
from datetime import datetime

import wx

from core import __version__
from core import winapi
from core.binaries import ensure_binaries
from core.config import load_config, save_config
from core.downloader import download_with_auto_update
from core.history import add_history, load_history
from core.i18n import tr

_QUALITIES = ["best", "1080p", "720p"]

# yt-dlp 출력에서 현재 받는 파일명을 추출한다.
_DEST_RE = re.compile(r"\[download\] Destination: (.+)")


def _resource_path(name):
	# PyInstaller로 묶이면 리소스는 임시폴더(sys._MEIPASS)에 풀린다.
	# 개발 모드에서는 ui/의 부모인 프로젝트 루트에서 찾는다.
	base = getattr(sys, "_MEIPASS", None)
	if base is None:
		base = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
	return os.path.join(base, name)


class _ReadOnlyText(wx.TextCtrl):
	# 읽기 전용 경로 표시 필드 — 키보드 탭 순서에서는 건너뛰되,
	# 마우스 클릭 포커스(경로 드래그·복사)는 그대로 둔다.
	def AcceptsFocusFromKeyboard(self):
		return False


class _ProgressBar(wx.Panel):
	# 네이티브 wx.Gauge는 다크 테마에서 흰 트랙이 남는다. 트랙/채움을 직접 그려
	# 라이트·다크 양쪽에서 일관된 진행바를 만든다. wx.Gauge와 동일한
	# SetValue/GetValue(0~100) 인터페이스라 워커 코드는 그대로 쓸 수 있다.
	def __init__(self, parent, height=16):
		super().__init__(parent, size=(-1, height))
		self.SetBackgroundStyle(wx.BG_STYLE_PAINT)
		self._value = 0
		self._trough = wx.Colour(225, 225, 225)
		self._fill = wx.Colour(0, 120, 215)
		self.Bind(wx.EVT_PAINT, self._on_paint)
		self.Bind(wx.EVT_SIZE, lambda event: self.Refresh())

	def SetValue(self, value):
		value = max(0, min(100, int(value)))
		if value != self._value:
			self._value = value
			self.Refresh()

	def GetValue(self):
		return self._value

	def set_colours(self, trough, fill):
		self._trough = trough
		self._fill = fill
		self.Refresh()

	def _on_paint(self, event):
		dc = wx.AutoBufferedPaintDC(self)
		width, height = self.GetClientSize()
		dc.SetPen(wx.TRANSPARENT_PEN)
		dc.SetBrush(wx.Brush(self._trough))
		dc.DrawRectangle(0, 0, width, height)
		if self._value > 0:
			fill_w = int(width * self._value / 100)
			dc.SetBrush(wx.Brush(self._fill))
			dc.DrawRectangle(0, 0, fill_w, height)


class MainFrame(wx.Frame):
	def __init__(self):
		super().__init__(None, title=f"XDownloader v{__version__}", size=(660, 600))
		self.config = load_config()
		self.lang = self.config.get("lang", "ko")
		self.theme = self._init_theme()
		self.cancel_event = None
		self.worker = None
		self._proc = None
		self._lang_enabled = True
		# 컨트롤 생성 전에 앱 다크모드를 설정해야 스크롤바 등이 처음부터 다크로 뜬다.
		winapi.enable_app_dark_mode(self.theme == "dark")
		self._set_icon()
		self._build_ui()
		self.Bind(wx.EVT_CLOSE, self._on_close)
		self.Bind(wx.EVT_CHAR_HOOK, self._on_char_hook)
		# 타이틀바 다크는 창이 실제로 표시된 뒤 재적용해야 wx가 덮어쓰지 않는다.
		self.Bind(wx.EVT_SHOW, self._on_show)
		# 창을 이보다 더 줄이면 컨트롤이 겹치므로 최소 크기를 고정한다.
		self.SetMinSize((560, 520))
		self.Centre()

	def _t(self, key):
		return tr(self.lang, key)

	def _init_theme(self):
		# 저장된 테마가 있으면 그대로, 없으면(최초 실행) OS 다크모드 설정을 따른다
		# — DESIGN.md "OS 시스템 색을 따른다" 원칙을 초기값에 반영한다.
		theme = self.config.get("theme")
		if theme in ("light", "dark"):
			return theme
		dark = False
		try:
			dark = wx.SystemSettings.GetAppearance().IsDark()
		except Exception:
			dark = False
		theme = "dark" if dark else "light"
		self.config["theme"] = theme
		save_config(self.config)
		return theme

	def _set_icon(self):
		# 멀티 해상도 .ico를 IconBundle로 한 번에 등록하면 타이틀바·작업표시줄이
		# 각각 최적 크기를 고른다. 아이콘 로드 실패는 앱 실행을 막지 않는다.
		try:
			path = _resource_path("xdownload.ico")
			if os.path.exists(path):
				self.SetIcons(wx.IconBundle(path, wx.BITMAP_TYPE_ICO))
		except Exception:
			pass

	def _make_toggle_label(self, text, handler):
		# 클릭 가능한 텍스트 토글 — 손가락 커서 + 좌클릭 핸들러.
		label = wx.StaticText(self.panel, label=text)
		label.SetCursor(wx.Cursor(wx.CURSOR_HAND))
		label.Bind(wx.EVT_LEFT_DOWN, handler)
		return label

	def _build_ui(self):
		self.panel = wx.Panel(self)
		root = wx.BoxSizer(wx.VERTICAL)

		# YouTube 링크 라벨 줄 우측에 토글(한/En + 다크 아이콘)을 함께 둔다.
		url_row = wx.BoxSizer(wx.HORIZONTAL)
		# 섹션 헤더는 볼드로 본문과 위계를 둔다(다운로드 버튼과 동일한 강조 패턴).
		self.url_label = wx.StaticText(self.panel, label=self._t("url_label"))
		self.url_label.SetFont(self.url_label.GetFont().Bold())
		url_row.Add(self.url_label, 0, wx.ALIGN_CENTER_VERTICAL)
		url_row.AddStretchSpacer()
		self.lang_ko_lbl = self._make_toggle_label("한", self._on_click_ko)
		self.lang_sep = wx.StaticText(self.panel, label="|")
		self.lang_en_lbl = self._make_toggle_label("En", self._on_click_en)
		self.theme_btn = self._make_toggle_label("☾", self._on_click_theme)
		self.theme_btn.SetToolTip(self._t("tip_theme"))
		icon_font = self.theme_btn.GetFont()
		icon_font.SetPointSize(icon_font.GetPointSize() + 3)
		self.theme_btn.SetFont(icon_font)
		url_row.Add(self.lang_ko_lbl, 0, wx.ALIGN_CENTER_VERTICAL)
		url_row.Add(
			self.lang_sep, 0, wx.ALIGN_CENTER_VERTICAL | wx.LEFT | wx.RIGHT, 4
		)
		url_row.Add(self.lang_en_lbl, 0, wx.ALIGN_CENTER_VERTICAL)
		url_row.Add(self.theme_btn, 0, wx.ALIGN_CENTER_VERTICAL | wx.LEFT, 16)
		root.Add(url_row, 0, wx.EXPAND | wx.LEFT | wx.RIGHT | wx.TOP, 12)
		self.url_text = wx.TextCtrl(self.panel, style=wx.TE_MULTILINE, size=(-1, 90))
		root.Add(self.url_text, 0, wx.EXPAND | wx.LEFT | wx.RIGHT, 12)

		row = wx.BoxSizer(wx.HORIZONTAL)
		# 포맷 — RadioBox 대신 StaticBox+RadioButton로 구성한다. RadioBox의 항목
		# 텍스트는 MSW에서 색 지정이 무시돼 다크 모드에서 검게 남는 반면,
		# RadioButton 라벨은 색을 존중하므로 다크에서도 밝게 보인다.
		fbox = wx.StaticBoxSizer(wx.VERTICAL, self.panel, self._t("format_box"))
		self.fbox_box = fbox.GetStaticBox()
		self.rb_mp4 = wx.RadioButton(
			self.fbox_box, label=self._t("format_mp4"), style=wx.RB_GROUP
		)
		self.rb_mp3 = wx.RadioButton(self.fbox_box, label=self._t("format_mp3"))
		if self.config.get("format") == "mp3":
			self.rb_mp3.SetValue(True)
		else:
			self.rb_mp4.SetValue(True)
		self.rb_mp4.Bind(wx.EVT_RADIOBUTTON, self._on_format_change)
		self.rb_mp3.Bind(wx.EVT_RADIOBUTTON, self._on_format_change)
		frow = wx.BoxSizer(wx.HORIZONTAL)
		frow.Add(self.rb_mp4, 0, wx.RIGHT, 12)
		frow.Add(self.rb_mp3, 0)
		fbox.AddStretchSpacer()
		fbox.Add(frow, 0, wx.LEFT | wx.RIGHT, 6)
		fbox.AddStretchSpacer()
		row.Add(fbox, 0, wx.EXPAND | wx.RIGHT, 12)

		qbox = wx.StaticBoxSizer(wx.VERTICAL, self.panel, self._t("quality_box"))
		self.qbox_box = qbox.GetStaticBox()
		quality_labels = [
			self._t("quality_best"),
			self._t("quality_1080"),
			self._t("quality_720"),
		]
		self.quality_choice = wx.Choice(self.qbox_box, choices=quality_labels)
		stored_quality = self.config.get("quality", "best")
		self.quality_choice.SetSelection(
			_QUALITIES.index(stored_quality)
			if stored_quality in _QUALITIES
			else 0
		)
		# 드롭다운을 박스 안에서 수직 가운데에 둬 "포맷" 박스와 높이를 맞춘다.
		qbox.AddStretchSpacer()
		qbox.Add(self.quality_choice, 0, wx.LEFT | wx.RIGHT, 6)
		qbox.AddStretchSpacer()
		row.Add(qbox, 0, wx.EXPAND)
		root.Add(row, 0, wx.LEFT | wx.RIGHT | wx.TOP, 12)

		folder_row = wx.BoxSizer(wx.HORIZONTAL)
		self.folder_label = wx.StaticText(self.panel, label=self._t("folder_label"))
		folder_row.Add(
			self.folder_label, 0, wx.ALIGN_CENTER_VERTICAL | wx.RIGHT, 6
		)
		self.folder_text = _ReadOnlyText(
			self.panel, value=self.config["download_dir"], style=wx.TE_READONLY
		)
		folder_row.Add(
			self.folder_text, 1, wx.ALIGN_CENTER_VERTICAL | wx.RIGHT, 6
		)
		self.folder_btn = wx.Button(self.panel, label=self._t("folder_btn"))
		self.folder_btn.Bind(wx.EVT_BUTTON, self._on_pick_folder)
		folder_row.Add(self.folder_btn, 0)
		root.Add(folder_row, 0, wx.EXPAND | wx.LEFT | wx.RIGHT | wx.TOP, 12)

		# 주 액션은 볼드+높이로 시각 비중을 키우고 기본 버튼으로 지정해,
		# 입력창 밖에 포커스가 있을 때 Enter로 바로 실행되게 한다.
		self.download_btn = wx.Button(self.panel, label=self._t("download_btn"))
		self.download_btn.SetFont(self.download_btn.GetFont().Bold())
		self.download_btn.SetMinSize((-1, 40))
		self.download_btn.Bind(wx.EVT_BUTTON, self._on_download)
		self.download_btn.SetDefault()
		root.Add(
			self.download_btn, 0, wx.EXPAND | wx.LEFT | wx.RIGHT | wx.TOP, 12
		)

		self.gauge = _ProgressBar(self.panel)
		root.Add(self.gauge, 0, wx.EXPAND | wx.LEFT | wx.RIGHT | wx.TOP, 12)
		self.status_text = wx.StaticText(self.panel, label=self._t("status_idle"))
		root.Add(self.status_text, 0, wx.LEFT | wx.RIGHT | wx.TOP, 4)

		self.history_label = wx.StaticText(
			self.panel, label=self._t("history_label")
		)
		self.history_label.SetFont(self.history_label.GetFont().Bold())
		root.Add(self.history_label, 0, wx.LEFT | wx.RIGHT | wx.TOP, 12)
		self.history_list = wx.ListBox(self.panel)
		self.history_list.Bind(wx.EVT_LISTBOX_DCLICK, self._on_history_open)
		root.Add(self.history_list, 1, wx.EXPAND | wx.ALL, 12)

		self.panel.SetSizer(root)
		self._on_format_change()
		self._apply_theme()
		self._refresh_history()

	def _palette(self):
		# 다크는 고정 팔레트, 라이트는 OS 시스템 색을 그대로 쓴다(DESIGN.md 원칙).
		if self.theme == "dark":
			return {
				"bg": wx.Colour(30, 30, 30),
				"input_bg": wx.Colour(45, 45, 48),
				"fg": wx.Colour(220, 220, 220),
				"ok": wx.Colour(78, 201, 118),
				"warn": wx.Colour(240, 105, 105),
			}
		get = wx.SystemSettings.GetColour
		return {
			"bg": get(wx.SYS_COLOUR_BTNFACE),
			"input_bg": get(wx.SYS_COLOUR_WINDOW),
			"fg": get(wx.SYS_COLOUR_WINDOWTEXT),
			"ok": wx.Colour(0, 128, 0),
			"warn": wx.Colour(180, 0, 0),
		}

	def _apply_theme(self):
		# 위젯을 파괴하지 않고 현재 테마 색으로 다시 칠한다(다운로드 중에도 안전).
		# Windows 네이티브 한계로 게이지·드롭다운 팝업은 색이 부분만 먹을 수 있다.
		palette = self._palette()
		self._fg = palette["fg"]
		self._ok = palette["ok"]
		self._warn = palette["warn"]
		self.panel.SetBackgroundColour(palette["bg"])
		surface = (
			self.url_label,
			self.fbox_box,
			self.qbox_box,
			self.rb_mp4,
			self.rb_mp3,
			self.folder_label,
			self.folder_btn,
			self.download_btn,
			self.history_label,
			self.status_text,
		)
		for widget in surface:
			widget.SetBackgroundColour(palette["bg"])
			widget.SetForegroundColour(palette["fg"])
		inputs = (
			self.url_text,
			self.folder_text,
			self.quality_choice,
			self.history_list,
		)
		for widget in inputs:
			widget.SetBackgroundColour(palette["input_bg"])
			widget.SetForegroundColour(palette["fg"])
		if self.theme == "dark":
			self.gauge.set_colours(wx.Colour(60, 60, 63), wx.Colour(0, 140, 236))
		else:
			self.gauge.set_colours(wx.Colour(220, 220, 220), wx.Colour(0, 120, 215))
		self._style_toggles(palette)
		self._apply_native_dark()
		self.panel.Refresh()

	def _on_show(self, event):
		event.Skip()
		if event.IsShown():
			# 표시 완료 후 한 박자 뒤 타이틀바 다크를 다시 적용한다.
			wx.CallAfter(
				winapi.set_titlebar_dark, self.GetHandle(), self.theme == "dark"
			)

	def _apply_native_dark(self):
		# wxWidgets 3.2엔 다크 API가 없어 타이틀바·스크롤바·드롭다운 등 네이티브
		# 컨트롤은 Win32(uxtheme/dwmapi)로 직접 다크 테마를 입힌다(best-effort).
		dark = self.theme == "dark"
		winapi.enable_app_dark_mode(dark)
		winapi.set_titlebar_dark(self.GetHandle(), dark)
		for ctrl in (self.url_text, self.folder_text, self.history_list):
			winapi.set_control_dark(ctrl.GetHandle(), dark)
		winapi.set_control_dark(self.quality_choice.GetHandle(), dark, kind="combo")

	def _style_toggles(self, palette):
		# 상단 토글: 현재 언어 강조(볼드+accent), 비활성/다운로드중은 muted.
		accent = (
			wx.Colour(90, 160, 255)
			if self.theme == "dark"
			else wx.Colour(0, 102, 204)
		)
		muted = wx.Colour(140, 140, 140)
		font = self.lang_ko_lbl.GetFont()
		font.SetWeight(wx.FONTWEIGHT_NORMAL)
		bold = font.Bold()
		for label, code in ((self.lang_ko_lbl, "ko"), (self.lang_en_lbl, "en")):
			label.SetFont(bold if code == self.lang else font)
			active = self._lang_enabled and code == self.lang
			label.SetForegroundColour(accent if active else muted)
			label.SetBackgroundColour(palette["bg"])
		self.lang_sep.SetForegroundColour(muted)
		self.lang_sep.SetBackgroundColour(palette["bg"])
		# 다크면 해(클릭→라이트), 라이트면 달(클릭→다크) 아이콘을 보여준다.
		self.theme_btn.SetLabel("☀" if self.theme == "dark" else "☾")
		self.theme_btn.SetForegroundColour(palette["fg"])
		self.theme_btn.SetBackgroundColour(palette["bg"])

	def _rebuild_ui(self):
		# 언어 변경 시 입력/선택 상태를 보존했다가 새 언어로 다시 그린다.
		state = {
			"url": self.url_text.GetValue(),
			"format": "mp3" if self.rb_mp3.GetValue() else "mp4",
			"quality": self.quality_choice.GetSelection(),
			"gauge": self.gauge.GetValue(),
		}
		self.panel.Destroy()
		self._build_ui()
		self.url_text.SetValue(state["url"])
		if state["format"] == "mp3":
			self.rb_mp3.SetValue(True)
		else:
			self.rb_mp4.SetValue(True)
		if state["quality"] >= 0:
			self.quality_choice.SetSelection(state["quality"])
		self.gauge.SetValue(state["gauge"])
		self._on_format_change()
		self.Layout()

	def _on_click_ko(self, event):
		if self._lang_enabled:
			self._set_lang("ko")

	def _on_click_en(self, event):
		if self._lang_enabled:
			self._set_lang("en")

	def _on_click_theme(self, event):
		self.theme = "light" if self.theme == "dark" else "dark"
		self.config["theme"] = self.theme
		save_config(self.config)
		self._apply_theme()

	def _set_lang(self, lang):
		if lang == self.lang:
			return
		self.lang = lang
		self.config["lang"] = lang
		save_config(self.config)
		self._rebuild_ui()

	def _set_lang_toggle_enabled(self, enabled):
		# 다운로드 중 패널 재생성은 워커가 잡은 위젯 참조를 깨뜨리므로 언어 전환을 막는다.
		self._lang_enabled = enabled
		self._apply_theme()

	def _on_format_change(self, event=None):
		self.quality_choice.Enable(self.rb_mp4.GetValue())

	def _on_pick_folder(self, event):
		with wx.DirDialog(
			self, self._t("pick_folder"), defaultPath=self.config["download_dir"]
		) as dlg:
			if dlg.ShowModal() == wx.ID_OK:
				self.config["download_dir"] = dlg.GetPath()
				self.folder_text.SetValue(dlg.GetPath())
				save_config(self.config)

	def _on_download(self, event):
		if self.worker and self.worker.is_alive():
			if self.cancel_event:
				self.cancel_event.set()
			self._terminate_proc()
			self.status_text.SetLabel(self._t("cancelling"))
			return
		urls = [
			line.strip()
			for line in self.url_text.GetValue().splitlines()
			if line.strip()
		]
		if not urls:
			wx.MessageBox(
				self._t("need_url"), "XDownloader", wx.OK | wx.ICON_INFORMATION
			)
			return
		fmt = "mp4" if self.rb_mp4.GetValue() else "mp3"
		selection = self.quality_choice.GetSelection()
		quality = _QUALITIES[selection] if selection >= 0 else "best"
		self.config["format"] = fmt
		self.config["quality"] = quality
		save_config(self.config)

		self.cancel_event = threading.Event()
		self.download_btn.SetLabel(self._t("cancel_btn"))
		self._set_lang_toggle_enabled(False)
		# 이전 실행의 실패색이 남지 않도록 시작 시 테마 기본 텍스트색으로 되돌린다.
		self.status_text.SetForegroundColour(self._fg)
		self.gauge.SetValue(0)
		self.worker = threading.Thread(
			target=self._download_worker,
			args=(urls, fmt, quality, self.config["download_dir"]),
			daemon=True,
		)
		self.worker.start()

	def _download_worker(self, urls, fmt, quality, out_dir):
		wx.CallAfter(self.status_text.SetLabel, self._t("prep_binaries"))
		try:
			ytdlp, ffmpeg = ensure_binaries(
				on_progress=lambda label, frac: wx.CallAfter(
					self.gauge.SetValue, int(frac * 100)
				)
			)
		except Exception as error:
			wx.CallAfter(
				self._on_finish,
				self._t("binary_fail").format(error=error),
				self._warn,
			)
			return

		total = len(urls)
		success = 0
		failed = 0
		for index, url in enumerate(urls, start=1):
			if self.cancel_event.is_set():
				break
			wx.CallAfter(self.gauge.SetValue, 0)
			wx.CallAfter(
				self.status_text.SetLabel,
				self._t("downloading").format(i=index, n=total),
			)
			try:
				code = download_with_auto_update(
					url=url,
					fmt=fmt,
					quality=quality,
					out_dir=out_dir,
					ytdlp=ytdlp,
					ffmpeg=ffmpeg,
					on_progress=lambda pct: wx.CallAfter(
						self.gauge.SetValue, int(pct)
					),
					on_line=lambda line, i=index: self._on_dl_line(
						line, i, total
					),
					cancel=self.cancel_event,
					on_status=lambda message: wx.CallAfter(
						self.status_text.SetLabel, message
					),
					on_proc=lambda proc: setattr(self, "_proc", proc),
				)
			except Exception as error:
				failed += 1
				wx.CallAfter(
					self.status_text.SetLabel,
					self._t("item_error").format(i=index, n=total, error=error),
				)
				continue
			if code == 0:
				success += 1
				wx.CallAfter(self._record_history, url, fmt, quality, out_dir)
			elif code == -1:
				break
			else:
				failed += 1

		# 다중 다운로드에서 개별 실패가 다음 항목 상태에 덮여 사라지지 않도록
		# 종료 시 성공/실패 건수를 집계해 보여준다.
		if self.cancel_event.is_set():
			message = self._t("cancelled").format(s=success, f=failed)
			color = self._warn if failed else None
		elif failed:
			message = self._t("done_fail").format(s=success, f=failed)
			color = self._warn
		else:
			message = self._t("done_ok").format(s=success)
			color = self._ok
		wx.CallAfter(self._on_finish, message, color)

	def _record_history(self, url, fmt, quality, out_dir):
		add_history(
			{
				"url": url,
				"format": fmt,
				"quality": quality,
				"path": out_dir,
				"when": datetime.now().strftime("%Y-%m-%d %H:%M"),
			}
		)
		self._refresh_history()

	def _refresh_history(self):
		self.history_list.Clear()
		for item in load_history():
			label = (
				f"[{item.get('when', '')}] "
				f"{item.get('format', '').upper()} · {item.get('url', '')}"
			)
			self.history_list.Append(label, item)

	def _on_history_open(self, event):
		selection = event.GetSelection()
		if selection < 0:
			return
		item = self.history_list.GetClientData(selection)
		path = item.get("path") if item else None
		if path and os.path.isdir(path):
			try:
				os.startfile(path)
			except Exception:
				pass

	def _on_finish(self, message, color=None):
		self.gauge.SetValue(0)
		self.status_text.SetLabel(message)
		self.status_text.SetForegroundColour(color if color is not None else self._fg)
		self.status_text.Refresh()
		self.download_btn.SetLabel(self._t("download_btn"))
		self._set_lang_toggle_enabled(True)
		self.cancel_event = None
		self._proc = None

	def _on_char_hook(self, event):
		# 다운로드 중 Esc로 취소 — 버튼/창닫기와 같은 토글 경로로 수렴시킨다.
		if (
			event.GetKeyCode() == wx.WXK_ESCAPE
			and self.worker
			and self.worker.is_alive()
		):
			self._on_download(None)
		else:
			event.Skip()

	def _on_dl_line(self, line, index, total):
		# yt-dlp가 알려주는 목적지 파일명을 진행 상태에 노출한다(없으면 무시).
		match = _DEST_RE.search(line)
		if match:
			name = os.path.basename(match.group(1).strip())
			wx.CallAfter(
				self.status_text.SetLabel,
				self._t("progress_file").format(i=index, n=total, name=name),
			)

	def _terminate_proc(self):
		"""실행 중인 yt-dlp/ffmpeg 프로세스를 즉시 종료한다(취소·창닫기 공용)."""
		if self._proc:
			try:
				self._proc.terminate()
			except Exception:
				pass

	def _on_close(self, event):
		# 다운로드 중 창을 닫아도 yt-dlp/ffmpeg가 고아 프로세스로 남지 않게 정리한다.
		if self.worker and self.worker.is_alive():
			if self.cancel_event:
				self.cancel_event.set()
			self._terminate_proc()
		self.Destroy()
