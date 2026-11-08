//! ChatML prompt construction with context-window trimming.
//! Both Qwen 2.5 and the fine-tuned student use the ChatML template.

use crate::web::SearchResult;

#[derive(Debug, Clone)]
pub struct Turn {
    pub role: String,
    pub content: String,
}

/// Rough token estimate (~4 chars per token) used only for trimming.
fn estimate_tokens(s: &str) -> usize {
    s.chars().count() / 4 + 4
}

/// Format web results as a numbered context block for grounded answers.
pub fn web_context(results: &[SearchResult]) -> String {
    let mut out = String::from("Web search results (cite as [n]):\n");
    for (i, r) in results.iter().enumerate() {
        out.push_str(&format!("[{}] {} ({})\n{}\n\n", i + 1, r.title, r.url, r.snippet));
    }
    out
}

/// Build a ChatML prompt that fits inside `n_ctx - reserve_for_output` tokens.
/// The oldest turns are dropped first; the system prompt and last turn are kept.
pub fn build(system: &str, web: Option<&str>, history: &[Turn], n_ctx: u32, reserve: u32) -> String {
    let budget = n_ctx.saturating_sub(reserve) as usize;
    let mut system_text = system.to_string();
    if let Some(ctx) = web {
        system_text.push_str("\n\n");
        system_text.push_str(ctx);
    }

    let mut used = estimate_tokens(&system_text);
    let mut kept: Vec<&Turn> = Vec::new();
    for turn in history.iter().rev() {
        let cost = estimate_tokens(&turn.content);
        if used + cost > budget && !kept.is_empty() {
            break;
        }
        used += cost;
        kept.push(turn);
    }
    kept.reverse();

    let mut prompt = format!("<|im_start|>system\n{}<|im_end|>\n", system_text);
    for t in kept {
        prompt.push_str(&format!("<|im_start|>{}\n{}<|im_end|>\n", t.role, t.content));
    }
    prompt.push_str("<|im_start|>assistant\n");
    prompt
}

/// Fit a (possibly huge) video timeline into `budget` characters.
/// Lines that match the question (keywords or a typed time code like 12:30) are kept first,
/// then the rest of the budget is filled with evenly spaced moments so the whole story is covered.
pub fn select_timeline(timeline: &str, question: &str, budget: usize) -> String {
    if timeline.chars().count() <= budget {
        return timeline.to_string();
    }
    let lines: Vec<&str> = timeline.lines().collect();
    let q = question.to_lowercase();
    let words: Vec<&str> = q.split(|c: char| !c.is_alphanumeric()).filter(|w| w.chars().count() >= 4).collect();
    let times: Vec<String> = q
        .split_whitespace()
        .map(|t| t.trim_matches(|c: char| !c.is_ascii_digit() && c != ':').to_string())
        .filter(|t| t.contains(':') && t.chars().all(|c| c.is_ascii_digit() || c == ':'))
        .collect();

    let score = |line: &str| -> usize {
        let l = line.to_lowercase();
        words.iter().filter(|w| l.contains(**w)).count() + times.iter().filter(|t| l.contains(&format!("[{t}"))).count() * 5
    };
    let mut chosen = std::collections::BTreeSet::new();
    let mut used = 0usize;
    let try_add = |i: usize, used: &mut usize, chosen: &mut std::collections::BTreeSet<usize>, limit: usize| {
        let cost = lines[i].chars().count() + 1;
        if !chosen.contains(&i) && *used + cost <= limit {
            *used += cost;
            chosen.insert(i);
        }
    };

    if lines.first().map(|l| l.starts_with("Video:")).unwrap_or(false) {
        try_add(0, &mut used, &mut chosen, budget);
    }
    let mut ranked: Vec<(usize, usize)> = lines.iter().enumerate().map(|(i, l)| (score(l), i)).filter(|(s, _)| *s > 0).collect();
    ranked.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
    for (_, i) in ranked {
        try_add(i, &mut used, &mut chosen, budget * 6 / 10);
    }
    // Evenly spaced overview for the remaining budget.
    let avg = (timeline.chars().count() / lines.len().max(1)).max(1);
    let slots = (budget.saturating_sub(used) / avg).max(1).min(lines.len());
    for j in 0..slots {
        try_add(j * lines.len() / slots, &mut used, &mut chosen, budget);
    }

    let mut out: Vec<&str> = chosen.iter().map(|i| lines[*i]).collect();
    let note = format!(
        "[{} of {} moments shown - ask about a specific scene or time code for more detail]",
        chosen.len(),
        lines.len()
    );
    out.push(&note);
    out.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn turn(role: &str, content: &str) -> Turn {
        Turn { role: role.into(), content: content.into() }
    }

    #[test]
    fn long_timeline_keeps_relevant_and_spread_moments() {
        let tl: String = (0..400)
            .map(|i| format!("[{:02}:{:02}] SEEN: scene {i}{}", i / 60, i % 60, if i == 321 { " a red car explodes" } else { "" }))
            .collect::<Vec<_>>()
            .join("\n");
        let out = select_timeline(&tl, "when does the red car explode?", 2000);
        assert!(out.chars().count() < 2300);
        assert!(out.contains("red car explodes"));
        assert!(out.contains("scene 0") && out.contains("moments shown"));
    }

    #[test]
    fn short_timeline_is_untouched() {
        assert_eq!(select_timeline("[00:00] SEEN: a", "q", 1000), "[00:00] SEEN: a");
    }

    #[test]
    fn keeps_system_and_last_turn() {
        let history = vec![turn("user", &"a".repeat(8000)), turn("user", "latest")];
        let p = build("SYS", None, &history, 512, 128);
        assert!(p.contains("SYS"));
        assert!(p.contains("latest"));
        assert!(!p.contains(&"a".repeat(100)));
        assert!(p.ends_with("<|im_start|>assistant\n"));
    }
}
