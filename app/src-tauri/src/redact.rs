//! Best-effort removal of personal data and secrets before anything leaves the device.
//! This reduces risk but cannot guarantee anonymity (e.g. names in prose), which is why
//! sharing is opt-in and limited to answers the user explicitly rates as helpful.

use regex::{Captures, Regex};
use std::sync::OnceLock;

struct Rules {
    secrets: Regex,
    bearer: Regex,
    key_value: Regex,
    email: Regex,
    long_token: Regex,
    win_user: Regex,
    unix_home: Regex,
    ipv4: Regex,
    digits: Regex,
}

fn rules() -> &'static Rules {
    static RULES: OnceLock<Rules> = OnceLock::new();
    RULES.get_or_init(|| Rules {
        secrets: Regex::new(
            r"\b(?:sk-[A-Za-z0-9_\-]{16,}|gh[pousr]_[A-Za-z0-9]{20,}|github_pat_[A-Za-z0-9_]{20,}|AKIA[0-9A-Z]{16}|xox[baprs]-[A-Za-z0-9\-]{10,}|AIza[0-9A-Za-z_\-]{30,})",
        )
        .unwrap(),
        bearer: Regex::new(r"(?i)bearer\s+[A-Za-z0-9._\-]{20,}").unwrap(),
        key_value: Regex::new(r#"(?i)\b(password|passwd|pwd|secret|token|api[_-]?key)(\s*[:=]\s*)[^\s"',;]+"#).unwrap(),
        email: Regex::new(r"[A-Za-z0-9._%+\-]+@[A-Za-z0-9.\-]+\.[A-Za-z]{2,}").unwrap(),
        long_token: Regex::new(r"\b[A-Za-z0-9+/=_\-]{40,}\b").unwrap(),
        win_user: Regex::new(r#"(?i)([A-Z]:\\Users\\)[^\\\s"'<>|]+"#).unwrap(),
        unix_home: Regex::new(r#"(/home/|/Users/)[^/\s"']+"#).unwrap(),
        ipv4: Regex::new(r"\b(?:\d{1,3}\.){3}\d{1,3}\b").unwrap(),
        digits: Regex::new(r"\+?\d[\d\-\s().]{7,}\d").unwrap(),
    })
}

fn digit_count(s: &str) -> usize {
    s.chars().filter(|c| c.is_ascii_digit()).count()
}

pub fn redact(text: &str) -> String {
    let r = rules();
    let t = r.secrets.replace_all(text, "[SECRET]");
    let t = r.bearer.replace_all(&t, "Bearer [SECRET]");
    let t = r.key_value.replace_all(&t, "$1$2[SECRET]");
    let t = r.email.replace_all(&t, "[EMAIL]");
    let t = r.long_token.replace_all(&t, "[SECRET]");
    let t = r.win_user.replace_all(&t, "${1}<user>");
    let t = r.unix_home.replace_all(&t, "${1}<user>");
    let t = r.ipv4.replace_all(&t, "[IP]");
    // Phone / card numbers: only when there are enough digits, so dates and versions survive.
    let t = r.digits.replace_all(&t, |c: &Captures| {
        let m = &c[0];
        if digit_count(m) >= 9 { "[NUMBER]".to_string() } else { m.to_string() }
    });
    t.into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn removes_emails_keys_and_paths() {
        let out = redact("mail me at jane.doe@example.com, key sk-abcdefghijklmnopqrstuv, file C:\\Users\\Jane\\Desktop\\a.txt");
        assert!(!out.contains("jane.doe"));
        assert!(!out.contains("sk-abcdef"));
        assert!(!out.contains("Jane"));
        assert!(out.contains("[EMAIL]") && out.contains("[SECRET]") && out.contains("<user>"));
    }

    #[test]
    fn removes_assignments_and_phones_but_keeps_dates() {
        let out = redact("password = hunter2 call +972 52-123-4567 on 2024-01-15 using v1.2.3");
        assert!(!out.contains("hunter2"));
        assert!(!out.contains("123-4567"));
        assert!(out.contains("2024-01-15") && out.contains("v1.2.3"));
    }

    #[test]
    fn keeps_normal_code_intact() {
        let code = "fn add(a: i32, b: i32) -> i32 { a + b } // 42";
        assert_eq!(redact(code), code);
    }
}
