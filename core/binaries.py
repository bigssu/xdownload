"""yt-dlp / ffmpeg 바이너리를 첫 실행 시 자동 다운로드해 캐시한다.

exe에 바이너리를 번들하지 않으므로(작은 배포 크기) 최초 1회 인터넷에서 받는다.
yt-dlp는 자가 업데이트(-U)로 최신을 유지하므로 YouTube 변경에 자동 대응한다.
"""

import shutil
import tempfile
import zipfile
from pathlib import Path

import requests

from core.config import APP_DIR

BIN_DIR = APP_DIR / "bin"

YTDLP_URL = (
	"https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp.exe"
)
# BtbN essentials win64 빌드(zip) — 압축 안의 bin/ffmpeg.exe만 추출한다.
FFMPEG_ZIP_URL = (
	"https://github.com/BtbN/FFmpeg-Builds/releases/download/latest/"
	"ffmpeg-master-latest-win64-gpl.zip"
)


def _download_file(url, dest, on_progress=None, label=""):
	"""스트리밍 다운로드 → .part 임시파일 → 원자적 교체."""
	tmp = dest.with_suffix(dest.suffix + ".part")
	with requests.get(url, stream=True, timeout=120) as resp:
		resp.raise_for_status()
		total = int(resp.headers.get("content-length", 0) or 0)
		done = 0
		with open(tmp, "wb") as handle:
			for chunk in resp.iter_content(chunk_size=1 << 20):
				if not chunk:
					continue
				handle.write(chunk)
				done += len(chunk)
				if on_progress and total:
					on_progress(label, done / total)
	tmp.replace(dest)


def _download_ffmpeg(dest, on_progress=None):
	with tempfile.TemporaryDirectory() as tmpdir:
		zip_path = Path(tmpdir) / "ffmpeg.zip"
		_download_file(FFMPEG_ZIP_URL, zip_path, on_progress, "ffmpeg")
		with zipfile.ZipFile(zip_path) as archive:
			member = next(
				(n for n in archive.namelist() if n.endswith("/bin/ffmpeg.exe")),
				None,
			)
			if member is None:
				raise RuntimeError("압축에서 ffmpeg.exe를 찾지 못했습니다.")
			with archive.open(member) as src, open(dest, "wb") as out:
				shutil.copyfileobj(src, out)


def ensure_binaries(on_progress=None):
	"""yt-dlp.exe, ffmpeg.exe 경로를 보장해 (ytdlp_path, ffmpeg_path)로 반환한다."""
	BIN_DIR.mkdir(parents=True, exist_ok=True)
	ytdlp = BIN_DIR / "yt-dlp.exe"
	ffmpeg = BIN_DIR / "ffmpeg.exe"
	if not ytdlp.exists():
		_download_file(YTDLP_URL, ytdlp, on_progress, "yt-dlp")
	if not ffmpeg.exists():
		_download_ffmpeg(ffmpeg, on_progress)
	return str(ytdlp), str(ffmpeg)
