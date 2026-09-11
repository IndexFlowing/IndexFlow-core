// crates/indexflow-seo/src/evaluator.rs
use crate::extractor::inspect_html;
use crate::gate::evaluate_gate_block;
use crate::headers::{parse_http_link_header, parse_x_robots_header};
use crate::models::{HttpLinkHeader, SeoAuditResult};
use crate::warnings::compute_warnings;
use tracing::debug;

/// 综合评估 HTML 内容、HTTP 状态码及网络层 Header
pub fn evaluate_html(
    page_url: &str,
    status_code: i32,
    elapsed_ms: i32,
    x_robots_header: Option<&str>,
    location_header: Option<&str>,
    link_header: Option<&str>,
    html_body: &str,
) -> SeoAuditResult {
    // 1. 网络层协议头解析 (显式声明 Vec 所有权)
    let (header_robots, header_ai) = parse_x_robots_header(x_robots_header.unwrap_or(""));
    let http_links_vec: Vec<HttpLinkHeader> = link_header
        .map(parse_http_link_header)
        .unwrap_or_default();
    let redirect_location: Option<String> = location_header.map(|s| s.trim().to_string());

    // 2. DOM 与内容实体提取
    let inspected = inspect_html(html_body);
    let has_noindex = header_robots.noindex || inspected.has_noindex;
    let has_nofollow = header_robots.nofollow || inspected.has_nofollow;

    let mut ai_directives = inspected.ai_directives.clone();
    ai_directives.merge(&header_ai);

    let robots_directive = {
        let mut parts = Vec::new();
        if let Some(h) = x_robots_header.map(str::trim).filter(|s| !s.is_empty()) {
            parts.push(format!("x-robots-tag: {h}"));
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

    let payload_bytes = i32::try_from(html_body.len()).unwrap_or(i32::MAX);

    // 3. 核心技术 SEO 门禁裁决 (借用切片)
    let block_reason = evaluate_gate_block(
        page_url,
        status_code,
        has_noindex,
        inspected.canonical_url.as_deref(),
        inspected.page_title.as_deref(),
        redirect_location.as_deref(),
        &http_links_vec,
        &inspected.video_objects,
    );

    // 4. 优化建议与工程警示列表 (借用切片)
    let warnings = compute_warnings(
        status_code,
        &inspected,
        &ai_directives,
        redirect_location.as_deref(),
        &http_links_vec,
    );

    let canonical_url = inspected.canonical_url;
    let has_canonical = canonical_url.as_ref().map(|s| !s.is_empty()).unwrap_or(false);
    let passed = block_reason.is_none();

    debug!(
        url = %page_url,
        status = status_code,
        has_noindex,
        passed,
        reason = ?block_reason,
        "SEO audit evaluated"
    );

    // 5. 转移 http_links_vec 的所有权至实体，杜绝任何类型混淆
    SeoAuditResult {
        http_status: Some(status_code),
        response_time_ms: Some(elapsed_ms),
        payload_bytes: Some(payload_bytes),
        word_count: inspected.word_count,
        page_title: inspected.page_title,
        meta_description: inspected.meta_description,
        h1_content: inspected.h1_content,
        h1_count: inspected.h1_count,
        headings: inspected.headings,
        canonical_url,
        has_canonical,
        has_noindex,
        has_nofollow,
        robots_directive,
        hreflang: inspected.hreflangs,
        opengraph: inspected.opengraph,
        twitter_card: inspected.twitter_card,
        json_ld: inspected.json_ld,
        video_objects: inspected.video_objects,
        ai_directives,
        redirect_location,
        http_links: http_links_vec,
        has_viewport: inspected.has_viewport,
        html_lang: inspected.html_lang,
        images_missing_alt: inspected.images_missing_alt,
        passed,
        block_reason,
        warnings,
    }
}