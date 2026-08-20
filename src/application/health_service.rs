use crate::domain::{canonical_matches_page, HreflangAlt, QualityGateResult};
use crate::infrastructure::INTERNAL_CRAWLER_UA;
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
            .user_agent(INTERNAL_CRAWLER_UA)
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
                    passed: false,
                    block_reason: Some(format!("request failed: {e}")),
                    ..QualityGateResult::default()
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

    let robots_directive = {
        let mut parts = Vec::new();
        if !x_robots.trim().is_empty() {
            parts.push(format!("x-robots-tag: {}", x_robots.trim()));
        }
        if let Some(m) = inspected.robots_meta.as_deref() {
            parts.push(format!("meta robots: {m}"));
        }
        if parts.is_empty() {
            None
        } else {
            Some(parts.join("; "))
        }
    };

    let payload_bytes = i32::try_from(body.len()).ok().or(Some(i32::MAX));

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
    } else if inspected
        .page_title
        .as_ref()
        .map(|t| t.is_empty())
        .unwrap_or(true)
    {
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
        meta_description: inspected.meta_description,
        h1_content: inspected.h1_content,
        robots_directive,
        hreflang: inspected.hreflang,
        payload_bytes,
        passed,
        block_reason,
    }
}

#[derive(Debug, Default)]
struct HtmlInspect {
    has_noindex: bool,
    page_title: Option<String>,
    canonical_url: Option<String>,
    meta_description: Option<String>,
    h1_content: Option<String>,
    robots_meta: Option<String>,
    hreflang: Vec<HreflangAlt>,
}

fn inspect_html(html: &str) -> HtmlInspect {
    let lower = html.to_ascii_lowercase();
    let robots_meta = extract_meta_content(html, &lower, "robots")
        .or_else(|| extract_meta_content(html, &lower, "googlebot"));
    let robots_l = robots_meta
        .as_deref()
        .unwrap_or("")
        .to_ascii_lowercase();
    HtmlInspect {
        has_noindex: robots_l.contains("noindex") || detect_noindex(&lower),
        page_title: extract_title(html, &lower),
        canonical_url: extract_canonical(html, &lower),
        meta_description: extract_meta_content(html, &lower, "description"),
        h1_content: extract_h1(html, &lower),
        robots_meta,
        hreflang: extract_hreflang(html, &lower),
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

/// Slice `html[start..end]` only when both offsets are UTF-8 char boundaries.
fn safe_slice(html: &str, start: usize, end: usize) -> Option<&str> {
    if start <= end
        && end <= html.len()
        && html.is_char_boundary(start)
        && html.is_char_boundary(end)
    {
        Some(&html[start..end])
    } else {
        None
    }
}

fn extract_title(html: &str, lower: &str) -> Option<String> {
    inner_text(html, lower, "<title", "</title>")
}

fn extract_h1(html: &str, lower: &str) -> Option<String> {
    inner_text(html, lower, "<h1", "</h1>").filter(|s| !s.is_empty())
}

fn inner_text(html: &str, lower: &str, open: &str, close: &str) -> Option<String> {
    let start = lower.find(open)?;
    let gt_rel = html.get(start..)?.find('>')?;
    let after_gt = start + gt_rel + 1;
    let close_rel = lower.get(after_gt..)?.find(close)?;
    let end = after_gt + close_rel;
    let raw = safe_slice(html, after_gt, end)?.trim();
    if raw.is_empty() {
        return None;
    }
    let stripped = strip_tags(raw);
    let decoded = decode_basic_entities(&stripped);
    let decoded = decoded.trim();
    if decoded.is_empty() {
        None
    } else {
        Some(decoded.to_string())
    }
}

fn strip_tags(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut in_tag = false;
    for c in s.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(c),
            _ => {}
        }
    }
    out
}

fn extract_canonical(html: &str, lower: &str) -> Option<String> {
    let mut search_from = 0usize;
    while let Some(rel_idx) = lower.get(search_from..)?.find("canonical") {
        let abs = search_from + rel_idx;
        let tag_start = lower.get(..abs)?.rfind("<link")?;
        let tag_end_rel = lower.get(tag_start..)?.find('>')?;
        let tag_end = tag_start + tag_end_rel;
        let tag = safe_slice(html, tag_start, tag_end)?;
        let tag_l = safe_slice(lower, tag_start, tag_end)?;

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

/// UTF-8-safe extraction of `<meta name="{name}" content="...">` (attribute order independent).
fn extract_meta_content(html: &str, lower: &str, name: &str) -> Option<String> {
    let mut search_from = 0usize;
    let needle = format!("name=\"{name}\"");
    let needle_sq = format!("name='{name}'");
    while let Some(rel) = lower.get(search_from..)?.find("<meta") {
        let abs = search_from + rel;
        let end_rel = lower.get(abs..)?.find('>')?;
        let tag_end = abs + end_rel;
        let tag = safe_slice(html, abs, tag_end)?;
        let tag_l = safe_slice(lower, abs, tag_end)?;
        if tag_l.contains(&needle) || tag_l.contains(&needle_sq) {
            if let Some(content) = attr_value(tag, "content") {
                let content = decode_basic_entities(content.trim());
                if !content.is_empty() {
                    return Some(content);
                }
            }
        }
        search_from = abs + 5;
    }
    None
}

fn extract_hreflang(html: &str, lower: &str) -> Vec<HreflangAlt> {
    let mut out = Vec::new();
    let mut search_from = 0usize;
    while let Some(rel) = lower.get(search_from..).and_then(|s| s.find("<link")) {
        let abs = search_from + rel;
        let Some(end_rel) = lower.get(abs..).and_then(|s| s.find('>')) else {
            break;
        };
        let tag_end = abs + end_rel;
        if let (Some(tag), Some(tag_l)) = (
            safe_slice(html, abs, tag_end),
            safe_slice(lower, abs, tag_end),
        ) {
            if tag_l.contains("hreflang") {
                if let (Some(lang), Some(href)) =
                    (attr_value(tag, "hreflang"), attr_value(tag, "href"))
                {
                    let lang = lang.trim();
                    let href = href.trim();
                    if !lang.is_empty() && !href.is_empty() {
                        out.push(HreflangAlt {
                            lang: lang.to_string(),
                            href: href.to_string(),
                        });
                    }
                }
            }
        }
        search_from = abs + 5;
    }
    out
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
    let end = rest
        .find(|c: char| c.is_whitespace() || c == '>' || c == '/')
        .unwrap_or(rest.len());
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

    #[test]
    fn extract_utf8_meta_and_h1() {
        let html = r#"<html><head>
            <title>中文标题 · IndexFlow</title>
            <meta name="description" content="这是一段 UTF-8 描述 &amp; more">
            <meta content="index,follow" name="robots">
            <link rel="alternate" hreflang="zh-CN" href="https://ex.com/zh">
            <link rel="alternate" href="https://ex.com/en" hreflang="en">
        </head><body><h1>主标题 <span>副文</span></h1></body></html>"#;
        let r = evaluate_page("https://ex.com/zh", 200, 12, &headers(), html);
        assert!(r.passed, "{:?}", r.block_reason);
        assert_eq!(r.page_title.as_deref(), Some("中文标题 · IndexFlow"));
        assert_eq!(
            r.meta_description.as_deref(),
            Some("这是一段 UTF-8 描述 & more")
        );
        assert_eq!(r.h1_content.as_deref(), Some("主标题 副文"));
        assert_eq!(r.hreflang.len(), 2);
        assert_eq!(r.hreflang[0].lang, "zh-CN");
        assert_eq!(r.hreflang[1].lang, "en");
        assert!(r.robots_directive.as_deref().unwrap().contains("index,follow"));
        assert!(r.payload_bytes.unwrap() > 0);
    }
}
