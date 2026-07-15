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
