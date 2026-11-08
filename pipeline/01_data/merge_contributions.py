"""Fold opted-in user contributions (from server/contributions.sqlite) into the raw training set."""
from __future__ import annotations

import json
import sqlite3
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import config  # noqa: E402

DB = Path(__file__).resolve().parents[2] / "server" / "contributions.sqlite"


def main() -> None:
    if not DB.exists():
        sys.exit(f"No contribution database at {DB}")
    config.DATA_DIR.mkdir(parents=True, exist_ok=True)
    n = 0
    with sqlite3.connect(DB) as db, open(config.RAW_DATASET, "a", encoding="utf-8") as out:
        for user, assistant in db.execute("SELECT user_text, assistant_text FROM contributions ORDER BY id"):
            row = {
                "category": "community",
                "messages": [
                    {"role": "system", "content": config.SYSTEM_PROMPT},
                    {"role": "user", "content": user},
                    {"role": "assistant", "content": assistant},
                ],
            }
            out.write(json.dumps(row, ensure_ascii=False) + "\n")
            n += 1
    print(f"Merged {n} community rows into {config.RAW_DATASET}")


if __name__ == "__main__":
    main()
