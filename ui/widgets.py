"""Xchange 이식 공용 UI 위젯 — 디자인 토큰 기반 커스텀 페인트."""

import wx
import wx.adv

from ui.theme_tokens import PAD_M, c, dip, f


class BasePanel(wx.Panel):
	def __init__(self, parent, bg_key="bg", **kw):
		style = kw.pop("style", 0) | wx.CLIP_CHILDREN
		super().__init__(parent, style=style, **kw)
		self.SetBackgroundColour(c(bg_key))
		self.SetDoubleBuffered(True)


class FlatButton(wx.Window):
	"""커스텀 페인트 플랫 버튼 (비트맵 캐시)."""

	H = 32
	RADIUS = 6

	def __init__(self, parent, label, btn_style="secondary", min_w=80):
		h = dip(self.H)
		mw = dip(min_w)
		super().__init__(parent, size=wx.Size(mw, h))
		self.label = label
		self.btn_style = btn_style
		self._hover = False
		self._cache_state = None
		self._cache_bmp = None
		self._dip_1 = dip(1)
		self._dip_radius = dip(self.RADIUS)
		self.SetMinSize(wx.Size(mw, h))
		self.SetBackgroundStyle(wx.BG_STYLE_PAINT)
		self.SetCursor(wx.Cursor(wx.CURSOR_HAND))
		self.Bind(wx.EVT_PAINT, self._on_paint)
		self.Bind(wx.EVT_ENTER_WINDOW, lambda e: self._set_hover(True))
		self.Bind(wx.EVT_LEAVE_WINDOW, lambda e: self._set_hover(False))
		self.Bind(wx.EVT_LEFT_UP, self._on_click)

	def _on_click(self, event):
		# 커스텀 위젯이므로 표준 버튼 이벤트를 직접 발생시킨다.
		if self.IsEnabled():
			evt = wx.CommandEvent(wx.EVT_BUTTON.typeId, self.GetId())
			evt.SetEventObject(self)
			self.GetEventHandler().ProcessEvent(evt)
		event.Skip()

	def SetLabel(self, label):
		self.label = label
		self.Refresh()

	def _set_hover(self, value):
		if self._hover != value:
			self._hover = value
			self.Refresh()

	def _on_paint(self, _):
		dc = wx.AutoBufferedPaintDC(self)
		w, h = self.GetClientSize()
		enabled = self.IsEnabled()
		state = (self._hover, enabled, self.btn_style, self.label, w, h)
		if self._cache_state == state and self._cache_bmp is not None:
			dc.DrawBitmap(self._cache_bmp, 0, 0)
			return

		bmp = wx.Bitmap(w, h)
		mdc = wx.MemoryDC(bmp)
		mdc.SetBackground(wx.Brush(self.GetParent().GetBackgroundColour()))
		mdc.Clear()
		gc = wx.GraphicsContext.Create(mdc)

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

		gc.SetBrush(gc.CreateBrush(wx.Brush(bg)))
		border_col = c("border") if enabled else c("text_dim")
		gc.SetPen(gc.CreatePen(wx.Pen(border_col, self._dip_1)))
		gc.DrawRoundedRectangle(0, 0, w - 1, h - 1, self._dip_radius)
		gc.SetFont(gc.CreateFont(f(0), text_col))
		tw, th = gc.GetTextExtent(self.label)
		gc.DrawText(self.label, (w - tw) / 2, (h - th) / 2)
		del gc
		mdc.SelectObject(wx.NullBitmap)

		self._cache_state = state
		self._cache_bmp = bmp
		dc.DrawBitmap(bmp, 0, 0)


class ToggleSwitch(wx.Window):
	"""커스텀 페인트 on/off 토글 스위치."""

	TW = 44
	TH = 24

	def __init__(self, parent, value=True):
		tw, th = dip(self.TW), dip(self.TH)
		super().__init__(parent, size=wx.Size(tw, th))
		self.value = value
		self.SetMinSize(wx.Size(tw, th))
		self.SetBackgroundStyle(wx.BG_STYLE_PAINT)
		self.SetCursor(wx.Cursor(wx.CURSOR_HAND))
		self.Bind(wx.EVT_PAINT, self._on_paint)
		self.Bind(wx.EVT_LEFT_UP, lambda e: self._toggle())

	def _toggle(self):
		self.value = not self.value
		self.Refresh()
		event = wx.CommandEvent(wx.EVT_CHECKBOX.typeId, self.GetId())
		event.SetEventObject(self)
		event.SetInt(int(self.value))
		self.GetEventHandler().ProcessEvent(event)

	def _on_paint(self, _):
		dc = wx.AutoBufferedPaintDC(self)
		dc.SetBackground(wx.Brush(self.GetParent().GetBackgroundColour()))
		dc.Clear()
		gc = wx.GraphicsContext.Create(dc)
		w, h = self.GetClientSize()

		gc.SetBrush(gc.CreateBrush(wx.Brush(c("toggle_on") if self.value else c("toggle_off"))))
		gc.SetPen(gc.CreatePen(wx.TRANSPARENT_PEN))
		gc.DrawRoundedRectangle(0, 0, w, h, h / 2)

		pad = dip(2)
		td = h - pad * 2
		gc.SetBrush(gc.CreateBrush(wx.Brush(c("toggle_knob"))))
		gc.DrawEllipse((w - td - pad) if self.value else pad, pad, td, td)


class DarkChoice(wx.adv.OwnerDrawnComboBox):
	"""다크 테마 드롭다운 (커스텀 페인트 팝업)."""

	def __init__(self, parent, choices):
		super().__init__(parent, choices=choices, style=wx.CB_READONLY)
		self.SetFont(f())
		self.SetForegroundColour(c("text"))
		self.SetBackgroundColour(c("surface"))

	def OnDrawBackground(self, dc, rect, item, flags):
		if flags & wx.adv.ODCB_PAINTING_SELECTED:
			bg = c("accent")
		else:
			bg = c("surface")
		dc.SetBrush(wx.Brush(bg))
		dc.SetPen(wx.TRANSPARENT_PEN)
		dc.DrawRectangle(rect)

	def OnDrawItem(self, dc, rect, item, flags):
		if item == wx.NOT_FOUND:
			return
		dc.SetFont(self.GetFont())
		dc.SetTextForeground(c("text"))
		text = self.GetString(item)
		th = dc.GetTextExtent(text).height
		dc.DrawText(text, rect.x + dip(4), rect.y + (rect.height - th) // 2)

	def OnMeasureItem(self, item):
		return self.GetCharHeight() + dip(6)


class SectionCard(wx.Panel):
	"""헤더 + 본문 sizer를 가진 라운드 카드 패널."""

	RADIUS = 10

	def __init__(self, parent, title, icon=""):
		super().__init__(parent, style=wx.BORDER_NONE | wx.CLIP_CHILDREN)
		self.SetBackgroundColour(c("card"))
		self.SetDoubleBuffered(True)
		self._last_size = (0, 0)
		self._border_pen = wx.Pen(c("border_card"), dip(1))
		self._dip_radius = dip(self.RADIUS)

		outer = wx.BoxSizer(wx.VERTICAL)
		title_text = f"{icon}  {title}" if icon else title
		lbl = wx.StaticText(self, label=title_text)
		lbl.SetFont(f(0, bold=True))
		lbl.SetForegroundColour(c("text"))
		hdr = wx.BoxSizer(wx.HORIZONTAL)
		hdr.Add(lbl, 0, wx.ALL, PAD_M)
		outer.Add(hdr, 0, wx.EXPAND)

		self.body = wx.BoxSizer(wx.VERTICAL)
		outer.Add(self.body, 1, wx.EXPAND | wx.LEFT | wx.RIGHT | wx.BOTTOM, PAD_M)

		self.SetSizer(outer)
		self.Bind(wx.EVT_PAINT, self._on_paint)
		self.Bind(wx.EVT_SIZE, self._on_size)

	def _on_size(self, event):
		sz = (event.GetSize().width, event.GetSize().height)
		if sz != self._last_size:
			self._last_size = sz
			self.Refresh()
		event.Skip()

	def _on_paint(self, event):
		dc = wx.PaintDC(self)
		w, h = self.GetClientSize()
		dc.SetBrush(wx.TRANSPARENT_BRUSH)
		dc.SetPen(self._border_pen)
		dc.DrawRoundedRectangle(0, 0, w - 1, h - 1, self._dip_radius)
		event.Skip()


class Divider(wx.Panel):
	"""수평 구분선."""

	def __init__(self, parent):
		super().__init__(parent)
		self.SetBackgroundColour(c("divider"))
		self.SetDoubleBuffered(True)
		self.SetMinSize(wx.Size(-1, dip(1)))
