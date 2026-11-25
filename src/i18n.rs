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
    ("tab_download", "다운로드", "Download"),
    ("tab_convert", "변환", "Convert"),
    ("tab_history", "기록", "History"),
    ("tab_settings", "설정", "Settings"),
    ("url_label", "YouTube 링크 (여러 개는 줄바꿈)", "YouTube links (one per line)"),
    ("opt_subs", "자막 넣기", "Subtitles"),
    ("opt_subs_tip", "한국어·영어 자막을 영상 파일 안에 넣습니다 (MP4)", "Embeds Korean and English subtitles in the video (MP4)"),
    ("opt_info", "썸네일·정보 넣기", "Thumbnail & tags"),
    ("opt_info_tip", "썸네일과 제목·올린이 정보를 파일에 넣습니다", "Embeds the thumbnail, title and uploader in the file"),
    ("opt_playlist", "플레이리스트 전체", "Whole playlist"),
    ("opt_playlist_tip", "영상 링크에 재생목록이 붙어 있으면 목록 전체를 받습니다", "When a video link carries a playlist, download the whole list"),
    ("folder_tip", "저장 폴더 변경", "Change the save folder"),
    ("need_url", "링크를 입력하세요.", "Enter at least one link."),
    ("added", "{n}개를 작업 목록에 넣었습니다.", "Added {n} to the queue."),
    ("already", "이미 작업 목록에 있습니다.", "Already in the queue."),
    ("pasted", "복사한 링크 {n}개를 넣었습니다.", "Pasted {n} copied link(s)."),
    ("queue_empty", "다운로드나 변환을 시작하면 여기에 진행 상황이 표시됩니다.", "Downloads and conversions show their progress here."),
    ("cancel_all", "모두 취소 · Esc", "Cancel all · Esc"),
    ("clear_done", "끝난 항목 지우기", "Clear finished"),
    ("st_waiting", "대기 중", "Waiting"),
    ("st_preparing", "yt-dlp/ffmpeg 준비 중 (최초 1회)", "Preparing yt-dlp/ffmpeg (first run only)"),
    ("st_retrying", "yt-dlp 업데이트 후 재시도 중", "Updating yt-dlp and retrying"),
    ("st_running", "진행 중", "Working"),
    ("st_done", "완료", "Done"),
    ("st_failed_tip", "실패", "Failed"),
    ("st_cancelled", "취소됨", "Cancelled"),
    ("st_cancelling", "취소 중...", "Cancelling..."),
    ("act_cancel", "취소", "Cancel"),
    ("act_retry", "다시 시도", "Retry"),
    ("act_remove", "목록에서 지우기", "Remove from the list"),
    ("act_open_folder", "폴더 열기", "Open folder"),
    ("conv_video", "동영상", "Video"),
    ("conv_image", "이미지", "Image"),
    ("size_tip", "이보다 큰 영상만 줄입니다. 작은 영상은 키우지 않습니다", "Only larger videos are scaled down; small ones are never enlarged"),
    ("subfolders", "하위 폴더 포함", "Include subfolders"),
    ("need_convert_files", "변환할 파일이 없습니다.", "No files to convert."),
    ("encoder_label", "동영상 인코더: {name}", "Video encoder: {name}"),
    ("history_empty", "받은 파일이 여기에 기록됩니다.", "Downloaded files are listed here."),
    ("history_tip", "두 번 클릭하면 파일을 엽니다", "Double-click to open the file"),
    ("act_redo", "다시 받기", "Download again"),
    ("act_delete", "기록에서 지우기", "Remove from history"),
    ("set_parallel", "동시에 받는 개수", "Downloads at once"),
    ("set_gpu", "그래픽카드로 동영상 변환", "Convert video on the graphics card"),
    ("set_gpu_tip", "NVIDIA·Intel·AMD 인코더를 자동으로 고르고, 실패하면 CPU로 다시 변환합니다", "Picks an NVIDIA, Intel or AMD encoder automatically and falls back to the CPU on failure"),
    ("set_autopaste", "복사한 유튜브 링크 자동으로 넣기", "Paste copied YouTube links automatically"),
    ("set_open_done", "다 받으면 저장 폴더 열기", "Open the save folder when downloads finish"),
    ("set_data", "데이터 폴더 열기", "Open the data folder"),
    ("set_data_tip", "설정·기록·yt-dlp·ffmpeg가 있는 폴더", "Folder holding settings, history, yt-dlp and ffmpeg"),
    ("update_available", "새 버전 {v} 받기", "Get version {v}"),
    ("prep_tools", "yt-dlp/ffmpeg 준비 중 (최초 1회)... {p}%", "Preparing yt-dlp/ffmpeg (first run only)... {p}%"),
    ("prep_update", "yt-dlp 최신 버전 확인 중...", "Checking for a newer yt-dlp..."),
    ("prep_fail", "준비 실패(필요할 때 다시 시도): {error}", "Setup failed (will retry when needed): {error}"),
    ("ready", "준비됨", "Ready"),
    ("pick_folder", "저장 폴더 선택", "Select save folder"),
    ("pick_video_files", "동영상 파일 선택", "Select video files"),
    ("pick_video_folder", "동영상 폴더 선택", "Select video folder"),
    ("pick_image_files", "이미지 파일 선택", "Select image files"),
    ("pick_image_folder", "이미지 폴더 선택", "Select image folder"),
    ("video_files", "동영상 파일", "Video files"),
    ("image_files", "이미지 파일", "Image files"),
    ("all_files", "모든 파일", "All files"),
    ("tip_running", "실행 중", "Running"),
    ("tip_waiting", "대기 중", "Waiting"),
    ("tip_done", "완료", "Done"),
    ("tip_video_file", "동영상 파일 고르기", "Choose video files"),
    ("tip_video_folder", "동영상 폴더 고르기", "Choose a video folder"),
    ("tip_image_file", "이미지 파일 고르기", "Choose image files"),
    ("tip_image_folder", "이미지 폴더 고르기", "Choose an image folder"),
    ("tip_best", "최고 화질 (있으면 4K 이상)", "Best available (4K or more when offered)"),
    ("tip_mp4", "MP4 영상", "MP4 video"),
    ("tip_mp3", "MP3 음원 (어디서나 재생)", "MP3 audio (plays everywhere)"),
    ("tip_m4a", "M4A 음원 (원본 음질)", "M4A audio (original quality)"),
    ("tip_source", "원본 크기 그대로", "Keep the source size"),
    ("tip_vq_quality", "화질 우선 (파일이 큼)", "Quality first (larger file)"),
    ("tip_vq_balanced", "균형", "Balanced"),
    ("tip_vq_small", "용량 우선 (화질 조금 낮음)", "Small file (slightly lower quality)"),
    ("tip_to_mp4", "MP4로 (H.264·AAC)", "To MP4 (H.264, AAC)"),
    ("tip_to_mp3", "MP3로 (소리만)", "To MP3 (audio only)"),
    ("tip_to_gif", "움직이는 GIF로", "To animated GIF"),
    ("tip_to_jpg", "JPG로", "To JPG"),
    ("tip_to_png", "PNG로 (투명 유지)", "To PNG (keeps transparency)"),
    ("tip_to_webp", "WebP로 (작은 용량)", "To WebP (small files)"),
    ("tip_download", "다운로드 · Enter", "Download · Enter"),
    ("tip_drop", "원본은 그대로 두고 옆에 새 파일을 만듭니다. 창에 끌어다 놓아도 됩니다", "Keeps the original and writes the new file next to it. Dropping on the window works too"),
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
            for name in ["{n}", "{name}", "{error}", "{r}", "{w}", "{d}", "{v}", "{p}", "{eta}"] {
                assert_eq!(row.1.contains(name), row.2.contains(name), "{} {name}", row.0);
            }
        }
    }

    #[test]
    fn lookup_follows_language_and_fills_placeholders() {
        // The language is process-global, so this is the only test that switches it.
        set_language(1);
        assert_eq!(tr("act_cancel"), "Cancel");
        assert_eq!(fill("encoder_label", &[("name", "CPU")]), "Video encoder: CPU");
        set_language(7);
        assert_eq!(tr("act_cancel"), "취소");
        assert_eq!(fill("added", &[("n", "3")]), "3개를 작업 목록에 넣었습니다.");
        assert_eq!(tr("no-such-key"), "no-such-key");
    }
}
