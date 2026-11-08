//! Turning a clip plan into a video file with ffmpeg: cut, join, optional voice-over.

use crate::media::MediaEvent;
use crate::recap_plan::Clip;
use crate::tools::{self, s, Tools};
use anyhow::{anyhow, Result};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

/// ffmpeg concat-demuxer list; single quotes in paths must be escaped as '\''.
pub fn concat_list(parts: &[PathBuf]) -> String {
    parts
        .iter()
        .map(|p| format!("file '{}'\n", p.to_string_lossy().replace('\\', "/").replace('\'', "'\\''")))
        .collect()
}

/// Never overwrite an existing file: recap.mp4 -> recap_2.mp4 -> recap_3.mp4 ...
pub fn unique_path(wanted: &Path) -> PathBuf {
    if !wanted.exists() {
        return wanted.to_path_buf();
    }
    let stem = wanted.file_stem().map(|x| x.to_string_lossy().into_owned()).unwrap_or_default();
    let ext = wanted.extension().map(|x| x.to_string_lossy().into_owned()).unwrap_or_else(|| "mp4".into());
    (2..)
        .map(|n| wanted.with_file_name(format!("{stem}_{n}.{ext}")))
        .find(|p| !p.exists())
        .unwrap()
}

pub async fn cut_and_join<F: Fn(MediaEvent)>(
    tools: &Tools,
    src: &Path,
    clips: &[Clip],
    out: &Path,
    work: &Path,
    cancel: &AtomicBool,
    emit: &F,
) -> Result<()> {
    let ffmpeg = tools.resolve("ffmpeg");
    let mut parts = Vec::new();
    for (i, c) in clips.iter().enumerate() {
        if cancel.load(Ordering::Relaxed) {
            return Err(anyhow!("cancelled"));
        }
        emit(MediaEvent::Progress { done: i, total: clips.len() });
        let part = work.join(format!("part_{i:04}.mp4"));
        // Re-encode every part with identical settings so the join is clean at any cut point.
        tools::run(&ffmpeg, &[
            "-y".into(), "-ss".into(), format!("{:.3}", c.start), "-i".into(), s(src),
            "-t".into(), format!("{:.3}", c.end - c.start),
            "-c:v".into(), "libx264".into(), "-preset".into(), "veryfast".into(), "-crf".into(), "23".into(),
            "-pix_fmt".into(), "yuv420p".into(), "-c:a".into(), "aac".into(), "-b:a".into(), "160k".into(),
            "-ar".into(), "48000".into(), "-ac".into(), "2".into(), s(&part),
        ]).await?;
        parts.push(part);
    }
    emit(MediaEvent::Progress { done: clips.len(), total: clips.len() });

    emit(MediaEvent::Stage { label: "Joining the clips".into() });
    let list = work.join("parts.txt");
    tokio::fs::write(&list, concat_list(&parts)).await?;
    tools::run(&ffmpeg, &[
        "-y".into(), "-f".into(), "concat".into(), "-safe".into(), "0".into(), "-i".into(), s(&list),
        "-c".into(), "copy".into(), "-movflags".into(), "+faststart".into(), s(out),
    ]).await?;
    Ok(())
}

/// Mix a narration track over the recap, lowering the film's own sound underneath it.
pub async fn add_voiceover(tools: &Tools, video: &Path, narration_wav: &Path, out: &Path) -> Result<()> {
    tools::run(&tools.resolve("ffmpeg"), &[
        "-y".into(), "-i".into(), s(video), "-i".into(), s(narration_wav),
        "-filter_complex".into(),
        "[0:a]volume=0.30[bg];[1:a]volume=1.4[vo];[bg][vo]amix=inputs=2:duration=first:dropout_transition=0[a]".into(),
        "-map".into(), "0:v".into(), "-map".into(), "[a]".into(),
        "-c:v".into(), "copy".into(), "-c:a".into(), "aac".into(), "-b:a".into(), "160k".into(), s(out),
    ]).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escapes_quotes_and_backslashes_in_concat_list() {
        let list = concat_list(&[PathBuf::from("C:\\tmp\\it's.mp4")]);
        assert_eq!(list, "file 'C:/tmp/it'\\''s.mp4'\n");
    }

    #[test]
    fn unique_path_never_returns_an_existing_file() {
        let dir = std::env::temp_dir().join(format!("lai_test_{}", crate::tools::now_ms()));
        std::fs::create_dir_all(&dir).unwrap();
        let a = dir.join("x.mp4");
        std::fs::write(&a, b"1").unwrap();
        assert_eq!(unique_path(&a), dir.join("x_2.mp4"));
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
