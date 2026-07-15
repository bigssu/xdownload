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
	"lang": "ko",  # ko | en
	# theme(light|dark)은 의도적으로 제외 — 최초 실행 시 OS 다크모드 설정을
	# 감지해 초기화하기 위해 "키 없음" 상태가 필요하다(main_frame._init_theme).
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


def atomic_write_json(path, obj):
	# 쓰기 도중 실패해도 기존 파일이 손상되지 않도록 임시파일에 쓰고 교체한다.
	# 저장 실패는 치명적이지 않으므로 조용히 무시한다.
	try:
		path.parent.mkdir(parents=True, exist_ok=True)
		tmp = path.with_suffix(path.suffix + ".tmp")
		with open(tmp, "w", encoding="utf-8") as handle:
			json.dump(obj, handle, ensure_ascii=False, indent=2)
		tmp.replace(path)
	except Exception:
		pass


def save_config(config):
	atomic_write_json(CONFIG_PATH, config)
