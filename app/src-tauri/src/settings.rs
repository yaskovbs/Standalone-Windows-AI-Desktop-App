//! Typed application settings persisted as JSON in the SQLite settings table.

use crate::db::Db;
use serde::{Deserialize, Serialize};

pub const DEFAULT_MODEL_URL: &str =
    "https://huggingface.co/Qwen/Qwen2.5-3B-Instruct-GGUF/resolve/main/qwen2.5-3b-instruct-q4_k_m.gguf";
pub const DEFAULT_SYSTEM_PROMPT: &str =
    "You are a direct, precise assistant. Think step by step inside <reasoning> tags, \
     then give the final result inside <answer> tags. When web results are provided, \
     use only facts from them and cite them as [1], [2].";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub model_url: String,
    pub model_file: String,
    pub system_prompt: String,
    pub temperature: f32,
    pub max_tokens: u32,
    pub context_size: u32,
    pub web_search_default: bool,
    pub search_results: u8,
    pub check_updates: bool,
    pub update_repo: String, // "owner/repo" on GitHub
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            model_url: DEFAULT_MODEL_URL.into(),
            model_file: "custom_assistant_q4_k_m.gguf".into(),
            system_prompt: DEFAULT_SYSTEM_PROMPT.into(),
            temperature: 0.7,
            max_tokens: 1024,
            context_size: 4096,
            web_search_default: true,
            search_results: 4,
            check_updates: true,
            update_repo: "yaskovbs/Standalone-Offline-Windows-AI-Desktop-App".into(),
        }
    }
}

impl Settings {
    pub fn load(db: &Db) -> Self {
        db.get_setting("app")
            .ok()
            .flatten()
            .and_then(|raw| serde_json::from_str(&raw).ok())
            .unwrap_or_default()
    }

    pub fn save(&self, db: &Db) -> anyhow::Result<()> {
        db.set_setting("app", &serde_json::to_string(self)?)
    }

    /// Clamp user-provided values to safe ranges.
    pub fn sanitized(mut self) -> Self {
        self.temperature = self.temperature.clamp(0.0, 2.0);
        self.context_size = self.context_size.clamp(512, 32768);
        self.max_tokens = self.max_tokens.clamp(16, self.context_size / 2);
        self.search_results = self.search_results.clamp(1, 8);
        self
    }
}
