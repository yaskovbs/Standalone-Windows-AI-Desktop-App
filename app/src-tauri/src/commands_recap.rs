//! Recap maker commands: build a recap video from a film using only on-device models.

use crate::commands_media::vision_files;
use crate::db::RecapRow;
use crate::hardware;
use crate::media::{MediaEvent, Models};
use crate::recap::{self, Ctx, RecapRequest, RecapResult};
use crate::settings::Settings;
use crate::AppState;
use std::path::Path;
use std::sync::atomic::Ordering;
use tauri::ipc::Channel;
use tauri::State;

type CmdResult<T> = Result<T, String>;

fn err<E: std::fmt::Display>(e: E) -> String {
    e.to_string()
}

#[tauri::command]
pub async fn make_recap(state: State<'_, AppState>, request: RecapRequest, on_event: Channel<MediaEvent>) -> CmdResult<RecapResult> {
    let engine = state.engine.lock().unwrap().clone().ok_or("Load the language model first.")?;
    let (vision_model, mmproj) = vision_files(&state)?;
    if state.media_busy.swap(true, Ordering::SeqCst) {
        return Err("Already working on a video.".into());
    }
    if state.busy.swap(true, Ordering::SeqCst) {
        state.media_busy.store(false, Ordering::SeqCst);
        return Err("A response is already being generated.".into());
    }
    state.media_cancel.store(false, Ordering::Relaxed);

    let result = run(&state, engine, &request, &vision_model, &mmproj, &on_event).await;

    state.busy.store(false, Ordering::SeqCst);
    state.media_busy.store(false, Ordering::SeqCst);
    let result = result.map_err(err)?;
    let title = if request.title.trim().is_empty() { "Untitled recap" } else { request.title.trim() };
    state.db.add_recap(title, &result.output_path, result.narration.as_deref(), result.total_secs).map_err(err)?;
    Ok(result)
}

async fn run(
    state: &AppState,
    engine: std::sync::Arc<crate::engine::Engine>,
    request: &RecapRequest,
    vision_model: &Path,
    mmproj: &Path,
    on_event: &Channel<MediaEvent>,
) -> anyhow::Result<RecapResult> {
    let settings = Settings::load(&state.db).sanitized();
    let whisper_model = state.models_dir.join(&settings.whisper_model_file);
    let models = Models {
        vision: vision_model,
        mmproj,
        whisper: whisper_model.is_file().then_some((whisper_model.as_path(), settings.whisper_language.as_str())),
    };
    let ctx = Ctx {
        tools: &state.tools,
        engine,
        settings: &settings,
        models: &models,
        gpu: state.hardware.gpu_name.is_some(),
        threads: hardware::cpu_threads(&state.hardware),
        cancel: state.media_cancel.clone(),
    };
    recap::make(&ctx, request, &|e| {
        let _ = on_event.send(e);
    })
    .await
}

#[tauri::command]
pub fn list_recaps(state: State<'_, AppState>) -> CmdResult<Vec<RecapRow>> {
    state.db.list_recaps().map_err(err)
}

/// Removes the history entry only; the video file stays on disk.
#[tauri::command]
pub fn delete_recap(state: State<'_, AppState>, id: i64) -> CmdResult<()> {
    state.db.delete_recap(id).map_err(err)
}

/// Show a saved recap in Explorer. Only files recorded in the recap history can be revealed.
#[tauri::command]
pub fn reveal_file(state: State<'_, AppState>, path: String) -> CmdResult<()> {
    let known = state.db.list_recaps().map_err(err)?.iter().any(|r| r.path == path);
    if !known || !Path::new(&path).is_file() {
        return Err("That file is not in your recap history.".into());
    }
    #[cfg(windows)]
    {
        std::process::Command::new("explorer").arg(format!("/select,{path}")).spawn().map_err(err)?;
        Ok(())
    }
    #[cfg(not(windows))]
    Err("Opening Explorer is only available on Windows.".into())
}
