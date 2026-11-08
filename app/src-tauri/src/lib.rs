//! LocalAutonomousAI - Tauri v2 backend (internet-connected, local LLM).

mod commands;
mod db;
mod download;
mod engine;
mod hardware;
mod prompt;
mod settings;
mod web;

use std::path::PathBuf;
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
        .setup(|app| {
            let data_dir = app.path().app_data_dir()?;
            let models_dir = data_dir.join("models");
            std::fs::create_dir_all(&models_dir)?;
            let db = db::Db::open(&data_dir.join("local_db.sqlite"))?;

            app.manage(AppState {
                db,
                engine: Mutex::new(None),
                hardware: hardware::detect(),
                models_dir,
                cancel: Arc::new(AtomicBool::new(false)),
                busy: AtomicBool::new(false),
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
        ])
        .run(tauri::generate_context!())
        .expect("error while running LocalAutonomousAI");
}
