//! Choosing which moments go into a recap. Pure functions, no I/O, fully unit-tested.
//!
//! Strategy: split the film into one bucket per wanted clip and take the best-scoring
//! moment of each bucket. That keeps the recap in story order and covers the whole film
//! (a plain top-N would cluster around one loud action scene).

#[derive(Debug, Clone, PartialEq)]
pub struct Clip {
    pub start: f32,
    pub end: f32,
}

/// Per-window signals, each normalised to 0..1. `llm` is None when the local model
/// could not rate the moments (the heuristics then carry the whole decision).
pub struct Signals<'a> {
    pub llm: Option<&'a [f32]>,
    pub audio: &'a [f32],
    pub scene: &'a [f32],
    pub speech: &'a [f32],
}

fn at(v: &[f32], i: usize) -> f32 {
    v.get(i).copied().unwrap_or(0.0).clamp(0.0, 1.0)
}

pub fn score(sig: &Signals, i: usize, t: f32, duration: f32) -> f32 {
    let (wl, wa, ws, wp) = if sig.llm.is_some() { (0.55, 0.20, 0.15, 0.10) } else { (0.0, 0.45, 0.30, 0.25) };
    let llm = sig.llm.map(|l| at(l, i)).unwrap_or(0.0);
    let s = wl * llm + wa * at(sig.audio, i) + ws * at(sig.scene, i) + wp * at(sig.speech, i);
    // End credits make poor recap material.
    if duration > 1200.0 && t > duration * 0.96 { s * 0.3 } else { s }
}

/// Number of clips needed for the requested recap length.
pub fn clip_count(target_secs: f32, clip_secs: f32, windows: usize) -> usize {
    ((target_secs / clip_secs.max(1.0)).round() as usize).clamp(1, windows.max(1))
}

pub fn plan(sig: &Signals, windows: usize, interval: f32, duration: f32, target_secs: f32, clip_secs: f32) -> Vec<Clip> {
    if windows == 0 {
        return Vec::new();
    }
    let n = clip_count(target_secs, clip_secs, windows);
    let clip_len = clip_secs.min(duration.max(1.0));
    let mut picks: Vec<usize> = Vec::new();
    for b in 0..n {
        let (lo, hi) = (b * windows / n, ((b + 1) * windows / n).max(b * windows / n + 1));
        let best = (lo..hi.min(windows))
            .max_by(|&x, &y| {
                let sx = score(sig, x, x as f32 * interval, duration);
                let sy = score(sig, y, y as f32 * interval, duration);
                sx.partial_cmp(&sy).unwrap_or(std::cmp::Ordering::Equal)
            })
            .unwrap_or(lo);
        picks.push(best);
    }

    // Centre each clip on the sampled frame the model described.
    let mut clips: Vec<Clip> = picks
        .iter()
        .map(|&i| {
            let centre = i as f32 * interval;
            let start = (centre - clip_len / 2.0).clamp(0.0, (duration - clip_len).max(0.0));
            Clip { start, end: (start + clip_len).min(duration) }
        })
        .collect();
    clips.sort_by(|a, b| a.start.partial_cmp(&b.start).unwrap_or(std::cmp::Ordering::Equal));

    let mut merged: Vec<Clip> = Vec::new();
    for c in clips {
        match merged.last_mut() {
            Some(last) if c.start <= last.end => last.end = last.end.max(c.end),
            _ => merged.push(c),
        }
    }
    merged
}

/// Read the model's digits ("7,3,9,...") into 0..1 scores. None if the count does not match.
pub fn parse_scores(output: &str, n: usize) -> Option<Vec<f32>> {
    let digits: Vec<f32> = output.chars().filter_map(|c| c.to_digit(10)).map(|d| d as f32 / 9.0).collect();
    (digits.len() == n && n > 0).then_some(digits)
}

/// Words of narration that fit the recap length at a calm speaking pace.
pub fn narration_words(total_secs: f32) -> usize {
    ((total_secs * 2.2) as usize).clamp(30, 900)
}

/// Remove <reasoning>/<answer> wrappers the fine-tuned model may emit.
pub fn strip_tags(text: &str) -> String {
    let mut out = text.to_string();
    while let (Some(a), Some(b)) = (out.find("<reasoning>"), out.find("</reasoning>")) {
        if b < a {
            break;
        }
        out.replace_range(a..b + 12, "");
    }
    out.replace("<answer>", "").replace("</answer>", "").trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sig<'a>(llm: Option<&'a [f32]>, audio: &'a [f32], zeros: &'a [f32]) -> Signals<'a> {
        Signals { llm, audio, scene: zeros, speech: zeros }
    }

    #[test]
    fn picks_best_moment_per_bucket_in_story_order() {
        let mut llm = vec![0.1f32; 100];
        llm[10] = 1.0;
        llm[55] = 1.0;
        llm[90] = 1.0;
        let zeros = vec![0.0f32; 100];
        let clips = plan(&sig(Some(&llm), &zeros, &zeros), 100, 10.0, 1000.0, 30.0, 10.0);
        assert_eq!(clips.len(), 3);
        assert!((clips[0].start - 95.0).abs() < 0.01); // centred on t=100
        assert!(clips[0].start < clips[1].start && clips[1].start < clips[2].start);
    }

    #[test]
    fn merges_overlapping_clips_and_stays_inside_the_film() {
        let zeros = vec![0.0f32; 4];
        let audio = vec![1.0f32, 1.0, 1.0, 1.0];
        let clips = plan(&sig(None, &audio, &zeros), 4, 5.0, 20.0, 40.0, 10.0);
        assert!(clips.iter().all(|c| c.start >= 0.0 && c.end <= 20.0));
        assert!(clips.windows(2).all(|w| w[0].end < w[1].start));
    }

    #[test]
    fn falls_back_to_heuristics_without_llm() {
        let mut audio = vec![0.0f32; 10];
        audio[7] = 1.0;
        let zeros = vec![0.0f32; 10];
        let clips = plan(&sig(None, &audio, &zeros), 10, 10.0, 100.0, 10.0, 10.0);
        assert_eq!(clips.len(), 1);
        assert!((clips[0].start - 65.0).abs() < 0.01);
    }

    #[test]
    fn credits_are_penalised() {
        let zeros = vec![0.0f32; 100];
        let mut audio = vec![0.5f32; 100];
        audio[99] = 0.6; // slightly louder, but in the credits
        let s = sig(None, &audio, &zeros);
        assert!(score(&s, 99, 9900.0, 10000.0) < score(&s, 50, 5000.0, 10000.0));
    }

    #[test]
    fn parses_scores() {
        assert_eq!(parse_scores("9, 0, 4", 3).unwrap().len(), 3);
        assert!(parse_scores("9, 0", 3).is_none());
    }

    #[test]
    fn strips_reasoning_tags() {
        assert_eq!(strip_tags("<reasoning>think</reasoning><answer>Hello</answer>"), "Hello");
    }
}
