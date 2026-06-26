"""Windows 다크 모드 네이티브 보정.

wxWidgets 3.2엔 다크 API가 없어 타이틀바·스크롤바·드롭다운 같은 OS 네이티브
컨트롤은 SetBackgroundColour로 색이 먹지 않는다. uxtheme/dwmapi를 직접 호출해
다크로 맞춘다. 모두 best-effort — Windows가 아니거나 실패하면 조용히 무시한다.
"""

import ctypes
import sys
from ctypes import wintypes


def _is_win():
	return sys.platform == "win32"


def enable_app_dark_mode(dark):
	# uxtheme!SetPreferredAppMode (ordinal 135, Win10 1903+).
	# 0=Default / 1=AllowDark / 2=ForceDark / 3=ForceLight
	if not _is_win():
		return
	try:
		fn = ctypes.windll.uxtheme[135]
		fn.argtypes = [ctypes.c_int]
		fn.restype = ctypes.c_int
		fn(2 if dark else 3)
	except Exception:
		pass


def set_titlebar_dark(hwnd, dark):
	# dwmapi!DwmSetWindowAttribute(DWMWA_USE_IMMERSIVE_DARK_MODE).
	# 속성 번호는 빌드마다 달라 20(2004+) → 19(1809~1903) 순으로 시도한다.
	if not _is_win() or not hwnd:
		return
	try:
		value = ctypes.c_int(1 if dark else 0)
		applied = False
		for attr in (20, 19):
			result = ctypes.windll.dwmapi.DwmSetWindowAttribute(
				wintypes.HWND(hwnd),
				ctypes.c_uint(attr),
				ctypes.byref(value),
				ctypes.sizeof(value),
			)
			if result == 0:
				applied = True
				break
		if applied:
			# 속성만 바꾸면 이미 그려진 타이틀바가 갱신되지 않는다. 비클라이언트
			# 영역 변경을 알려 DWM이 캡션을 다시 그리게 한다.
			SWP = 0x0001 | 0x0002 | 0x0004 | 0x0010 | 0x0020  # NOSIZE|NOMOVE|NOZORDER|NOACTIVATE|FRAMECHANGED
			ctypes.windll.user32.SetWindowPos(
				wintypes.HWND(hwnd), None, 0, 0, 0, 0, ctypes.c_uint(SWP)
			)
	except Exception:
		pass


def set_control_dark(hwnd, dark, kind="explorer"):
	# uxtheme!SetWindowTheme으로 컨트롤(스크롤바 포함)에 다크 테마를 입힌다.
	# 콤보박스(드롭다운)는 전용 테마명 "DarkMode_CFD"를 쓴다.
	if not _is_win() or not hwnd:
		return
	try:
		if dark:
			theme = "DarkMode_CFD" if kind == "combo" else "DarkMode_Explorer"
		else:
			theme = "CFD" if kind == "combo" else "Explorer"
		ctypes.windll.uxtheme.SetWindowTheme(
			wintypes.HWND(hwnd), ctypes.c_wchar_p(theme), None
		)
	except Exception:
		pass
