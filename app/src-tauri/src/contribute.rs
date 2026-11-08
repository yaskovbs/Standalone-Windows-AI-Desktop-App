//! Opt-in sharing of helpful answers with the project's own server, so future
//! versions of the model can be trained on them. Only text pairs the user rated 👍
//! are sent, after redaction. Media, audio, screenshots and settings are never sent.

use crate::redact::redact;
use anyhow::{anyhow, Result};
use serde::Serialize;
use serde_json::json;

/// Fixed training system prompt - the user's custom system prompt is never uploaded.
pub const TRAINING_SYSTEM: &str = "You are a direct, precise assistant. Think step by step inside \
<reasoning> tags, then give the final result inside <answer> tags.";
const MAX_CHARS: usize = 6000;

#[derive(Debug, Clone, Serialize)]
pub struct Message {
    pub role: &'static str,
    pub content: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Item {
    pub messages: Vec<Message>,
}

/// Build a redacted training example, or None if it is unsuitable to share.
pub fn build_item(user: &str, assistant: &str) -> Option<Item> {
    let (user, assistant) = (redact(user.trim()), redact(assistant.trim()));
    if user.is_empty() || assistant.len() < 40 || user.len() + assistant.len() > MAX_CHARS {
        return None;
    }
    Some(Item {
        messages: vec![
            Message { role: "system", content: TRAINING_SYSTEM.into() },
            Message { role: "user", content: user },
            Message { role: "assistant", content: assistant },
        ],
    })
}

/// Only https is allowed (plain http only for a local test server).
pub fn check_base_url(url: &str) -> Result<String> {
    let url = url.trim().trim_end_matches('/').to_string();
    let local = url.starts_with("http://localhost") || url.starts_with("http://127.0.0.1");
    if url.starts_with("https://") || local {
        Ok(url)
    } else {
        Err(anyhow!("the sharing server must use https://"))
    }
}

pub async fn send(base_url: &str, install_id: &str, items: Vec<Item>) -> Result<()> {
    let base = check_base_url(base_url)?;
    let body = json!({
        "install_id": install_id,
        "app_version": env!("CARGO_PKG_VERSION"),
        "items": items,
    });
    let resp = crate::web::client()?.post(format!("{base}/v1/contribute")).json(&body).send().await?;
    if !resp.status().is_success() {
        return Err(anyhow!("sharing server answered HTTP {}", resp.status()));
    }
    Ok(())
}

/// Ask the server to delete everything stored for this install.
pub async fn withdraw(base_url: &str, install_id: &str) -> Result<()> {
    let base = check_base_url(base_url)?;
    let resp = crate::web::client()?
        .delete(format!("{base}/v1/contributions/{install_id}"))
        .send()
        .await?;
    if !resp.status().is_success() {
        return Err(anyhow!("deletion request failed: HTTP {}", resp.status()));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redacts_and_never_includes_custom_system_prompt() {
        let item = build_item("my mail is a@b.com", &"<reasoning>x</reasoning><answer>".to_string().repeat(3)).unwrap();
        assert_eq!(item.messages[0].content, TRAINING_SYSTEM);
        assert!(item.messages[1].content.contains("[EMAIL]"));
    }

    #[test]
    fn rejects_short_or_huge_pairs() {
        assert!(build_item("hi", "short").is_none());
        assert!(build_item(&"x".repeat(7000), &"y".repeat(100)).is_none());
    }

    #[test]
    fn only_https_or_localhost() {
        assert!(check_base_url("https://example.com/").is_ok());
        assert!(check_base_url("http://localhost:8787").is_ok());
        assert!(check_base_url("http://example.com").is_err());
    }
}
