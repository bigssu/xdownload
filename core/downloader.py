"""yt-dlp 호출 + 진행률 파싱. mazelines / Xcut ytdl.ts 에서 검증된 방식.

H.264(avc1) 우선 format으로 어느 플레이어에서나 재생되게 하고, mp4는 재인코딩 없이
remux로 컨테이너만 보장한다. mp3는 bestaudio를 ffmpeg로 추출한다.
"""

import re
import subprocess
from pathlib import Path

# 화질 라벨 → 최대 높이.
_HEIGHTS = {"best": 9999, "1080p": 1080, "720p": 720}

# mp4: H.264 우선 → mp4 → 임의 best 폴백(라이브/DASH·1080p 초과 전용 영상까지 보존).
_MP4_FORMAT = (
	"bestvideo[height<={h}][vcodec^=avc1]+bestaudio[ext=m4a]/"
	"bestvideo[height<={h}][ext=mp4]+bestaudio/"
	"best[height<={h}][ext=mp4]/best[height<={h}]/b[ext=mp4]/b"
)

# [download]  42.3% of 10.00MiB ...
_PERCENT_RE = re.compile(r"\[download\]\s+([0-9.]+)%")
# Windows에서 콘솔 창이 깜빡이지 않게 한다.
_NO_WINDOW = 0x08000000  # CREATE_NO_WINDOW


def build_args(*, url, fmt, quality, out_dir, ytdlp, ffmpeg):
	out_template = str(Path(out_dir) / "%(title)s.%(ext)s")
	args = [
		ytdlp,
		"--newline",
		"--no-warnings",
		"--no-progress",
		"--progress",
		"--concurrent-fragments",
		"4",
		"--ffmpeg-location",
		ffmpeg,
		"-o",
		out_template,
	]
	if fmt == "mp3":
		args += [
			"-f",
			"bestaudio/best",
			"-x",
			"--audio-format",
			"mp3",
			"--audio-quality",
			"0",
		]
	else:
		height = _HEIGHTS.get(quality, 9999)
		args += [
			"-f",
			_MP4_FORMAT.format(h=height),
			"--merge-output-format",
			"mp4",
			"--remux-video",
			"mp4",
		]
	args.append(url)
	return args


def download(
	*,
	url,
	fmt,
	quality,
	out_dir,
	ytdlp,
	ffmpeg,
	on_progress=None,
	on_line=None,
	cancel=None,
):
	"""한 URL을 받는다. 성공 시 0, 취소 시 -1, 그 외 yt-dlp 종료코드를 반환.

	on_progress(percent: float 0-100), on_line(str), cancel: threading.Event.
	"""
	args = build_args(
		url=url,
		fmt=fmt,
		quality=quality,
		out_dir=out_dir,
		ytdlp=ytdlp,
		ffmpeg=ffmpeg,
	)
	proc = subprocess.Popen(
		args,
		stdout=subprocess.PIPE,
		stderr=subprocess.STDOUT,
		text=True,
		encoding="utf-8",
		errors="replace",
		bufsize=1,
		creationflags=_NO_WINDOW,
	)
	try:
		for line in proc.stdout:
			if cancel is not None and cancel.is_set():
				proc.terminate()
				return -1
			line = line.rstrip()
			match = _PERCENT_RE.search(line)
			if match and on_progress:
				try:
					on_progress(float(match.group(1)))
				except ValueError:
					pass
			if on_line and line:
				on_line(line)
	finally:
		proc.stdout.close()
	return proc.wait()


def update_ytdlp(ytdlp):
	"""yt-dlp 자가 업데이트. 이미 최신이면 즉시 끝나 거의 비용이 없다."""
	try:
		subprocess.run(
			[ytdlp, "-U"],
			stdout=subprocess.PIPE,
			stderr=subprocess.STDOUT,
			text=True,
			encoding="utf-8",
			errors="replace",
			timeout=120,
			creationflags=_NO_WINDOW,
		)
	except Exception:
		# 업데이트 실패는 치명적이지 않다 — 기존 바이너리로 재시도한다.
		pass


def download_with_auto_update(*, on_status=None, **kwargs):
	"""다운로드 → 실패(봇 감지·추출 오류 등) 시 yt-dlp를 업데이트하고 1회 재시도.

	YouTube가 봇 감지를 바꿔 구버전 yt-dlp가 막혔을 때 자동으로 복구한다.
	취소(-1)·성공(0)은 그대로 반환하고, 실패일 때만 한 번 더 시도한다.
	"""
	cancel = kwargs.get("cancel")
	code = download(**kwargs)
	if code in (0, -1):
		return code
	if cancel is not None and cancel.is_set():
		return code
	if on_status:
		on_status("yt-dlp 업데이트 후 재시도 중...")
	update_ytdlp(kwargs["ytdlp"])
	return download(**kwargs)
