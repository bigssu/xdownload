//! App icons: Phosphor (Regular) glyphs looked up by a short name.
use eframe::egui::{self, Color32, Rect};
use egui_phosphor::regular as ph;

/// Every icon name the app uses.
#[cfg(test)]
pub const NAMES: [&str; 12] = ["close", "min", "max", "download", "cancel", "folder", "file", "film", "music", "video", "image", "logo"];

pub fn glyph(name: &str) -> Option<&'static str> {
    Some(match name {
        "close" | "cancel" => ph::X,
        "min" => ph::MINUS,
        "max" => ph::SQUARE,
        "download" | "logo" => ph::DOWNLOAD_SIMPLE,
        "folder" => ph::FOLDER_OPEN,
        "file" => ph::FILE,
        "film" => ph::FILM_STRIP,
        "music" => ph::MUSIC_NOTES,
        "video" => ph::FILE_VIDEO,
        "image" => ph::IMAGE,
        _ => return None,
    })
}

/// Draws the icon centred in `rect`; an unknown name is drawn as text.
pub fn paint(p: &egui::Painter, rect: Rect, name: &str, color: Color32) {
    let (text, size) = match glyph(name) {
        Some(g) => (g, rect.height()),
        None => (name, rect.height() * 0.8),
    };
    p.text(rect.center(), egui::Align2::CENTER_CENTER, text, egui::FontId::proportional(size), color);
}

#[cfg(test)]
mod tests {
    #[test]
    fn every_used_icon_name_has_a_glyph() {
        for name in super::NAMES {
            assert!(super::glyph(name).is_some(), "missing glyph for {name}");
        }
        assert!(super::glyph("not-an-icon").is_none());
    }
}
