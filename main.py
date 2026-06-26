"""XDownloader 진입점 — wxPython App을 띄운다."""

import ctypes

import wx

from ui.main_frame import MainFrame


def _fix_dpi_awareness():
	# wxPython 4.2(wxWidgets 3.2)는 per-monitor v2를 지원하지 않고 v1을 강제하는데,
	# v1은 배율이 다른 멀티모니터에서 자식 컨트롤(입력창 등)에 흰 잔상을 남긴다.
	# system DPI aware로 고정하면 모든 모니터를 단일 배율로 처리해 DPI 변경에 따른
	# 잔상이 사라진다(보조 모니터는 OS 비트맵 확대로 약간 흐릴 수 있음). 실제 적용은
	# manifest(XDownloader.manifest)의 dpiAware=true가 담당하고, 아래는 개발 모드
	# (python 직접 실행) 폴백이다. wx.App 생성 전에 호출해야 한다.
	try:
		ctypes.windll.shcore.SetProcessDpiAwareness(1)  # PROCESS_SYSTEM_DPI_AWARE
	except Exception:
		try:
			ctypes.windll.user32.SetProcessDPIAware()
		except Exception:
			pass


def _force_system_dpi_context():
	# wxWidgets는 wx.App 초기화 중 thread DPI를 per-monitor v1으로 바꾼다(manifest로
	# 막을 수 없다). v1은 배율이 다른 멀티모니터에서 자식 컨트롤에 흰 잔상을 남기므로,
	# 창 생성 '직전'에 thread context를 system aware로 되돌려 창이 단일 배율로 그려지게
	# 한다. 창은 생성 시점의 thread context를 따르므로 이 순서가 핵심이다.
	# DPI_AWARENESS_CONTEXT_SYSTEM_AWARE = -2
	try:
		user32 = ctypes.windll.user32
		user32.SetThreadDpiAwarenessContext.restype = ctypes.c_void_p
		user32.SetThreadDpiAwarenessContext.argtypes = [ctypes.c_void_p]
		user32.SetThreadDpiAwarenessContext(ctypes.c_void_p(-2))
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
	_force_system_dpi_context()
	frame = MainFrame()
	frame.Show()
	app.MainLoop()


if __name__ == "__main__":
	main()
