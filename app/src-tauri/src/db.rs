//! SQLite persistence: conversations, messages and key/value settings.

use anyhow::Result;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::Mutex;

pub struct Db(Mutex<Connection>);

#[derive(Debug, Clone, Serialize)]
pub struct Conversation {
    pub id: i64,
    pub title: String,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct RecapRow {
    pub id: i64,
    pub title: String,
    pub path: String,
    pub narration: Option<String>,
    pub duration: f32,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredMessage {
    pub id: i64,
    pub role: String,
    pub content: String,
    pub sources: Option<String>, // JSON array of web sources
    pub created_at: i64,
    pub rating: i64, // 1 helpful, -1 not helpful, 0 unrated
}

impl Db {
    pub fn open(path: &Path) -> Result<Self> {
        let conn = Connection::open(path)?;
        conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA foreign_keys = ON;
             CREATE TABLE IF NOT EXISTS conversations (
                 id INTEGER PRIMARY KEY AUTOINCREMENT,
                 title TEXT NOT NULL,
                 updated_at INTEGER NOT NULL DEFAULT (strftime('%s','now'))
             );
             CREATE TABLE IF NOT EXISTS messages (
                 id INTEGER PRIMARY KEY AUTOINCREMENT,
                 conversation_id INTEGER NOT NULL REFERENCES conversations(id) ON DELETE CASCADE,
                 role TEXT NOT NULL,
                 content TEXT NOT NULL,
                 sources TEXT,
                 created_at INTEGER NOT NULL DEFAULT (strftime('%s','now')),
                 rating INTEGER NOT NULL DEFAULT 0
             );
             CREATE INDEX IF NOT EXISTS idx_messages_conv ON messages(conversation_id, id);
             CREATE TABLE IF NOT EXISTS settings (key TEXT PRIMARY KEY, value TEXT NOT NULL);
             CREATE TABLE IF NOT EXISTS media_notes (
                 conversation_id INTEGER PRIMARY KEY REFERENCES conversations(id) ON DELETE CASCADE,
                 timeline TEXT NOT NULL
             );
             CREATE TABLE IF NOT EXISTS recaps (
                 id INTEGER PRIMARY KEY AUTOINCREMENT,
                 title TEXT NOT NULL,
                 path TEXT NOT NULL,
                 narration TEXT,
                 duration REAL NOT NULL,
                 created_at INTEGER NOT NULL DEFAULT (strftime('%s','now'))
             );
             CREATE TABLE IF NOT EXISTS bookmarks (
                 id INTEGER PRIMARY KEY AUTOINCREMENT,
                 prompt TEXT NOT NULL UNIQUE
             );",
        )?;
        // Databases created before ratings existed get the column added (error = already there).
        let _ = conn.execute("ALTER TABLE messages ADD COLUMN rating INTEGER NOT NULL DEFAULT 0", []);
        Ok(Self(Mutex::new(conn)))
    }

    pub fn list_conversations(&self) -> Result<Vec<Conversation>> {
        let conn = self.0.lock().unwrap();
        let mut stmt =
            conn.prepare("SELECT id, title, updated_at FROM conversations ORDER BY updated_at DESC, id DESC")?;
        let rows = stmt.query_map([], |r| {
            Ok(Conversation { id: r.get(0)?, title: r.get(1)?, updated_at: r.get(2)? })
        })?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    pub fn create_conversation(&self, title: &str) -> Result<i64> {
        let conn = self.0.lock().unwrap();
        conn.execute("INSERT INTO conversations (title) VALUES (?1)", params![title])?;
        Ok(conn.last_insert_rowid())
    }

    pub fn rename_conversation(&self, id: i64, title: &str) -> Result<()> {
        let conn = self.0.lock().unwrap();
        conn.execute("UPDATE conversations SET title = ?1 WHERE id = ?2", params![title, id])?;
        Ok(())
    }

    pub fn delete_conversation(&self, id: i64) -> Result<()> {
        let conn = self.0.lock().unwrap();
        conn.execute("DELETE FROM conversations WHERE id = ?1", params![id])?;
        Ok(())
    }

    pub fn add_message(&self, conv: i64, role: &str, content: &str, sources: Option<&str>) -> Result<i64> {
        let conn = self.0.lock().unwrap();
        conn.execute(
            "INSERT INTO messages (conversation_id, role, content, sources) VALUES (?1, ?2, ?3, ?4)",
            params![conv, role, content, sources],
        )?;
        conn.execute(
            "UPDATE conversations SET updated_at = strftime('%s','now') WHERE id = ?1",
            params![conv],
        )?;
        Ok(conn.last_insert_rowid())
    }

    pub fn messages(&self, conv: i64) -> Result<Vec<StoredMessage>> {
        let conn = self.0.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, role, content, sources, created_at, rating FROM messages WHERE conversation_id = ?1 ORDER BY id",
        )?;
        let rows = stmt.query_map(params![conv], |r| {
            Ok(StoredMessage {
                id: r.get(0)?,
                role: r.get(1)?,
                content: r.get(2)?,
                sources: r.get(3)?,
                created_at: r.get(4)?,
                rating: r.get(5)?,
            })
        })?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    pub fn get_setting(&self, key: &str) -> Result<Option<String>> {
        let conn = self.0.lock().unwrap();
        let mut stmt = conn.prepare("SELECT value FROM settings WHERE key = ?1")?;
        let mut rows = stmt.query(params![key])?;
        Ok(match rows.next()? {
            Some(row) => Some(row.get(0)?),
            None => None,
        })
    }

    pub fn set_setting(&self, key: &str, value: &str) -> Result<()> {
        let conn = self.0.lock().unwrap();
        conn.execute(
            "INSERT INTO settings (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![key, value],
        )?;
        Ok(())
    }

    pub fn set_rating(&self, message_id: i64, rating: i64) -> Result<()> {
        let conn = self.0.lock().unwrap();
        conn.execute("UPDATE messages SET rating = ?1 WHERE id = ?2 AND role = 'assistant'", params![rating, message_id])?;
        Ok(())
    }

    /// The assistant message plus the user message that immediately precedes it.
    pub fn pair_for(&self, conv: i64, assistant_id: i64) -> Result<Option<(String, String)>> {
        let conn = self.0.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT a.content, (SELECT u.content FROM messages u
                 WHERE u.conversation_id = a.conversation_id AND u.id < a.id AND u.role = 'user'
                 ORDER BY u.id DESC LIMIT 1)
             FROM messages a WHERE a.id = ?1 AND a.conversation_id = ?2 AND a.role = 'assistant'",
        )?;
        let mut rows = stmt.query(params![assistant_id, conv])?;
        Ok(match rows.next()? {
            Some(r) => {
                let assistant: String = r.get(0)?;
                let user: Option<String> = r.get(1)?;
                user.map(|u| (u, assistant))
            }
            None => None,
        })
    }

    pub fn set_media(&self, conv: i64, timeline: &str) -> Result<()> {
        let conn = self.0.lock().unwrap();
        conn.execute(
            "INSERT INTO media_notes (conversation_id, timeline) VALUES (?1, ?2)
             ON CONFLICT(conversation_id) DO UPDATE SET timeline = excluded.timeline",
            params![conv, timeline],
        )?;
        Ok(())
    }

    pub fn get_media(&self, conv: i64) -> Result<Option<String>> {
        let conn = self.0.lock().unwrap();
        let mut stmt = conn.prepare("SELECT timeline FROM media_notes WHERE conversation_id = ?1")?;
        let mut rows = stmt.query(params![conv])?;
        Ok(match rows.next()? {
            Some(row) => Some(row.get(0)?),
            None => None,
        })
    }

    pub fn clear_media(&self, conv: i64) -> Result<()> {
        let conn = self.0.lock().unwrap();
        conn.execute("DELETE FROM media_notes WHERE conversation_id = ?1", params![conv])?;
        Ok(())
    }

    pub fn add_recap(&self, title: &str, path: &str, narration: Option<&str>, duration: f32) -> Result<i64> {
        let conn = self.0.lock().unwrap();
        conn.execute(
            "INSERT INTO recaps (title, path, narration, duration) VALUES (?1, ?2, ?3, ?4)",
            params![title, path, narration, duration],
        )?;
        Ok(conn.last_insert_rowid())
    }

    pub fn list_recaps(&self) -> Result<Vec<RecapRow>> {
        let conn = self.0.lock().unwrap();
        let mut stmt = conn.prepare("SELECT id, title, path, narration, duration, created_at FROM recaps ORDER BY id DESC")?;
        let rows = stmt.query_map([], |r| {
            Ok(RecapRow { id: r.get(0)?, title: r.get(1)?, path: r.get(2)?, narration: r.get(3)?, duration: r.get(4)?, created_at: r.get(5)? })
        })?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    pub fn delete_recap(&self, id: i64) -> Result<()> {
        let conn = self.0.lock().unwrap();
        conn.execute("DELETE FROM recaps WHERE id = ?1", params![id])?;
        Ok(())
    }

    pub fn bookmarks(&self) -> Result<Vec<String>> {
        let conn = self.0.lock().unwrap();
        let mut stmt = conn.prepare("SELECT prompt FROM bookmarks ORDER BY id DESC")?;
        let rows = stmt.query_map([], |r| r.get::<_, String>(0))?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    pub fn add_bookmark(&self, prompt: &str) -> Result<()> {
        let conn = self.0.lock().unwrap();
        conn.execute("INSERT OR IGNORE INTO bookmarks (prompt) VALUES (?1)", params![prompt])?;
        Ok(())
    }

    pub fn remove_bookmark(&self, prompt: &str) -> Result<()> {
        let conn = self.0.lock().unwrap();
        conn.execute("DELETE FROM bookmarks WHERE prompt = ?1", params![prompt])?;
        Ok(())
    }
}
