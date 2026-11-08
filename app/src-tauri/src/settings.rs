//! Typed application settings persisted as JSON in the SQLite settings table.

use crate::db::Db;
use serde::{Deserialize, Serialize};

pub const DEFAULT_MODEL_URL: &str =
    "https://huggingface.co/Qwen/Qwen2.5-3B-Instruct-GGUF/resolve/main/qwen2.5-3b-instruct-q4_k_m.gguf";
pub const DEFAULT_SYSTEM_PROMPT: &str =
    "You are a direct, precise assistant. Think step by step inside <reasoning> tags, \
     then give the final result inside <answer> tags. When web results are provided, \
     use only facts from them and cite them as [1], [2]. When a video timeline is provided, \
     answer from it and quote its time codes; say so if the timeline does not cover a question.";

// Defaults below are editable in Settings; verify the URLs still exist before shipping.
const WHISPER_URL: &str = "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.bin";
const VISION_URL: &str =
    "https://huggingface.co/ggml-org/Qwen2.5-VL-3B-Instruct-GGUF/resolve/main/Qwen2.5-VL-3B-Instruct-Q4_K_M.gguf";
const MMPROJ_URL: &str =
    "https://huggingface.co/ggml-org/Qwen2.5-VL-3B-Instruct-GGUF/resolve/main/mmproj-Qwen2.5-VL-3B-Instruct-Q8_0.gguf";

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
    pub update_repo: String, // "owner/repo" on GitHub (updates + self-update PRs)
    // hearing / speaking
    pub whisper_model_url: String,
    pub whisper_model_file: String,
    pub whisper_language: String, // "auto", "en", "he", ...
    pub speech_rate: i32,         // -10..10
    pub speak_replies: bool,
    // seeing / watching
    pub vision_model_url: String,
    pub vision_model_file: String,
    pub mmproj_url: String,
    pub mmproj_file: String,
    pub frame_interval_secs: u32,
    pub max_frames: u32,
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
            whisper_model_url: WHISPER_URL.into(),
            whisper_model_file: "ggml-base.bin".into(),
            whisper_language: "auto".into(),
            speech_rate: 0,
            speak_replies: false,
            vision_model_url: VISION_URL.into(),
            vision_model_file: "vision-model-q4_k_m.gguf".into(),
            mmproj_url: MMPROJ_URL.into(),
            mmproj_file: "vision-mmproj.gguf".into(),
            frame_interval_secs: 20,
            max_frames: 40,
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

    /// Clamp user-provided values to safe ranges and reject path-like file names.
    pub fn sanitized(mut self) -> Self {
        let defaults = Self::default();
        self.temperature = self.temperature.clamp(0.0, 2.0);
        self.context_size = self.context_size.clamp(512, 32768);
        self.max_tokens = self.max_tokens.clamp(16, self.context_size / 2);
        self.search_results = self.search_results.clamp(1, 8);
        self.speech_rate = self.speech_rate.clamp(-10, 10);
        self.frame_interval_secs = self.frame_interval_secs.clamp(2, 600);
        self.max_frames = self.max_frames.clamp(4, 200);
        let safe = |f: &str| !f.is_empty() && !f.contains(['/', '\\', ':']) && !f.contains("..");
        if !safe(&self.model_file) { self.model_file = defaults.model_file; }
        if !safe(&self.whisper_model_file) { self.whisper_model_file = defaults.whisper_model_file; }
        if !safe(&self.vision_model_file) { self.vision_model_file = defaults.vision_model_file; }
        if !safe(&self.mmproj_file) { self.mmproj_file = defaults.mmproj_file; }
        self
    }
}
