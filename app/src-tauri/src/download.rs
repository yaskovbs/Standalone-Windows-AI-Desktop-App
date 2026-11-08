//! Resumable model download with progress events.

use crate::web::client;
use anyhow::{anyhow, Result};
use futures_util::StreamExt;
use serde::Serialize;
use std::path::Path;
use tokio::io::AsyncWriteExt;

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "event", content = "data", rename_all = "camelCase")]
pub enum DownloadEvent {
    Progress { downloaded: u64, total: u64 },
    Done { path: String },
    Error { message: String },
}

/// Download `url` to `dest`, resuming from `dest.part` if it exists.
/// `on_progress(downloaded, total)` is throttled to ~10 updates per second.
pub async fn download<F>(url: &str, dest: &Path, mut on_progress: F) -> Result<()>
where
    F: FnMut(u64, u64),
{
    if !url.starts_with("https://") {
        return Err(anyhow!("model URL must use https://"));
    }
    if let Some(parent) = dest.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    let part = dest.with_extension("gguf.part");
    let existing = tokio::fs::metadata(&part).await.map(|m| m.len()).unwrap_or(0);

    let http = reqwest::Client::builder()
        .user_agent("LocalAutonomousAI/0.1")
        .connect_timeout(std::time::Duration::from_secs(20))
        .build()?;
    let mut req = http.get(url);
    if existing > 0 {
        req = req.header("Range", format!("bytes={existing}-"));
    }
    let resp = req.send().await?;
    let status = resp.status();
    if !status.is_success() {
        return Err(anyhow!("download failed: HTTP {status}"));
    }
    let resumed = status.as_u16() == 206;
    let offset = if resumed { existing } else { 0 };
    let total = resp.content_length().map(|l| l + offset).unwrap_or(0);

    let mut file = tokio::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .append(resumed)
        .truncate(!resumed)
        .open(&part)
        .await?;

    let mut downloaded = offset;
    let mut last_emit = std::time::Instant::now();
    let mut stream = resp.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        file.write_all(&chunk).await?;
        downloaded += chunk.len() as u64;
        if last_emit.elapsed().as_millis() >= 100 {
            on_progress(downloaded, total);
            last_emit = std::time::Instant::now();
        }
    }
    file.flush().await?;
    drop(file);

    if total > 0 && downloaded < total {
        return Err(anyhow!("download incomplete ({downloaded}/{total}); retry to resume"));
    }
    tokio::fs::rename(&part, dest).await?;
    on_progress(downloaded, downloaded.max(total));
    Ok(())
}

/// Sanity-check that a file looks like a GGUF model (magic bytes "GGUF").
pub fn is_gguf(path: &Path) -> bool {
    use std::io::Read;
    let mut magic = [0u8; 4];
    std::fs::File::open(path)
        .and_then(|mut f| f.read_exact(&mut magic))
        .map(|_| &magic == b"GGUF")
        .unwrap_or(false)
}
