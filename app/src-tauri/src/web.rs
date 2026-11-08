//! Internet features: live web search, update check.

use anyhow::{anyhow, Result};
use scraper::{Html, Selector};
use serde::{Deserialize, Serialize};
use std::time::Duration;

const USER_AGENT: &str = "LocalAutonomousAI/0.1 (desktop)";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub title: String,
    pub url: String,
    pub snippet: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct UpdateInfo {
    pub current: String,
    pub latest: String,
    pub update_available: bool,
    pub url: String,
}

pub fn client() -> Result<reqwest::Client> {
    Ok(reqwest::Client::builder()
        .user_agent(USER_AGENT)
        .timeout(Duration::from_secs(15))
        .build()?)
}

/// DuckDuckGo wraps result links as //duckduckgo.com/l/?uddg=<encoded-url>.
fn unwrap_redirect(href: &str) -> String {
    let full = if href.starts_with("//") { format!("https:{href}") } else { href.to_string() };
    if let Some(pos) = full.find("uddg=") {
        let rest = &full[pos + 5..];
        let end = rest.find('&').unwrap_or(rest.len());
        if let Ok(decoded) = urlencoding::decode(&rest[..end]) {
            return decoded.into_owned();
        }
    }
    full
}

/// Parse the HTML result page. Separated from the network call so it is unit-testable.
pub fn parse_results(html: &str, limit: usize) -> Vec<SearchResult> {
    let doc = Html::parse_document(html);
    let result_sel = Selector::parse(".result").unwrap();
    let title_sel = Selector::parse(".result__a").unwrap();
    let snippet_sel = Selector::parse(".result__snippet").unwrap();

    let mut out = Vec::new();
    for node in doc.select(&result_sel) {
        let Some(a) = node.select(&title_sel).next() else { continue };
        let title = a.text().collect::<String>().trim().to_string();
        let url = unwrap_redirect(a.value().attr("href").unwrap_or(""));
        let snippet = node
            .select(&snippet_sel)
            .next()
            .map(|s| s.text().collect::<String>().trim().to_string())
            .unwrap_or_default();
        if title.is_empty() || !url.starts_with("http") {
            continue;
        }
        out.push(SearchResult { title, url, snippet });
        if out.len() >= limit {
            break;
        }
    }
    out
}

pub async fn search(query: &str, limit: usize) -> Result<Vec<SearchResult>> {
    let url = format!("https://html.duckduckgo.com/html/?q={}", urlencoding::encode(query));
    let resp = client()?.get(url).send().await?.error_for_status()?;
    let body = resp.text().await?;
    Ok(parse_results(&body, limit))
}

#[derive(Deserialize)]
struct Release {
    tag_name: String,
    html_url: String,
}

fn parse_version(v: &str) -> Vec<u64> {
    v.trim_start_matches('v')
        .split('.')
        .map(|p| p.chars().take_while(|c| c.is_ascii_digit()).collect::<String>().parse().unwrap_or(0))
        .collect()
}

pub async fn check_update(repo: &str, current: &str) -> Result<UpdateInfo> {
    if !repo.contains('/') {
        return Err(anyhow!("update repo must look like owner/repo"));
    }
    let url = format!("https://api.github.com/repos/{repo}/releases/latest");
    let rel: Release = client()?.get(url).send().await?.error_for_status()?.json().await?;
    Ok(UpdateInfo {
        current: current.into(),
        update_available: parse_version(&rel.tag_name) > parse_version(current),
        latest: rel.tag_name,
        url: rel.html_url,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_duckduckgo_markup() {
        let html = r#"<div class="result"><a class="result__a"
            href="//duckduckgo.com/l/?uddg=https%3A%2F%2Fexample.com%2Fa&rut=x">Example</a>
            <a class="result__snippet">Hello world</a></div>"#;
        let r = parse_results(html, 5);
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].url, "https://example.com/a");
        assert_eq!(r[0].snippet, "Hello world");
    }

    #[test]
    fn compares_versions() {
        assert!(parse_version("v0.2.0") > parse_version("0.1.9"));
        assert!(parse_version("1.0") == parse_version("v1.0"));
    }
}
