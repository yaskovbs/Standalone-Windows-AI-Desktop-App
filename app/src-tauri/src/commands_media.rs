//! Commands for hearing, speaking, seeing and watching video.

use crate::download::{self, DownloadEvent};
use crate::media::{self, MediaConfig, MediaEvent, MediaMemory, Models};
use crate::settings::Settings;
use crate::tools::now_ms;
use crate::voice::{self, Recorder};
use crate::{vision, AppState};
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
use tauri::ipc::Channel;
use tauri::State;

type CmdResult<T> = Result<T, String>;

fn err<E: std::fmt::Display>(e: E) -> String {
    e.to_string()
}

#[derive(Serialize)]
pub struct ToolsStatus {
    pub tools_dir: String,
    pub ffmpeg: bool,
    pub whisper_cli: bool,
    pub vision_cli: bool,
    pub whisper_model: bool,
    pub vision_model: bool,
    pub mmproj: bool,
}

#[tauri::command]
pub fn tools_status(state: State<'_, AppState>) -> ToolsStatus {
    let s = Settings::load(&state.db);
    let has = |f: &str| state.models_dir.join(f).is_file();
    ToolsStatus {
        tools_dir: state.tools.primary_dir().display().to_string(),
        ffmpeg: state.tools.find("ffmpeg").is_some(),
        whisper_cli: state.tools.find("whisper-cli").is_some(),
        vision_cli: state.tools.find("llama-mtmd-cli").is_some(),
        whisper_model: has(&s.whisper_model_file),
        vision_model: has(&s.vision_model_file),
        mmproj: has(&s.mmproj_file),
    }
}

/// kind: "whisper" | "vision" | "mmproj"
#[tauri::command]
pub async fn download_asset(state: State<'_, AppState>, kind: String, on_event: Channel<DownloadEvent>) -> CmdResult<()> {
    let s = Settings::load(&state.db);
    let (url, file) = match kind.as_str() {
        "whisper" => (s.whisper_model_url, s.whisper_model_file),
        "vision" => (s.vision_model_url, s.vision_model_file),
        "mmproj" => (s.mmproj_url, s.mmproj_file),
        _ => return Err("unknown asset".into()),
    };
    let dest = state.models_dir.join(file);
    let ch = on_event.clone();
    let res = download::download(&url, &dest, move |downloaded, total| {
        let _ = ch.send(DownloadEvent::Progress { downloaded, total });
    })
    .await;
    match res {
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

// ---------- hearing & speaking ----------

#[tauri::command]
pub fn voice_start(state: State<'_, AppState>) -> CmdResult<()> {
    let mut slot = state.recorder.lock().unwrap();
    if slot.is_some() {
        return Err("Already recording.".into());
    }
    *slot = Some(Recorder::start().map_err(err)?);
    Ok(())
}

#[tauri::command]
pub async fn voice_stop(state: State<'_, AppState>) -> CmdResult<String> {
    let rec = state.recorder.lock().unwrap().take().ok_or("Not recording.")?;
    let samples = tauri::async_runtime::spawn_blocking(move || rec.finish()).await.map_err(err)?;
    if voice::is_silent(&samples) {
        return Ok(String::new());
    }
    let s = Settings::load(&state.db);
    let model = state.models_dir.join(&s.whisper_model_file);
    if !model.is_file() {
        return Err("Speech model missing - download it from the Media panel.".into());
    }
    let wav = std::env::temp_dir().join(format!("lai_voice_{}.wav", now_ms()));
    voice::write_wav(&wav, &samples).map_err(err)?;
    let text = voice::transcribe(&state.tools, &model, &wav, &s.whisper_language, false).await;
    let _ = std::fs::remove_file(&wav);
    text.map(|t| voice::clean_transcript(&t)).map_err(err)
}

#[tauri::command]
pub fn speak(state: State<'_, AppState>, text: String) -> CmdResult<()> {
    let rate = Settings::load(&state.db).speech_rate;
    voice::speak(&text, rate, &state.tts).map_err(err)
}

#[tauri::command]
pub fn stop_speaking(state: State<'_, AppState>) {
    voice::stop_speaking(&state.tts);
}

// ---------- seeing ----------

pub(crate) fn vision_files(state: &AppState) -> CmdResult<(PathBuf, PathBuf)> {
    let s = Settings::load(&state.db);
    let (model, mmproj) = (state.models_dir.join(s.vision_model_file), state.models_dir.join(s.mmproj_file));
    if !model.is_file() || !mmproj.is_file() {
        return Err("Vision model missing - download it from the Media panel.".into());
    }
    Ok((model, mmproj))
}

async fn describe_file(state: &AppState, image: &Path, question: &str) -> CmdResult<String> {
    let (model, mmproj) = vision_files(state)?;
    let gpu = state.hardware.gpu_name.is_some();
    vision::describe(&state.tools, &model, &mmproj, image, question, 300, gpu).await.map_err(err)
}

#[tauri::command]
pub async fn describe_image(state: State<'_, AppState>, path: String, question: String) -> CmdResult<String> {
    describe_file(&state, Path::new(&path), &question).await
}

#[tauri::command]
pub async fn describe_screen(state: State<'_, AppState>, question: String) -> CmdResult<String> {
    let raw = vision::temp_path("lai_screen", "png");
    let small = vision::temp_path("lai_screen_small", "jpg");
    let target = raw.clone();
    tauri::async_runtime::spawn_blocking(move || vision::capture_screen(&target)).await.map_err(err)?.map_err(err)?;
    let result = match vision::shrink(&state.tools, &raw, &small, 1024).await {
        Ok(()) => describe_file(&state, &small, &question).await,
        Err(e) => Err(err(e)),
    };
    let _ = std::fs::remove_file(&raw);
    let _ = std::fs::remove_file(&small);
    result
}

// ---------- watching video ----------

#[tauri::command]
pub async fn watch_media(state: State<'_, AppState>, path: String, on_event: Channel<MediaEvent>) -> CmdResult<MediaMemory> {
    if state.media_busy.swap(true, Ordering::SeqCst) {
        return Err("Already watching something.".into());
    }
    state.media_cancel.store(false, Ordering::Relaxed);
    let result = run_watch(&state, &path, on_event).await;
    state.media_busy.store(false, Ordering::SeqCst);
    result
}

async fn run_watch(state: &AppState, path: &str, on_event: Channel<MediaEvent>) -> CmdResult<MediaMemory> {
    let s = Settings::load(&state.db);
    let (vision_model, mmproj) = vision_files(state)?;
    let whisper_model = state.models_dir.join(&s.whisper_model_file);
    let models = Models {
        vision: &vision_model,
        mmproj: &mmproj,
        whisper: whisper_model.is_file().then_some((whisper_model.as_path(), s.whisper_language.as_str())),
    };
    let cfg = MediaConfig {
        interval_secs: s.frame_interval_secs,
        max_frames: s.max_frames,
        gpu: state.hardware.gpu_name.is_some(),
        scene_detect: false,
    };
    media::watch(&state.tools, Path::new(path), &models, &cfg, &*state.media_cancel, |e| {
        let _ = on_event.send(e);
    })
    .await
    .map_err(err)
}

#[tauri::command]
pub fn cancel_media(state: State<'_, AppState>) {
    state.media_cancel.store(true, Ordering::Relaxed);
}

#[tauri::command]
pub fn attach_media(state: State<'_, AppState>, conversation_id: i64, memory: MediaMemory) -> CmdResult<()> {
    let header = format!(
        "Video: {} ({} long, {} frames sampled{}).\n",
        memory.title,
        media::fmt_ts(memory.duration_secs),
        memory.frames,
        if memory.heard_audio { ", audio transcribed" } else { ", no audio transcript" }
    );
    state.db.set_media(conversation_id, &(header + &memory.timeline)).map_err(err)
}

#[tauri::command]
pub fn clear_media(state: State<'_, AppState>, conversation_id: i64) -> CmdResult<()> {
    state.db.clear_media(conversation_id).map_err(err)
}
