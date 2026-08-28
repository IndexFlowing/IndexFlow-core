use crate::canonical::canonical_matches_page;
use crate::extractor::{inspect_html, RobotsTokens};
use crate::models::{AiBotDirectives, SeoAuditResult};
use tracing::debug;

/// Pure HTML / header evaluation (no I/O). Safe to call from unit tests.
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

    let mut ai_directives = inspected.ai_directives;
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
        page_title: inspected.page_title,
        meta_description: inspected.meta_description,
        h1_content: inspected.h1_content,
        h1_count: inspected.h1_count,
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
        passed,
        block_reason,
    }
}

/// Parse `X-Robots-Tag` (possibly comma-joined multi-header) into a global
/// robots token set plus per-AI-bot flags.
///
/// Accepts both `noindex, nofollow` and `gptbot: noindex` forms.
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
            // A UA prefix must look like a token, not a URL (`https://...` has `:`).
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn x_robots_per_bot_and_global() {
        let (g, ai) = parse_x_robots_header("noindex, gptbot: nofollow, google-extended: noai");
        assert!(g.noindex);
        assert!(!g.nofollow);
        assert!(!ai.gptbot_blocked); // nofollow ≠ indexing block
        assert!(ai.google_extended_blocked);
    }

    #[test]
    fn x_robots_gptbot_noindex() {
        let (g, ai) = parse_x_robots_header("GPTBot: noindex");
        assert!(!g.noindex);
        assert!(ai.gptbot_blocked);
    }

    #[test]
    fn none_token_blocks_both() {
        let html = r#"<html><head><title>T</title><meta name="robots" content="none"></head></html>"#;
        let res = evaluate_html("https://example.com/", 200, 1, None, html);
        assert!(!res.passed);
        assert!(res.has_noindex);
        assert!(res.has_nofollow);
    }
}
