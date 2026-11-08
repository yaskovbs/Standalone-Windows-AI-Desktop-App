//! "Watching" a movie or episode from a local video file.
//!
//! The file is sampled: ffmpeg extracts a frame every N seconds plus the audio track,
//! whisper.cpp transcribes the audio with timestamps, and the vision model describes
//! each frame. Everything is merged into a time-coded timeline the chat model can use.

use crate::tools::{self, now_ms, s, Tools};
use crate::{vision, voice};
use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

const FRAME_PROMPT: &str = "This is one frame from a movie or TV episode. In one or two sentences \
describe the setting, who is visible, what they are doing, and any readable on-screen text.";

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "event", content = "data", rename_all = "camelCase")]
pub enum MediaEvent {
    Stage { label: String },
    Progress { done: usize, total: usize },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaMemory {
    pub title: String,
    pub duration_secs: f32,
    pub frames: usize,
    pub heard_audio: bool,
    pub timeline: String,
}

pub struct MediaConfig {
    pub interval_secs: u32,
    pub max_frames: u32,
    pub gpu: bool,
    pub scene_detect: bool,
}

pub struct Models<'a> {
    pub vision: &'a Path,
    pub mmproj: &'a Path,
    pub whisper: Option<(&'a Path, &'a str)>, // (model, language)
}

pub fn fmt_ts(secs: f32) -> String {
    let t = secs.max(0.0) as u32;
    if t >= 3600 {
        format!("{}:{:02}:{:02}", t / 3600, (t % 3600) / 60, t % 60)
    } else {
        format!("{:02}:{:02}", t / 60, t % 60)
    }
}

/// Parse `Duration: 01:32:10.05` from ffmpeg's banner.
pub fn parse_duration(banner: &str) -> Option<f32> {
    let rest = banner.split("Duration:").nth(1)?;
    let ts = rest.split(',').next()?.trim();
    let p: Vec<&str> = ts.split(':').collect();
    if p.len() != 3 {
        return None;
    }
    Some(p[0].parse::<f32>().ok()? * 3600.0 + p[1].parse::<f32>().ok()? * 60.0 + p[2].parse::<f32>().ok()?)
}

pub fn pick_interval(duration: f32, cfg: &MediaConfig) -> f32 {
    (cfg.interval_secs as f32).max(duration / cfg.max_frames.max(1) as f32).max(1.0)
}

/// Merge frame descriptions and speech into one chronological text.
pub fn build_timeline(seen: &[(f32, String)], heard: &[(f32, String)], interval: f32) -> String {
    let mut lines = Vec::new();
    for (t, desc) in seen {
        let said: Vec<&str> = heard
            .iter()
            .filter(|(st, _)| *st >= *t && *st < *t + interval)
            .map(|(_, x)| x.as_str())
            .collect();
        let mut line = format!("[{}] SEEN: {}", fmt_ts(*t), desc.trim().replace('\n', " "));
        if !said.is_empty() {
            line.push_str(&format!(" | HEARD: \"{}\"", said.join(" ")));
        }
        lines.push(line);
    }
    lines.join("\n")
}

/// Everything learned about a film, kept structured so other features (recaps) can reuse it.
pub struct Analysis {
    pub title: String,
    pub duration: f32,
    pub interval: f32,
    pub seen: Vec<(f32, String)>,
    pub heard: Vec<(f32, String)>,
    pub energy: Vec<f32>, // per window, 0..1 (empty unless signals were requested)
    pub scene: Vec<f32>,
}

impl Analysis {
    pub fn memory(&self) -> MediaMemory {
        MediaMemory {
            title: self.title.clone(),
            duration_secs: self.duration,
            frames: self.seen.len(),
            heard_audio: !self.heard.is_empty(),
            timeline: build_timeline(&self.seen, &self.heard, self.interval),
        }
    }
}

pub async fn watch<F>(
    tools: &Tools,
    path: &Path,
    models: &Models<'_>,
    cfg: &MediaConfig,
    cancel: &AtomicBool,
    emit: F,
) -> Result<MediaMemory>
where
    F: Fn(MediaEvent),
{
    Ok(analyze(tools, path, models, cfg, cancel, &emit, false).await?.memory())
}

/// Sample, listen to and describe a whole film. `signals` adds loudness and scene-cut analysis.
pub async fn analyze<F: Fn(MediaEvent)>(
    tools: &Tools,
    path: &Path,
    models: &Models<'_>,
    cfg: &MediaConfig,
    cancel: &AtomicBool,
    emit: &F,
    signals: bool,
) -> Result<Analysis> {
    if !path.is_file() {
        return Err(anyhow!("file not found: {}", path.display()));
    }
    let work = std::env::temp_dir().join(format!("lai_media_{}", now_ms()));
    tokio::fs::create_dir_all(&work).await?;
    let result = pipeline(tools, path, models, cfg, cancel, emit, &work, signals).await;
    let _ = tokio::fs::remove_dir_all(&work).await;
    result
}

#[allow(clippy::too_many_arguments)]
async fn pipeline<F: Fn(MediaEvent)>(
    tools: &Tools,
    path: &Path,
    models: &Models<'_>,
    cfg: &MediaConfig,
    cancel: &AtomicBool,
    emit: &F,
    work: &Path,
    signals: bool,
) -> Result<Analysis> {
    let ffmpeg = tools.resolve("ffmpeg");
    emit(MediaEvent::Stage { label: "Reading video info".into() });
    let banner = tools::run_raw(&ffmpeg, &["-i".into(), s(path)]).await?;
    let duration = parse_duration(&String::from_utf8_lossy(&banner.stderr))
        .ok_or_else(|| anyhow!("ffmpeg could not read this file (is it a video?)"))?;
    let interval = pick_interval(duration, cfg);

    emit(MediaEvent::Stage { label: format!("Extracting a frame every {interval:.0}s") });
    let pattern = work.join("f_%04d.jpg");
    tools::run(&ffmpeg, &[
        "-y".into(), "-i".into(), s(path),
        "-vf".into(), format!("fps=1/{interval},scale=640:-2"),
        "-frames:v".into(), cfg.max_frames.to_string(),
        "-q:v".into(), "4".into(), s(&pattern),
    ]).await?;
    let mut frames: Vec<PathBuf> = std::fs::read_dir(work)?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().map(|x| x == "jpg").unwrap_or(false))
        .collect();
    frames.sort();
    if frames.is_empty() {
        return Err(anyhow!("no frames could be extracted"));
    }

    // Audio is needed for speech-to-text and for loudness; a film without sound simply skips both.
    let mut heard = Vec::new();
    let mut energy = Vec::new();
    let wav = work.join("audio.wav");
    let has_audio = (models.whisper.is_some() || signals)
        && tools::run(&ffmpeg, &[
            "-y".into(), "-i".into(), s(path), "-vn".into(),
            "-ac".into(), "1".into(), "-ar".into(), "16000".into(), s(&wav),
        ]).await.is_ok();
    if !has_audio && (models.whisper.is_some() || signals) {
        emit(MediaEvent::Stage { label: "No audio track found".into() });
    }
    if has_audio {
        if let Some((model, lang)) = models.whisper {
            emit(MediaEvent::Stage { label: "Listening to the audio track".into() });
            heard = voice::parse_segments(&voice::transcribe(tools, model, &wav, lang, true).await?);
        }
        if signals {
            energy = crate::signals::audio_energy(&wav, interval, frames.len()).unwrap_or_default();
        }
    }

    let mut scene = Vec::new();
    if signals && cfg.scene_detect {
        emit(MediaEvent::Stage { label: "Detecting scene changes".into() });
        if let Ok(times) = crate::signals::scene_times(tools, path).await {
            scene = crate::signals::per_window(&times, interval, frames.len());
        }
    }

    emit(MediaEvent::Stage { label: "Watching the frames".into() });
    let mut seen = Vec::new();
    for (i, frame) in frames.iter().enumerate() {
        if cancel.load(Ordering::Relaxed) {
            return Err(anyhow!("cancelled"));
        }
        emit(MediaEvent::Progress { done: i, total: frames.len() });
        let desc = match vision::describe(tools, models.vision, models.mmproj, frame, FRAME_PROMPT, 120, cfg.gpu).await {
            Ok(d) => d,
            Err(e) if i == 0 => return Err(e), // the vision setup itself is broken
            Err(e) => format!("(frame unreadable: {e})"),
        };
        seen.push((i as f32 * interval, desc));
    }
    emit(MediaEvent::Progress { done: frames.len(), total: frames.len() });

    Ok(Analysis {
        title: path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(),
        duration,
        interval,
        seen,
        heard,
        energy,
        scene,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_ffmpeg_duration() {
        let banner = "  Duration: 01:32:10.50, start: 0.000000, bitrate: 2000 kb/s";
        assert!((parse_duration(banner).unwrap() - 5530.5).abs() < 0.01);
    }

    #[test]
    fn merges_speech_into_frame_windows() {
        let seen = vec![(0.0, "A kitchen".to_string()), (20.0, "A street".to_string())];
        let heard = vec![(5.0, "Hello".to_string()), (25.0, "Run!".to_string())];
        let t = build_timeline(&seen, &heard, 20.0);
        assert_eq!(t, "[00:00] SEEN: A kitchen | HEARD: \"Hello\"\n[00:20] SEEN: A street | HEARD: \"Run!\"");
    }

    #[test]
    fn interval_respects_frame_cap() {
        let cfg = MediaConfig { interval_secs: 10, max_frames: 40, gpu: false, scene_detect: false };
        assert_eq!(pick_interval(7200.0, &cfg), 180.0);
        assert_eq!(pick_interval(100.0, &cfg), 10.0);
    }
}
