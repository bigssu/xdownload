"""로컬 미디어 변환 — ffmpeg로 동영상/이미지를 호환 포맷으로 만든다."""

import re
import subprocess
from pathlib import Path

VIDEO_EXTS = {
	".3gp",
	".avi",
	".flv",
	".m2ts",
	".m4v",
	".mkv",
	".mov",
	".mp4",
	".mts",
	".ts",
	".webm",
	".wmv",
}

IMAGE_EXTS = {
	".avif",
	".bmp",
	".gif",
	".heic",
	".heif",
	".jfif",
	".jpeg",
	".png",
	".tif",
	".tiff",
	".webp",
}

VIDEO_SUFFIX = "_h264_1080p"

_DURATION_RE = re.compile(r"Duration:\s*(\d+):(\d+):(\d+(?:\.\d+)?)")
_OUT_TIME_RE = re.compile(r"out_time=(\d+):(\d+):(\d+(?:\.\d+)?)")
_NO_WINDOW = 0x08000000  # CREATE_NO_WINDOW


def _seconds(hours, minutes, seconds):
	return int(hours) * 3600 + int(minutes) * 60 + float(seconds)


def is_video_file(path):
	return Path(path).suffix.lower() in VIDEO_EXTS


def is_image_file(path):
	return Path(path).suffix.lower() in IMAGE_EXTS


def _is_generated_video(path):
	stem = Path(path).stem.lower()
	return stem.endswith(VIDEO_SUFFIX) or re.search(
		rf"{re.escape(VIDEO_SUFFIX)}-\d+$", stem
	)


def collect_media_files(sources, kind):
	"""파일 또는 폴더 목록에서 변환 대상 파일을 모은다.

	폴더는 바로 아래 파일만 처리한다. 변환 결과물을 같은 폴더에 만들기 때문에
	동영상 변환 산출물을 다시 입력으로 잡는 것은 건너뛴다.
	"""
	if isinstance(sources, (str, Path)):
		sources = [sources]

	files = []
	for source in sources:
		path = Path(source)
		if path.is_file():
			if kind == "video" and is_video_file(path):
				files.append(path)
			elif kind == "image" and is_image_file(path):
				files.append(path)
			continue
		if not path.is_dir():
			continue
		for child in sorted(path.iterdir(), key=lambda p: p.name.lower()):
			if not child.is_file():
				continue
			if kind == "video" and is_video_file(child) and not _is_generated_video(child):
				files.append(child)
			elif kind == "image" and is_image_file(child):
				files.append(child)
	return files


def _unique_path(path):
	path = Path(path)
	if not path.exists():
		return path
	for index in range(2, 10000):
		candidate = path.with_name(f"{path.stem}-{index}{path.suffix}")
		if not candidate.exists():
			return candidate
	raise RuntimeError(f"출력 파일명을 만들 수 없습니다: {path}")


def video_output_path(source):
	source = Path(source)
	if source.suffix.lower() == ".mp4":
		return _unique_path(source.with_name(f"{source.stem}{VIDEO_SUFFIX}.mp4"))
	return _unique_path(source.with_suffix(".mp4"))


def image_output_path(source):
	source = Path(source)
	if source.suffix.lower() == ".jpg":
		return None
	return _unique_path(source.with_suffix(".jpg"))


def build_video_args(source, dest, ffmpeg):
	return [
		ffmpeg,
		"-y",
		"-hide_banner",
		"-progress",
		"pipe:1",
		"-nostats",
		"-i",
		str(source),
		"-map",
		"0:v:0",
		"-map",
		"0:a:0?",
		"-vf",
		"scale=-2:min(1080\\,ih)",
		"-c:v",
		"libx264",
		"-preset",
		"medium",
		"-crf",
		"23",
		"-pix_fmt",
		"yuv420p",
		"-c:a",
		"aac",
		"-b:a",
		"160k",
		"-movflags",
		"+faststart",
		str(dest),
	]


def build_image_args(source, dest, ffmpeg):
	return [
		ffmpeg,
		"-y",
		"-hide_banner",
		"-i",
		str(source),
		"-frames:v",
		"1",
		"-vf",
		"format=rgb24",
		"-q:v",
		"2",
		str(dest),
	]


def run_ffmpeg(args, *, on_progress=None, cancel=None, on_proc=None):
	"""ffmpeg를 실행하고 종료코드(성공 0, 취소 -1)를 반환한다."""
	proc = subprocess.Popen(
		args,
		stdout=subprocess.PIPE,
		stderr=subprocess.STDOUT,
		text=True,
		encoding="utf-8",
		errors="replace",
		bufsize=1,
		stdin=subprocess.DEVNULL,
		creationflags=_NO_WINDOW,
	)
	if on_proc:
		on_proc(proc)
	duration = None
	last_pct = -1
	try:
		for line in proc.stdout:
			if cancel is not None and cancel.is_set():
				proc.terminate()
				break
			line = line.strip()
			duration_match = _DURATION_RE.search(line)
			if duration_match:
				duration = _seconds(*duration_match.groups())
			time_match = _OUT_TIME_RE.search(line)
			if time_match and duration and on_progress:
				current = _seconds(*time_match.groups())
				pct = int(min(99, max(0, current / duration * 100)))
				if pct != last_pct:
					last_pct = pct
					on_progress(pct)
	finally:
		proc.stdout.close()
	code = proc.wait()
	if cancel is not None and cancel.is_set():
		return -1
	if code == 0 and on_progress:
		on_progress(100)
	return code


def convert_video(source, ffmpeg, *, on_progress=None, cancel=None, on_proc=None):
	source = Path(source)
	dest = video_output_path(source)
	return run_ffmpeg(
		build_video_args(source, dest, ffmpeg),
		on_progress=on_progress,
		cancel=cancel,
		on_proc=on_proc,
	)


def convert_image(source, ffmpeg, *, cancel=None, on_proc=None):
	source = Path(source)
	dest = image_output_path(source)
	if dest is None:
		return 0
	code = run_ffmpeg(
		build_image_args(source, dest, ffmpeg),
		cancel=cancel,
		on_proc=on_proc,
	)
	return code
