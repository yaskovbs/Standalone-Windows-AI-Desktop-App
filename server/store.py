"""SQLite storage for opted-in contributions."""
from __future__ import annotations

import hashlib
import sqlite3
import threading
import time
from pathlib import Path

DB_PATH = Path(__file__).with_name("contributions.sqlite")


class Store:
    def __init__(self, path: Path = DB_PATH) -> None:
        self._lock = threading.Lock()
        self._db = sqlite3.connect(path, check_same_thread=False)
        self._db.execute(
            """CREATE TABLE IF NOT EXISTS contributions (
                   id INTEGER PRIMARY KEY AUTOINCREMENT,
                   install_id TEXT NOT NULL,
                   created_at INTEGER NOT NULL,
                   app_version TEXT NOT NULL,
                   user_text TEXT NOT NULL,
                   assistant_text TEXT NOT NULL,
                   digest TEXT NOT NULL UNIQUE)"""
        )
        self._db.execute("CREATE INDEX IF NOT EXISTS idx_install ON contributions(install_id)")
        self._db.commit()

    def add(self, install_id: str, app_version: str, user: str, assistant: str) -> bool:
        digest = hashlib.sha256((user + "\x00" + assistant).encode()).hexdigest()
        with self._lock:
            cur = self._db.execute(
                "INSERT OR IGNORE INTO contributions "
                "(install_id, created_at, app_version, user_text, assistant_text, digest) VALUES (?,?,?,?,?,?)",
                (install_id, int(time.time()), app_version, user, assistant, digest),
            )
            self._db.commit()
            return cur.rowcount == 1

    def delete_install(self, install_id: str) -> int:
        with self._lock:
            cur = self._db.execute("DELETE FROM contributions WHERE install_id = ?", (install_id,))
            self._db.commit()
            return cur.rowcount

    def count(self) -> int:
        return self._db.execute("SELECT COUNT(*) FROM contributions").fetchone()[0]

    def export(self):
        yield from self._db.execute("SELECT user_text, assistant_text FROM contributions ORDER BY id")
