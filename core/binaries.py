"""yt-dlp / ffmpeg 바이너리를 첫 실행 시 자동 다운로드해 캐시한다.

exe에 바이너리를 번들하지 않으므로(작은 배포 크기) 최초 1회 인터넷에서 받는다.
yt-dlp는 자가 업데이트(-U)로 최신을 유지하므로 YouTube 변경에 자동 대응한다.
"""

import hashlib
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

# 같은 릴리스가 게시한 SHA256 목록 — 받은 바이너리의 전송 무결성을 검증한다.
YTDLP_SUMS_URL = (
	"https://github.com/yt-dlp/yt-dlp/releases/latest/download/SHA2-256SUMS"
)
FFMPEG_SUMS_URL = (
	"https://github.com/BtbN/FFmpeg-Builds/releases/download/latest/"
	"checksums.sha256"
)


def _sha256(path):
	digest = hashlib.sha256()
	with open(path, "rb") as handle:
		for chunk in iter(lambda: handle.read(1 << 20), b""):
			digest.update(chunk)
	return digest.hexdigest()


def _fetch_expected_sha256(sums_url, filename):
	"""체크섬 목록에서 filename의 기대 SHA256(소문자)을 찾아 반환한다.

	목록은 표준 sha256sum 포맷('<hex>␣␣<파일명>')이다. 부분 매칭은 win64-gpl과
	win64-gpl-shared 같은 변형을 혼동하므로 파일명을 정확히 일치시킨다.
	"""
	resp = requests.get(sums_url, timeout=60)
	resp.raise_for_status()
	for line in resp.text.splitlines():
		parts = line.split()
		if len(parts) == 2 and parts[1] == filename:
			return parts[0].lower()
	raise RuntimeError(f"{filename}의 체크섬을 목록에서 찾지 못했습니다.")


def _verify_sha256(path, expected, label):
	"""path의 SHA256이 expected와 다르면 파일을 지우고 RuntimeError를 던진다."""
	if _sha256(path).lower() != expected:
		try:
			path.unlink()
		except OSError:
			pass
		raise RuntimeError(f"{label} 무결성 검증 실패")


def _download_file(url, dest, on_progress=None, label="", expected_sha=None):
	"""스트리밍 다운로드 → .part 임시파일 → (검증) → 원자적 교체."""
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
	# 교체 전에 검증한다 — 손상/변조된 .part가 절대 dest로 승격되지 않게.
	if expected_sha is not None:
		_verify_sha256(tmp, expected_sha, label or dest.name)
	tmp.replace(dest)


def _download_ffmpeg(dest, on_progress=None):
	with tempfile.TemporaryDirectory() as tmpdir:
		zip_path = Path(tmpdir) / "ffmpeg.zip"
		# zip을 검증하면 그 안의 ffmpeg.exe까지 신뢰가 전파되므로 추출 전에 받는다.
		zip_name = FFMPEG_ZIP_URL.rsplit("/", 1)[-1]
		expected = _fetch_expected_sha256(FFMPEG_SUMS_URL, zip_name)
		_download_file(
			FFMPEG_ZIP_URL, zip_path, on_progress, "ffmpeg", expected_sha=expected
		)
		with zipfile.ZipFile(zip_path) as archive:
			member = next(
				(n for n in archive.namelist() if n.endswith("/bin/ffmpeg.exe")),
				None,
			)
			if member is None:
				raise RuntimeError("압축에서 ffmpeg.exe를 찾지 못했습니다.")
			# 쓰기가 중단돼도 손상된 ffmpeg.exe가 남아 재사용되지 않도록
			# .part에 먼저 쓰고 성공 시 원자적으로 교체한다.
			tmp = dest.with_suffix(dest.suffix + ".part")
			with archive.open(member) as src, open(tmp, "wb") as out:
				shutil.copyfileobj(src, out)
			tmp.replace(dest)


def ensure_binaries(on_progress=None):
	"""yt-dlp.exe, ffmpeg.exe 경로를 보장해 (ytdlp_path, ffmpeg_path)로 반환한다."""
	BIN_DIR.mkdir(parents=True, exist_ok=True)
	ytdlp = BIN_DIR / "yt-dlp.exe"
	ffmpeg = BIN_DIR / "ffmpeg.exe"
	if not ytdlp.exists():
		expected = _fetch_expected_sha256(YTDLP_SUMS_URL, "yt-dlp.exe")
		_download_file(
			YTDLP_URL, ytdlp, on_progress, "yt-dlp", expected_sha=expected
		)
	if not ffmpeg.exists():
		_download_ffmpeg(ffmpeg, on_progress)
	return str(ytdlp), str(ffmpeg)
