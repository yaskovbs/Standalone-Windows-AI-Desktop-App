//! Non-AI signals used to rank moments: audio loudness, scene-change density, speech density.

use crate::tools::{self, s, Tools};
use anyhow::Result;
use std::path::Path;

/// RMS loudness per window, normalised so the loudest window is 1.0.
pub fn energy_from_samples(samples: impl Iterator<Item = i16>, rate: u32, interval: f32, n: usize) -> Vec<f32> {
    let (mut sum, mut count) = (vec![0f64; n], vec![0u64; n]);
    for (i, x) in samples.enumerate() {
        let w = (i as f32 / rate as f32 / interval) as usize;
        if w >= n {
            break;
        }
        sum[w] += (x as f64).powi(2);
        count[w] += 1;
    }
    let rms: Vec<f32> = sum.iter().zip(&count).map(|(s, c)| if *c == 0 { 0.0 } else { (s / *c as f64).sqrt() as f32 }).collect();
    normalise(rms)
}

pub fn audio_energy(wav: &Path, interval: f32, n: usize) -> Result<Vec<f32>> {
    let mut reader = hound::WavReader::open(wav)?;
    let rate = reader.spec().sample_rate;
    Ok(energy_from_samples(reader.samples::<i16>().filter_map(|x| x.ok()), rate, interval, n))
}

pub fn normalise(v: Vec<f32>) -> Vec<f32> {
    let max = v.iter().cloned().fold(0.0f32, f32::max);
    if max <= 0.0 { vec![0.0; v.len()] } else { v.into_iter().map(|x| x / max).collect() }
}

/// Count events (scene cuts, speech segments) per window, normalised to 0..1.
pub fn per_window(times: &[f32], interval: f32, n: usize) -> Vec<f32> {
    let mut counts = vec![0f32; n];
    for t in times {
        let w = (t / interval) as usize;
        if w < n {
            counts[w] += 1.0;
        }
    }
    normalise(counts)
}

/// Extract `pts_time:` values printed by ffmpeg's showinfo filter.
pub fn parse_scene_times(stderr: &str) -> Vec<f32> {
    stderr
        .lines()
        .filter_map(|l| l.split("pts_time:").nth(1))
        .filter_map(|rest| rest.split_whitespace().next()?.parse::<f32>().ok())
        .collect()
}

/// One extra pass over the film at low resolution to find hard cuts.
pub async fn scene_times(tools: &Tools, video: &Path) -> Result<Vec<f32>> {
    let args = vec![
        "-hide_banner".into(), "-i".into(), s(video), "-an".into(),
        "-vf".into(), "scale=320:-2,select=gt(scene\\,0.35),showinfo".into(),
        "-f".into(), "null".into(), "-".into(),
    ];
    let out = tools::run_raw(&tools.resolve("ffmpeg"), &args).await?;
    Ok(parse_scene_times(&String::from_utf8_lossy(&out.stderr)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loud_window_scores_highest() {
        // 2 windows of 1 s at 100 Hz: quiet then loud
        let samples = std::iter::repeat(10i16).take(100).chain(std::iter::repeat(1000i16).take(100));
        let e = energy_from_samples(samples, 100, 1.0, 2);
        assert!(e[0] < 0.02 && (e[1] - 1.0).abs() < 1e-6);
    }

    #[test]
    fn parses_showinfo_lines() {
        let log = "[Parsed_showinfo_2 @ 0x1] n:   0 pts:  12 pts_time:12.5 pos: 1 fmt:yuv420p\nnoise\n[x] n: 1 pts_time:80.04 pos";
        assert_eq!(parse_scene_times(log), vec![12.5, 80.04]);
    }

    #[test]
    fn buckets_events() {
        let v = per_window(&[1.0, 2.0, 12.0], 10.0, 3);
        assert_eq!(v, vec![1.0, 0.5, 0.0]);
    }
}
