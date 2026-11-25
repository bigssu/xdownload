//! User settings and download history, kept as JSON in %APPDATA%\Xdownload2.
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

pub const HISTORY_MAX: usize = 100;
/// Largest video height offered, in menu order; 0 means no limit.
pub const HEIGHTS: [u32; 6] = [0, 2160, 1440, 1080, 720, 480];

/// %APPDATA%\Xdownload2 — settings, history, cached binaries and logs.
pub fn app_dir() -> Option<PathBuf> {
    std::env::var_os("APPDATA").map(|dir| PathBuf::from(dir).join("Xdownload2"))
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Debug)]
#[serde(default)]
pub struct Settings {
    /// 0 Korean, 1 English.
    pub language: u8,
    pub download_dir: String,
    // Download
    /// 0 MP4, 1 MP3, 2 M4A.
    pub format: u8,
    /// Largest video height; 0 = best available.
    pub height: u32,
    /// Embed Korean / English subtitles (MP4 only).
    pub subs: bool,
    /// Embed the thumbnail and title / uploader tags.
    pub embed_info: bool,
    /// A playlist link downloads the whole list instead of the one video.
    pub playlist: bool,
    /// Downloads running at once, 1..=5.
    pub parallel: u8,
    // Conversion
    /// 0 MP4, 1 MP3, 2 GIF.
    pub video_target: u8,
    /// Largest output height for MP4; 0 keeps the source size.
    pub video_height: u32,
    /// 0 quality first, 1 balanced, 2 small file.
    pub video_quality: u8,
    /// 0 JPG, 1 PNG, 2 WebP.
    pub image_target: u8,
    /// A picked or dropped folder also contributes the files in its subfolders.
    pub subfolders: bool,
    /// Use the graphics card's H.264 encoder when one works.
    pub gpu: bool,
    // App
    /// Add a YouTube link found on the clipboard when the window gets focus.
    pub auto_paste: bool,
    /// Open the save folder when every queued download has finished.
    pub open_when_done: bool,
    /// Local date (YYYY-MM-DD) of the last yt-dlp self-update, so it runs once a day.
    pub last_update_day: String,
}

impl Default for Settings {
    fn default() -> Self {
        let home = std::env::var_os("USERPROFILE").map(PathBuf::from).unwrap_or_default();
        Self {
            language: 0, download_dir: home.join("Downloads").to_string_lossy().into_owned(),
            format: 0, height: 0, subs: false, embed_info: false, playlist: false, parallel: 3,
            video_target: 0, video_height: 1080, video_quality: 1, image_target: 0, subfolders: false, gpu: true,
            auto_paste: true, open_when_done: false, last_update_day: String::new(),
        }
    }
}

impl Settings {
    pub fn load() -> Self {
        app_dir().map(|dir| read_json(&dir.join("settings.json"))).unwrap_or_default()
    }

    pub fn store(&self) {
        if let Some(dir) = app_dir() {
            write_json(&dir.join("settings.json"), self);
        }
    }

    pub fn parallel(&self) -> usize {
        self.parallel.clamp(1, 5) as usize
    }
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Debug, Default)]
#[serde(default)]
pub struct Entry {
    pub url: String,
    /// Video title; empty in entries written by 1.0.
    pub title: String,
    /// The downloaded file; empty in entries written by 1.0.
    pub file: String,
    pub format: String,
    pub quality: String,
    /// Folder the file was saved in.
    pub path: String,
    pub when: String,
}

pub fn load_history() -> Vec<Entry> {
    app_dir().map(|dir| read_json(&dir.join("history.json"))).unwrap_or_default()
}

pub fn store_history(items: &[Entry]) {
    if let Some(dir) = app_dir() {
        write_json(&dir.join("history.json"), &items);
    }
}

/// Newest first, capped at `HISTORY_MAX`.
pub fn push_history(items: &mut Vec<Entry>, entry: Entry) {
    items.insert(0, entry);
    items.truncate(HISTORY_MAX);
}

/// A missing or damaged file reads as the default value.
fn read_json<T: Default + serde::de::DeserializeOwned>(path: &Path) -> T {
    std::fs::read_to_string(path).ok().and_then(|text| serde_json::from_str(&text).ok()).unwrap_or_default()
}

/// Writes to a temporary file and renames it, so a failed write never damages the old file.
/// Saving is best effort: a failure is not worth interrupting the user for.
fn write_json<T: Serialize>(path: &Path, value: &T) {
    let _ = (|| -> std::io::Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let tmp = path.with_extension("json.tmp");
        std::fs::write(&tmp, serde_json::to_string_pretty(value)?)?;
        std::fs::rename(tmp, path)
    })();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_and_partial_files_fall_back_to_defaults() {
        let dir = std::env::temp_dir().join(format!("xdownload2_settings_{}", std::process::id()));
        let file = dir.join("nested/settings.json");
        assert_eq!(read_json::<Settings>(&file), Settings::default());
        let saved = Settings { language: 1, download_dir: "D:/v".into(), format: 2, height: 720, parallel: 5, gpu: false, ..Default::default() };
        write_json(&file, &saved);
        assert_eq!(read_json::<Settings>(&file), saved);
        assert!(!file.with_extension("json.tmp").exists());
        // Unknown fields (1.0 wrote "mp3" and "quality") and missing ones are tolerated.
        std::fs::write(&file, r#"{"mp3": true, "quality": 2, "language": 1}"#).unwrap();
        let partial: Settings = read_json(&file);
        assert_eq!(partial, Settings { language: 1, ..Default::default() });
        std::fs::write(&file, "not json").unwrap();
        assert_eq!(read_json::<Settings>(&file), Settings::default());
        // A 1.0 history entry has no title or file.
        std::fs::write(&file, r#"[{"url": "u", "format": "mp4", "quality": "best", "path": "C:/d", "when": "w"}]"#).unwrap();
        let history: Vec<Entry> = read_json(&file);
        assert_eq!((history.len(), history[0].url.as_str(), history[0].file.as_str()), (1, "u", ""));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn parallel_downloads_are_clamped() {
        assert_eq!(Settings::default().parallel(), 3);
        assert_eq!(Settings { parallel: 0, ..Default::default() }.parallel(), 1);
        assert_eq!(Settings { parallel: 99, ..Default::default() }.parallel(), 5);
    }

    #[test]
    fn history_is_newest_first_and_capped() {
        let mut items = Vec::new();
        for n in 0..HISTORY_MAX + 5 {
            push_history(&mut items, Entry { url: n.to_string(), ..Default::default() });
        }
        assert_eq!(items.len(), HISTORY_MAX);
        assert_eq!(items[0].url, (HISTORY_MAX + 4).to_string());
        assert_eq!(items[HISTORY_MAX - 1].url, "5");
    }
}
