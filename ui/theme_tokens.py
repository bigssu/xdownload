"""UI 디자인 토큰 — Xchange 파생 (다크/라이트 테마 인식).

색은 C_DARK/C_LIGHT → c("key")로만 접근(하드코딩 금지). 폰트는 f(delta, bold),
모든 px는 dip() 통과. 다크는 Xchange 원본 값, 라이트는 파생 값.
"""

import wx

FONT_BASE = 10
PAD_XS = 4
PAD_S = 8
PAD_M = 14

C_DARK = {
	"bg": "#13121F", "toolbar": "#1A1928", "card": "#21203A", "card2": "#2A2945",
	"surface": "#2F2E50", "status_bar": "#100F1C", "border": "#3B3A60", "border_card": "#302F52",
	"btn_active_a": "#5A3FCC", "btn_active_b": "#3A72D4", "btn_active_bd": "#44EE88",
	"btn_normal": "#252440", "btn_hover": "#302F52", "accent": "#0078D4", "accent_h": "#1A8AE0",
	"secondary": "#383762", "secondary_h": "#454470", "toggle_on": "#0078D4", "toggle_off": "#3B3A60",
	"toggle_knob": "#FFFFFF", "text": "#E6E4FF", "text_dim": "#7F7DAA", "text_card": "#C8C5F0",
	"info": "#5AB4FF", "warn": "#FFB930", "gpu_dot": "#3DDF84", "pause_dot": "#5A5880",
	"divider": "#252440", "arrow": "#7B6EE8", "danger": "#B42D3A", "danger_h": "#DC3C4B",
	"icon_disabled": "#A8A8A8", "icon_disabled_acc": "#848484", "icon_inactive": "#E6E4FF",
	"icon_active_acc": "#44EE88", "icon_ready_acc": "#5AB4FF", "copy_paper_bg": "#21203A",
	"overlay_disabled": "#484848", "ok": "#4EC976", "err": "#F06969", "on_fill": "#FFFFFF",
}

C_LIGHT = {
	"bg": "#F4F3FA", "toolbar": "#ECEAF5", "card": "#FFFFFF", "card2": "#F2F1FA",
	"surface": "#EAE8F5", "status_bar": "#E8E6F2", "border": "#C9C6E0", "border_card": "#DAD7EC",
	"btn_active_a": "#5A3FCC", "btn_active_b": "#3A72D4", "btn_active_bd": "#44EE88",
	"btn_normal": "#E4E2F2", "btn_hover": "#D8D5EC", "accent": "#0078D4", "accent_h": "#1A8AE0",
	"secondary": "#E0DEF0", "secondary_h": "#D4D1E8", "toggle_on": "#0078D4", "toggle_off": "#C4C1DC",
	"toggle_knob": "#FFFFFF", "text": "#1E1D2E", "text_dim": "#6D6B8C", "text_card": "#3A3856",
	"info": "#0A6FC0", "warn": "#B57400", "gpu_dot": "#1F9E57", "pause_dot": "#9C99BE",
	"divider": "#DDDBEC", "arrow": "#6A5DD6", "danger": "#B42D3A", "danger_h": "#DC3C4B",
	"icon_disabled": "#9A98AE", "icon_disabled_acc": "#B8B6C8", "icon_inactive": "#3A3856",
	"icon_active_acc": "#1F9E57", "icon_ready_acc": "#0A6FC0", "copy_paper_bg": "#FFFFFF",
	"overlay_disabled": "#C0C0C0", "ok": "#17803F", "err": "#B42D3A", "on_fill": "#FFFFFF",
}

_THEMES = {"dark": C_DARK, "light": C_LIGHT}
_theme = "dark"
_colour_cache = {}
_font_cache = {}
_dip_cache = {}


def set_theme(name):
	# 테마 전환 시 색 캐시만 비운다(폰트/dip은 테마 무관).
	global _theme
	if name not in _THEMES:
		return
	_theme = name
	_colour_cache.clear()


def get_theme():
	return _theme


def c(key):
	cached = _colour_cache.get(key)
	if cached is not None:
		return cached
	h = _THEMES[_theme][key].lstrip("#")
	col = wx.Colour(int(h[0:2], 16), int(h[2:4], 16), int(h[4:6], 16))
	_colour_cache[key] = col
	return col


def f(delta=0, bold=False):
	cache_key = (delta, bold)
	cached = _font_cache.get(cache_key)
	if cached is not None:
		return cached
	font = wx.Font(
		FONT_BASE + delta,
		wx.FONTFAMILY_DEFAULT,
		wx.FONTSTYLE_NORMAL,
		wx.FONTWEIGHT_BOLD if bold else wx.FONTWEIGHT_NORMAL,
		faceName="Segoe UI",
	)
	_font_cache[cache_key] = font
	return font


def dip(px):
	cached = _dip_cache.get(px)
	if cached is not None:
		return cached
	try:
		result = wx.Window.FromDIP(px, None)
	except Exception:
		result = px
	_dip_cache[px] = result
	return result
