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
