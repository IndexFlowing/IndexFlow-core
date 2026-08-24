use crate::canonical::canonical_matches_page;
use crate::extractor::inspect_html;
use crate::models::SeoAuditResult;
use tracing::debug;

/// 纯文本/HTML 评估函数（脱离 HTTP，可 100% 独立单测）
pub fn evaluate_html(
    page_url: &str,
    status_code: i32,
    elapsed_ms: i32,
    x_robots_header: Option<&str>,
    html_body: &str,
) -> SeoAuditResult {
    let x_robots = x_robots_header.unwrap_or("").to_ascii_lowercase();
    let has_noindex_header = x_robots.contains("noindex");
    let has_nofollow_header = x_robots.contains("nofollow");

    let inspected = inspect_html(html_body);
    let has_noindex = has_noindex_header || inspected.has_noindex;
    let has_nofollow = has_nofollow_header || inspected.has_nofollow;

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

    let payload_bytes = i32::try_from(html_body.len()).ok().or(Some(i32::MAX));

    // 核心硬性拦截逻辑
    let block_reason = if status_code != 200 {
        Some(format!("HTTP {status_code}"))
    } else if has_noindex {
        Some("noindex directive present".to_string())
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
        "SEO audit evaluated"
    );

    SeoAuditResult {
        http_status: Some(status_code),
        response_time_ms: Some(elapsed_ms),
        payload_bytes,
        page_title: inspected.page_title,
        meta_description: inspected.meta_description,
        h1_content: inspected.h1_content,
        h1_count: inspected.h1_count,
        canonical_url: inspected.canonical_url.clone(),
        has_canonical: inspected.canonical_url.is_some(),
        has_noindex,
        has_nofollow,
        robots_directive,
        hreflang: inspected.hreflangs,
        opengraph: inspected.opengraph,
        twitter_card: inspected.twitter_card,
        json_ld: inspected.json_ld,
        ai_directives: inspected.ai_directives,
        passed,
        block_reason,
    }
}