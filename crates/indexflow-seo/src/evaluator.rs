use crate::canonical::canonical_matches_page;
use crate::extractor::{inspect_html, RawHtmlInspection, RobotsTokens};
use crate::models::{AiBotDirectives, SeoAuditResult};
use tracing::debug;

pub fn evaluate_html(
    page_url: &str,
    status_code: i32,
    elapsed_ms: i32,
    x_robots_header: Option<&str>,
    html_body: &str,
) -> SeoAuditResult {
    let (header_robots, header_ai) = parse_x_robots_header(x_robots_header.unwrap_or(""));

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
    let warnings = compute_warnings(&inspected, &ai_directives);

    let canonical_url = inspected.canonical_url;
    let has_canonical = canonical_url.as_ref().map(|s| !s.is_empty()).unwrap_or(false);

    let block_reason = if status_code != 200 {
        Some(format!("HTTP {status_code}"))
    } else if has_noindex {
        Some("noindex directive present".to_string())
    } else if let Some(ref canon) = canonical_url {
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
        ai_directives,
        has_viewport: inspected.has_viewport,
        html_lang: inspected.html_lang,
        images_missing_alt: inspected.images_missing_alt,
        passed,
        block_reason,
        warnings,
    }
}

pub fn compute_warnings(inspected: &RawHtmlInspection, ai: &AiBotDirectives) -> Vec<String> {
    let mut warnings = Vec::new();
    if inspected.h1_count == 0 { warnings.push("缺少 H1 标题标签".to_string()); }
    if inspected.h1_count > 1 { warnings.push(format!("存在 {} 个 H1 标签，建议全页仅保留一个核心 H1", inspected.h1_count)); }
    if inspected.has_nofollow { warnings.push("页面带有 nofollow 指令".to_string()); }
    let blocked = ai.blocked_names();
    if !blocked.is_empty() { warnings.push(format!("屏蔽了 AI 爬虫抓取: {}", blocked.join(", "))); }
    let og = &inspected.opengraph;
    if [og.title.as_ref(), og.description.as_ref(), og.image.as_ref(), og.og_type.as_ref(), og.url.as_ref(), og.site_name.as_ref()].iter().all(Option::is_none) {
        warnings.push("缺少 OpenGraph 社交卡片标签".to_string());
    }
    if inspected.twitter_card.card.is_none() { warnings.push("缺少 X (Twitter) Card 标记".to_string()); }
    if inspected.json_ld.is_empty() { warnings.push("未发现 JSON-LD 结构化数据".to_string()); }
    if !inspected.has_viewport { warnings.push("缺少 viewport 移动适配标签".to_string()); }
    if inspected.html_lang.is_none() { warnings.push("未声明 <html lang> 语言".to_string()); }
    if inspected.images_missing_alt > 0 { warnings.push(format!("{} 张图片缺失 alt 替代文本", inspected.images_missing_alt)); }
    match inspected.meta_description.as_deref() {
        None => warnings.push("缺少 meta description 描述".to_string()),
        Some(s) if !(50..=160).contains(&s.chars().count()) => warnings.push(format!("meta description 长度为 {} 字，建议在 50~160 字之间", s.chars().count())),
        _ => {}
    }
    if inspected.page_title.as_ref().is_some_and(|s| s.chars().count() > 60) { warnings.push("page title 超过 60 字符，在搜索结果中可能被截断".to_string()); }
    warnings
}

pub(crate) fn parse_x_robots_header(header: &str) -> (RobotsTokens, AiBotDirectives) {
    let mut global = RobotsTokens::parse("");
    let mut ai = AiBotDirectives::default();
    for part in header.split(',') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        if let Some((ua, rest)) = part.split_once(':') {
            let ua = ua.trim();
            let rest = rest.trim();
            if ua.chars().any(|c| c == '/' || c == ' ') || ua.len() > 64 {
                let tok = RobotsTokens::parse(part);
                merge_robots(&mut global, &tok);
                continue;
            }
            let tok = RobotsTokens::parse(rest);
            if ua.eq_ignore_ascii_case("robots")
                || ua.eq_ignore_ascii_case("googlebot")
                || ua.eq_ignore_ascii_case("googlebot-news")
            {
                merge_robots(&mut global, &tok);
            }
            apply_bot_ua(ua, &tok, &mut ai);
        } else {
            let tok = RobotsTokens::parse(part);
            merge_robots(&mut global, &tok);
        }
    }
    (global, ai)
}

fn merge_robots(dst: &mut RobotsTokens, src: &RobotsTokens) {
    dst.noindex |= src.noindex;
    dst.nofollow |= src.nofollow;
    dst.none |= src.none;
    dst.noai |= src.noai;
}

fn apply_bot_ua(ua: &str, tok: &RobotsTokens, ai: &mut AiBotDirectives) {
    if !tok.blocks_indexing() {
        return;
    }
    if eq_any(ua, &["gptbot", "chatgpt-user", "chatgptuser"]) {
        ai.gptbot_blocked = true;
    } else if eq_any(ua, &["perplexitybot", "perplexity"]) {
        ai.perplexity_blocked = true;
    } else if eq_any(ua, &["claudebot", "anthropic-ai", "claude-web"]) {
        ai.claudebot_blocked = true;
    } else if eq_any(ua, &["google-extended"]) {
        ai.google_extended_blocked = true;
    }
}

fn eq_any(ua: &str, names: &[&str]) -> bool {
    names.iter().any(|n| ua.eq_ignore_ascii_case(n))
}