# XChange UI 스타일·방식 이식 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Xchange의 디자인 토큰·커스텀 위젯·딥 퍼플 다크 팔레트를 XDownloader에 이식해 UI를 재스킨한다(기능 동작 불변).

**Architecture:** 신규 모듈 2개(`ui/theme_tokens.py` 테마 인식 토큰, `ui/widgets.py` 커스텀 위젯)를 이식하고, `main_frame.py`의 UI 빌드/테마 메서드만 이 토큰·위젯으로 교체한다. 로직 계층(core/*)은 손대지 않는다. 라이트/다크 토글은 유지하되 테마 전환은 패널 재생성으로 처리한다.

**Tech Stack:** Python 3.11, wxPython 4.2, PyInstaller(빌드), unittest(테스트).

## Global Constraints

- **프레임워크:** wxPython 4.2 고정 — 다른 UI 프레임워크 도입 금지.
- **앱 용량:** 빌드된 `dist/XDownloader.exe` ≤ **16.54 MB** (기준선 15.03 MB의 +10%). 신규 pip 의존성 0, 신규 대형 애셋 0.
- **코드 스타일:** 탭 들여쓰기, `except Exception:`(bare except 금지), 불필요한 타입 힌트/docstring 지양, 주석은 한국어.
- **기능 불변:** 다운로드·로컬 변환·히스토리·i18n 동작 변경 없음.
- **디자인 토큰:** 색·폰트·px 하드코딩 금지 — `c()/f()/dip()/PAD_*` 로만 접근. 레이아웃은 Sizer 전용.
- **테마 모델:** 다크=Xchange 원본 값, 라이트=파생 값(스펙 §5.2). 두 딕셔너리는 동일 키 집합.

참조 스펙: `docs/superpowers/specs/2026-07-15-xchange-ui-port-design.md`
Xchange 원본: `D:\_NOAV\Xchange\src\ui\theme_tokens.py`, `D:\_NOAV\Xchange\src\ui\widgets.py`

---

## File Structure

- **Create** `ui/theme_tokens.py` — `C_DARK`/`C_LIGHT` 토큰 표 + `c/f/dip/PAD_*` + `set_theme/get_theme`.
- **Create** `ui/widgets.py` — `BasePanel, FlatButton, ToggleSwitch, DarkChoice, SectionCard, Divider` (Xchange 이식 + on_fill 적응).
- **Create** `tests/test_theme_tokens.py` — 토큰 키 정합 + 테마 전환 + WCAG 대비 회귀 테스트.
- **Modify** `ui/main_frame.py` — `_build_ui`/`_apply_theme` 재작성, `_palette` 제거, 테마 토글 재생성, `_ProgressBar` 색 토큰화, mp4/mp3 세그먼트.
- **Modify** `core/__init__.py` — 버전 0.68.
- **Modify** `DESIGN.md` — Xchange 파생 디자인 시스템으로 갱신.

---

### Task 1: `ui/theme_tokens.py` + 대비 회귀 테스트

**Files:**
- Create: `ui/theme_tokens.py`
- Test: `tests/test_theme_tokens.py`

**Interfaces:**
- Produces:
  - `C_DARK: dict`, `C_LIGHT: dict` (동일 키 집합)
  - `FONT_BASE=10`, `PAD_XS=4`, `PAD_S=8`, `PAD_M=14`
  - `set_theme(name: str) -> None` ("dark"|"light"; 그 외 무시), `get_theme() -> str`
  - `c(key) -> wx.Colour` (현재 테마), `f(delta=0, bold=False) -> wx.Font`, `dip(px) -> int`

- [ ] **Step 1: 실패 테스트 작성** — `tests/test_theme_tokens.py`

```python
import unittest

from ui import theme_tokens as t


def _lin(v):
	v = v / 255
	return v / 12.92 if v <= 0.03928 else ((v + 0.055) / 1.055) ** 2.4


def _lum(hex_):
	h = hex_.lstrip("#")
	r, g, b = (int(h[i:i + 2], 16) for i in (0, 2, 4))
	return 0.2126 * _lin(r) + 0.7152 * _lin(g) + 0.0722 * _lin(b)


def _ratio(fg, bg):
	a, b = _lum(fg), _lum(bg)
	hi, lo = max(a, b), min(a, b)
	return (hi + 0.05) / (lo + 0.05)


class ThemeTokenTests(unittest.TestCase):
	def test_dark_and_light_share_identical_keys(self):
		self.assertEqual(set(t.C_DARK), set(t.C_LIGHT))

	def test_set_theme_switches_active_table(self):
		t.set_theme("light")
		self.assertEqual(t.get_theme(), "light")
		t.set_theme("dark")
		self.assertEqual(t.get_theme(), "dark")
		t.set_theme("bogus")  # 무시
		self.assertEqual(t.get_theme(), "dark")

	def test_text_background_pairs_meet_wcag_aa(self):
		# (fg_key, bg_key, 최소기준) — 본문 4.5, 채움버튼 3.0
		checks = [
			("text", "bg", 4.5), ("text", "card", 4.5), ("text", "surface", 4.5),
			("text_card", "card", 4.5), ("ok", "card", 4.5), ("err", "card", 4.5),
			("on_fill", "accent", 3.0), ("on_fill", "danger", 3.0),
		]
		for theme in ("dark", "light"):
			table = t.C_DARK if theme == "dark" else t.C_LIGHT
			for fg, bg, thr in checks:
				r = _ratio(table[fg], table[bg])
				self.assertGreaterEqual(
					r, thr, f"{theme}: {fg} on {bg} = {r:.2f} < {thr}"
				)


if __name__ == "__main__":
	unittest.main()
```

- [ ] **Step 2: 실패 확인**

Run: `PYTHONPATH=. .venv/Scripts/python.exe tests/test_theme_tokens.py -v`
Expected: FAIL — `ModuleNotFoundError: No module named 'ui.theme_tokens'`

- [ ] **Step 3: 모듈 구현** — `ui/theme_tokens.py` (탭 들여쓰기)

```python
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
```

- [ ] **Step 4: 통과 확인**

Run: `PYTHONPATH=. .venv/Scripts/python.exe tests/test_theme_tokens.py -v`
Expected: PASS (3 tests). 대비 테스트가 통과하면 팔레트가 AA를 만족함을 회귀로 고정.

- [ ] **Step 5: 커밋**

```bash
git add ui/theme_tokens.py tests/test_theme_tokens.py
git commit -m "feat(ui): 테마 인식 디자인 토큰(Xchange 파생) + 대비 회귀 테스트"
```

---

### Task 2: `ui/widgets.py` (Xchange 위젯 이식 + on_fill 적응)

**Files:**
- Create: `ui/widgets.py`
- Test: `tests/test_widgets.py`

**Interfaces:**
- Consumes: `ui.theme_tokens` (`c, f, dip, PAD_M`)
- Produces:
  - `BasePanel(parent, bg_key="bg", **kw)`
  - `FlatButton(parent, label, btn_style="secondary"|"accent"|"danger", min_w=80)` — 커스텀 페인트 버튼, `wx.EVT_LEFT_UP` 로 클릭 처리(아래 Step 3 참고)
  - `ToggleSwitch(parent, value=True)` — `wx.EVT_CHECKBOX` 발생
  - `DarkChoice(parent, choices)` — `GetSelection/SetSelection/GetString` (OwnerDrawnComboBox)
  - `SectionCard(parent, title, icon="")` — `.body`(wx.BoxSizer) 에 자식 Add
  - `Divider(parent)`

- [ ] **Step 1: 위젯 이식 작성** — `ui/widgets.py`

`D:\_NOAV\Xchange\src\ui\widgets.py` 를 **탭 들여쓰기로 재포맷**해 이식하되 다음 3가지를 변경한다:
1. import 를 `from ui.theme_tokens import PAD_M, c, dip, f` 로 (원본 `src.ui.theme_tokens`).
2. `from __future__ import annotations` 및 타입 힌트·docstring 은 제거(로직 불변).
3. **FlatButton `_on_paint` 의 색 선택 블록**을 아래로 교체 — accent/danger 라벨을 `c("text")` → `c("on_fill")`:

```python
		if not enabled:
			bg = c("secondary")
			text_col = c("text_dim")
		elif self.btn_style == "accent":
			bg = c("accent_h") if self._hover else c("accent")
			text_col = c("on_fill")
		elif self.btn_style == "danger":
			bg = c("danger_h") if self._hover else c("danger")
			text_col = c("on_fill")
		else:
			bg = c("secondary_h") if self._hover else c("secondary")
			text_col = c("text")
```

4. **FlatButton 클릭 이벤트 추가** — 원본은 클릭 핸들러가 없다(Xchange는 상위에서 처리). `__init__` 의 Bind 목록에 다음을 추가해 표준 버튼 이벤트를 발생시킨다:

```python
		self.Bind(wx.EVT_LEFT_UP, self._on_click)
```
그리고 메서드 추가:
```python
	def _on_click(self, event):
		if self.IsEnabled():
			evt = wx.CommandEvent(wx.EVT_BUTTON.typeId, self.GetId())
			evt.SetEventObject(self)
			self.GetEventHandler().ProcessEvent(evt)
		event.Skip()
```

나머지(BasePanel, ToggleSwitch, DarkChoice, SectionCard, Divider)는 원본 로직 그대로(탭 재포맷만).

- [ ] **Step 2: 스모크 테스트 작성** — `tests/test_widgets.py`

```python
import unittest

import wx

from ui import theme_tokens as t
from ui import widgets


class WidgetSmokeTests(unittest.TestCase):
	@classmethod
	def setUpClass(cls):
		cls.app = wx.App(False)

	def test_widgets_construct_under_both_themes(self):
		for theme in ("dark", "light"):
			t.set_theme(theme)
			frame = wx.Frame(None)
			widgets.BasePanel(frame)
			widgets.FlatButton(frame, "OK", btn_style="accent")
			widgets.FlatButton(frame, "Cancel", btn_style="secondary")
			widgets.ToggleSwitch(frame, value=True)
			widgets.DarkChoice(frame, ["a", "b"])
			card = widgets.SectionCard(frame, "제목")
			self.assertIsNotNone(card.body)
			widgets.Divider(frame)
			frame.Destroy()
		t.set_theme("dark")


if __name__ == "__main__":
	unittest.main()
```

- [ ] **Step 3: 실행** — 구성 오류 없이 통과 확인

Run: `PYTHONPATH=. .venv/Scripts/python.exe tests/test_widgets.py -v`
Expected: PASS (1 test). wx.App 생성이 필요하므로 Windows 데스크톱 세션에서 실행.

- [ ] **Step 4: py_compile 확인**

Run: `.venv/Scripts/python.exe -m py_compile ui/widgets.py`
Expected: 오류 없음.

- [ ] **Step 5: 커밋**

```bash
git add ui/widgets.py tests/test_widgets.py
git commit -m "feat(ui): Xchange 커스텀 위젯 이식(FlatButton/SectionCard/DarkChoice/ToggleSwitch) + on_fill 적응"
```

---

### Task 3: `main_frame.py` — UI 빌드 재스킨 (구조)

`_build_ui` 를 SectionCard/FlatButton/DarkChoice + mp4/mp3 세그먼트로 재작성하고, `_ProgressBar` 색을 토큰화한다. 이 태스크는 **다크 테마 기준**으로 렌더링을 맞춘다(테마 토글은 Task 4).

**Files:**
- Modify: `ui/main_frame.py`

**Interfaces:**
- Consumes: `ui.theme_tokens`(`c, f, dip, PAD_XS, PAD_S, PAD_M, set_theme, get_theme`), `ui.widgets`(`FlatButton, DarkChoice, SectionCard, Divider, BasePanel`)
- Produces: 재스킨된 `_build_ui`; 신규 `self.rb_mp4/rb_mp3` 대체 상태 `self.fmt`("mp4"|"mp3") + `_on_format_toggle`

- [ ] **Step 1: import 추가** — `ui/main_frame.py` 상단 import 구역에 추가

```python
from ui import theme_tokens
from ui.theme_tokens import PAD_M, PAD_S, PAD_XS, c, dip, f
from ui.widgets import DarkChoice, Divider, FlatButton, SectionCard
```

- [ ] **Step 2: `_init_theme` 직후 활성 테마를 토큰 모듈에 반영**

`__init__` 에서 `self.theme = self._init_theme()` 다음 줄에 추가:
```python
		theme_tokens.set_theme(self.theme)
```

- [ ] **Step 3: `_ProgressBar` 색을 토큰화**

`_ProgressBar.__init__` 의 하드코딩 색을 토큰으로 교체:
```python
		self._trough = c("surface")
		self._fill = c("accent")
```
(상단에서 `from ui.theme_tokens import c` 가 임포트되어 있어야 함 — Step 1 반영.)

- [ ] **Step 4: `_build_ui` 재작성 — StaticBox→SectionCard, Button→FlatButton, Choice→DarkChoice**

포맷/화질/변환/히스토리 4개 `wx.StaticBoxSizer` 를 `SectionCard` 로, 버튼을 `FlatButton` 으로, 화질 `wx.Choice` 를 `DarkChoice` 로 바꾼다. mp4/mp3 는 2-버튼 세그먼트로 대체한다. 핵심 규칙:
- 패널 배경은 `c("bg")`, 카드 내부 컨트롤 부모는 `card`(= `SectionCard` 인스턴스). 카드 자식은 `card.body.Add(...)`.
- 간격은 `dip(PAD_M)` 거터, `dip(PAD_S)` 내부, `dip(PAD_XS)` 밀접. 모든 wx.Size/여백은 `dip()` 통과.
- 다운로드 = `FlatButton(..., btn_style="accent", min_w=...)`; 변환 4개·폴더변경 = `btn_style="secondary"`.

mp4/mp3 세그먼트 구현(라디오 대체):
```python
	def _build_format_segment(self, parent):
		self.fmt = "mp3" if self.config.get("format") == "mp3" else "mp4"
		self.seg_mp4 = FlatButton(parent, self._t("format_mp4"), min_w=70)
		self.seg_mp3 = FlatButton(parent, self._t("format_mp3"), min_w=70)
		self.seg_mp4.Bind(wx.EVT_BUTTON, lambda e: self._on_format_toggle("mp4"))
		self.seg_mp3.Bind(wx.EVT_BUTTON, lambda e: self._on_format_toggle("mp3"))
		self._refresh_format_segment()
		row = wx.BoxSizer(wx.HORIZONTAL)
		row.Add(self.seg_mp4, 0, wx.RIGHT, dip(PAD_XS))
		row.Add(self.seg_mp3, 0)
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
```

`_on_download` 의 포맷 판정을 라디오→`self.fmt` 로 변경:
```python
		fmt = self.fmt
```
(기존 `fmt = "mp4" if self.rb_mp4.GetValue() else "mp3"` 대체.)
`_on_format_change`(라디오 핸들러)와 `self.rb_mp4/rb_mp3` 참조는 제거하고, 화질 Enable 은 `_on_format_toggle`/빌드 말미에서 `self.quality_choice.Enable(self.fmt == "mp4")` 로 처리.

> 전체 `_build_ui` 본문은 기존 구조(같은 위젯 순서: url→포맷/화질 row→폴더→다운로드→변환→게이지→상태→히스토리)를 유지하되 위 위젯 치환만 적용한다. `_rebuild_ui` 의 상태 보존 딕셔너리에서 `format` 은 `self.fmt` 기준으로, quality 는 그대로 유지.

- [ ] **Step 5: py_compile + 실기동 스모크(다크)**

Run: `.venv/Scripts/python.exe -m py_compile ui/main_frame.py`
그다음 실기동: `.venv/Scripts/python.exe main.py` — 창이 뜨고 카드/버튼/드롭다운/게이지가 다크 퍼플로 렌더링, 위젯 ghost·깜빡임 없음(규칙 C/E), mp4/mp3 세그먼트가 accent/secondary 로 토글되는지 육안 확인. 확인 후 창 닫기.
Expected: 정상 렌더 + 세그먼트 토글 동작.

- [ ] **Step 6: 커밋**

```bash
git add ui/main_frame.py
git commit -m "feat(ui): main_frame 재스킨 — SectionCard/FlatButton/DarkChoice + mp4/mp3 세그먼트"
```

---

### Task 4: `main_frame.py` — 테마 시스템 교체 (라이트/다크 토글)

`_palette`/`_apply_theme` 의 자체 팔레트 로직을 제거하고, 테마 토글을 `theme_tokens.set_theme` + 패널 재생성으로 처리한다.

**Files:**
- Modify: `ui/main_frame.py`

- [ ] **Step 1: `_on_click_theme` 을 재생성 기반으로 교체**

```python
	def _on_click_theme(self, event):
		self.theme = "light" if self.theme == "dark" else "dark"
		self.config["theme"] = self.theme
		save_config(self.config)
		theme_tokens.set_theme(self.theme)
		self._rebuild_ui()
```

- [ ] **Step 2: `_apply_theme` 축소** — 자체 팔레트 제거

기존 `_palette()` 와 `_apply_theme()` 의 위젯별 SetColour 루프는 제거한다(토큰 위젯이 자체 색을 가짐). `_apply_theme` 는 **네이티브 다크 보정 + 토글 라벨 스타일만** 남긴다:
```python
	def _apply_theme(self):
		self.panel.SetBackgroundColour(c("bg"))
		self._fg = c("text")
		self._ok = c("ok")
		self._warn = c("err")
		self._style_toggles()
		self._apply_native_dark()
		self.panel.Refresh()
```
`self._fg/_ok/_warn` 는 상태줄 색으로 계속 쓰이므로 토큰에서 채운다(기존 `_on_finish`/워커의 색 참조 유지).

- [ ] **Step 3: `_style_toggles` 를 토큰화** — 인자 없는 시그니처로

`_style_toggles(self, palette)` → `_style_toggles(self)`, 내부 색을 토큰으로:
```python
	def _style_toggles(self):
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
		self.theme_btn.SetLabel("☀" if self.theme == "dark" else "☾")
		self.theme_btn.SetForegroundColour(c("text"))
		self.theme_btn.SetBackgroundColour(c("bg"))
```
`_apply_theme` 내 `self._style_toggles(palette)` 호출을 `self._style_toggles()` 로 수정. `_palette` 메서드는 삭제. 게이지 색 재설정(`self.gauge.set_colours(...)`)은 `_ProgressBar` 가 토큰을 쓰므로 제거하거나 `self.gauge.set_colours(c("surface"), c("accent"))` 로 단순화.

- [ ] **Step 4: py_compile + 실기동 스모크(테마 토글)**

Run: `.venv/Scripts/python.exe -m py_compile ui/main_frame.py`
실기동: `.venv/Scripts/python.exe main.py` — ☾/☀ 클릭 시 다크↔라이트 즉시 전환(입력·선택값 보존), 한/En 클릭 시 언어 전환 유지, 상태줄 성공/실패 색이 각 테마에서 읽히는지 확인.
Expected: 두 테마 모두 정상, 상태 유지, 깜빡임 억제.

- [ ] **Step 5: 커밋**

```bash
git add ui/main_frame.py
git commit -m "feat(ui): 테마 토글을 토큰+패널재생성으로 교체 (_palette 제거)"
```

---

### Task 5: `DESIGN.md` 갱신 + 버전 0.68

**Files:**
- Modify: `DESIGN.md`
- Modify: `core/__init__.py`

- [ ] **Step 1: `core/__init__.py` 버전 상향**

```python
__version__ = "0.68"
```
(기존 `"0.67"` 대체.)

- [ ] **Step 2: `DESIGN.md` 를 Xchange 파생 디자인 시스템으로 갱신**

frontmatter 토큰(colors/typography/spacing/components)과 본문을 스펙 §5.2 값 기준으로 교체한다. 핵심: "OS 시스템 색을 따른다" 원칙 → "Xchange 파생 토큰 시스템(딥 퍼플 다크 / 파생 라이트), 하드코딩 금지, `c()/f()/dip()` 접근" 으로 대체. 커스텀 위젯(SectionCard/FlatButton/DarkChoice/ToggleSwitch) 규칙 반영. 라이트/다크 토글·한/En 유지 서술.

- [ ] **Step 3: 커밋**

```bash
git add DESIGN.md core/__init__.py
git commit -m "docs: DESIGN.md를 Xchange 파생 디자인 시스템으로 갱신 + v0.68"
```

---

### Task 6: 빌드 + 용량 검증 + 회귀

**Files:** 없음(검증 전용)

- [ ] **Step 1: 회귀 테스트 전체**

Run: `PYTHONPATH=. .venv/Scripts/python.exe -m unittest discover -s tests -t . -v`
Expected: theme_tokens·widgets·converter 테스트 전부 PASS.

- [ ] **Step 2: 재빌드**

Run: `.venv/Scripts/python.exe -m PyInstaller XDownloader.spec --noconfirm`
Expected: `dist/XDownloader.exe` 생성.

- [ ] **Step 3: 용량 수용 기준 확인**

Run: `ls -la dist/XDownloader.exe`
Expected: 크기 ≤ **16,540,000 bytes (16.54 MB)**. 초과 시 원인(신규 수집 모듈/누락 제외) 조사 후 재빌드.

- [ ] **Step 4: 최종 실기동 스모크**

`.venv/Scripts/python.exe main.py` — 다운로드/변환/히스토리 기능 동작 + 다크/라이트·한/En 토글 정상 최종 확인.

- [ ] **Step 5: 커밋(있으면)** — 빌드 산출물은 `.gitignore` 대상이므로 커밋 없음. 검증 로그만 보고.

---

## Self-Review

- **Spec coverage:** §5.1 토큰 모듈→Task1 · §5.3 위젯→Task2 · §6 재스킨→Task3/4 · §7 테마전환→Task4 · §8 용량→Task6 · §10 테스트→Task1/2/6 · §12 DESIGN.md→Task5. 전 항목 매핑됨.
- **Placeholder scan:** 코드 스텝은 실제 코드 포함. `_build_ui` 전체 치환은 "기존 구조 유지 + 위젯 치환"으로 명시(위젯별 코드 제시). 누락 없음.
- **Type/이름 정합:** `set_theme/get_theme/c/f/dip`, `FlatButton(btn_style=)`, `SectionCard.body`, `self.fmt/_on_format_toggle/_refresh_format_segment` 가 Task 간 일관.
- **컨벤션:** 탭·`except Exception:`·한국어 주석·토큰 접근 규칙 전 태스크 준수.
