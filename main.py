"""XDownloader 진입점 — wxPython App을 띄운다."""

import ctypes

import wx

from ui.main_frame import MainFrame


def _fix_dpi_awareness():
	# PyInstaller exe를 빌드 PC와 배율(DPI)이 다른 PC에서 실행하면 라벨 텍스트
	# 위에 흰 잔상이 남는다. wx.App 생성 전에 프로세스 DPI 인식을 고정해
	# wxPython의 늦은 전환을 선점하면 어느 배율에서도 일관되게 렌더링된다.
	try:
		ctypes.windll.shcore.SetProcessDpiAwareness(2)
	except Exception:
		try:
			ctypes.windll.user32.SetProcessDPIAware()
		except Exception:
			pass


def _set_app_id():
	# Windows 작업표시줄이 이 앱을 Python 인터프리터가 아닌 독립 앱으로 묶어
	# 지정한 아이콘을 표시하도록 고유 AppUserModelID를 등록한다.
	try:
		ctypes.windll.shell32.SetCurrentProcessExplicitAppUserModelID(
			"XDownloader"
		)
	except Exception:
		pass


def main():
	_fix_dpi_awareness()
	_set_app_id()
	app = wx.App(False)
	frame = MainFrame()
	frame.Show()
	app.MainLoop()


if __name__ == "__main__":
	main()
