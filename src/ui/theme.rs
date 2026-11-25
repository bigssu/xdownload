//! Shared dark design tokens and installed-system-font setup (the Xgif2 design system).
use eframe::egui::{self, Color32, FontFamily, FontId, Stroke, Vec2};
use std::sync::Arc;

// Elevation ladder: WELL (sunken fields) < BG (window chrome) < PANEL (content) < RAISED
// (controls, cards) < HOVER.
pub const WELL: Color32 = Color32::from_rgb(0x08, 0x0a, 0x0e);
pub const BG: Color32 = Color32::from_rgb(0x0f, 0x12, 0x18);
pub const PANEL: Color32 = Color32::from_rgb(0x19, 0x1d, 0x29);
pub const RAISED: Color32 = Color32::from_rgb(0x22, 0x28, 0x36);
pub const HOVER: Color32 = Color32::from_rgb(0x2c, 0x33, 0x44);
pub const LINE: Color32 = Color32::from_rgb(0x29, 0x2f, 0x3e);
pub const ELEVATED: Color32 = Color32::from_rgb(0x2a, 0x31, 0x44);
pub const ELEVATED_LINE: Color32 = Color32::from_rgb(0x41, 0x4b, 0x65);
/// Tooltips are inverted (light card, dark ink) so they never blend into the dark panels.
pub const TIP_BG: Color32 = Color32::from_rgb(0xe8, 0xec, 0xf3);
pub const TIP_TEXT: Color32 = Color32::from_rgb(0x12, 0x16, 0x1f);
/// Chosen option in segmented controls: a light pill that reads at a glance.
pub const SELECTED: Color32 = TEXT;
pub const ON_SELECTED: Color32 = TIP_TEXT;
pub const TEXT: Color32 = Color32::from_rgb(0xe8, 0xec, 0xf3);
pub const MUTED: Color32 = Color32::from_rgb(0x8d, 0x96, 0xa6);
pub const ACCENT: Color32 = Color32::from_rgb(0x6e, 0x8b, 0xff);
pub const ACCENT_END: Color32 = Color32::from_rgb(142, 108, 255);
/// Errors, the close button and the cancel button.
pub const RED: Color32 = Color32::from_rgb(0xff, 0x4d, 0x5e);
pub const RED_TOP: Color32 = Color32::from_rgb(255, 90, 105);
pub const RED_BOTTOM: Color32 = Color32::from_rgb(240, 62, 80);
pub const SUCCESS: Color32 = Color32::from_rgb(0x2b, 0xd3, 0x8a);
pub const WINDOW_R: u8 = 14;

pub fn setup(ctx: &egui::Context) {
    ctx.set_theme(egui::Theme::Dark);
    let mut fonts = egui::FontDefinitions::default();
    for (name, paths, first) in [
        ("hangul", &["C:/Windows/Fonts/malgun.ttf", "C:/Windows/Fonts/gulim.ttc"][..], false),
        ("latin", &["C:/Windows/Fonts/SegUIVar.ttf", "C:/Windows/Fonts/segoeui.ttf"][..], true),
    ] {
        for path in paths {
            if let Ok(bytes) = std::fs::read(path) {
                fonts.font_data.insert(name.into(), Arc::new(egui::FontData::from_owned(bytes)));
                let list = fonts.families.entry(FontFamily::Proportional).or_default();
                if first {
                    list.insert(0, name.into());
                } else {
                    list.push(name.into());
                }
                // Links and paths typed in the monospace family still need Hangul.
                if name == "hangul" {
                    fonts.families.entry(FontFamily::Monospace).or_default().push(name.into());
                }
                break;
            }
        }
    }
    // Phosphor icon glyphs live in the private-use area, so they never shadow text glyphs.
    egui_phosphor::add_to_fonts(&mut fonts, egui_phosphor::Variant::Regular);
    ctx.set_fonts(fonts);
    let mut visuals = egui::Visuals::dark();
    visuals.panel_fill = PANEL;
    visuals.window_fill = ELEVATED;
    visuals.extreme_bg_color = WELL;
    visuals.faint_bg_color = RAISED;
    visuals.override_text_color = Some(TEXT);
    visuals.selection.bg_fill = ACCENT.gamma_multiply(0.55);
    visuals.selection.stroke = Stroke::new(1.0_f32, ACCENT);
    visuals.window_corner_radius = WINDOW_R.into();
    visuals.window_stroke = Stroke::new(1.0_f32, ELEVATED_LINE);
    for widget in [&mut visuals.widgets.inactive, &mut visuals.widgets.noninteractive] {
        widget.bg_fill = RAISED;
        widget.weak_bg_fill = RAISED;
        widget.bg_stroke = Stroke::new(1.0_f32, LINE);
        widget.fg_stroke = Stroke::new(1.0_f32, TEXT);
        widget.corner_radius = 8.into();
    }
    visuals.widgets.hovered.bg_fill = HOVER;
    visuals.widgets.hovered.weak_bg_fill = HOVER;
    visuals.widgets.hovered.bg_stroke = Stroke::new(1.0_f32, ACCENT);
    visuals.widgets.hovered.corner_radius = 8.into();
    visuals.widgets.active.bg_fill = ACCENT.gamma_multiply(0.35);
    visuals.widgets.active.weak_bg_fill = ACCENT.gamma_multiply(0.35);
    visuals.widgets.active.corner_radius = 8.into();
    ctx.set_visuals(visuals);
    ctx.style_mut(|style| {
        style.animation_time = 0.18;
        style.spacing.button_padding = Vec2::new(10.0, 5.0);
        style.spacing.item_spacing = Vec2::new(8.0, 8.0);
        style.spacing.interact_size.y = 26.0;
        style.text_styles.insert(egui::TextStyle::Body, FontId::proportional(13.5));
        style.text_styles.insert(egui::TextStyle::Button, FontId::proportional(13.0));
        style.text_styles.insert(egui::TextStyle::Small, FontId::proportional(11.5));
        style.text_styles.insert(egui::TextStyle::Heading, FontId::proportional(20.0));
        style.text_styles.insert(egui::TextStyle::Monospace, FontId::monospace(13.0));
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn luminance(c: Color32) -> f32 {
        let lin = |v: u8| {
            let v = v as f32 / 255.0;
            if v <= 0.03928 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) }
        };
        0.2126 * lin(c.r()) + 0.7152 * lin(c.g()) + 0.0722 * lin(c.b())
    }

    fn ratio(a: Color32, b: Color32) -> f32 {
        let (a, b) = (luminance(a), luminance(b));
        (a.max(b) + 0.05) / (a.min(b) + 0.05)
    }

    #[test]
    fn text_and_background_pairs_meet_wcag_aa() {
        for (fg, bg, min) in [
            (TEXT, BG, 4.5), (TEXT, PANEL, 4.5), (TEXT, RAISED, 4.5), (TEXT, WELL, 4.5), (MUTED, PANEL, 4.5),
            (SUCCESS, PANEL, 4.5), (RED, PANEL, 4.5), (ON_SELECTED, SELECTED, 4.5), (TIP_TEXT, TIP_BG, 4.5),
            (Color32::WHITE, ACCENT, 3.0),
        ] {
            assert!(ratio(fg, bg) >= min, "{fg:?} on {bg:?} = {:.2} < {min}", ratio(fg, bg));
        }
    }

    #[test]
    fn setup_applies_the_window_radius() {
        let ctx = egui::Context::default();
        setup(&ctx);
        assert_eq!(ctx.style().visuals.window_corner_radius, egui::CornerRadius::same(WINDOW_R));
    }
}
