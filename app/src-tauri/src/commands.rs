//! Tauri commands: the bridge between the React UI and the Rust backend.

use crate::db::{Conversation, StoredMessage};
use crate::download::{self, DownloadEvent};
use crate::engine::{Engine, GenParams, GenStats};
use crate::hardware::{self, HardwareInfo};
use crate::prompt::{self, Turn};
use crate::settings::Settings;
use crate::web::{self, SearchResult, UpdateInfo};
use crate::AppState;
use serde::Serialize;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use tauri::ipc::Channel;
use tauri::State;

type CmdResult<T> = Result<T, String>;

fn err<E: std::fmt::Display>(e: E) -> String {
    e.to_string()
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "event", content = "data", rename_all = "camelCase")]
pub enum ChatEvent {
    Sources { items: Vec<SearchResult> },
    Token { text: String },
    Done { stats: GenStats, message_id: i64 },
    Error { message: String },
}

#[derive(Serialize)]
pub struct ModelStatus {
    pub present: bool,
    pub loaded: bool,
    pub name: String,
    pub gpu_layers: u32,
}

// ---------- hardware & model ----------

#[tauri::command]
pub fn hardware_info(state: State<'_, AppState>) -> HardwareInfo {
    state.hardware.clone()
}

#[tauri::command]
pub fn model_status(state: State<'_, AppState>) -> ModelStatus {
    let settings = Settings::load(&state.db);
    let path = state.models_dir.join(&settings.model_file);
    let engine = state.engine.lock().unwrap();
    ModelStatus {
        present: path.exists(),
        loaded: engine.is_some(),
        name: settings.model_file,
        gpu_layers: engine.as_ref().map(|e| e.gpu_layers).unwrap_or(0),
    }
}

#[tauri::command]
pub async fn load_model(state: State<'_, AppState>) -> CmdResult<ModelStatus> {
    let settings = Settings::load(&state.db);
    let path = state.models_dir.join(&settings.model_file);
    if !download::is_gguf(&path) {
        return Err("Model file is missing or not a valid GGUF. Download it first.".into());
    }
    let hw = state.hardware.clone();
    let engine = tauri::async_runtime::spawn_blocking(move || Engine::load(&path, &hw))
        .await
        .map_err(err)?
        .map_err(err)?;
    *state.engine.lock().unwrap() = Some(Arc::new(engine));
    Ok(model_status(state))
}

#[tauri::command]
pub async fn download_model(state: State<'_, AppState>, on_event: Channel<DownloadEvent>) -> CmdResult<()> {
    let settings = Settings::load(&state.db);
    let dest = state.models_dir.join(&settings.model_file);
    let ch = on_event.clone();
    let result = download::download(&settings.model_url, &dest, move |downloaded, total| {
        let _ = ch.send(DownloadEvent::Progress { downloaded, total });
    })
    .await;
    match result {
        Ok(()) => {
            let _ = on_event.send(DownloadEvent::Done { path: dest.display().to_string() });
            Ok(())
        }
        Err(e) => {
            let _ = on_event.send(DownloadEvent::Error { message: e.to_string() });
            Err(e.to_string())
        }
    }
}

// ---------- conversations ----------

#[tauri::command]
pub fn list_conversations(state: State<'_, AppState>) -> CmdResult<Vec<Conversation>> {
    state.db.list_conversations().map_err(err)
}

#[tauri::command]
pub fn create_conversation(state: State<'_, AppState>) -> CmdResult<i64> {
    state.db.create_conversation("New chat").map_err(err)
}

#[tauri::command]
pub fn delete_conversation(state: State<'_, AppState>, id: i64) -> CmdResult<()> {
    state.db.delete_conversation(id).map_err(err)
}

#[tauri::command]
pub fn rename_conversation(state: State<'_, AppState>, id: i64, title: String) -> CmdResult<()> {
    state.db.rename_conversation(id, title.trim()).map_err(err)
}

#[tauri::command]
pub fn get_messages(state: State<'_, AppState>, id: i64) -> CmdResult<Vec<StoredMessage>> {
    state.db.messages(id).map_err(err)
}

// ---------- chat ----------

#[tauri::command]
pub fn stop_generation(state: State<'_, AppState>) {
    state.cancel.store(true, Ordering::Relaxed);
}

#[tauri::command]
pub async fn chat(
    state: State<'_, AppState>,
    conversation_id: i64,
    content: String,
    web: bool,
    on_event: Channel<ChatEvent>,
) -> CmdResult<()> {
    let engine = state.engine.lock().unwrap().clone().ok_or("Model is not loaded yet.")?;
    if state.busy.swap(true, Ordering::SeqCst) {
        return Err("A response is already being generated.".into());
    }
    let result = run_chat(&state, engine, conversation_id, content, web, &on_event).await;
    state.busy.store(false, Ordering::SeqCst);
    if let Err(e) = &result {
        let _ = on_event.send(ChatEvent::Error { message: e.clone() });
    }
    result
}

async fn run_chat(
    state: &State<'_, AppState>,
    engine: Arc<Engine>,
    conv: i64,
    content: String,
    web_on: bool,
    on_event: &Channel<ChatEvent>,
) -> CmdResult<()> {
    let settings = Settings::load(&state.db).sanitized();
    state.db.add_message(conv, "user", &content, None).map_err(err)?;
    let history = state.db.messages(conv).map_err(err)?;
    if history.len() == 1 {
        let title: String = content.chars().take(40).collect();
        state.db.rename_conversation(conv, &title).map_err(err)?;
    }

    // Internet step: fetch live results; fall back silently to the local model on failure.
    let mut sources: Vec<SearchResult> = Vec::new();
    if web_on {
        match web::search(&content, settings.search_results as usize).await {
            Ok(r) => sources = r,
            Err(e) => eprintln!("web search failed, answering locally: {e}"),
        }
        if !sources.is_empty() {
            let _ = on_event.send(ChatEvent::Sources { items: sources.clone() });
        }
    }
    // Extra system context: live web results and/or the timeline of an attached video.
    let mut extra: Vec<String> = Vec::new();
    if !sources.is_empty() {
        extra.push(prompt::web_context(&sources));
    }
    if let Some(timeline) = state.db.get_media(conv).map_err(err)? {
        extra.push(format!("Video timeline (what was seen and heard):\n{timeline}"));
    }
    let web_block = (!extra.is_empty()).then(|| extra.join("\n\n"));
    let turns: Vec<Turn> = history
        .iter()
        .map(|m| Turn { role: m.role.clone(), content: m.content.clone() })
        .collect();
    let prompt_text =
        prompt::build(&settings.system_prompt, web_block.as_deref(), &turns, settings.context_size, settings.max_tokens);

    state.cancel.store(false, Ordering::Relaxed);
    let cancel = state.cancel.clone();
    let params = GenParams {
        temperature: settings.temperature,
        max_tokens: settings.max_tokens,
        n_ctx: settings.context_size,
        threads: hardware::cpu_threads(&state.hardware),
    };
    let ch = on_event.clone();
    let (stats, full): (GenStats, String) = tauri::async_runtime::spawn_blocking(move || {
        let mut full = String::new();
        let stats = engine.generate(&prompt_text, &params, &cancel, |piece| {
            full.push_str(piece);
            ch.send(ChatEvent::Token { text: piece.to_string() }).is_ok()
        })?;
        Ok::<_, anyhow::Error>((stats, full))
    })
    .await
    .map_err(err)?
    .map_err(err)?;

    let sources_json = (!sources.is_empty()).then(|| serde_json::to_string(&sources).unwrap_or_default());
    let message_id = state
        .db
        .add_message(conv, "assistant", full.trim(), sources_json.as_deref())
        .map_err(err)?;
    let _ = on_event.send(ChatEvent::Done { stats, message_id });
    Ok(())
}

// ---------- settings, bookmarks, internet utilities ----------

#[tauri::command]
pub fn get_settings(state: State<'_, AppState>) -> Settings {
    Settings::load(&state.db)
}

#[tauri::command]
pub fn save_settings(state: State<'_, AppState>, settings: Settings) -> CmdResult<Settings> {
    let clean = settings.sanitized();
    clean.save(&state.db).map_err(err)?;
    Ok(clean)
}

#[tauri::command]
pub fn list_bookmarks(state: State<'_, AppState>) -> CmdResult<Vec<String>> {
    state.db.bookmarks().map_err(err)
}

#[tauri::command]
pub fn add_bookmark(state: State<'_, AppState>, prompt: String) -> CmdResult<()> {
    state.db.add_bookmark(prompt.trim()).map_err(err)
}

#[tauri::command]
pub fn remove_bookmark(state: State<'_, AppState>, prompt: String) -> CmdResult<()> {
    state.db.remove_bookmark(&prompt).map_err(err)
}

#[tauri::command]
pub async fn web_search(state: State<'_, AppState>, query: String) -> CmdResult<Vec<SearchResult>> {
    let n = Settings::load(&state.db).search_results as usize;
    web::search(&query, n).await.map_err(err)
}

#[tauri::command]
pub async fn check_update(state: State<'_, AppState>) -> CmdResult<UpdateInfo> {
    let settings = Settings::load(&state.db);
    web::check_update(&settings.update_repo, env!("CARGO_PKG_VERSION")).await.map_err(err)
}

#[tauri::command]
pub async fn is_online() -> bool {
    match web::client() {
        Ok(c) => c.head("https://www.cloudflare.com/cdn-cgi/trace").send().await.is_ok(),
        Err(_) => false,
    }
}
