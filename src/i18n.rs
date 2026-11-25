//! Static, allocation-free UI text lookup (Korean / English).
use std::sync::atomic::{AtomicU8, Ordering};

static LANGUAGE: AtomicU8 = AtomicU8::new(0);

/// 0 Korean, 1 English. Unknown values render as Korean.
pub fn set_language(value: u8) {
    LANGUAGE.store(u8::from(value == 1), Ordering::Relaxed);
}

pub fn language() -> u8 {
    LANGUAGE.load(Ordering::Relaxed)
}

/// Text for `key` in the current language; an unknown key is shown as-is rather than crashing.
pub fn tr(key: &'static str) -> &'static str {
    TABLE.iter().find(|row| row.0 == key).map(|row| if language() == 1 { row.2 } else { row.1 }).unwrap_or(key)
}

/// `tr(key)` with every `{name}` placeholder replaced.
pub fn fill(key: &'static str, values: &[(&str, &str)]) -> String {
    values.iter().fold(tr(key).to_owned(), |text, (name, value)| text.replace(&format!("{{{name}}}"), value))
}

pub static TABLE: &[(&str, &str, &str)] = &[
    ("url_label", "YouTube 링크 (여러 개는 줄바꿈)", "YouTube links (one per line)"),
    ("format_mp4", "MP4 (영상)", "MP4 (video)"),
    ("format_mp3", "MP3 (음원)", "MP3 (audio)"),
    ("quality_best", "최고 화질", "Best"),
    ("quality_tip", "화질 (MP4)", "Quality (MP4)"),
    ("folder_tip", "저장 폴더 변경", "Change the save folder"),
    ("download_btn", "다운로드", "Download"),
    ("cancel_btn", "취소", "Cancel"),
    ("convert_box", "로컬 파일 변환", "Local file conversion"),
    ("video_file_btn", "동영상을 MP4로", "Video to MP4"),
    ("video_folder_btn", "폴더를 MP4로", "Folder to MP4"),
    ("image_file_btn", "이미지를 JPG로", "Image to JPG"),
    ("image_folder_btn", "폴더를 JPG로", "Folder to JPG"),
    ("video_tip", "원본은 그대로 두고 옆에 MP4(H.264·AAC·1080p 이하)를 만듭니다", "Keeps the original and writes an MP4 (H.264, AAC, up to 1080p) next to it"),
    ("image_tip", "원본은 그대로 두고 옆에 JPG를 만듭니다", "Keeps the original and writes a JPG next to it"),
    ("status_idle", "대기 중", "Idle"),
    ("history_label", "최근 다운로드", "Recent downloads"),
    ("history_tip", "두 번 클릭하면 폴더를 엽니다", "Double-click to open the folder"),
    ("prep_binaries", "yt-dlp/ffmpeg 준비 중 (최초 1회)...", "Preparing yt-dlp/ffmpeg (first run only)..."),
    ("prep_ffmpeg", "ffmpeg 준비 중 (최초 1회)...", "Preparing ffmpeg (first run only)..."),
    ("downloading", "({i}/{n}) 다운로드 중...", "({i}/{n}) Downloading..."),
    ("progress_file", "({i}/{n}) {name}", "({i}/{n}) {name}"),
    ("convert_progress_file", "({i}/{n}) 변환 중: {name}", "({i}/{n}) Converting: {name}"),
    ("binary_fail", "바이너리 준비 실패: {error}", "Binary setup failed: {error}"),
    ("item_error", "({i}/{n}) 오류: {error}", "({i}/{n}) Error: {error}"),
    ("convert_item_error", "({i}/{n}) 변환 오류: {error}", "({i}/{n}) Conversion error: {error}"),
    ("retry_update", "yt-dlp 업데이트 후 재시도 중...", "Updating yt-dlp and retrying..."),
    ("cancelling", "취소 중...", "Cancelling..."),
    ("cancelled", "취소됨 — 성공 {s}, 실패 {f}", "Cancelled — {s} ok, {f} failed"),
    ("done_fail", "완료 — 성공 {s}, 실패 {f}", "Done — {s} ok, {f} failed"),
    ("done_ok", "완료 — {s}개 다운로드", "Done — {s} downloaded"),
    ("convert_done_ok", "완료 — {s}개 변환", "Done — {s} converted"),
    ("need_url", "링크를 입력하세요.", "Enter at least one link."),
    ("need_convert_files", "변환할 파일이 없습니다.", "No files to convert."),
    ("pick_folder", "저장 폴더 선택", "Select save folder"),
    ("pick_video_files", "동영상 파일 선택", "Select video files"),
    ("pick_video_folder", "동영상 폴더 선택", "Select video folder"),
    ("pick_image_files", "이미지 파일 선택", "Select image files"),
    ("pick_image_folder", "이미지 폴더 선택", "Select image folder"),
    ("video_files", "동영상 파일", "Video files"),
    ("image_files", "이미지 파일", "Image files"),
    ("all_files", "모든 파일", "All files"),
    ("lang_tip", "언어: 한국어 / English", "Language: 한국어 / English"),
    ("close", "닫기", "Close"),
    ("maximize", "최대화", "Maximize"),
    ("minimize", "최소화", "Minimize"),
    ("checksum_missing", "{name}의 체크섬을 목록에서 찾지 못했습니다.", "No checksum listed for {name}."),
    ("checksum_fail", "{name} 무결성 검증 실패", "{name} failed its integrity check"),
    ("zip_no_ffmpeg", "압축에서 ffmpeg.exe를 찾지 못했습니다.", "ffmpeg.exe was not found in the archive."),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_key_is_unique_and_translated_in_both_languages() {
        for (i, row) in TABLE.iter().enumerate() {
            assert!(!row.1.is_empty() && !row.2.is_empty(), "{}", row.0);
            assert!(TABLE[..i].iter().all(|other| other.0 != row.0), "duplicate key {}", row.0);
            // Both languages must carry the same placeholders.
            for name in ["{i}", "{n}", "{name}", "{error}", "{s}", "{f}"] {
                assert_eq!(row.1.contains(name), row.2.contains(name), "{} {name}", row.0);
            }
        }
    }

    #[test]
    fn lookup_follows_language_and_fills_placeholders() {
        // The language is process-global, so this is the only test that switches it.
        set_language(1);
        assert_eq!(tr("cancel_btn"), "Cancel");
        assert_eq!(fill("cancelled", &[("s", "2"), ("f", "1")]), "Cancelled — 2 ok, 1 failed");
        set_language(7);
        assert_eq!(tr("cancel_btn"), "취소");
        assert_eq!(fill("downloading", &[("i", "1"), ("n", "3")]), "(1/3) 다운로드 중...");
        assert_eq!(tr("no-such-key"), "no-such-key");
    }
}
