//! Consent, rating and withdrawal commands for the opt-in learning programme.

use crate::contribute;
use crate::settings::Settings;
use crate::AppState;
use serde::Serialize;
use tauri::State;

type CmdResult<T> = Result<T, String>;

fn err<E: std::fmt::Display>(e: E) -> String {
    e.to_string()
}

#[derive(Serialize)]
pub struct ShareStatus {
    pub available: bool, // a server URL is configured
    pub asked: bool,     // the user has seen the notice and chosen
    pub enabled: bool,
    pub server: String,
}

fn status_of(s: &Settings) -> ShareStatus {
    ShareStatus {
        available: !s.contribute_url.trim().is_empty(),
        asked: s.share_asked,
        enabled: s.share_enabled && !s.install_id.is_empty(),
        server: s.contribute_url.clone(),
    }
}

#[tauri::command]
pub fn share_status(state: State<'_, AppState>) -> ShareStatus {
    status_of(&Settings::load(&state.db))
}

/// Record the user's choice from the notice dialog. Nothing is ever sent before `enabled` is true.
#[tauri::command]
pub async fn set_share_consent(state: State<'_, AppState>, enabled: bool) -> CmdResult<ShareStatus> {
    let mut s = Settings::load(&state.db);
    s.share_asked = true;
    if enabled {
        s.share_enabled = true;
        if s.install_id.is_empty() {
            s.install_id = uuid::Uuid::new_v4().to_string(); // random, not linked to any account
        }
    } else if s.share_enabled || !s.install_id.is_empty() {
        let _ = contribute::withdraw(&s.contribute_url, &s.install_id).await; // best effort
        s.share_enabled = false;
        s.install_id.clear();
    }
    s.save(&state.db).map_err(err)?;
    Ok(status_of(&s))
}

/// Withdraw consent and ask the server to delete everything it holds for this install.
#[tauri::command]
pub async fn withdraw_data(state: State<'_, AppState>) -> CmdResult<ShareStatus> {
    let mut s = Settings::load(&state.db);
    if !s.install_id.is_empty() {
        contribute::withdraw(&s.contribute_url, &s.install_id).await.map_err(err)?;
    }
    s.share_enabled = false;
    s.install_id.clear();
    s.save(&state.db).map_err(err)?;
    Ok(status_of(&s))
}

/// Rate an answer: 1 = helpful, -1 = not helpful, 0 = clear. A 👍 is shared only with consent.
#[tauri::command]
pub async fn rate_message(state: State<'_, AppState>, conversation_id: i64, message_id: i64, rating: i64) -> CmdResult<String> {
    let rating = rating.clamp(-1, 1);
    state.db.set_rating(message_id, rating).map_err(err)?;
    let s = Settings::load(&state.db);
    let sharing = s.share_enabled && !s.install_id.is_empty() && !s.contribute_url.trim().is_empty();
    if rating != 1 || !sharing {
        return Ok("saved".into());
    }
    // Conversations that include a video timeline are private: never share them.
    if state.db.get_media(conversation_id).map_err(err)?.is_some() {
        return Ok("saved (not shared: video conversation)".into());
    }
    let Some((user, assistant)) = state.db.pair_for(conversation_id, message_id).map_err(err)? else {
        return Ok("saved".into());
    };
    match contribute::build_item(&user, &assistant) {
        Some(item) => {
            contribute::send(&s.contribute_url, &s.install_id, vec![item]).await.map_err(err)?;
            Ok("shared".into())
        }
        None => Ok("saved (too short or too long to share)".into()),
    }
}
