use crate::models::{
    AiBotDirectives, HreflangItem, JsonLdBlock, OpenGraphMeta, TwitterCardMeta,
};

#[derive(Debug, Default)]
pub struct RawHtmlInspection {
    pub page_title: Option<String>,
    pub meta_description: Option<String>,
    pub canonical_url: Option<String>,
    pub h1_content: Option<String>,
    pub h1_count: usize,
    pub has_noindex: bool,
    pub has_nofollow: bool,
    pub robots_meta: Option<String>,
    pub hreflangs: Vec<HreflangItem>,
    pub opengraph: OpenGraphMeta,
    pub twitter_card: TwitterCardMeta,
    pub json_ld: Vec<JsonLdBlock>,
    pub ai_directives: AiBotDirectives,
}

pub fn inspect_html(html: &str) -> RawHtmlInspection {
    let lower = html.to_ascii_lowercase();

    // 1. 基础标签提取
    let page_title = extract_title(html, &lower);
    let (h1_content, h1_count) = extract_h1(html, &lower);
    let canonical_url = extract_canonical(html, &lower);
    let meta_description = extract_meta_content(html, &lower, "name", "description");

    // 2. Robots 指令
    let robots_meta = extract_meta_content(html, &lower, "name", "robots")
        .or_else(|| extract_meta_content(html, &lower, "name", "googlebot"));
    let robots_l = robots_meta.as_deref().unwrap_or("").to_ascii_lowercase();
    let has_noindex = robots_l.contains("noindex") || detect_noindex_tag(&lower);
    let has_nofollow = robots_l.contains("nofollow");

    // 3. AI 爬虫限制嗅探
    let gptbot = extract_meta_content(html, &lower, "name", "gptbot")
        .unwrap_or_default()
        .to_ascii_lowercase();
    let perplexity = extract_meta_content(html, &lower, "name", "perplexitybot")
        .unwrap_or_default()
        .to_ascii_lowercase();
    let claudebot = extract_meta_content(html, &lower, "name", "claudebot")
        .unwrap_or_default()
        .to_ascii_lowercase();
    let google_ext = extract_meta_content(html, &lower, "name", "google-extended")
        .unwrap_or_default()
        .to_ascii_lowercase();

    let ai_directives = AiBotDirectives {
        gptbot_blocked: gptbot.contains("noindex"),
        perplexity_blocked: perplexity.contains("noindex"),
        claudebot_blocked: claudebot.contains("noindex"),
        google_extended_blocked: google_ext.contains("noindex"),
    };

    // 4. OpenGraph & Twitter
    let opengraph = OpenGraphMeta {
        title: extract_meta_content(html, &lower, "property", "og:title"),
        description: extract_meta_content(html, &lower, "property", "og:description"),
        image: extract_meta_content(html, &lower, "property", "og:image"),
        og_type: extract_meta_content(html, &lower, "property", "og:type"),
        url: extract_meta_content(html, &lower, "property", "og:url"),
        site_name: extract_meta_content(html, &lower, "property", "og:site_name"),
    };

    let twitter_card = TwitterCardMeta {
        card: extract_meta_content(html, &lower, "name", "twitter:card"),
        title: extract_meta_content(html, &lower, "name", "twitter:title"),
        description: extract_meta_content(html, &lower, "name", "twitter:description"),
        image: extract_meta_content(html, &lower, "name", "twitter:image"),
    };

    // 5. JSON-LD 结构化数据
    let json_ld = extract_json_ld(html, &lower);

    // 6. Hreflang
    let hreflangs = extract_hreflang(html, &lower);

    RawHtmlInspection {
        page_title,
        meta_description,
        canonical_url,
        h1_content,
        h1_count,
        has_noindex,
        has_nofollow,
        robots_meta,
        hreflangs,
        opengraph,
        twitter_card,
        json_ld,
        ai_directives,
    }
}

fn detect_noindex_tag(lower: &str) -> bool {
    lower.contains(r#"content="noindex"#) || lower.contains(r#"content='noindex"#)
}

fn extract_title(html: &str, lower: &str) -> Option<String> {
    inner_text(html, lower, "<title", "</title>")
}

fn extract_h1(html: &str, lower: &str) -> (Option<String>, usize) {
    let mut count = 0;
    let mut first_content = None;
    let mut search_from = 0;

    while let Some(idx) = lower[search_from..].find("<h1") {
        let abs_start = search_from + idx;
        if let Some(gt_rel) = html[abs_start..].find('>') {
            let after_gt = abs_start + gt_rel + 1;
            if let Some(close_rel) = lower[after_gt..].find("</h1>") {
                let end = after_gt + close_rel;
                if let Some(raw) = safe_slice(html, after_gt, end) {
                    let cleaned = decode_basic_entities(&strip_tags(raw));
                    let trimmed = cleaned.trim().to_string();
                    if !trimmed.is_empty() {
                        count += 1;
                        if first_content.is_none() {
                            first_content = Some(trimmed);
                        }
                    }
                }
                search_from = end + 5;
                continue;
            }
        }
        break;
    }
    (first_content, count)
}

fn extract_canonical(html: &str, lower: &str) -> Option<String> {
    let mut search_from = 0;
    while let Some(rel) = lower[search_from..].find("<link") {
        let abs = search_from + rel;
        let Some(tag_end_rel) = lower[abs..].find('>') else { break; };
        let tag_end = abs + tag_end_rel;
        if let Some(tag) = safe_slice(html, abs, tag_end) {
            let rel_val = attr_value(tag, "rel").unwrap_or_default().to_ascii_lowercase();
            if rel_val.split_whitespace().any(|r| r == "canonical") {
                if let Some(href) = attr_value(tag, "href") {
                    let href = href.trim();
                    if !href.is_empty() {
                        return Some(href.to_string());
                    }
                }
            }
        }
        search_from = tag_end + 1;
    }
    None
}

pub fn extract_meta_content(html: &str, lower: &str, attr_key: &str, attr_val: &str) -> Option<String> {
    let mut search_from = 0;
    while let Some(rel) = lower[search_from..].find("<meta") {
        let abs = search_from + rel;
        let Some(end_rel) = lower[abs..].find('>') else { break; };
        let tag_end = abs + end_rel;
        if let Some(tag) = safe_slice(html, abs, tag_end) {
            let key = attr_value(tag, attr_key).unwrap_or_default().to_ascii_lowercase();
            if key == attr_val.to_ascii_lowercase() {
                if let Some(content) = attr_value(tag, "content") {
                    let decoded = decode_basic_entities(content.trim());
                    if !decoded.is_empty() {
                        return Some(decoded);
                    }
                }
            }
        }
        search_from = tag_end + 1;
    }
    None
}

fn extract_hreflang(html: &str, lower: &str) -> Vec<HreflangItem> {
    let mut out = Vec::new();
    let mut search_from = 0;
    while let Some(rel) = lower[search_from..].find("<link") {
        let abs = search_from + rel;
        let Some(end_rel) = lower[abs..].find('>') else { break; };
        let tag_end = abs + end_rel;
        if let Some(tag) = safe_slice(html, abs, tag_end) {
            let rel_val = attr_value(tag, "rel").unwrap_or_default().to_ascii_lowercase();
            if rel_val.is_empty() || rel_val.contains("alternate") {
                if let (Some(lang), Some(href)) = (attr_value(tag, "hreflang"), attr_value(tag, "href")) {
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
        search_from = tag_end + 1;
    }
    out
}

fn extract_json_ld(html: &str, lower: &str) -> Vec<JsonLdBlock> {
    let mut blocks = Vec::new();
    let mut search_from = 0;

    while let Some(rel) = lower[search_from..].find("<script") {
        let abs = search_from + rel;
        let Some(tag_end_rel) = lower[abs..].find('>') else { break; };
        let tag_end = abs + tag_end_rel;
        if let Some(open_tag) = safe_slice(html, abs, tag_end) {
            let type_val = attr_value(open_tag, "type").unwrap_or_default().to_ascii_lowercase();
            if type_val == "application/ld+json" {
                if let Some(close_rel) = lower[tag_end..].find("</script>") {
                    let script_body_end = tag_end + close_rel;
                    if let Some(raw_body) = safe_slice(html, tag_end + 1, script_body_end) {
                        let trimmed = raw_body.trim();
                        if !trimmed.is_empty() {
                            let schema_type = serde_json::from_str::<serde_json::Value>(trimmed)
                                .ok()
                                .and_then(|v| {
                                    v.get("@type")
                                        .and_then(|t| t.as_str())
                                        .map(|s| s.to_string())
                                });

                            blocks.push(JsonLdBlock {
                                schema_type,
                                raw_json: trimmed.to_string(),
                            });
                        }
                    }
                    search_from = script_body_end + 9;
                    continue;
                }
            }
        }
        search_from = tag_end + 1;
    }
    blocks
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

fn safe_slice(html: &str, start: usize, end: usize) -> Option<&str> {
    if start <= end && end <= html.len() && html.is_char_boundary(start) && html.is_char_boundary(end) {
        Some(&html[start..end])
    } else {
        None
    }
}

fn attr_value(tag: &str, name: &str) -> Option<String> {
    let lower = tag.to_ascii_lowercase();
    let name_l = name.to_ascii_lowercase();
    let mut search_idx = 0;

    while let Some(idx) = lower[search_idx..].find(&name_l) {
        let abs_idx = search_idx + idx;
        let is_word_boundary_before = if abs_idx == 0 {
            true
        } else {
            let prev = lower.as_bytes()[abs_idx - 1];
            prev.is_ascii_whitespace() || prev == b'<'
        };

        if is_word_boundary_before {
            let after = &tag[abs_idx + name_l.len()..];
            let trimmed = after.trim_start();
            if trimmed.starts_with('=') {
                let rest = trimmed[1..].trim_start();
                if let Some(quote) = rest.chars().next() {
                    if quote == '"' || quote == '\'' {
                        if let Some(end) = rest[1..].find(quote) {
                            return Some(rest[1..1 + end].to_string());
                        }
                    } else {
                        let end = rest
                            .find(|c: char| c.is_whitespace() || c == '>' || c == '/')
                            .unwrap_or(rest.len());
                        return Some(rest[..end].to_string());
                    }
                }
            }
        }
        search_idx = abs_idx + name_l.len();
    }
    None
}

pub fn decode_basic_entities(s: &str) -> String {
    s.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&nbsp;", " ")
}