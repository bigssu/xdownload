"""메인 창 — URL 입력, 포맷/화질, 진행률, 히스토리.

다운로드는 별도 스레드에서 돌리고 UI 갱신은 wx.CallAfter로 메인 스레드에 위임한다
(wxPython은 메인 스레드에서만 위젯을 건드릴 수 있다).
"""

import os
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


class MainFrame(wx.Frame):
	def __init__(self):
		super().__init__(None, title=f"XDownloader v{__version__}", size=(660, 600))
		self.config = load_config()
		self.cancel_event = None
		self.worker = None
		self._build_ui()
		self._refresh_history()
		self.Centre()

	def _build_ui(self):
		panel = wx.Panel(self)
		root = wx.BoxSizer(wx.VERTICAL)

		root.Add(
			wx.StaticText(panel, label="YouTube 링크 (여러 개는 줄바꿈)"),
			0,
			wx.LEFT | wx.TOP,
			12,
		)
		self.url_text = wx.TextCtrl(panel, style=wx.TE_MULTILINE, size=(-1, 90))
		root.Add(self.url_text, 0, wx.EXPAND | wx.ALL, 12)

		row = wx.BoxSizer(wx.HORIZONTAL)
		self.format_radio = wx.RadioBox(
			panel, label="포맷", choices=["Mp4 (영상)", "Mp3 (음원)"]
		)
		self.format_radio.Bind(wx.EVT_RADIOBOX, self._on_format_change)
		if self.config.get("format") == "mp3":
			self.format_radio.SetSelection(1)
		row.Add(self.format_radio, 0, wx.RIGHT, 12)

		qbox = wx.StaticBoxSizer(wx.VERTICAL, panel, "화질 (Mp4)")
		self.quality_choice = wx.Choice(panel, choices=_QUALITY_LABELS)
		stored_quality = self.config.get("quality", "best")
		self.quality_choice.SetSelection(
			_QUALITIES.index(stored_quality)
			if stored_quality in _QUALITIES
			else 0
		)
		qbox.Add(self.quality_choice, 0, wx.ALL, 6)
		row.Add(qbox, 0)
		root.Add(row, 0, wx.LEFT | wx.RIGHT, 12)

		folder_row = wx.BoxSizer(wx.HORIZONTAL)
		folder_row.Add(
			wx.StaticText(panel, label="저장 폴더:"),
			0,
			wx.ALIGN_CENTER_VERTICAL | wx.RIGHT,
			6,
		)
		self.folder_text = wx.TextCtrl(
			panel, value=self.config["download_dir"], style=wx.TE_READONLY
		)
		folder_row.Add(
			self.folder_text, 1, wx.ALIGN_CENTER_VERTICAL | wx.RIGHT, 6
		)
		folder_btn = wx.Button(panel, label="변경")
		folder_btn.Bind(wx.EVT_BUTTON, self._on_pick_folder)
		folder_row.Add(folder_btn, 0)
		root.Add(folder_row, 0, wx.EXPAND | wx.ALL, 12)

		self.download_btn = wx.Button(panel, label="다운로드")
		self.download_btn.Bind(wx.EVT_BUTTON, self._on_download)
		root.Add(self.download_btn, 0, wx.EXPAND | wx.LEFT | wx.RIGHT, 12)

		self.gauge = wx.Gauge(panel, range=100)
		root.Add(self.gauge, 0, wx.EXPAND | wx.ALL, 12)
		self.status_text = wx.StaticText(panel, label="대기 중")
		root.Add(self.status_text, 0, wx.LEFT | wx.RIGHT, 12)

		root.Add(
			wx.StaticText(panel, label="최근 다운로드 (더블클릭 → 폴더 열기)"),
			0,
			wx.LEFT | wx.TOP,
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
		self.download_btn.SetLabel("취소")
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
			wx.CallAfter(self._on_finish, f"바이너리 준비 실패: {error}")
			return

		total = len(urls)
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
					cancel=self.cancel_event,
					on_status=lambda message: wx.CallAfter(
						self.status_text.SetLabel, message
					),
				)
			except Exception as error:
				wx.CallAfter(
					self.status_text.SetLabel,
					f"({index}/{total}) 오류: {error}",
				)
				continue
			if code == 0:
				wx.CallAfter(self._record_history, url, fmt, quality, out_dir)
			elif code == -1:
				break

		message = "취소됨" if self.cancel_event.is_set() else "완료"
		wx.CallAfter(self._on_finish, message)

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

	def _on_finish(self, message):
		self.gauge.SetValue(0)
		self.status_text.SetLabel(message)
		self.download_btn.SetLabel("다운로드")
		self.cancel_event = None
