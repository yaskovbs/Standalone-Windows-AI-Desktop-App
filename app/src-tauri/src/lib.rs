//! LocalAutonomousAI - Tauri v2 backend (internet-connected, local LLM, voice + vision).

mod commands;
mod commands_github;
mod commands_media;
mod commands_recap;
mod commands_share;
mod contribute;
mod db;
mod download;
mod engine;
mod github;
mod hardware;
mod media;
mod prompt;
mod proposals;
mod recap;
mod recap_plan;
mod recap_render;
mod redact;
mod settings;
mod signals;
mod tools;
mod vision;
mod voice;
mod web;

use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Child;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use tauri::menu::{Menu, MenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{Manager, WindowEvent};

pub struct AppState {
    pub db: db::Db,
    pub engine: Mutex<Option<Arc<engine::Engine>>>,
    pub hardware: hardware::HardwareInfo,
    pub models_dir: PathBuf,
    pub cancel: Arc<AtomicBool>,
    pub busy: AtomicBool,
    pub tools: tools::Tools,
    pub recorder: Mutex<Option<voice::Recorder>>,
    pub tts: Mutex<Option<Child>>,
    pub media_cancel: Arc<AtomicBool>,
    pub media_busy: AtomicBool,
    pub proposals: Mutex<HashMap<String, proposals::Proposal>>,
}

fn setup_tray(app: &tauri::App) -> tauri::Result<()> {
    let show = MenuItem::with_id(app, "show", "Show", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show, &quit])?;
    let mut builder = TrayIconBuilder::new().menu(&menu).on_menu_event(|app, event| match event.id.as_ref() {
        "show" => {
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.show();
                let _ = w.set_focus();
            }
        }
        "quit" => app.exit(0),
        _ => {}
    });
    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }
    builder.build(app)?;
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let data_dir = app.path().app_data_dir()?;
            let models_dir = data_dir.join("models");
            let tools_dir = data_dir.join("tools");
            std::fs::create_dir_all(&models_dir)?;
            std::fs::create_dir_all(&tools_dir)?;
            let mut tool_dirs = vec![tools_dir];
            if let Ok(res) = app.path().resource_dir() {
                tool_dirs.push(res.join("tools"));
            }

            app.manage(AppState {
                db: db::Db::open(&data_dir.join("local_db.sqlite"))?,
                engine: Mutex::new(None),
                hardware: hardware::detect(),
                models_dir,
                cancel: Arc::new(AtomicBool::new(false)),
                busy: AtomicBool::new(false),
                tools: tools::Tools::new(tool_dirs),
                recorder: Mutex::new(None),
                tts: Mutex::new(None),
                media_cancel: Arc::new(AtomicBool::new(false)),
                media_busy: AtomicBool::new(false),
                proposals: Mutex::new(HashMap::new()),
            });
            setup_tray(app)?;
            Ok(())
        })
        // Closing the window hides it to the tray so the loaded model stays warm.
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                let _ = window.hide();
                api.prevent_close();
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::hardware_info,
            commands::model_status,
            commands::load_model,
            commands::download_model,
            commands::list_conversations,
            commands::create_conversation,
            commands::delete_conversation,
            commands::rename_conversation,
            commands::get_messages,
            commands::chat,
            commands::stop_generation,
            commands::get_settings,
            commands::save_settings,
            commands::list_bookmarks,
            commands::add_bookmark,
            commands::remove_bookmark,
            commands::web_search,
            commands::check_update,
            commands::is_online,
            commands_media::tools_status,
            commands_media::download_asset,
            commands_media::voice_start,
            commands_media::voice_stop,
            commands_media::speak,
            commands_media::stop_speaking,
            commands_media::describe_image,
            commands_media::describe_screen,
            commands_media::watch_media,
            commands_media::cancel_media,
            commands_media::attach_media,
            commands_media::clear_media,
            commands_recap::make_recap,
            commands_recap::list_recaps,
            commands_recap::delete_recap,
            commands_recap::reveal_file,
            commands_share::share_status,
            commands_share::set_share_consent,
            commands_share::withdraw_data,
            commands_share::rate_message,
            commands_github::github_status,
            commands_github::github_save_token,
            commands_github::github_clear_token,
            commands_github::github_propose,
            commands_github::github_reject,
            commands_github::github_approve,
        ])
        .run(tauri::generate_context!())
        .expect("error while running LocalAutonomousAI");
}
