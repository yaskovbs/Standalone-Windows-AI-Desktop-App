//! Vision: describe an image or the screen with a multimodal GGUF model
//! (llama.cpp `llama-mtmd-cli` sidecar + the model's mmproj file).

use crate::tools::{self, now_ms, s, Tools};
use anyhow::{anyhow, Result};
use std::path::{Path, PathBuf};

pub async fn describe(
    tools: &Tools,
    model: &Path,
    mmproj: &Path,
    image: &Path,
    prompt: &str,
    max_tokens: u32,
    gpu: bool,
) -> Result<String> {
    let mut args: Vec<String> = vec![
        "-m".into(), s(model),
        "--mmproj".into(), s(mmproj),
        "--image".into(), s(image),
        "-p".into(), prompt.into(),
        "-n".into(), max_tokens.to_string(),
        "--temp".into(), "0.2".into(),
    ];
    if gpu {
        args.extend(["-ngl".into(), "99".into()]);
    }
    let out = tools::run(&tools.resolve("llama-mtmd-cli"), &args).await?;
    let text = out.stdout.trim().to_string();
    if text.is_empty() {
        return Err(anyhow!("the vision model returned no text"));
    }
    Ok(text)
}

/// Downscale with ffmpeg so large screenshots do not exhaust the context window.
pub async fn shrink(tools: &Tools, src: &Path, dst: &Path, width: u32) -> Result<()> {
    let args = vec![
        "-y".into(), "-i".into(), s(src),
        "-vf".into(), format!("scale='min({width},iw)':-2"),
        "-q:v".into(), "4".into(), s(dst),
    ];
    tools::run(&tools.resolve("ffmpeg"), &args).await?;
    Ok(())
}

/// Capture the primary monitor to a PNG file.
pub fn capture_screen(dest: &Path) -> Result<()> {
    let monitors = xcap::Monitor::all().map_err(|e| anyhow!("screen capture failed: {e}"))?;
    let monitor = monitors.into_iter().next().ok_or_else(|| anyhow!("no monitor found"))?;
    let img = monitor.capture_image().map_err(|e| anyhow!("screen capture failed: {e}"))?;
    img.save(dest).map_err(|e| anyhow!("could not save screenshot: {e}"))?;
    Ok(())
}

pub fn temp_path(prefix: &str, ext: &str) -> PathBuf {
    std::env::temp_dir().join(format!("{prefix}_{}.{ext}", now_ms()))
}
