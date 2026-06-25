"""메인 창 — URL 입력, 포맷/화질, 진행률, 히스토리.

다운로드는 별도 스레드에서 돌리고 UI 갱신은 wx.CallAfter로 메인 스레드에 위임한다
(wxPython은 메인 스레드에서만 위젯을 건드릴 수 있다).
"""

import os
import re
import sys
import threading
from datetime import datetime

import wx

from core import __version__
from core.binaries import ensure_binaries
from core.config import load_config, save_config
from core.downloader import download_with_auto_update
from core.history import add_history, load_history

_QUALITIES = ["best", "1080p", "720p"]
_QUALITY_LABELS = ["최고 화질", "1080p", "720p"]

# 다운로드/취소 토글 버튼 라벨 — (&D) 니모닉을 양쪽에서 유지한다.
_BTN_DOWNLOAD = "다운로드(&D)"
_BTN_CANCEL = "취소"

# 상태 메시지 의미색 — 성공/실패를 색으로도 구분(네이티브 톤 유지).
_STATUS_OK = wx.Colour(0, 128, 0)
_STATUS_WARN = wx.Colour(180, 0, 0)
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


class MainFrame(wx.Frame):
	def __init__(self):
		super().__init__(None, title=f"XDownloader v{__version__}", size=(660, 600))
		self.config = load_config()
		self.cancel_event = None
		self.worker = None
		self._proc = None
		self._set_icon()
		self._build_ui()
		self._refresh_history()
		self.Bind(wx.EVT_CLOSE, self._on_close)
		self.Bind(wx.EVT_CHAR_HOOK, self._on_char_hook)
		# 창을 이보다 더 줄이면 컨트롤이 겹치므로 최소 크기를 고정한다.
		self.SetMinSize((560, 520))
		self.Centre()

	def _set_icon(self):
		# 멀티 해상도 .ico를 IconBundle로 한 번에 등록하면 타이틀바·작업표시줄이
		# 각각 최적 크기를 고른다. 아이콘 로드 실패는 앱 실행을 막지 않는다.
		try:
			path = _resource_path("xdownload.ico")
			if os.path.exists(path):
				self.SetIcons(wx.IconBundle(path, wx.BITMAP_TYPE_ICO))
		except Exception:
			pass

	def _build_ui(self):
		panel = wx.Panel(self)
		root = wx.BoxSizer(wx.VERTICAL)

		# 섹션 시작 요소엔 TOP 간격(12)으로 구분을 주고, 라벨↔필드는 붙여
		# "라벨이 어느 필드의 것인지" 묶임이 분명히 보이도록 한다.
		root.Add(
			wx.StaticText(panel, label="YouTube 링크 (여러 개는 줄바꿈)"),
			0,
			wx.LEFT | wx.RIGHT | wx.TOP,
			12,
		)
		self.url_text = wx.TextCtrl(panel, style=wx.TE_MULTILINE, size=(-1, 90))
		root.Add(self.url_text, 0, wx.EXPAND | wx.LEFT | wx.RIGHT, 12)

		row = wx.BoxSizer(wx.HORIZONTAL)
		self.format_radio = wx.RadioBox(
			panel, label="포맷", choices=["Mp4 (영상)", "Mp3 (음원)"]
		)
		self.format_radio.Bind(wx.EVT_RADIOBOX, self._on_format_change)
		if self.config.get("format") == "mp3":
			self.format_radio.SetSelection(1)
		row.Add(self.format_radio, 0, wx.EXPAND | wx.RIGHT, 12)

		qbox = wx.StaticBoxSizer(wx.VERTICAL, panel, "화질 (Mp4)")
		self.quality_choice = wx.Choice(panel, choices=_QUALITY_LABELS)
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
		folder_row.Add(
			wx.StaticText(panel, label="저장 폴더:"),
			0,
			wx.ALIGN_CENTER_VERTICAL | wx.RIGHT,
			6,
		)
		self.folder_text = _ReadOnlyText(
			panel, value=self.config["download_dir"], style=wx.TE_READONLY
		)
		folder_row.Add(
			self.folder_text, 1, wx.ALIGN_CENTER_VERTICAL | wx.RIGHT, 6
		)
		folder_btn = wx.Button(panel, label="변경(&B)")
		folder_btn.Bind(wx.EVT_BUTTON, self._on_pick_folder)
		folder_row.Add(folder_btn, 0)
		root.Add(folder_row, 0, wx.EXPAND | wx.LEFT | wx.RIGHT | wx.TOP, 12)

		# 주 액션은 볼드+높이로 시각 비중을 키우고 기본 버튼으로 지정해,
		# 입력창 밖에 포커스가 있을 때 Enter로 바로 실행되게 한다.
		self.download_btn = wx.Button(panel, label=_BTN_DOWNLOAD)
		self.download_btn.SetFont(self.download_btn.GetFont().Bold())
		self.download_btn.SetMinSize((-1, 40))
		self.download_btn.Bind(wx.EVT_BUTTON, self._on_download)
		self.download_btn.SetDefault()
		root.Add(
			self.download_btn, 0, wx.EXPAND | wx.LEFT | wx.RIGHT | wx.TOP, 12
		)

		self.gauge = wx.Gauge(panel, range=100)
		root.Add(self.gauge, 0, wx.EXPAND | wx.LEFT | wx.RIGHT | wx.TOP, 12)
		self.status_text = wx.StaticText(panel, label="대기 중")
		root.Add(self.status_text, 0, wx.LEFT | wx.RIGHT | wx.TOP, 4)

		root.Add(
			wx.StaticText(panel, label="최근 다운로드 (더블클릭 → 폴더 열기)"),
			0,
			wx.LEFT | wx.RIGHT | wx.TOP,
			12,
		)
		self.history_list = wx.ListBox(panel)
		self.history_list.Bind(wx.EVT_LISTBOX_DCLICK, self._on_history_open)
		root.Add(self.history_list, 1, wx.EXPAND | wx.ALL, 12)

		panel.SetSizer(root)
		self._on_format_change()

	def _on_format_change(self, event=None):
		self.quality_choice.Enable(self.format_radio.GetSelection() == 0)

	def _on_pick_folder(self, event):
		with wx.DirDialog(
			self, "저장 폴더 선택", defaultPath=self.config["download_dir"]
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
			self.status_text.SetLabel("취소 중...")
			return
		urls = [
			line.strip()
			for line in self.url_text.GetValue().splitlines()
			if line.strip()
		]
		if not urls:
			wx.MessageBox(
				"링크를 입력하세요.", "XDownloader", wx.OK | wx.ICON_INFORMATION
			)
			return
		fmt = "mp4" if self.format_radio.GetSelection() == 0 else "mp3"
		selection = self.quality_choice.GetSelection()
		quality = _QUALITIES[selection] if selection >= 0 else "best"
		self.config["format"] = fmt
		self.config["quality"] = quality
		save_config(self.config)

		self.cancel_event = threading.Event()
		self.download_btn.SetLabel(_BTN_CANCEL)
		# 이전 실행의 실패색이 남지 않도록 시작 시 기본 텍스트색으로 되돌린다.
		self.status_text.SetForegroundColour(
			wx.SystemSettings.GetColour(wx.SYS_COLOUR_WINDOWTEXT)
		)
		self.gauge.SetValue(0)
		self.worker = threading.Thread(
			target=self._download_worker,
			args=(urls, fmt, quality, self.config["download_dir"]),
			daemon=True,
		)
		self.worker.start()

	def _download_worker(self, urls, fmt, quality, out_dir):
		wx.CallAfter(
			self.status_text.SetLabel, "yt-dlp/ffmpeg 준비 중 (최초 1회)..."
		)
		try:
			ytdlp, ffmpeg = ensure_binaries(
				on_progress=lambda label, frac: wx.CallAfter(
					self.gauge.SetValue, int(frac * 100)
				)
			)
		except Exception as error:
			wx.CallAfter(
				self._on_finish, f"바이너리 준비 실패: {error}", _STATUS_WARN
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
				self.status_text.SetLabel, f"({index}/{total}) 다운로드 중..."
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
					f"({index}/{total}) 오류: {error}",
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
			message = f"취소됨 — 성공 {success}, 실패 {failed}"
			color = _STATUS_WARN if failed else None
		elif failed:
			message = f"완료 — 성공 {success}, 실패 {failed}"
			color = _STATUS_WARN
		else:
			message = f"완료 — {success}개 다운로드"
			color = _STATUS_OK
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
		if color is None:
			color = wx.SystemSettings.GetColour(wx.SYS_COLOUR_WINDOWTEXT)
		self.status_text.SetForegroundColour(color)
		self.status_text.Refresh()
		self.download_btn.SetLabel(_BTN_DOWNLOAD)
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
				self.status_text.SetLabel, f"({index}/{total}) {name}"
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
