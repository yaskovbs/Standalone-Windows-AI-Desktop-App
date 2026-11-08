//! Self-update commands. The model drafts a change; only an explicit user approval
//! (github_approve) pushes anything, and only to a new ai/ branch as a draft PR.

use crate::engine::GenParams;
use crate::github::{self, GitHub, MAX_FILES};
use crate::hardware;
use crate::prompt::{self, Turn};
use crate::proposals::{self, FileChange, Proposal};
use crate::settings::Settings;
use crate::tools::now_ms;
use crate::AppState;
use serde::Serialize;
use std::collections::HashMap;
use std::sync::atomic::Ordering;
use tauri::State;

type CmdResult<T> = Result<T, String>;

fn err<E: std::fmt::Display>(e: E) -> String {
    e.to_string()
}

fn token_entry() -> CmdResult<keyring::Entry> {
    keyring::Entry::new("LocalAutonomousAI", "github-token").map_err(err)
}

fn client(state: &AppState) -> CmdResult<GitHub> {
    let token = token_entry()?.get_password().map_err(|_| "No GitHub token saved yet.".to_string())?;
    GitHub::new(&Settings::load(&state.db).update_repo, &token).map_err(err)
}

#[derive(Serialize)]
pub struct GithubStatus {
    pub has_token: bool,
    pub repo: String,
}

#[tauri::command]
pub fn github_status(state: State<'_, AppState>) -> GithubStatus {
    GithubStatus {
        has_token: token_entry().and_then(|e| e.get_password().map_err(err)).is_ok(),
        repo: Settings::load(&state.db).update_repo,
    }
}

#[tauri::command]
pub async fn github_save_token(state: State<'_, AppState>, token: String) -> CmdResult<String> {
    let token = token.trim().to_string();
    token_entry()?.set_password(&token).map_err(err)?;
    // Verify it works before reporting success.
    match client(&state)?.default_branch().await {
        Ok(branch) => Ok(format!("Token saved. Default branch: {branch}")),
        Err(e) => {
            let _ = token_entry()?.delete_credential();
            Err(format!("Token rejected: {e}"))
        }
    }
}

#[tauri::command]
pub fn github_clear_token() -> CmdResult<()> {
    let _ = token_entry()?.delete_credential();
    Ok(())
}

#[tauri::command]
pub async fn github_propose(state: State<'_, AppState>, instruction: String, paths: Vec<String>) -> CmdResult<Proposal> {
    if instruction.trim().is_empty() || paths.is_empty() || paths.len() > MAX_FILES {
        return Err(format!("Give an instruction and 1-{MAX_FILES} file paths."));
    }
    for p in &paths {
        github::check_path(p).map_err(err)?;
    }
    let engine = state.engine.lock().unwrap().clone().ok_or("Model is not loaded yet.")?;
    if state.busy.swap(true, Ordering::SeqCst) {
        return Err("A response is already being generated.".into());
    }
    let result = draft(&state, engine, instruction, paths).await;
    state.busy.store(false, Ordering::SeqCst);
    result
}

async fn draft(
    state: &State<'_, AppState>,
    engine: std::sync::Arc<crate::engine::Engine>,
    instruction: String,
    paths: Vec<String>,
) -> CmdResult<Proposal> {
    let settings = Settings::load(&state.db).sanitized();
    let gh = client(state)?;
    let base = gh.default_branch().await.map_err(err)?;

    let mut current: HashMap<String, (String, String)> = HashMap::new();
    for p in &paths {
        if let Some(found) = gh.file(p, &base).await.map_err(err)? {
            current.insert(p.clone(), found);
        }
    }
    let files: Vec<(String, String)> = paths
        .iter()
        .map(|p| (p.clone(), current.get(p).map(|c| c.0.clone()).unwrap_or_default()))
        .collect();

    let user_text = proposals::user_prompt(&instruction, &files);
    let out_budget = settings.context_size / 2;
    let in_budget_chars = (settings.context_size.saturating_sub(out_budget) as usize) * 3;
    if user_text.len() + proposals::system_prompt().len() > in_budget_chars {
        return Err("These files are too large for the current context size. Raise it in Settings or pick fewer/smaller files.".into());
    }

    let prompt_text = prompt::build(
        proposals::system_prompt(),
        None,
        &[Turn { role: "user".into(), content: user_text }],
        settings.context_size,
        out_budget,
    );
    state.cancel.store(false, Ordering::Relaxed);
    let cancel = state.cancel.clone();
    let params = GenParams {
        temperature: 0.2,
        max_tokens: out_budget,
        n_ctx: settings.context_size,
        threads: hardware::cpu_threads(&state.hardware),
    };
    let reply = tauri::async_runtime::spawn_blocking(move || {
        let mut text = String::new();
        engine.generate(&prompt_text, &params, &cancel, |piece| {
            text.push_str(piece);
            true
        })?;
        Ok::<_, anyhow::Error>(text)
    })
    .await
    .map_err(err)?
    .map_err(err)?;

    let old_text = |p: &str| current.get(p).map(|c| c.0.clone());
    let kept = proposals::validate(proposals::parse_changes(&reply), &paths, &old_text);
    if kept.is_empty() {
        return Err("The model did not produce a usable change. Try a more specific instruction.".into());
    }

    let changes: Vec<FileChange> = kept
        .into_iter()
        .map(|(path, new_content)| {
            let old = old_text(&path);
            FileChange {
                diff: proposals::diff_view(old.as_deref().unwrap_or(""), &new_content),
                is_new: old.is_none(),
                sha: current.get(&path).map(|c| c.1.clone()),
                path,
                new_content,
            }
        })
        .collect();

    let proposal = Proposal {
        id: (now_ms() / 1000).to_string(),
        title: instruction.chars().take(60).collect(),
        description: instruction,
        changes,
    };
    state.proposals.lock().unwrap().insert(proposal.id.clone(), proposal.clone());
    Ok(proposal)
}

#[tauri::command]
pub fn github_reject(state: State<'_, AppState>, id: String) {
    state.proposals.lock().unwrap().remove(&id);
}

/// The only code path that writes to GitHub. Returns the draft PR URL.
#[tauri::command]
pub async fn github_approve(state: State<'_, AppState>, id: String) -> CmdResult<String> {
    let proposal = state
        .proposals
        .lock()
        .unwrap()
        .get(&id)
        .cloned()
        .ok_or("This proposal no longer exists. Generate it again.")?;
    for c in &proposal.changes {
        github::check_path(&c.path).map_err(err)?; // re-validate at the point of use
    }

    let gh = client(&state)?;
    let base = gh.default_branch().await.map_err(err)?;
    let base_sha = gh.branch_sha(&base).await.map_err(err)?;
    let branch = format!("ai/update-{}", proposal.id);
    gh.create_branch(&branch, &base_sha).await.map_err(err)?;

    let message = format!("AI proposal: {}", proposal.title);
    for c in &proposal.changes {
        gh.put_file(&c.path, &branch, &c.new_content, &message, c.sha.as_deref()).await.map_err(err)?;
    }
    let body = format!(
        "{}\n\n---\nDrafted by the on-device assistant and approved by the user inside the app.\n\
         **Review carefully before merging** - nothing here was run or tested automatically.",
        proposal.description
    );
    let url = gh.open_pr(&format!("[AI] {}", proposal.title), &body, &branch, &base).await.map_err(err)?;
    state.proposals.lock().unwrap().remove(&id);
    Ok(url)
}
