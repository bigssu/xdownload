//! App icons: Phosphor (Regular) glyphs looked up by a short name.
use eframe::egui::{self, Color32, Rect};
use egui_phosphor::regular as ph;

/// Every icon name the app uses.
#[cfg(test)]
pub const NAMES: [&str; 39] = [
    "close", "min", "max", "download", "cancel", "folder", "file", "film", "music", "video", "image", "logo",
    "retry", "trash", "done", "failed", "waiting", "stopped", "sweep", "open", "update",
    "convert", "history", "settings", "subs", "info", "playlist", "best", "running", "quality", "balanced", "small",
    "source", "subfolders", "gpu", "cpu", "files", "paste", "parallel",
];

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
        "retry" => ph::ARROW_CLOCKWISE,
        "trash" => ph::TRASH,
        "done" => ph::CHECK_CIRCLE,
        "failed" => ph::WARNING_CIRCLE,
        "waiting" => ph::HOURGLASS,
        "stopped" => ph::PROHIBIT,
        "sweep" => ph::BROOM,
        "open" => ph::ARROW_SQUARE_OUT,
        "update" => ph::ARROW_CIRCLE_UP,
        "convert" => ph::SWAP,
        "history" => ph::CLOCK_COUNTER_CLOCKWISE,
        "settings" => ph::GEAR,
        "subs" => ph::CLOSED_CAPTIONING,
        "info" => ph::TAG,
        "playlist" => ph::PLAYLIST,
        "best" => ph::CROWN_SIMPLE,
        "running" => ph::PLAY,
        "quality" => ph::DIAMOND,
        "balanced" => ph::SCALES,
        "small" => ph::FEATHER,
        "source" => ph::ARROWS_OUT,
        "subfolders" => ph::TREE_STRUCTURE,
        "gpu" => ph::GRAPHICS_CARD,
        "cpu" => ph::CPU,
        "files" => ph::FILES,
        "paste" => ph::CLIPBOARD_TEXT,
        "parallel" => ph::STACK,
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
