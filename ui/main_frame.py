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
from core.binaries import ensure_binaries, ensure_ffmpeg
from core.config import load_config, save_config
from core.converter import (
	IMAGE_EXTS,
	VIDEO_EXTS,
	collect_media_files,
	convert_image,
	convert_video,
)
from core.downloader import download_with_auto_update
from core.history import add_history, load_history
from core.i18n import tr
from ui import theme_tokens
from ui.theme_tokens import PAD_M, PAD_S, PAD_XS, c, dip, f
from ui.widgets import DarkChoice, FlatButton, SectionCard

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
		super().__init__(None, title=f"XDownloader v{__version__}", size=(680, 700))
		self.config = load_config()
		self.lang = self.config.get("lang", "ko")
		self.theme = self._init_theme()
		theme_tokens.set_theme(self.theme)
		self.cancel_event = None
		self.worker = None
		self._proc = None
		self._lang_enabled = True
		self.convert_buttons = []
		# 컨트롤 생성 전에 앱 다크모드를 설정해야 스크롤바 등이 처음부터 다크로 뜬다.
		winapi.enable_app_dark_mode(self.theme == "dark")
		self._set_icon()
		self._build_ui()
		self.Bind(wx.EVT_CLOSE, self._on_close)
		self.Bind(wx.EVT_CHAR_HOOK, self._on_char_hook)
		# 타이틀바 다크는 창이 실제로 표시된 뒤 재적용해야 wx가 덮어쓰지 않는다.
		self.Bind(wx.EVT_SHOW, self._on_show)
		# 창을 이보다 더 줄이면 컨트롤이 겹치므로 최소 크기를 고정한다.
		self.SetMinSize((600, 620))
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

	def _btn_label(self, key):
		# 커스텀 FlatButton은 액셀러레이터(&)를 처리하지 않으므로 표시용으로 제거한다.
		return re.sub(r"\s*\(&.\)", "", self._t(key))

	def _build_ui(self):
		self.panel = wx.Panel(self)
		self.panel.SetBackgroundColour(c("bg"))
		root = wx.BoxSizer(wx.VERTICAL)

		# URL 라벨 줄 우측에 토글(한/En + 테마 아이콘)을 함께 둔다.
		url_row = wx.BoxSizer(wx.HORIZONTAL)
		self.url_label = wx.StaticText(self.panel, label=self._t("url_label"))
		self.url_label.SetFont(f(0, bold=True))
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
			self.lang_sep, 0, wx.ALIGN_CENTER_VERTICAL | wx.LEFT | wx.RIGHT, dip(PAD_XS)
		)
		url_row.Add(self.lang_en_lbl, 0, wx.ALIGN_CENTER_VERTICAL)
		url_row.Add(self.theme_btn, 0, wx.ALIGN_CENTER_VERTICAL | wx.LEFT, dip(PAD_M))
		root.Add(url_row, 0, wx.EXPAND | wx.LEFT | wx.RIGHT | wx.TOP, dip(PAD_M))
		self.url_text = wx.TextCtrl(
			self.panel, style=wx.TE_MULTILINE, size=(-1, dip(90))
		)
		root.Add(self.url_text, 0, wx.EXPAND | wx.LEFT | wx.RIGHT, dip(PAD_M))

		# 포맷(세그먼트) + 화질(드롭다운) 카드를 좌우로 둔다.
		row = wx.BoxSizer(wx.HORIZONTAL)
		fmt_card = SectionCard(self.panel, self._t("format_box"))
		fmt_card.body.Add(self._build_format_segment(fmt_card), 0, wx.EXPAND)
		row.Add(fmt_card, 1, wx.EXPAND | wx.RIGHT, dip(PAD_S))

		q_card = SectionCard(self.panel, self._t("quality_box"))
		quality_labels = [
			self._t("quality_best"),
			self._t("quality_1080"),
			self._t("quality_720"),
		]
		self.quality_choice = DarkChoice(q_card, quality_labels)
		stored_quality = self.config.get("quality", "best")
		self.quality_choice.SetSelection(
			_QUALITIES.index(stored_quality)
			if stored_quality in _QUALITIES
			else 0
		)
		q_card.body.Add(self.quality_choice, 0, wx.EXPAND)
		row.Add(q_card, 1, wx.EXPAND)
		root.Add(row, 0, wx.EXPAND | wx.LEFT | wx.RIGHT | wx.TOP, dip(PAD_M))

		# 저장 폴더
		folder_row = wx.BoxSizer(wx.HORIZONTAL)
		self.folder_label = wx.StaticText(self.panel, label=self._t("folder_label"))
		folder_row.Add(
			self.folder_label, 0, wx.ALIGN_CENTER_VERTICAL | wx.RIGHT, dip(PAD_S)
		)
		self.folder_text = _ReadOnlyText(
			self.panel, value=self.config["download_dir"], style=wx.TE_READONLY
		)
		folder_row.Add(
			self.folder_text, 1, wx.ALIGN_CENTER_VERTICAL | wx.RIGHT, dip(PAD_S)
		)
		self.folder_btn = FlatButton(self.panel, self._btn_label("folder_btn"), min_w=72)
		self.folder_btn.Bind(wx.EVT_BUTTON, self._on_pick_folder)
		folder_row.Add(self.folder_btn, 0, wx.ALIGN_CENTER_VERTICAL)
		root.Add(folder_row, 0, wx.EXPAND | wx.LEFT | wx.RIGHT | wx.TOP, dip(PAD_M))

		# 주 액션 — 다운로드(accent). Enter 실행은 _on_char_hook이 보존한다.
		self.download_btn = FlatButton(
			self.panel, self._btn_label("download_btn"), btn_style="accent"
		)
		self.download_btn.Bind(wx.EVT_BUTTON, self._on_download)
		root.Add(
			self.download_btn, 0, wx.EXPAND | wx.LEFT | wx.RIGHT | wx.TOP, dip(PAD_M)
		)

		# 로컬 변환 카드 (2x2 FlatButton)
		conv_card = SectionCard(self.panel, self._t("convert_box"))
		convert_grid = wx.FlexGridSizer(2, 2, dip(PAD_XS), dip(PAD_XS))
		convert_grid.AddGrowableCol(0)
		convert_grid.AddGrowableCol(1)
		self.video_file_btn = FlatButton(conv_card, self._t("video_file_btn"))
		self.video_folder_btn = FlatButton(conv_card, self._t("video_folder_btn"))
		self.image_file_btn = FlatButton(conv_card, self._t("image_file_btn"))
		self.image_folder_btn = FlatButton(conv_card, self._t("image_folder_btn"))
		self.video_file_btn.Bind(wx.EVT_BUTTON, self._on_convert_video_files)
		self.video_folder_btn.Bind(wx.EVT_BUTTON, self._on_convert_video_folder)
		self.image_file_btn.Bind(wx.EVT_BUTTON, self._on_convert_image_files)
		self.image_folder_btn.Bind(wx.EVT_BUTTON, self._on_convert_image_folder)
		self.convert_buttons = [
			self.video_file_btn,
			self.video_folder_btn,
			self.image_file_btn,
			self.image_folder_btn,
		]
		for button in self.convert_buttons:
			convert_grid.Add(button, 1, wx.EXPAND)
		conv_card.body.Add(convert_grid, 0, wx.EXPAND)
		root.Add(conv_card, 0, wx.EXPAND | wx.LEFT | wx.RIGHT | wx.TOP, dip(PAD_M))

		self.gauge = _ProgressBar(self.panel)
		root.Add(self.gauge, 0, wx.EXPAND | wx.LEFT | wx.RIGHT | wx.TOP, dip(PAD_M))
		self.status_text = wx.StaticText(self.panel, label=self._t("status_idle"))
		root.Add(self.status_text, 0, wx.LEFT | wx.RIGHT | wx.TOP, dip(PAD_XS))

		# 히스토리 카드
		hist_card = SectionCard(self.panel, self._t("history_label"))
		self.history_list = wx.ListBox(hist_card)
		self.history_list.Bind(wx.EVT_LISTBOX_DCLICK, self._on_history_open)
		hist_card.body.Add(self.history_list, 1, wx.EXPAND)
		root.Add(hist_card, 1, wx.EXPAND | wx.ALL, dip(PAD_M))

		self.panel.SetSizer(root)
		self._refresh_format_segment()
		self.quality_choice.Enable(self.fmt == "mp4")
		self._apply_theme()
		self._refresh_history()

	def _build_format_segment(self, parent):
		# mp4/mp3를 2-버튼 세그먼트로 표현한다(active=accent).
		self.fmt = "mp3" if self.config.get("format") == "mp3" else "mp4"
		self.seg_mp4 = FlatButton(parent, self._t("format_mp4"), min_w=90)
		self.seg_mp3 = FlatButton(parent, self._t("format_mp3"), min_w=90)
		self.seg_mp4.Bind(wx.EVT_BUTTON, lambda e: self._on_format_toggle("mp4"))
		self.seg_mp3.Bind(wx.EVT_BUTTON, lambda e: self._on_format_toggle("mp3"))
		row = wx.BoxSizer(wx.HORIZONTAL)
		row.Add(self.seg_mp4, 1, wx.EXPAND | wx.RIGHT, dip(PAD_XS))
		row.Add(self.seg_mp3, 1, wx.EXPAND)
		return row

	def _refresh_format_segment(self):
		self.seg_mp4.btn_style = "accent" if self.fmt == "mp4" else "secondary"
		self.seg_mp3.btn_style = "accent" if self.fmt == "mp3" else "secondary"
		self.seg_mp4.Refresh()
		self.seg_mp3.Refresh()

	def _on_format_toggle(self, fmt):
		self.fmt = fmt
		self.config["format"] = fmt
		save_config(self.config)
		self._refresh_format_segment()
		self.quality_choice.Enable(fmt == "mp4")

	def _apply_theme(self):
		# 토큰 위젯(SectionCard/FlatButton/DarkChoice)은 자체 색을 갖고, 테마 전환 시
		# 패널이 재생성된다. 여기서는 네이티브/텍스트 위젯 색과 토글만 토큰으로 맞춘다.
		self._fg = c("text")
		self._ok = c("ok")
		self._warn = c("err")
		self.panel.SetBackgroundColour(c("bg"))
		for widget in (self.url_label, self.folder_label, self.status_text):
			widget.SetBackgroundColour(c("bg"))
			widget.SetForegroundColour(c("text"))
		for widget in (self.url_text, self.folder_text, self.history_list):
			widget.SetBackgroundColour(c("surface"))
			widget.SetForegroundColour(c("text"))
		self.gauge.set_colours(c("surface"), c("accent"))
		self._style_toggles()
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

	def _style_toggles(self):
		# 상단 토글: 현재 언어 강조(볼드+accent), 비활성/작업중은 muted.
		accent = c("accent")
		muted = c("text_dim")
		font = self.lang_ko_lbl.GetFont()
		font.SetWeight(wx.FONTWEIGHT_NORMAL)
		bold = font.Bold()
		for label, code in ((self.lang_ko_lbl, "ko"), (self.lang_en_lbl, "en")):
			label.SetFont(bold if code == self.lang else font)
			active = self._lang_enabled and code == self.lang
			label.SetForegroundColour(accent if active else muted)
			label.SetBackgroundColour(c("bg"))
		self.lang_sep.SetForegroundColour(muted)
		self.lang_sep.SetBackgroundColour(c("bg"))
		# 다크면 해(클릭→라이트), 라이트면 달(클릭→다크) 아이콘을 보여준다.
		self.theme_btn.SetLabel("☀" if self.theme == "dark" else "☾")
		self.theme_btn.SetForegroundColour(c("text"))
		self.theme_btn.SetBackgroundColour(c("bg"))

	def _rebuild_ui(self):
		# 언어/테마 변경 시 입력/선택 상태를 보존했다가 다시 그린다.
		# 포맷(self.fmt)은 config에 동기화돼 있어 _build_ui가 그대로 복원한다.
		state = {
			"url": self.url_text.GetValue(),
			"quality": self.quality_choice.GetSelection(),
			"gauge": self.gauge.GetValue(),
		}
		self.panel.Destroy()
		self._build_ui()
		self.url_text.SetValue(state["url"])
		if state["quality"] >= 0:
			self.quality_choice.SetSelection(state["quality"])
		self.gauge.SetValue(state["gauge"])
		self.quality_choice.Enable(self.fmt == "mp4")
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
		theme_tokens.set_theme(self.theme)
		self._rebuild_ui()

	def _set_lang(self, lang):
		if lang == self.lang:
			return
		self.lang = lang
		self.config["lang"] = lang
		save_config(self.config)
		self._rebuild_ui()

	def _set_lang_toggle_enabled(self, enabled):
		# 작업 중 패널 재생성은 워커가 잡은 위젯 참조를 깨뜨리므로 언어 전환을 막는다.
		self._lang_enabled = enabled
		self._apply_theme()

	def _set_conversion_controls_enabled(self, enabled):
		for button in self.convert_buttons:
			button.Enable(enabled)

	def _launch_worker(self, target, *args):
		self.cancel_event = threading.Event()
		self.download_btn.SetLabel(self._t("cancel_btn"))
		self._set_conversion_controls_enabled(False)
		self._set_lang_toggle_enabled(False)
		# 이전 실행의 실패색이 남지 않도록 시작 시 테마 기본 텍스트색으로 되돌린다.
		self.status_text.SetForegroundColour(self._fg)
		self.gauge.SetValue(0)
		self.worker = threading.Thread(target=target, args=args, daemon=True)
		self.worker.start()

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
			self._cancel_active()
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
		fmt = self.fmt
		selection = self.quality_choice.GetSelection()
		quality = _QUALITIES[selection] if selection >= 0 else "best"
		self.config["format"] = fmt
		self.config["quality"] = quality
		save_config(self.config)

		self._launch_worker(
			self._download_worker, urls, fmt, quality, self.config["download_dir"]
		)

	def _media_wildcard(self, label_key, exts):
		patterns = ";".join(f"*{ext}" for ext in sorted(exts))
		return (
			f"{self._t(label_key)} ({patterns})|{patterns}|"
			f"{self._t('all_files')} (*.*)|*.*"
		)

	def _pick_media_files(self, kind):
		if kind == "video":
			message = self._t("pick_video_files")
			wildcard = self._media_wildcard("video_files", VIDEO_EXTS)
		else:
			message = self._t("pick_image_files")
			wildcard = self._media_wildcard("image_files", IMAGE_EXTS)
		with wx.FileDialog(
			self,
			message,
			wildcard=wildcard,
			style=wx.FD_OPEN | wx.FD_FILE_MUST_EXIST | wx.FD_MULTIPLE,
		) as dlg:
			if dlg.ShowModal() == wx.ID_OK:
				return dlg.GetPaths()
		return []

	def _pick_media_folder(self, kind):
		message = (
			self._t("pick_video_folder")
			if kind == "video"
			else self._t("pick_image_folder")
		)
		with wx.DirDialog(self, message) as dlg:
			if dlg.ShowModal() == wx.ID_OK:
				return [dlg.GetPath()]
		return []

	def _on_convert_video_files(self, event):
		self._start_conversion("video", self._pick_media_files("video"))

	def _on_convert_video_folder(self, event):
		self._start_conversion("video", self._pick_media_folder("video"))

	def _on_convert_image_files(self, event):
		self._start_conversion("image", self._pick_media_files("image"))

	def _on_convert_image_folder(self, event):
		self._start_conversion("image", self._pick_media_folder("image"))

	def _start_conversion(self, kind, sources):
		if self.worker and self.worker.is_alive():
			self._cancel_active()
			return
		if not sources:
			return
		files = collect_media_files(sources, kind)
		if not files:
			wx.MessageBox(
				self._t("need_convert_files"),
				"XDownloader",
				wx.OK | wx.ICON_INFORMATION,
			)
			return

		self._launch_worker(self._convert_worker, kind, files)

	def _convert_worker(self, kind, files):
		wx.CallAfter(self.status_text.SetLabel, self._t("prep_ffmpeg"))
		try:
			ffmpeg = ensure_ffmpeg(
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

		total = len(files)
		success = 0
		failed = 0
		for index, path in enumerate(files, start=1):
			if self.cancel_event.is_set():
				break
			wx.CallAfter(self.gauge.SetValue, 0)
			wx.CallAfter(
				self.status_text.SetLabel,
				self._t("convert_progress_file").format(
					i=index, n=total, name=path.name
				),
			)
			try:
				if kind == "video":
					code = convert_video(
						path,
						ffmpeg,
						on_progress=lambda pct: wx.CallAfter(
							self.gauge.SetValue, int(pct)
						),
						cancel=self.cancel_event,
						on_proc=lambda proc: setattr(self, "_proc", proc),
					)
				else:
					code = convert_image(
						path,
						ffmpeg,
						cancel=self.cancel_event,
						on_proc=lambda proc: setattr(self, "_proc", proc),
					)
					if code == 0:
						wx.CallAfter(self.gauge.SetValue, 100)
			except Exception as error:
				failed += 1
				wx.CallAfter(
					self.status_text.SetLabel,
					self._t("convert_item_error").format(
						i=index, n=total, error=error
					),
				)
				continue
			if code == 0:
				success += 1
			elif code == -1:
				break
			else:
				failed += 1

		self._report_counts("convert_done_ok", success, failed)

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
		self._report_counts("done_ok", success, failed)

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

	def _report_counts(self, done_ok_key, success, failed):
		if self.cancel_event.is_set():
			message = self._t("cancelled").format(s=success, f=failed)
			color = self._warn if failed else None
		elif failed:
			message = self._t("done_fail").format(s=success, f=failed)
			color = self._warn
		else:
			message = self._t(done_ok_key).format(s=success)
			color = self._ok
		wx.CallAfter(self._on_finish, message, color)

	def _on_finish(self, message, color=None):
		self.gauge.SetValue(0)
		self.status_text.SetLabel(message)
		self.status_text.SetForegroundColour(color if color is not None else self._fg)
		self.status_text.Refresh()
		self.download_btn.SetLabel(self._btn_label("download_btn"))
		self._set_conversion_controls_enabled(True)
		self._set_lang_toggle_enabled(True)
		self.cancel_event = None
		self._proc = None

	def _on_char_hook(self, event):
		code = event.GetKeyCode()
		# 작업 중 Esc로 취소 — 버튼/창닫기와 같은 토글 경로로 수렴시킨다.
		if code == wx.WXK_ESCAPE and self.worker and self.worker.is_alive():
			self._cancel_active()
			return
		# 입력창 밖에서 Enter로 다운로드 실행(멀티라인 입력창 안에서는 줄바꿈 유지).
		if code in (wx.WXK_RETURN, wx.WXK_NUMPAD_ENTER) and not isinstance(
			self.FindFocus(), wx.TextCtrl
		):
			if not (self.worker and self.worker.is_alive()):
				self._on_download(None)
				return
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

	def _cancel_active(self):
		if self.cancel_event:
			self.cancel_event.set()
		self._terminate_proc()
		self.status_text.SetLabel(self._t("cancelling"))

	def _on_close(self, event):
		# 작업 중 창을 닫아도 yt-dlp/ffmpeg가 고아 프로세스로 남지 않게 정리한다.
		if self.worker and self.worker.is_alive():
			self._cancel_active()
		self.Destroy()
