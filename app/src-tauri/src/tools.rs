//! Locating and running external sidecar tools (ffmpeg, whisper.cpp, llama.cpp multimodal CLI).

use anyhow::{anyhow, Result};
use std::path::{Path, PathBuf};
use std::process::{Output, Stdio};
use tokio::process::Command;

pub struct Tools {
    dirs: Vec<PathBuf>,
}

impl Tools {
    pub fn new(dirs: Vec<PathBuf>) -> Self {
        Self { dirs }
    }

    pub fn primary_dir(&self) -> &Path {
        &self.dirs[0]
    }

    fn file_name(name: &str) -> String {
        if cfg!(windows) { format!("{name}.exe") } else { name.to_string() }
    }

    /// Search the tool directories first, then every directory on PATH.
    pub fn find(&self, name: &str) -> Option<PathBuf> {
        let file = Self::file_name(name);
        let path_dirs = std::env::var_os("PATH")
            .map(|p| std::env::split_paths(&p).collect::<Vec<_>>())
            .unwrap_or_default();
        self.dirs
            .iter()
            .chain(path_dirs.iter())
            .map(|d| d.join(&file))
            .find(|p| p.is_file())
    }

    /// Path to run; falls back to the bare name so the OS reports a clear error.
    pub fn resolve(&self, name: &str) -> PathBuf {
        self.find(name).unwrap_or_else(|| PathBuf::from(Self::file_name(name)))
    }
}

pub fn now_ms() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0)
}

fn command(program: &Path, args: &[String]) -> Command {
    let mut cmd = Command::new(program);
    cmd.args(args).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped()).kill_on_drop(true);
    #[cfg(windows)]
    cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    cmd
}

pub fn tail(s: &str, max: usize) -> String {
    let chars: Vec<char> = s.trim().chars().collect();
    chars[chars.len().saturating_sub(max)..].iter().collect()
}

/// Run and return the output whatever the exit code (ffmpeg -i exits non-zero by design).
pub async fn run_raw(program: &Path, args: &[String]) -> Result<Output> {
    command(program, args)
        .output()
        .await
        .map_err(|e| anyhow!("could not start {}: {e}", program.display()))
}

pub struct Out {
    pub stdout: String,
    #[allow(dead_code)]
    pub stderr: String,
}

/// Run and fail with the tail of stderr on a non-zero exit code.
pub async fn run(program: &Path, args: &[String]) -> Result<Out> {
    let out = run_raw(program, args).await?;
    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
    if !out.status.success() {
        return Err(anyhow!("{} failed: {}", program.display(), tail(&stderr, 400)));
    }
    Ok(Out { stdout, stderr })
}

pub fn s(p: &Path) -> String {
    p.to_string_lossy().into_owned()
}
