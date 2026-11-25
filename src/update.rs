//! App update check and clipboard link detection.
//!
//! The portal publishes a small JSON next to the exe. A newer version only shows a notice
//! that opens the download page in the browser; the app never replaces itself.
use serde::Deserialize;

/// Written by `packaging/release.ps1` and uploaded with the exe.
pub const MANIFEST_URL: &str = "https://xtool.duckdns.org/dl/xdownload2.json";
/// The only place a download link may point to.
const TRUSTED_PREFIX: &str = "https://xtool.duckdns.org/";

#[derive(Deserialize, Clone, PartialEq, Debug)]
pub struct Release {
    pub version: String,
    pub url: String,
}

fn numbers(version: &str) -> Vec<u64> {
    version.trim().trim_start_matches('v').split('.').map(|part| part.parse().unwrap_or(0)).collect()
}

/// True when `candidate` is a higher version than `current` ("1.10.0" > "1.9.3").
pub fn newer(current: &str, candidate: &str) -> bool {
    let (mut a, mut b) = (numbers(current), numbers(candidate));
    let len = a.len().max(b.len());
    a.resize(len, 0);
    b.resize(len, 0);
    b > a
}

/// The release described by `json` when it is newer than `current` and its link is ours.
pub fn parse(json: &str, current: &str) -> Option<Release> {
    // Windows PowerShell 5 writes a byte-order mark in front of UTF-8 files.
    let release: Release = serde_json::from_str(json.trim_start_matches('\u{feff}')).ok()?;
    (newer(current, &release.version) && release.url.starts_with(TRUSTED_PREFIX) && !release.url.contains(char::is_whitespace)).then_some(release)
}

/// Asks the portal for a newer release. Any failure (offline, no file yet) is simply "none".
pub fn check() -> Option<Release> {
    parse(&crate::binaries::fetch_text(MANIFEST_URL).ok()?, env!("CARGO_PKG_VERSION"))
}

/// YouTube links found in `text`, in order, without repeats.
pub fn youtube_links(text: &str) -> Vec<String> {
    let mut links: Vec<String> = Vec::new();
    for word in text.split_whitespace() {
        let Some(rest) = word.strip_prefix("https://").or_else(|| word.strip_prefix("http://")) else { continue };
        let host = rest.split(['/', '?']).next().unwrap_or_default().to_lowercase();
        let youtube = host == "youtu.be" || host == "youtube.com" || host.ends_with(".youtube.com");
        if youtube && rest.contains('/') && !links.iter().any(|link| link == word) {
            links.push(word.to_owned());
        }
    }
    links
}

/// Links from `clipboard` that are not yet among the lines of `existing`.
pub fn new_links(clipboard: &str, existing: &str) -> Vec<String> {
    youtube_links(clipboard).into_iter().filter(|link| !existing.lines().any(|line| line.trim() == link)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions_compare_by_number() {
        assert!(newer("1.9.3", "1.10.0") && newer("2.0.0", "2.0.1") && newer("2.0", "2.0.1") && newer("1.0.0", "v2"));
        assert!(!newer("2.0.0", "2.0.0") && !newer("2.0.1", "2.0") && !newer("2.0.0", "1.99.99") && !newer("2.0.0", "garbage"));
    }

    #[test]
    fn only_a_newer_release_with_a_trusted_link_is_offered() {
        let json = |version: &str, url: &str| format!(r#"{{"version": "{version}", "url": "{url}", "sha256": "ab"}}"#);
        let good = "https://xtool.duckdns.org/dl/xdownload2.exe";
        assert_eq!(parse(&json("2.1.0", good), "2.0.0"), Some(Release { version: "2.1.0".into(), url: good.into() }));
        assert_eq!(parse(&json("2.0.0", good), "2.0.0"), None);
        assert!(parse(&format!("\u{feff}{}", json("2.1.0", good)), "2.0.0").is_some());
        assert_eq!(parse(&json("9.0.0", "https://evil.example/x.exe"), "2.0.0"), None);
        assert_eq!(parse(&json("9.0.0", "http://xtool.duckdns.org/dl/x.exe"), "2.0.0"), None);
        assert_eq!(parse(&json("9.0.0", "https://xtool.duckdns.org/a b"), "2.0.0"), None);
        assert_eq!(parse("<html>404</html>", "2.0.0"), None);
    }

    #[test]
    fn youtube_links_are_found_and_deduplicated() {
        let text = "look https://www.youtube.com/watch?v=abc and\nhttps://youtu.be/xyz https://youtu.be/xyz https://example.com/watch?v=1 youtube.com/nolink https://notyoutube.com/a https://music.youtube.com/watch?v=m";
        assert_eq!(youtube_links(text), ["https://www.youtube.com/watch?v=abc", "https://youtu.be/xyz", "https://music.youtube.com/watch?v=m"]);
        assert!(youtube_links("https://youtube.com").is_empty(), "the bare site is not a video");
        assert_eq!(new_links(text, "  https://youtu.be/xyz \nother"), ["https://www.youtube.com/watch?v=abc", "https://music.youtube.com/watch?v=m"]);
    }
}
