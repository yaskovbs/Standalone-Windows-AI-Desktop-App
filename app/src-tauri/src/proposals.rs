//! Change proposals: prompt construction, parsing the model's output, and diff views.
//! A proposal is only data until the user approves it in the UI.

use crate::github::{check_path, MAX_FILES, MAX_FILE_BYTES};
use serde::Serialize;

const SYSTEM: &str = "You are a careful senior software engineer improving this codebase. \
Make the smallest change that satisfies the request. Do not add dependencies, secrets, \
network calls to new hosts, or code that modifies git history. \
Reply ONLY with the complete new content of each changed file, using exactly this format:\n\
<<<FILE path/relative/to/repo>>>\n<full file content>\n<<<END>>>";

#[derive(Debug, Clone, Serialize)]
pub struct DiffView {
    pub before: Vec<String>,
    pub removed: Vec<String>,
    pub added: Vec<String>,
    pub after: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct FileChange {
    pub path: String,
    pub is_new: bool,
    pub sha: Option<String>,
    pub new_content: String,
    pub diff: DiffView,
}

#[derive(Debug, Clone, Serialize)]
pub struct Proposal {
    pub id: String,
    pub title: String,
    pub description: String,
    pub changes: Vec<FileChange>,
}

pub fn system_prompt() -> &'static str {
    SYSTEM
}

pub fn user_prompt(instruction: &str, files: &[(String, String)]) -> String {
    let mut out = format!("Request: {instruction}\n\nCurrent files:\n");
    for (path, content) in files {
        out.push_str(&format!("<<<FILE {path}>>>\n{content}\n<<<END>>>\n"));
    }
    out
}

/// Extract (path, content) pairs from the model reply.
pub fn parse_changes(reply: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut rest = reply;
    while let Some(start) = rest.find("<<<FILE ") {
        let after = &rest[start + 8..];
        let Some(header_end) = after.find(">>>") else { break };
        let path = after[..header_end].trim().to_string();
        let body = &after[header_end + 3..];
        let Some(end) = body.find("<<<END>>>") else { break };
        let content = body[..end].trim_matches('\n').trim_end().to_string() + "\n";
        out.push((path, content));
        rest = &body[end + 9..];
    }
    out
}

/// Keep only changes that are valid, were requested, are small enough and actually differ.
pub fn validate(parsed: Vec<(String, String)>, requested: &[String], current: &dyn Fn(&str) -> Option<String>) -> Vec<(String, String)> {
    parsed
        .into_iter()
        .filter(|(p, c)| {
            requested.contains(p)
                && check_path(p).is_ok()
                && c.len() <= MAX_FILE_BYTES
                && current(p).map(|old| old.trim_end() != c.trim_end()).unwrap_or(true)
        })
        .take(MAX_FILES)
        .collect()
}

/// Cheap line diff: trim the common prefix and suffix, show what is left.
pub fn diff_view(old: &str, new: &str) -> DiffView {
    let o: Vec<&str> = old.lines().collect();
    let n: Vec<&str> = new.lines().collect();
    let mut pre = 0;
    while pre < o.len() && pre < n.len() && o[pre] == n[pre] {
        pre += 1;
    }
    let mut suf = 0;
    while suf < o.len() - pre && suf < n.len() - pre && o[o.len() - 1 - suf] == n[n.len() - 1 - suf] {
        suf += 1;
    }
    let to_vec = |s: &[&str]| s.iter().map(|l| l.to_string()).collect::<Vec<_>>();
    DiffView {
        before: to_vec(&o[pre.saturating_sub(3)..pre]),
        removed: to_vec(&o[pre..o.len() - suf]),
        added: to_vec(&n[pre..n.len() - suf]),
        after: to_vec(&o[o.len() - suf..(o.len() - suf + 3.min(suf))]),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_file_blocks() {
        let reply = "Sure!\n<<<FILE app/src/a.ts>>>\nlet a = 1;\n<<<END>>>\n<<<FILE README.md>>>\nhi\n<<<END>>>";
        let p = parse_changes(reply);
        assert_eq!(p.len(), 2);
        assert_eq!(p[0], ("app/src/a.ts".into(), "let a = 1;\n".into()));
    }

    #[test]
    fn rejects_unrequested_and_protected_files() {
        let parsed = vec![
            ("app/src/a.ts".to_string(), "x\n".to_string()),
            ("app/src/other.ts".to_string(), "x\n".to_string()),
            ("app/src-tauri/src/github.rs".to_string(), "x\n".to_string()),
        ];
        let requested = vec!["app/src/a.ts".to_string(), "app/src-tauri/src/github.rs".to_string()];
        let kept = validate(parsed, &requested, &|_| None);
        assert_eq!(kept.len(), 1);
        assert_eq!(kept[0].0, "app/src/a.ts");
    }

    #[test]
    fn diff_shows_only_the_changed_lines() {
        let d = diff_view("a\nb\nc\nd\n", "a\nB\nc\nd\n");
        assert_eq!(d.removed, vec!["b"]);
        assert_eq!(d.added, vec!["B"]);
        assert_eq!(d.before, vec!["a"]);
        assert_eq!(d.after, vec!["c", "d"]);
    }
}
