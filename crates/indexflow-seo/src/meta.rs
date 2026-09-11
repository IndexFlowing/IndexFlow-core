// crates/indexflow-seo/src/meta.rs
use crate::headers::RobotsTokens;
use crate::html_utils::{
    attr_value, clamp_boundary, clip_html, find_open_tag, find_tag_end, inner_text,
    normalize_visible_text, safe_slice,
};
use crate::models::{AiBotDirectives, HreflangItem, OpenGraphMeta, TwitterCardMeta};

pub fn extract_title(html: &str) -> Option<String> {
    inner_text(html, "title", true)
}

pub fn extract_canonical(html: &str) -> Option<String> {
    let mut search_from = 0;
    while let Some(abs) = find_open_tag(html, search_from, "link", true) {
        let Some(tag_end) = find_tag_end(html, abs) else {
            break;
        };
        if let Some(tag) = safe_slice(html, abs, tag_end.saturating_add(1)) {
            let rel_val = attr_value(tag, "rel").unwrap_or_default();
            if rel_val
                .split_whitespace()
                .any(|r| r.eq_ignore_ascii_case("canonical"))
            {
                if let Some(href) = attr_value(tag, "href") {
                    let href = href.trim();
                    if !href.is_empty() {
                        return Some(href.to_string());
                    }
                }
            }
        }
        search_from = clamp_boundary(html, tag_end.saturating_add(1));
    }
    None
}

pub fn extract_meta_content(html: &str, attr_key: &str, attr_val: &str) -> Option<String> {
    let html = clip_html(html);
    let mut search_from = 0;
    while let Some(abs) = find_open_tag(html, search_from, "meta", true) {
        let Some(tag_end) = find_tag_end(html, abs) else {
            break;
        };
        if let Some(tag) = safe_slice(html, abs, tag_end.saturating_add(1)) {
            let key = attr_value(tag, attr_key).unwrap_or_default();
            if key.eq_ignore_ascii_case(attr_val) {
                if let Some(content) = attr_value(tag, "content") {
                    let decoded = normalize_visible_text(&content);
                    if !decoded.is_empty() {
                        return Some(decoded);
                    }
                }
            }
        }
        search_from = clamp_boundary(html, tag_end.saturating_add(1));
    }
    None
}

pub fn extract_meta_any(html: &str, keys: &[(&str, &str)]) -> Option<String> {
    for (k, v) in keys {
        if let Some(c) = extract_meta_content(html, k, v) {
            return Some(c);
        }
    }
    None
}

pub fn extract_hreflang(html: &str) -> Vec<HreflangItem> {
    let mut out = Vec::new();
    let mut search_from = 0;
    while let Some(abs) = find_open_tag(html, search_from, "link", true) {
        let Some(tag_end) = find_tag_end(html, abs) else {
            break;
        };
        if let Some(tag) = safe_slice(html, abs, tag_end.saturating_add(1)) {
            let rel_val = attr_value(tag, "rel").unwrap_or_default();
            let rel_ok = rel_val.is_empty()
                || rel_val
                    .split_whitespace()
                    .any(|r| r.eq_ignore_ascii_case("alternate"));
            if rel_ok {
                if let (Some(lang), Some(href)) =
                    (attr_value(tag, "hreflang"), attr_value(tag, "href"))
                {
                    let lang = lang.trim();
                    let href = href.trim();
                    if !lang.is_empty() && !href.is_empty() {
                        out.push(HreflangItem {
                            lang: lang.to_string(),
                            href: href.to_string(),
                        });
                    }
                }
            }
        }
        search_from = clamp_boundary(html, tag_end.saturating_add(1));
    }
    out
}

pub fn extract_ai_directives(html: &str) -> AiBotDirectives {
    AiBotDirectives {
        gptbot_blocked: bot_meta_blocked(html, &["gptbot", "chatgpt-user", "chatgptuser"]),
        perplexity_blocked: bot_meta_blocked(html, &["perplexitybot", "perplexity"]),
        claudebot_blocked: bot_meta_blocked(html, &["claudebot", "anthropic-ai", "claude-web"]),
        google_extended_blocked: bot_meta_blocked(html, &["google-extended"]),
    }
}

fn bot_meta_blocked(html: &str, names: &[&str]) -> bool {
    for name in names {
        if let Some(content) = extract_meta_content(html, "name", name) {
            if RobotsTokens::parse(&content).blocks_indexing() {
                return true;
            }
        }
    }
    false
}

pub fn extract_viewport(html: &str) -> bool {
    let html = clip_html(html);
    let mut search_from = 0;
    while let Some(abs) = find_open_tag(html, search_from, "meta", true) {
        let Some(end) = find_tag_end(html, abs) else { break };
        if let Some(tag) = safe_slice(html, abs, end.saturating_add(1)) {
            if attr_value(tag, "name").is_some_and(|v| v.eq_ignore_ascii_case("viewport")) {
                return true;
            }
        }
        search_from = clamp_boundary(html, end.saturating_add(1));
    }
    false
}

pub fn extract_html_lang(html: &str) -> Option<String> {
    let html = clip_html(html);
    let abs = find_open_tag(html, 0, "html", false)?;
    let end = find_tag_end(html, abs)?;
    let tag = safe_slice(html, abs, end.saturating_add(1))?;
    attr_value(tag, "lang")
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
}

pub fn count_images_missing_alt(html: &str) -> usize {
    let html = clip_html(html);
    let mut count = 0;
    let mut search_from = 0;
    while let Some(abs) = find_open_tag(html, search_from, "img", true) {
        let Some(end) = find_tag_end(html, abs) else { break };
        if let Some(tag) = safe_slice(html, abs, end.saturating_add(1)) {
            if attr_value(tag, "alt").is_none_or(|v| v.trim().is_empty()) {
                count += 1;
            }
        }
        search_from = clamp_boundary(html, end.saturating_add(1));
    }
    count
}

pub fn extract_opengraph(html: &str) -> OpenGraphMeta {
    OpenGraphMeta {
        title: extract_meta_any(html, &[("property", "og:title"), ("name", "og:title")]),
        description: extract_meta_any(
            html,
            &[("property", "og:description"), ("name", "og:description")],
        ),
        image: extract_meta_any(html, &[("property", "og:image"), ("name", "og:image")]),
        og_type: extract_meta_any(html, &[("property", "og:type"), ("name", "og:type")]),
        url: extract_meta_any(html, &[("property", "og:url"), ("name", "og:url")]),
        site_name: extract_meta_any(
            html,
            &[("property", "og:site_name"), ("name", "og:site_name")],
        ),
        video: extract_meta_any(
            html,
            &[
                ("property", "og:video"),
                ("name", "og:video"),
                ("property", "og:video:url"),
                ("property", "og:video:secure_url"),
            ],
        ),
    }
}

pub fn extract_twitter_card(html: &str) -> TwitterCardMeta {
    TwitterCardMeta {
        card: extract_meta_any(html, &[("name", "twitter:card"), ("property", "twitter:card")]),
        title: extract_meta_any(
            html,
            &[("name", "twitter:title"), ("property", "twitter:title")],
        ),
        description: extract_meta_any(
            html,
            &[
                ("name", "twitter:description"),
                ("property", "twitter:description"),
            ],
        ),
        image: extract_meta_any(
            html,
            &[("name", "twitter:image"), ("property", "twitter:image")],
        ),
    }
}