"""다운로드 히스토리 — 최근 항목을 JSON으로 보관(최대 100개)."""

import json
from pathlib import Path

from core.config import APP_DIR

HISTORY_PATH = APP_DIR / "history.json"
MAX_ITEMS = 100


def load_history():
	try:
		with open(HISTORY_PATH, encoding="utf-8") as handle:
			items = json.load(handle)
		return items if isinstance(items, list) else []
	except Exception:
		return []


def add_history(entry):
	"""entry: {title, url, format, quality, path, when}. 최신순으로 앞에 추가."""
	items = load_history()
	items.insert(0, entry)
	del items[MAX_ITEMS:]
	try:
		APP_DIR.mkdir(parents=True, exist_ok=True)
		# 쓰기 도중 실패해도 기존 히스토리가 손상되지 않도록 임시파일에 쓰고 교체한다.
		tmp = HISTORY_PATH.with_suffix(HISTORY_PATH.suffix + ".tmp")
		with open(tmp, "w", encoding="utf-8") as handle:
			json.dump(items, handle, ensure_ascii=False, indent=2)
		tmp.replace(HISTORY_PATH)
	except Exception:
		pass
	return items
