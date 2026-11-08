//! Recap maker: watches a film with the local models only (vision + speech + LLM, no cloud AI),
//! picks the story's key moments, cuts them with ffmpeg and optionally adds a spoken narration.

use crate::engine::{Engine, GenParams};
use crate::media::{self, MediaConfig, MediaEvent, Models};
use crate::prompt::{self, Turn};
use crate::recap_plan::{self, Clip, Signals};
use crate::recap_render;
use crate::settings::Settings;
use crate::signals;
use crate::tools::{now_ms, Tools};
use crate::voice;
use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

const SCORE_SYSTEM: &str = "You are a film editor choosing moments for a recap. Follow the output format exactly.";
const NARRATE_SYSTEM: &str = "You write short, vivid recap narration for films and TV episodes. \
Use the present tense, plain spoken sentences, no lists, no headings.";
const BATCH: usize = 20;

#[derive(Debug, Clone, Deserialize)]
pub struct RecapRequest {
    pub path: String,
    pub title: String,
    pub description: String,
    pub target_secs: u32,
    pub clip_secs: u32,
    pub max_frames: u32,
    pub narrate: bool,
    pub language: String,
    pub scene_detect: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct ClipInfo {
    pub start: f32,
    pub end: f32,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct RecapResult {
    pub output_path: String,
    pub clips: Vec<ClipInfo>,
    pub narration: Option<String>,
    pub voiceover: bool,
    pub total_secs: f32,
    pub rated_by_model: bool,
}

pub struct Ctx<'a> {
    pub tools: &'a Tools,
    pub engine: Arc<Engine>,
    pub settings: &'a Settings,
    pub models: &'a Models<'a>,
    pub gpu: bool,
    pub threads: i32,
    pub cancel: Arc<AtomicBool>,
}

fn truncate(s: &str, max: usize) -> String {
    s.chars().take(max).collect()
}

async fn ask(ctx: &Ctx<'_>, system: &str, user: String, max_tokens: u32, temperature: f32) -> Result<String> {
    let n_ctx = ctx.settings.context_size;
    let text = prompt::build(system, None, &[Turn { role: "user".into(), content: user }], n_ctx, max_tokens);
    let params = GenParams { temperature, max_tokens, n_ctx, threads: ctx.threads };
    let (engine, cancel) = (ctx.engine.clone(), ctx.cancel.clone());
    tauri::async_runtime::spawn_blocking(move || {
        let mut out = String::new();
        engine.generate(&text, &params, &cancel, |p| {
            out.push_str(p);
            true
        })?;
        Ok::<_, anyhow::Error>(out)
    })
    .await
    .map_err(|e| anyhow!(e.to_string()))?
}

/// Let the local LLM rate every sampled moment. None when too many batches were unreadable.
async fn score_moments<F: Fn(MediaEvent)>(ctx: &Ctx<'_>, lines: &[String], req: &RecapRequest, emit: &F) -> Option<Vec<f32>> {
    let total = lines.len().div_ceil(BATCH);
    let mut scores: Vec<Option<f32>> = Vec::new();
    for (b, chunk) in lines.chunks(BATCH).enumerate() {
        if ctx.cancel.load(Ordering::Relaxed) {
            return None;
        }
        emit(MediaEvent::Stage { label: format!("Rating story moments {}/{}", b + 1, total) });
        let list: String = chunk.iter().enumerate().map(|(i, l)| format!("{}. {}\n", i + 1, truncate(l, 240))).collect();
        let user = format!(
            "Film: {}\nNotes from the user: {}\n\nRate how important each moment is for a recap of the story: \
             0 = filler, 9 = key plot turn, climax or crucial dialogue. Reply with ONLY {} digits separated by commas, in order.\n\n{}",
            req.title, truncate(&req.description, 600), chunk.len(), list
        );
        let reply = ask(ctx, SCORE_SYSTEM, user, (chunk.len() * 3 + 8) as u32, 0.0).await.ok();
        match reply.and_then(|r| recap_plan::parse_scores(&r, chunk.len())) {
            Some(v) => scores.extend(v.into_iter().map(Some)),
            None => scores.extend(std::iter::repeat(None).take(chunk.len())),
        }
    }
    let known: Vec<f32> = scores.iter().flatten().copied().collect();
    if known.is_empty() || known.len() * 2 < lines.len() {
        return None;
    }
    let mean = known.iter().sum::<f32>() / known.len() as f32;
    Some(scores.into_iter().map(|x| x.unwrap_or(mean)).collect())
}

fn summary_for(a: &media::Analysis, clip: &Clip) -> String {
    let centre = (clip.start + clip.end) / 2.0;
    a.seen
        .iter()
        .min_by(|x, y| (x.0 - centre).abs().partial_cmp(&(y.0 - centre).abs()).unwrap_or(std::cmp::Ordering::Equal))
        .map(|(_, d)| d.trim().to_string())
        .unwrap_or_default()
}

pub async fn make<F: Fn(MediaEvent)>(ctx: &Ctx<'_>, req: &RecapRequest, emit: &F) -> Result<RecapResult> {
    let src = PathBuf::from(&req.path);
    let target = req.target_secs.clamp(15, 1800) as f32;
    let clip_secs = req.clip_secs.clamp(3, 30);
    let cfg = MediaConfig {
        interval_secs: clip_secs,
        max_frames: req.max_frames.clamp(20, 2000),
        gpu: ctx.gpu,
        scene_detect: req.scene_detect,
    };

    let a = media::analyze(ctx.tools, &src, ctx.models, &cfg, &ctx.cancel, emit, true).await?;
    let n = a.seen.len();
    let heard_times: Vec<f32> = a.heard.iter().map(|(t, _)| *t).collect();
    let speech = signals::per_window(&heard_times, a.interval, n);

    let lines: Vec<String> = a
        .seen
        .iter()
        .map(|s| media::build_timeline(std::slice::from_ref(s), &a.heard, a.interval))
        .collect();
    let llm = score_moments(ctx, &lines, req, emit).await;
    if ctx.cancel.load(Ordering::Relaxed) {
        return Err(anyhow!("cancelled"));
    }

    let sig = Signals { llm: llm.as_deref(), audio: &a.energy, scene: &a.scene, speech: &speech };
    let clips = recap_plan::plan(&sig, n, a.interval, a.duration, target, clip_secs as f32);
    if clips.is_empty() {
        return Err(anyhow!("no moments could be selected"));
    }
    let total: f32 = clips.iter().map(|c| c.end - c.start).sum();
    let infos: Vec<ClipInfo> = clips
        .iter()
        .map(|c| ClipInfo { start: c.start, end: c.end, summary: summary_for(&a, c) })
        .collect();

    let work = std::env::temp_dir().join(format!("lai_recap_{}", now_ms()));
    tokio::fs::create_dir_all(&work).await?;
    let result = render(ctx, req, &src, &clips, &infos, total, llm.is_some(), &work, emit).await;
    let _ = tokio::fs::remove_dir_all(&work).await;
    result
}

#[allow(clippy::too_many_arguments)]
async fn render<F: Fn(MediaEvent)>(
    ctx: &Ctx<'_>,
    req: &RecapRequest,
    src: &Path,
    clips: &[Clip],
    infos: &[ClipInfo],
    total: f32,
    rated: bool,
    work: &Path,
    emit: &F,
) -> Result<RecapResult> {
    let stem = src.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| "film".into());
    let out = recap_render::unique_path(&src.with_file_name(format!("{stem}_recap.mp4")));
    emit(MediaEvent::Stage { label: format!("Cutting {} clips", clips.len()) });
    let silent = if req.narrate { work.join("silent.mp4") } else { out.clone() };
    recap_render::cut_and_join(ctx.tools, src, clips, &silent, work, &ctx.cancel, emit).await?;

    let (mut narration, mut voiceover) = (None, false);
    if req.narrate {
        emit(MediaEvent::Stage { label: "Writing the narration".into() });
        let beats: String = infos
            .iter()
            .enumerate()
            .map(|(i, c)| format!("{}. {}\n", i + 1, truncate(&c.summary, 220)))
            .collect();
        let words = recap_plan::narration_words(total);
        let user = format!(
            "Film: {}\nNotes from the user: {}\n\nThese are the scenes of the recap, in order:\n{}\n\
             Write about {} words of narration in {} that tells this story scene by scene. Output only the narration.",
            req.title, truncate(&req.description, 600), beats, words, req.language
        );
        let text = recap_plan::strip_tags(&ask(ctx, NARRATE_SYSTEM, user, (words * 2) as u32, 0.6).await?);
        narration = Some(text.clone());

        emit(MediaEvent::Stage { label: "Recording the voice-over".into() });
        let wav = work.join("narration.wav");
        let rate = ctx.settings.speech_rate;
        let (t, w) = (text, wav.clone());
        let spoken = tauri::async_runtime::spawn_blocking(move || voice::speak_to_wav(&t, rate, &w)).await;
        let mixed = match spoken {
            Ok(Ok(())) => recap_render::add_voiceover(ctx.tools, &silent, &wav, &out).await.is_ok(),
            _ => false,
        };
        voiceover = mixed;
        if !mixed {
            tokio::fs::copy(&silent, &out).await?; // keep the recap even if voice-over is unavailable
        }
    }

    Ok(RecapResult {
        output_path: out.display().to_string(),
        clips: infos.to_vec(),
        narration,
        voiceover,
        total_secs: total,
        rated_by_model: rated,
    })
}
