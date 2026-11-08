//! GitHub client for the assistant's self-update flow.
//!
//! SAFETY MODEL (enforced here, not in the UI):
//!   * changes only ever go to a new `ai/…` branch and a DRAFT pull request;
//!   * there is no merge / push-to-default-branch code path at all;
//!   * only allow-listed paths can be written, and this file is protected from edits;
//!   * the token is read from the OS credential store and never sent to the frontend.

use anyhow::{anyhow, Result};
use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use reqwest::{Client, Method, RequestBuilder};
use serde_json::{json, Value};
use std::time::Duration;

pub const MAX_FILES: usize = 10;
pub const MAX_FILE_BYTES: usize = 100_000;
const ALLOWED_PREFIXES: [&str; 4] = ["app/src/", "app/src-tauri/src/", "pipeline/", "README.md"];
/// The guard itself must never be editable by the model.
const PROTECTED: [&str; 3] = [
    "app/src-tauri/src/github.rs",
    "app/src-tauri/src/proposals.rs",
    "app/src-tauri/src/commands_github.rs",
];

/// Validate a repo-relative path before reading or writing it.
pub fn check_path(path: &str) -> Result<()> {
    let bad_chars = path.contains("..") || path.contains('\\') || path.contains(':') || path.starts_with('/');
    if path.is_empty() || bad_chars || path.to_lowercase().contains(".github") {
        return Err(anyhow!("path not allowed: {path}"));
    }
    if PROTECTED.contains(&path) {
        return Err(anyhow!("{path} is protected and cannot be changed by the assistant"));
    }
    if !ALLOWED_PREFIXES.iter().any(|p| path == *p || (p.ends_with('/') && path.starts_with(p))) {
        return Err(anyhow!("{path} is outside the folders the assistant may edit"));
    }
    Ok(())
}

pub fn check_repo(repo: &str) -> Result<()> {
    let ok = repo.split('/').count() == 2
        && repo.split('/').all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_alphanumeric() || "-_.".contains(c)));
    if ok { Ok(()) } else { Err(anyhow!("repository must look like owner/name")) }
}

pub struct GitHub {
    http: Client,
    token: String,
    repo: String,
}

impl GitHub {
    pub fn new(repo: &str, token: &str) -> Result<Self> {
        check_repo(repo)?;
        let http = Client::builder()
            .user_agent("LocalAutonomousAI/0.1")
            .timeout(Duration::from_secs(30))
            .build()?;
        Ok(Self { http, token: token.to_string(), repo: repo.to_string() })
    }

    fn req(&self, method: Method, path: &str) -> RequestBuilder {
        self.http
            .request(method, format!("https://api.github.com/repos/{}{path}", self.repo))
            .bearer_auth(&self.token)
            .header("Accept", "application/vnd.github+json")
            .header("X-GitHub-Api-Version", "2022-11-28")
    }

    async fn json(rb: RequestBuilder) -> Result<Value> {
        let resp = rb.send().await?;
        let status = resp.status();
        let body: Value = resp.json().await.unwrap_or(Value::Null);
        if !status.is_success() {
            let msg = body["message"].as_str().unwrap_or("request failed");
            return Err(anyhow!("GitHub API {status}: {msg}"));
        }
        Ok(body)
    }

    pub async fn default_branch(&self) -> Result<String> {
        let v = Self::json(self.req(Method::GET, "")).await?;
        v["default_branch"].as_str().map(String::from).ok_or_else(|| anyhow!("no default branch"))
    }

    pub async fn branch_sha(&self, branch: &str) -> Result<String> {
        let v = Self::json(self.req(Method::GET, &format!("/git/ref/heads/{branch}"))).await?;
        v["object"]["sha"].as_str().map(String::from).ok_or_else(|| anyhow!("branch has no sha"))
    }

    /// Returns (text, blob sha), or None when the file does not exist on `branch`.
    pub async fn file(&self, path: &str, branch: &str) -> Result<Option<(String, String)>> {
        check_path(path)?;
        let resp = self.req(Method::GET, &format!("/contents/{path}?ref={branch}")).send().await?;
        if resp.status().as_u16() == 404 {
            return Ok(None);
        }
        if !resp.status().is_success() {
            return Err(anyhow!("GitHub API {}", resp.status()));
        }
        let v: Value = resp.json().await?;
        let raw = v["content"].as_str().ok_or_else(|| anyhow!("{path} is not a file"))?.replace('\n', "");
        let text = String::from_utf8(B64.decode(raw)?)?;
        Ok(Some((text, v["sha"].as_str().unwrap_or_default().to_string())))
    }

    pub async fn create_branch(&self, name: &str, from_sha: &str) -> Result<()> {
        if !name.starts_with("ai/") {
            return Err(anyhow!("assistant branches must start with ai/"));
        }
        let body = json!({ "ref": format!("refs/heads/{name}"), "sha": from_sha });
        Self::json(self.req(Method::POST, "/git/refs").json(&body)).await?;
        Ok(())
    }

    pub async fn put_file(&self, path: &str, branch: &str, content: &str, message: &str, sha: Option<&str>) -> Result<()> {
        check_path(path)?;
        if !branch.starts_with("ai/") {
            return Err(anyhow!("refusing to write outside an ai/ branch"));
        }
        let mut body = json!({ "message": message, "content": B64.encode(content), "branch": branch });
        if let Some(sha) = sha.filter(|s| !s.is_empty()) {
            body["sha"] = json!(sha);
        }
        Self::json(self.req(Method::PUT, &format!("/contents/{path}")).json(&body)).await?;
        Ok(())
    }

    /// Always opens a draft PR. Returns its URL.
    pub async fn open_pr(&self, title: &str, body: &str, head: &str, base: &str) -> Result<String> {
        let payload = json!({ "title": title, "body": body, "head": head, "base": base, "draft": true });
        let v = Self::json(self.req(Method::POST, "/pulls").json(&payload)).await?;
        v["html_url"].as_str().map(String::from).ok_or_else(|| anyhow!("PR created but no URL returned"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allows_app_sources() {
        assert!(check_path("app/src/App.tsx").is_ok());
        assert!(check_path("pipeline/config.py").is_ok());
        assert!(check_path("README.md").is_ok());
    }

    #[test]
    fn blocks_dangerous_paths() {
        for p in [
            ".github/workflows/ci.yml",
            "app/src/../../.github/x",
            "/etc/passwd",
            "app/src-tauri/Cargo.toml",
            "app/src-tauri/src/github.rs",
            "app/src-tauri/src/proposals.rs",
            "app\\src\\App.tsx",
            "",
        ] {
            assert!(check_path(p).is_err(), "{p} should be rejected");
        }
    }

    #[test]
    fn validates_repo_names() {
        assert!(check_repo("yaskovbs/Standalone-Offline-Windows-AI-Desktop-App").is_ok());
        assert!(check_repo("a/b/c").is_err());
        assert!(check_repo("bad repo/x").is_err());
    }
}
