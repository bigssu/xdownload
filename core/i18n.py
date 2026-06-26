"""UI 문자열 다국어 테이블 — 한국어(ko)/영어(en).

main_frame이 tr(lang, key)로 조회한다. 키가 없으면 한국어로, 그것도 없으면
키 자체를 돌려줘 번역 누락이 앱 크래시로 이어지지 않게 한다.
"""

LANGS = ("ko", "en")

_STRINGS = {
	"ko": {
		"tip_theme": "다크 모드 전환",
		"url_label": "YouTube 링크 (여러 개는 줄바꿈)",
		"format_box": "포맷",
		"format_mp4": "MP4 (영상)",
		"format_mp3": "MP3 (음원)",
		"quality_box": "화질 (MP4)",
		"quality_best": "최고 화질",
		"quality_1080": "1080p",
		"quality_720": "720p",
		"folder_label": "저장 폴더:",
		"folder_btn": "변경(&B)",
		"download_btn": "다운로드(&D)",
		"cancel_btn": "취소",
		"status_idle": "대기 중",
		"history_label": "최근 다운로드 (더블클릭 → 폴더 열기)",
		"prep_binaries": "yt-dlp/ffmpeg 준비 중 (최초 1회)...",
		"downloading": "({i}/{n}) 다운로드 중...",
		"progress_file": "({i}/{n}) {name}",
		"binary_fail": "바이너리 준비 실패: {error}",
		"item_error": "({i}/{n}) 오류: {error}",
		"cancelling": "취소 중...",
		"cancelled": "취소됨 — 성공 {s}, 실패 {f}",
		"done_fail": "완료 — 성공 {s}, 실패 {f}",
		"done_ok": "완료 — {s}개 다운로드",
		"need_url": "링크를 입력하세요.",
		"pick_folder": "저장 폴더 선택",
	},
	"en": {
		"tip_theme": "Toggle dark mode",
		"url_label": "YouTube links (one per line)",
		"format_box": "Format",
		"format_mp4": "MP4 (video)",
		"format_mp3": "MP3 (audio)",
		"quality_box": "Quality (MP4)",
		"quality_best": "Best",
		"quality_1080": "1080p",
		"quality_720": "720p",
		"folder_label": "Save folder:",
		"folder_btn": "Change(&B)",
		"download_btn": "Download(&D)",
		"cancel_btn": "Cancel",
		"status_idle": "Idle",
		"history_label": "Recent downloads (double-click → open folder)",
		"prep_binaries": "Preparing yt-dlp/ffmpeg (first run only)...",
		"downloading": "({i}/{n}) Downloading...",
		"progress_file": "({i}/{n}) {name}",
		"binary_fail": "Binary setup failed: {error}",
		"item_error": "({i}/{n}) Error: {error}",
		"cancelling": "Cancelling...",
		"cancelled": "Cancelled — {s} ok, {f} failed",
		"done_fail": "Done — {s} ok, {f} failed",
		"done_ok": "Done — {s} downloaded",
		"need_url": "Enter at least one link.",
		"pick_folder": "Select save folder",
	},
}


def tr(lang, key):
	table = _STRINGS.get(lang, _STRINGS["ko"])
	if key in table:
		return table[key]
	return _STRINGS["ko"].get(key, key)
