//! User settings and download history, kept as JSON in %APPDATA%\Xdownload2.
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

pub const HISTORY_MAX: usize = 100;
/// Quality choices in menu order; the index is what `Settings::quality` stores.
pub const QUALITIES: [&str; 3] = ["best", "1080p", "720p"];

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
    pub mp3: bool,
    /// Index into `QUALITIES`.
    pub quality: u8,
}

impl Default for Settings {
    fn default() -> Self {
        let home = std::env::var_os("USERPROFILE").map(PathBuf::from).unwrap_or_default();
        Self { language: 0, download_dir: home.join("Downloads").to_string_lossy().into_owned(), mp3: false, quality: 0 }
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

    pub fn quality_name(&self) -> &'static str {
        QUALITIES[(self.quality as usize).min(QUALITIES.len() - 1)]
    }
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Debug, Default)]
#[serde(default)]
pub struct Entry {
    pub url: String,
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
        let saved = Settings { language: 1, download_dir: "D:/v".into(), mp3: true, quality: 2 };
        write_json(&file, &saved);
        assert_eq!(read_json::<Settings>(&file), saved);
        assert!(!file.with_extension("json.tmp").exists());
        // Unknown and missing fields from other versions are tolerated.
        std::fs::write(&file, r#"{"mp3": true, "future_field": 1}"#).unwrap();
        let partial: Settings = read_json(&file);
        assert!(partial.mp3 && partial.download_dir == Settings::default().download_dir);
        std::fs::write(&file, "not json").unwrap();
        assert_eq!(read_json::<Settings>(&file), Settings::default());
        assert!(read_json::<Vec<Entry>>(&file).is_empty());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn quality_index_is_clamped() {
        assert_eq!(Settings { quality: 1, ..Default::default() }.quality_name(), "1080p");
        assert_eq!(Settings { quality: 200, ..Default::default() }.quality_name(), "720p");
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
