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

#[cfg(test)]
mod tests {
    use super::*;

    fn turn(role: &str, content: &str) -> Turn {
        Turn { role: role.into(), content: content.into() }
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
