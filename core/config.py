"""앱 설정 — 저장 폴더/포맷/화질을 %LOCALAPPDATA%/XDownloader/config.json 에 보관."""

import json
import os
from pathlib import Path

APP_DIR = (
	Path(os.environ.get("LOCALAPPDATA", str(Path.home()))) / "XDownloader"
)
CONFIG_PATH = APP_DIR / "config.json"

DEFAULTS = {
	"download_dir": str(Path.home() / "Downloads"),
	"format": "mp4",  # mp4 | mp3
	"quality": "best",  # best | 1080p | 720p
}


def load_config():
	try:
		with open(CONFIG_PATH, encoding="utf-8") as handle:
			stored = json.load(handle)
		if not isinstance(stored, dict):
			return dict(DEFAULTS)
		return {**DEFAULTS, **stored}
	except Exception:
		return dict(DEFAULTS)


def save_config(config):
	try:
		APP_DIR.mkdir(parents=True, exist_ok=True)
		# 쓰기 도중 실패해도 기존 설정이 손상되지 않도록 임시파일에 쓰고 교체한다.
		tmp = CONFIG_PATH.with_suffix(CONFIG_PATH.suffix + ".tmp")
		with open(tmp, "w", encoding="utf-8") as handle:
			json.dump(config, handle, ensure_ascii=False, indent=2)
		tmp.replace(CONFIG_PATH)
	except Exception:
		# 설정 저장 실패는 치명적이지 않다 — 다음 실행에서 기본값으로 동작.
		pass
