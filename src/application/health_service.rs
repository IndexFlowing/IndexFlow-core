use crate::domain::{canonical_matches_page, QualityGateResult};
use reqwest::header::HeaderMap;
use reqwest::redirect::Policy;
use reqwest::Client;
use std::time::{Duration, Instant};
use tracing::debug;

/// Inline SEO quality gate: one lightweight GET immediately before submit.
/// Does **not** follow redirects — non-200 (including 3xx) is a hard block.
#[derive(Clone)]
pub struct HealthService {
    client: Client,
}

impl HealthService {
    pub fn new(_shared: Client) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(15))
            .user_agent("MandarinClips-Internal-SeoBot-Secret888")
            .redirect(Policy::none())
            .build()
            .expect("failed to build quality-gate HTTP client");
        Self { client }
    }

    pub async fn check_url(&self, url: &str) -> QualityGateResult {
        let start = Instant::now();

        let response = match self.client.get(url).send().await {
            Ok(r) => r,
            Err(e) => {
                return QualityGateResult {
                    http_status: None,
                    response_time_ms: Some(start.elapsed().as_millis() as i32),
                    has_noindex: false,
                    has_canonical: false,
                    page_title: None,
                    canonical_url: None,
                    passed: false,
                    block_reason: Some(format!("request failed: {e}")),
                };
            }
        };

        let status_code = response.status().as_u16() as i32;
        let headers = response.headers().clone();
        let body = response.text().await.unwrap_or_default();
        let elapsed = start.elapsed().as_millis() as i32;

        evaluate_page(url, status_code, elapsed, &headers, &body)
    }
}

/// Pure gate evaluation — unit-tested independently of HTTP.
pub fn evaluate_page(
    page_url: &str,
    status_code: i32,
    elapsed_ms: i32,
    headers: &HeaderMap,
    body: &str,
) -> QualityGateResult {
    let x_robots = headers
        .get("x-robots-tag")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_ascii_lowercase();
    let has_noindex_header = x_robots.contains("noindex");

    let inspected = inspect_html(body);
    let has_noindex = has_noindex_header || inspected.has_noindex;

    let block_reason = if status_code != 200 {
        Some(format!("HTTP {status_code}"))
    } else if has_noindex {
        Some("noindex".to_string())
    } else if let Some(ref canon) = inspected.canonical_url {
        if !canonical_matches_page(page_url, canon) {
            Some(format!("Canonical URL mismatch: {canon}"))
        } else {
            None
        }
    } else if inspected.page_title.as_ref().map(|t| t.is_empty()).unwrap_or(true) {
        Some("Missing <title> tag".to_string())
    } else {
        None
    };

    let passed = block_reason.is_none();

    debug!(
        url = %page_url,
        status = status_code,
        has_noindex,
        passed,
        reason = ?block_reason,
        "quality gate done"
    );

    QualityGateResult {
        http_status: Some(status_code),
        response_time_ms: Some(elapsed_ms),
        has_noindex,
        has_canonical: inspected.canonical_url.is_some(),
        page_title: inspected.page_title,
        canonical_url: inspected.canonical_url,
        passed,
        block_reason,
    }
}

#[derive(Debug, Default)]
struct HtmlInspect {
    has_noindex: bool,
    page_title: Option<String>,
    canonical_url: Option<String>,
}

fn inspect_html(html: &str) -> HtmlInspect {
    let lower = html.to_ascii_lowercase();
    HtmlInspect {
        has_noindex: detect_noindex(&lower),
        page_title: extract_title(html, &lower),
        canonical_url: extract_canonical(html, &lower),
    }
}

fn detect_noindex(lower: &str) -> bool {
    if let Some(idx) = find_meta_robots(lower) {
        // Safety: inspect only the 400 characters after idx (char-bounded, not byte-sliced).
        let window: String = lower[idx..].chars().take(400).collect();
        if window.contains("noindex") {
            return true;
        }
    }
    lower.contains("content=\"noindex") || lower.contains("content='noindex")
}

fn find_meta_robots(lower: &str) -> Option<usize> {
    const NEEDLES: &[&str] = &[
        r#"name="robots""#,
        r#"name='robots'"#,
        r#"name="googlebot""#,
        r#"name='googlebot'"#,
    ];
    NEEDLES.iter().filter_map(|n| lower.find(n)).min()
}

fn extract_title(html: &str, lower: &str) -> Option<String> {
    let start = lower.find("<title")?;
    let after_gt = html[start..].find('>')? + start + 1;
    let end_rel = lower.get(after_gt..)?.find("</title>")?;
    let raw = html[after_gt..after_gt + end_rel].trim();
    if raw.is_empty() {
        return None;
    }
    Some(decode_basic_entities(raw))
}

fn extract_canonical(html: &str, lower: &str) -> Option<String> {
    let mut search_from = 0usize;
    while let Some(rel_idx) = lower.get(search_from..)?.find("canonical") {
        let abs = search_from + rel_idx;
        let tag_start = lower.get(..abs)?.rfind("<link")?;
        let tag_end_rel = lower.get(tag_start..)?.find('>')?;
        
        let tag = html.get(tag_start..tag_start + tag_end_rel)?;
        let tag_l = lower.get(tag_start..tag_start + tag_end_rel)?;
        
        if tag_l.contains("rel=") && tag_l.contains("canonical") {
            if let Some(href) = attr_value(tag, "href") {
                let href = href.trim();
                if !href.is_empty() {
                    return Some(href.to_string());
                }
            }
        }
        search_from = abs + 9;
    }
    None
}

fn attr_value(tag: &str, name: &str) -> Option<String> {
    let lower = tag.to_ascii_lowercase();
    let key = format!("{name}=");
    let idx = lower.find(&key)?;
    let rest = &tag[idx + key.len()..];
    let rest = rest.trim_start();
    let quote = rest.chars().next()?;
    if quote == '"' || quote == '\'' {
        let end = rest[1..].find(quote)?;
        return Some(rest[1..1 + end].to_string());
    }
    // unquoted
    let end = rest.find(|c: char| c.is_whitespace() || c == '>' || c == '/').unwrap_or(rest.len());
    Some(rest[..end].to_string())
}

fn decode_basic_entities(s: &str) -> String {
    s.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&nbsp;", " ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use reqwest::header::{HeaderMap, HeaderValue};

    fn headers() -> HeaderMap {
        HeaderMap::new()
    }

    #[test]
    fn pass_clean_page() {
        let html = r#"<html><head><title>Hello</title></head><body>ok</body></html>"#;
        let r = evaluate_page("https://ex.com/a", 200, 10, &headers(), html);
        assert!(r.passed, "{:?}", r.block_reason);
        assert_eq!(r.page_title.as_deref(), Some("Hello"));
    }

    #[test]
    fn block_non_200() {
        let html = r#"<html><head><title>Nope</title></head></html>"#;
        let r = evaluate_page("https://ex.com/missing", 404, 8, &headers(), html);
        assert!(!r.passed);
        assert_eq!(r.block_reason.as_deref(), Some("HTTP 404"));
    }

    #[test]
    fn block_500() {
        let html = r#"<html><head><title>Err</title></head></html>"#;
        let r = evaluate_page("https://ex.com/e", 500, 8, &headers(), html);
        assert!(!r.passed);
        assert_eq!(r.block_reason.as_deref(), Some("HTTP 500"));
    }

    #[test]
    fn block_noindex_meta() {
        let html = r#"<html><head>
            <title>Hidden</title>
            <meta name="robots" content="noindex,follow">
        </head></html>"#;
        let r = evaluate_page("https://ex.com/hidden", 200, 8, &headers(), html);
        assert!(!r.passed);
        assert_eq!(r.block_reason.as_deref(), Some("noindex"));
        assert!(r.has_noindex);
    }

    #[test]
    fn block_noindex_header() {
        let html = r#"<html><head><title>Hidden</title></head></html>"#;
        let mut h = HeaderMap::new();
        h.insert("x-robots-tag", HeaderValue::from_static("noindex"));
        let r = evaluate_page("https://ex.com/hidden", 200, 8, &h, html);
        assert!(!r.passed);
        assert_eq!(r.block_reason.as_deref(), Some("noindex"));
    }

    #[test]
    fn block_missing_title() {
        let html = r#"<html><head></head><body>no title</body></html>"#;
        let r = evaluate_page("https://ex.com/a", 200, 8, &headers(), html);
        assert!(!r.passed);
        assert_eq!(r.block_reason.as_deref(), Some("Missing <title> tag"));
    }

    #[test]
    fn block_empty_title() {
        let html = r#"<html><head><title>   </title></head></html>"#;
        let r = evaluate_page("https://ex.com/a", 200, 8, &headers(), html);
        assert!(!r.passed);
        assert_eq!(r.block_reason.as_deref(), Some("Missing <title> tag"));
    }

    #[test]
    fn block_canonical_mismatch() {
        let html = r#"<html><head>
            <title>Page</title>
            <link rel="canonical" href="https://ex.com/other">
        </head></html>"#;
        let r = evaluate_page("https://ex.com/this", 200, 8, &headers(), html);
        assert!(!r.passed);
        assert!(r.block_reason.as_deref().unwrap().contains("Canonical"));
    }

    #[test]
    fn pass_matching_canonical() {
        let html = r#"<html><head>
            <title>Page</title>
            <link rel="canonical" href="https://ex.com/this">
        </head></html>"#;
        let r = evaluate_page("https://ex.com/this", 200, 8, &headers(), html);
        assert!(r.passed, "{:?}", r.block_reason);
    }

    #[test]
    fn pass_without_canonical() {
        let html = r#"<html><head><title>Page</title></head></html>"#;
        let r = evaluate_page("https://ex.com/this", 200, 8, &headers(), html);
        assert!(r.passed);
        assert!(!r.has_canonical);
    }
}
