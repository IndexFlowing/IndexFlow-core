//! Zero-panic HTML inspector.
//!
//! Every slice of the input is gated by [`safe_slice`] / [`str::get`] so a
//! multi-byte character (CJK, emoji) can never land in the middle of a
//! `Range` and panic. Tag scanning is quote-aware and skips comments plus
//! `<script>` / `<style>` raw-text elements when looking for visible tags.

use crate::models::{
    collect_schema_types, AiBotDirectives, HreflangItem, JsonLdBlock, OpenGraphMeta, TwitterCardMeta,
};

/// Hard cap on HTML we will walk. A 50 MiB error page must not become a
/// quadratic scan; the prefix is cut on a char boundary.
const MAX_SCAN_BYTES: usize = 5 * 1024 * 1024;

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
    pub has_viewport: bool,
    pub html_lang: Option<String>,
    pub images_missing_alt: usize,
}

/// Inspect `html` without allocating a full-document lowercase copy.
pub fn inspect_html(html: &str) -> RawHtmlInspection {
    let html = clip_html(html);

    let page_title = extract_title(html);
    let (h1_content, h1_count) = extract_h1(html);
    let canonical_url = extract_canonical(html);
    let meta_description = extract_meta_content(html, "name", "description");

    let robots_meta = extract_meta_content(html, "name", "robots")
        .or_else(|| extract_meta_content(html, "name", "googlebot"))
        .or_else(|| extract_meta_content(html, "http-equiv", "robots"));

    let (has_noindex, has_nofollow) = robots_flags(robots_meta.as_deref());

    let ai_directives = extract_ai_directives(html);

    let opengraph = OpenGraphMeta {
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
    };

    let twitter_card = TwitterCardMeta {
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
    };

    let json_ld = extract_json_ld(html);
    let hreflangs = extract_hreflang(html);
    let has_viewport = extract_viewport(html);
    let html_lang = extract_html_lang(html);
    let images_missing_alt = count_images_missing_alt(html);

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
        has_viewport,
        html_lang,
        images_missing_alt,
    }
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
    attr_value(tag, "lang").map(|v| v.trim().to_string()).filter(|v| !v.is_empty())
}

pub fn count_images_missing_alt(html: &str) -> usize {
    let html = clip_html(html);
    let mut count = 0;
    let mut search_from = 0;
    while let Some(abs) = find_open_tag(html, search_from, "img", true) {
        let Some(end) = find_tag_end(html, abs) else { break };
        if let Some(tag) = safe_slice(html, abs, end.saturating_add(1)) {
            if attr_value(tag, "alt").is_none_or(|v| v.trim().is_empty()) { count += 1; }
        }
        search_from = clamp_boundary(html, end.saturating_add(1));
    }
    count
}

pub(crate) fn robots_flags(content: Option<&str>) -> (bool, bool) {
    let tokens = RobotsTokens::parse(content.unwrap_or(""));
    (tokens.noindex, tokens.nofollow)
}

pub(crate) struct RobotsTokens {
    pub noindex: bool,
    pub nofollow: bool,
    pub none: bool,
    pub noai: bool,
}

impl RobotsTokens {
    pub fn parse(content: &str) -> Self {
        let mut t = Self {
            noindex: false,
            nofollow: false,
            none: false,
            noai: false,
        };
        for tok in robots_iter(content) {
            if tok.eq_ignore_ascii_case("noindex") {
                t.noindex = true;
            } else if tok.eq_ignore_ascii_case("nofollow") {
                t.nofollow = true;
            } else if tok.eq_ignore_ascii_case("none") {
                t.none = true;
                t.noindex = true;
                t.nofollow = true;
            } else if tok.eq_ignore_ascii_case("noai") || tok.eq_ignore_ascii_case("noimageai") {
                t.noai = true;
            }
        }
        t
    }

    pub fn blocks_indexing(&self) -> bool {
        self.noindex || self.none || self.noai
    }
}

fn robots_iter(s: &str) -> impl Iterator<Item = &str> {
    s.split([',', ';'])
        .flat_map(|part| part.split_whitespace())
        .map(str::trim)
        .filter(|t| !t.is_empty())
}

fn extract_ai_directives(html: &str) -> AiBotDirectives {
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

fn extract_title(html: &str) -> Option<String> {
    inner_text(html, "title", true)
}

fn extract_h1(html: &str) -> (Option<String>, usize) {
    let mut count = 0;
    let mut first_content = None;
    let mut search_from = 0;

    while let Some(abs_start) = find_open_tag(html, search_from, "h1", true) {
        let Some(gt) = find_tag_end(html, abs_start) else {
            break;
        };
        let after_gt = gt + 1;
        let Some(close_at) = find_close_tag(html, after_gt, "h1") else {
            break;
        };
        if let Some(raw) = safe_slice(html, after_gt, close_at) {
            let cleaned = normalize_visible_text(&decode_basic_entities(&strip_tags(raw)));
            if !cleaned.is_empty() {
                count += 1;
                if first_content.is_none() {
                    first_content = Some(cleaned);
                }
            }
        }
        search_from = close_at.saturating_add(5);
        search_from = clamp_boundary(html, search_from);
    }
    (first_content, count)
}

fn extract_canonical(html: &str) -> Option<String> {
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
        search_from = tag_end.saturating_add(1);
        search_from = clamp_boundary(html, search_from);
    }
    None
}

/// First `<meta>` whose `attr_key` equals `attr_val` (ASCII case-insensitive).
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
        search_from = tag_end.saturating_add(1);
        search_from = clamp_boundary(html, search_from);
    }
    None
}

fn extract_meta_any(html: &str, keys: &[(&str, &str)]) -> Option<String> {
    for (k, v) in keys {
        if let Some(c) = extract_meta_content(html, k, v) {
            return Some(c);
        }
    }
    None
}

fn extract_hreflang(html: &str) -> Vec<HreflangItem> {
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
                if let (Some(lang), Some(href)) = (attr_value(tag, "hreflang"), attr_value(tag, "href"))
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
        search_from = tag_end.saturating_add(1);
        search_from = clamp_boundary(html, search_from);
    }
    out
}

fn extract_json_ld(html: &str) -> Vec<JsonLdBlock> {
    let mut blocks = Vec::new();
    let mut search_from = 0;

    while let Some(abs) = find_open_tag(html, search_from, "script", false) {
        let Some(tag_end) = find_tag_end(html, abs) else {
            break;
        };
        if let Some(open_tag) = safe_slice(html, abs, tag_end.saturating_add(1)) {
            let type_val = attr_value(open_tag, "type").unwrap_or_default();
            if is_ld_json_type(&type_val) {
                let body_start = tag_end.saturating_add(1);
                if let Some(close_at) = find_close_tag(html, body_start, "script") {
                    if let Some(raw_body) = safe_slice(html, body_start, close_at) {
                        emit_json_ld_blocks(raw_body, &mut blocks);
                    }
                    search_from = close_at.saturating_add(9);
                    search_from = clamp_boundary(html, search_from);
                    continue;
                }
            } else if let Some(close_at) = find_close_tag(html, tag_end + 1, "script") {
                search_from = close_at.saturating_add(9);
                search_from = clamp_boundary(html, search_from);
                continue;
            }
        }
        search_from = tag_end.saturating_add(1);
        search_from = clamp_boundary(html, search_from);
    }
    blocks
}

fn is_ld_json_type(t: &str) -> bool {
    let t = t.trim();
    t.eq_ignore_ascii_case("application/ld+json")
        || starts_with_ignore_ascii(t, "application/ld+json;")
}

fn emit_json_ld_blocks(raw_body: &str, blocks: &mut Vec<JsonLdBlock>) {
    let stripped = strip_json_ld_wrappers(raw_body);
    let trimmed = stripped.trim();
    if trimmed.is_empty() {
        return;
    }
    match serde_json::from_str::<serde_json::Value>(trimmed) {
        Ok(value) => expand_json_ld_value(&value, blocks),
        Err(_) => blocks.push(JsonLdBlock {
            schema_type: None,
            raw_json: trimmed.to_string(),
        }),
    }
}

fn expand_json_ld_value(value: &serde_json::Value, blocks: &mut Vec<JsonLdBlock>) {
    match value {
        serde_json::Value::Array(arr) => {
            for v in arr {
                expand_json_ld_value(v, blocks);
            }
        }
        serde_json::Value::Object(map) => {
            if let Some(graph) = map.get("@graph") {
                expand_json_ld_value(graph, blocks);
                if map.contains_key("@type") {
                    blocks.push(block_from_value(value));
                }
            } else {
                blocks.push(block_from_value(value));
            }
        }
        _ => {}
    }
}

fn block_from_value(value: &serde_json::Value) -> JsonLdBlock {
    let mut types = Vec::new();
    collect_schema_types(value, &mut types);
    JsonLdBlock {
        schema_type: types.first().cloned(),
        raw_json: value.to_string(),
    }
}

fn strip_json_ld_wrappers(s: &str) -> String {
    let mut t = s.trim().to_string();
    if let Some(rest) = t.strip_prefix("<!--") {
        if let Some(idx) = rest.rfind("-->") {
            t = rest[..idx].trim().to_string();
        }
    }
    let mut t = t.trim().to_string();
    if let Some(rest) = t.strip_prefix("//") {
        t = rest.trim_start().to_string();
    }
    if let Some(rest) = t.strip_prefix("<![CDATA[") {
        t = rest.to_string();
    }
    if let Some(rest) = t.strip_suffix("]]>") {
        t = rest.to_string();
    }
    if let Some(rest) = t.strip_suffix("//") {
        t = rest.to_string();
    }
    t.trim().to_string()
}

fn inner_text(html: &str, tag: &str, skip_raw: bool) -> Option<String> {
    let start = find_open_tag(html, 0, tag, skip_raw)?;
    let gt = find_tag_end(html, start)?;
    let after_gt = gt + 1;
    let close_at = find_close_tag(html, after_gt, tag)?;
    let raw = safe_slice(html, after_gt, close_at)?.trim();
    if raw.is_empty() {
        return None;
    }
    let decoded = normalize_visible_text(&decode_basic_entities(&strip_tags(raw)));
    if decoded.is_empty() {
        None
    } else {
        Some(decoded)
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

fn normalize_visible_text(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut prev_ws = false;
    for c in s.chars() {
        if c.is_whitespace() {
            if !prev_ws {
                out.push(' ');
                prev_ws = true;
            }
        } else {
            prev_ws = false;
            out.push(c);
        }
    }
    out.trim().to_string()
}

fn clip_html(html: &str) -> &str {
    if html.len() <= MAX_SCAN_BYTES {
        return html;
    }
    let mut end = MAX_SCAN_BYTES;
    while end > 0 && !html.is_char_boundary(end) {
        end -= 1;
    }
    match html.get(..end) {
        Some(s) => s,
        None => html,
    }
}

fn clamp_boundary(s: &str, mut i: usize) -> usize {
    if i >= s.len() {
        return s.len();
    }
    while i > 0 && !s.is_char_boundary(i) {
        i -= 1;
    }
    i
}

/// Byte-index slice that refuses to panic on a non-boundary.
fn safe_slice(html: &str, start: usize, end: usize) -> Option<&str> {
    if start <= end && end <= html.len() && html.is_char_boundary(start) && html.is_char_boundary(end)
    {
        html.get(start..end)
    } else {
        None
    }
}

/// Locate `<name ...>` (ASCII-case-insensitive) starting at `from`.
///
/// When `skip_raw` is set, `<script>` / `<style>` bodies (and comments) are
/// jumped over so a stray `<title>` inside JS never wins.
fn find_open_tag(html: &str, from: usize, name_lower: &str, skip_raw: bool) -> Option<usize> {
    let mut search = clamp_boundary(html, from);
    while search < html.len() {
        let slice = html.get(search..)?;
        let rel = slice.find('<')?;
        let i = search + rel;
        let after = match html.get(i..) {
            Some(s) => s,
            None => return None,
        };
        if after.starts_with("<!--") {
            let rest = html.get(i + 4..)?;
            match rest.find("-->") {
                Some(end) => {
                    search = i + 4 + end + 3;
                    continue;
                }
                None => return None,
            }
        }
        let after_lt = match html.get(i + 1..) {
            Some(s) => s,
            None => return None,
        };
        match after_lt.as_bytes().first().copied() {
            Some(b'/' | b'!' | b'?') => {
                search = i + 1;
                continue;
            }
            _ => {}
        }
        if skip_raw && (tag_name_eq(after_lt, "script") || tag_name_eq(after_lt, "style")) {
            let tag_end = find_tag_end(html, i).unwrap_or(i + 1);
            let close_name = if tag_name_eq(after_lt, "script") {
                "script"
            } else {
                "style"
            };
            search = find_close_tag(html, tag_end.saturating_add(1), close_name)
                .map(|p| p.saturating_add(close_name.len() + 3))
                .unwrap_or_else(|| html.len());
            search = clamp_boundary(html, search);
            continue;
        }
        if tag_name_eq(after_lt, name_lower) {
            return Some(i);
        }
        search = i + 1;
    }
    None
}

fn find_close_tag(html: &str, from: usize, name_lower: &str) -> Option<usize> {
    let mut search = clamp_boundary(html, from);
    while search < html.len() {
        let slice = html.get(search..)?;
        let rel = slice.find('<')?;
        let i = search + rel;
        let after = html.get(i..)?;
        if after.starts_with("<!--") {
            let rest = html.get(i + 4..)?;
            match rest.find("-->") {
                Some(end) => {
                    search = i + 4 + end + 3;
                    continue;
                }
                None => return None,
            }
        }
        let after_lt = html.get(i + 1..)?;
        if after_lt.as_bytes().first() == Some(&b'/') {
            let name_part = html.get(i + 2..)?;
            if tag_name_eq(name_part, name_lower) {
                return Some(i);
            }
        }
        search = i + 1;
    }
    None
}

fn tag_name_eq(after_lt_or_slash: &str, name_lower: &str) -> bool {
    let bytes = after_lt_or_slash.as_bytes();
    let n = name_lower.as_bytes();
    if bytes.len() < n.len() {
        return false;
    }
    if !bytes[..n.len()]
        .iter()
        .zip(n.iter())
        .all(|(a, b)| a.to_ascii_lowercase() == *b)
    {
        return false;
    }
    match bytes.get(n.len()) {
        None => true,
        Some(b) => is_tag_name_end(*b),
    }
}

fn is_tag_name_end(b: u8) -> bool {
    b.is_ascii_whitespace() || b == b'>' || b == b'/' || b == b'\n' || b == b'\r' || b == b'\t'
}

/// Index of the `>` that closes the start tag, ignoring `>` inside quotes.
fn find_tag_end(html: &str, tag_open: usize) -> Option<usize> {
    let bytes = html.as_bytes();
    if tag_open >= bytes.len() {
        return None;
    }
    let mut i = tag_open;
    let mut quote: Option<u8> = None;
    while i < bytes.len() {
        let b = bytes[i];
        match quote {
            Some(q) if b == q => quote = None,
            Some(_) => {}
            None if b == b'"' || b == b'\'' => quote = Some(b),
            None if b == b'>' => return Some(i),
            _ => {}
        }
        i += 1;
    }
    None
}

/// Parse `name='...'` / `name="..."` / `name=unquoted` from a raw start tag.
///
/// Attribute names are matched as whole tokens (so `href` does not steal
/// `hreflang`). Values are entity-decoded. Never panics.
fn attr_value(tag: &str, want: &str) -> Option<String> {
    let bytes = tag.as_bytes();
    let mut i = 0usize;
    if bytes.first() == Some(&b'<') {
        i = 1;
    }
    while i < bytes.len() && !is_tag_name_end(bytes[i]) {
        i += 1;
    }
    while i < bytes.len() {
        while i < bytes.len() && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        if i >= bytes.len() {
            break;
        }
        let b = bytes[i];
        if b == b'>' || b == b'/' {
            break;
        }
        let name_start = i;
        while i < bytes.len() && is_attr_name_char(bytes[i]) {
            i += 1;
        }
        let name = match safe_slice(tag, name_start, i) {
            Some(n) => n,
            None => break,
        };
        while i < bytes.len() && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        let value = if i < bytes.len() && bytes[i] == b'=' {
            i += 1;
            while i < bytes.len() && bytes[i].is_ascii_whitespace() {
                i += 1;
            }
            read_attr_value(tag, &mut i)
        } else {
            String::new()
        };
        if name.eq_ignore_ascii_case(want) {
            return Some(decode_basic_entities(&value));
        }
    }
    None
}

fn is_attr_name_char(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'-' || b == b'_' || b == b':' || b == b'.'
}

fn read_attr_value(tag: &str, i: &mut usize) -> String {
    let bytes = tag.as_bytes();
    if *i >= bytes.len() {
        return String::new();
    }
    let quote = bytes[*i];
    if quote == b'"' || quote == b'\'' {
        *i += 1;
        let start = *i;
        while *i < bytes.len() && bytes[*i] != quote {
            *i += 1;
        }
        let end = *i;
        if *i < bytes.len() {
            *i += 1;
        }
        return safe_slice(tag, start, end).unwrap_or("").to_string();
    }
    let start = *i;
    while *i < bytes.len() {
        let b = bytes[*i];
        // HTML unquoted values may contain `/` (URLs). They must not contain
        // whitespace, quotes, `=`, `<`, `>`, or backtick.
        if b.is_ascii_whitespace() || matches!(b, b'>' | b'"' | b'\'' | b'=' | b'<' | b'`') {
            break;
        }
        *i += 1;
    }
    safe_slice(tag, start, *i).unwrap_or("").to_string()
}

fn starts_with_ignore_ascii(s: &str, prefix_lower: &str) -> bool {
    let sb = s.as_bytes();
    let pb = prefix_lower.as_bytes();
    if sb.len() < pb.len() {
        return false;
    }
    sb[..pb.len()]
        .iter()
        .zip(pb.iter())
        .all(|(a, b)| a.to_ascii_lowercase() == *b)
}

/// Decode XML / HTML character references in a single left-to-right pass.
///
/// Supports:
/// * XML predefined (`&amp;` `&lt;` `&gt;` `&quot;` `&apos;`)
/// * HTML named entities (Latin-1 + common typography)
/// * Decimal `&#39;` and hexadecimal `&#x1F600;`
///
/// Bare `&` that is not a well-formed reference is preserved. Never panics.
pub fn decode_basic_entities(s: &str) -> String {
    if !s.as_bytes().contains(&b'&') {
        return s.to_string();
    }
    let mut out = String::with_capacity(s.len());
    let mut i = 0;
    while i < s.len() {
        let rest = match s.get(i..) {
            Some(r) => r,
            None => break,
        };
        if rest.as_bytes().first() == Some(&b'&') {
            if let Some((consumed, decoded)) = parse_entity(rest) {
                out.push_str(&decoded);
                i += consumed;
                continue;
            }
        }
        match rest.chars().next() {
            Some(ch) => {
                out.push(ch);
                i += ch.len_utf8();
            }
            None => break,
        }
    }
    out
}

fn parse_entity(s: &str) -> Option<(usize, String)> {
    let bytes = s.as_bytes();
    if bytes.first() != Some(&b'&') {
        return None;
    }
    let mut j = 1;
    if bytes.get(1) == Some(&b'#') {
        let hex = bytes.get(2) == Some(&b'x') || bytes.get(2) == Some(&b'X');
        j = if hex { 3 } else { 2 };
        let start = j;
        if hex {
            while j < bytes.len() && bytes[j].is_ascii_hexdigit() && j - start < 8 {
                j += 1;
            }
        } else {
            while j < bytes.len() && bytes[j].is_ascii_digit() && j - start < 10 {
                j += 1;
            }
        }
        if j == start || bytes.get(j) != Some(&b';') {
            return None;
        }
        let digits = s.get(start..j)?;
        let n = if hex {
            u32::from_str_radix(digits, 16).ok()?
        } else {
            digits.parse::<u32>().ok()?
        };
        let ch = char::from_u32(n).filter(|c| *c != '\0')?;
        Some((j + 1, ch.to_string()))
    } else {
        while j < bytes.len() && bytes[j].is_ascii_alphanumeric() && j < 33 {
            j += 1;
        }
        if j == 1 || bytes.get(j) != Some(&b';') {
            return None;
        }
        let name = s.get(1..j)?;
        let decoded = named_entity(name)?;
        Some((j + 1, decoded.to_string()))
    }
}

fn named_entity(name: &str) -> Option<&'static str> {
    Some(match name {
        "amp" | "AMP" => "&",
        "lt" | "LT" => "<",
        "gt" | "GT" => ">",
        "quot" | "QUOT" => "\"",
        "apos" => "'",
        "nbsp" => "\u{a0}",
        "iexcl" => "¡",
        "cent" => "¢",
        "pound" => "£",
        "curren" => "¤",
        "yen" => "¥",
        "brvbar" => "¦",
        "sect" => "§",
        "uml" => "¨",
        "copy" | "COPY" => "©",
        "ordf" => "ª",
        "laquo" => "«",
        "not" => "¬",
        "shy" => "\u{ad}",
        "reg" | "REG" => "®",
        "macr" => "¯",
        "deg" => "°",
        "plusmn" => "±",
        "sup2" => "²",
        "sup3" => "³",
        "acute" => "´",
        "micro" => "µ",
        "para" => "¶",
        "middot" => "·",
        "cedil" => "¸",
        "sup1" => "¹",
        "ordm" => "º",
        "raquo" => "»",
        "frac14" => "¼",
        "frac12" => "½",
        "frac34" => "¾",
        "iquest" => "¿",
        "times" => "×",
        "divide" => "÷",
        "Agrave" => "À",
        "Aacute" => "Á",
        "Acirc" => "Â",
        "Atilde" => "Ã",
        "Auml" => "Ä",
        "Aring" => "Å",
        "AElig" => "Æ",
        "Ccedil" => "Ç",
        "Egrave" => "È",
        "Eacute" => "É",
        "Ecirc" => "Ê",
        "Euml" => "Ë",
        "Igrave" => "Ì",
        "Iacute" => "Í",
        "Icirc" => "Î",
        "Iuml" => "Ï",
        "ETH" => "Ð",
        "Ntilde" => "Ñ",
        "Ograve" => "Ò",
        "Oacute" => "Ó",
        "Ocirc" => "Ô",
        "Otilde" => "Õ",
        "Ouml" => "Ö",
        "Oslash" => "Ø",
        "Ugrave" => "Ù",
        "Uacute" => "Ú",
        "Ucirc" => "Û",
        "Uuml" => "Ü",
        "Yacute" => "Ý",
        "THORN" => "Þ",
        "szlig" => "ß",
        "agrave" => "à",
        "aacute" => "á",
        "acirc" => "â",
        "atilde" => "ã",
        "auml" => "ä",
        "aring" => "å",
        "aelig" => "æ",
        "ccedil" => "ç",
        "egrave" => "è",
        "eacute" => "é",
        "ecirc" => "ê",
        "euml" => "ë",
        "igrave" => "ì",
        "iacute" => "í",
        "icirc" => "î",
        "iuml" => "ï",
        "eth" => "ð",
        "ntilde" => "ñ",
        "ograve" => "ò",
        "oacute" => "ó",
        "ocirc" => "ô",
        "otilde" => "õ",
        "ouml" => "ö",
        "oslash" => "ø",
        "ugrave" => "ù",
        "uacute" => "ú",
        "ucirc" => "û",
        "uuml" => "ü",
        "yacute" => "ý",
        "thorn" => "þ",
        "yuml" => "ÿ",
        "OElig" => "Œ",
        "oelig" => "œ",
        "Scaron" => "Š",
        "scaron" => "š",
        "Yuml" => "Ÿ",
        "fnof" => "ƒ",
        "circ" => "ˆ",
        "tilde" => "˜",
        "ndash" => "–",
        "mdash" => "—",
        "lsquo" => "‘",
        "rsquo" => "’",
        "sbquo" => "‚",
        "ldquo" => "“",
        "rdquo" => "”",
        "bdquo" => "„",
        "dagger" => "†",
        "Dagger" => "‡",
        "permil" => "‰",
        "lsaquo" => "‹",
        "rsaquo" => "›",
        "euro" => "€",
        "bull" => "•",
        "hellip" => "…",
        "trade" | "TRADE" => "™",
        "minus" => "−",
        "lowast" => "∗",
        "oplus" => "⊕",
        "otimes" => "⊗",
        "larr" => "←",
        "uarr" => "↑",
        "rarr" => "→",
        "darr" => "↓",
        "harr" => "↔",
        "crarr" => "↵",
        "lceil" => "⌈",
        "rceil" => "⌉",
        "lfloor" => "⌊",
        "rfloor" => "⌋",
        "loz" => "◊",
        "spades" => "♠",
        "clubs" => "♣",
        "hearts" => "♥",
        "diams" => "♦",
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cjk_and_emoji_never_panic_and_round_trip() {
        let html = "<!DOCTYPE html><html><head>\
            <title>你好🌍世界 &amp; SEO</title>\
            <meta name=\"description\" content=\"中文描述🎉与&nbsp;空格\" />\
            <link rel=\"canonical\" href=\"https://example.com/你好\" />\
            </head><body><h1>标题<span>嵌套</span>🚀</h1></body></html>";
        let r = inspect_html(html);
        assert_eq!(r.page_title.as_deref(), Some("你好🌍世界 & SEO"));
        assert_eq!(r.meta_description.as_deref(), Some("中文描述🎉与 空格"));
        assert_eq!(r.h1_content.as_deref(), Some("标题嵌套🚀"));
        assert_eq!(r.h1_count, 1);
        assert_eq!(r.canonical_url.as_deref(), Some("https://example.com/你好"));
    }

    #[test]
    fn multiline_and_mixed_quotes_meta() {
        let html = "<html><head><title>T</title>\n<meta\n  name='robots'\n  content=\"noindex, nofollow\"\n>\n</head></html>";
        let r = inspect_html(html);
        assert!(r.has_noindex);
        assert!(r.has_nofollow);
        assert_eq!(r.robots_meta.as_deref(), Some("noindex, nofollow"));
    }

    #[test]
    fn unquoted_attribute_and_gt_inside_quoted_value() {
        let html = r#"<html><head><title>T</title>
            <meta name=description content="a>b">
            <link rel=canonical href=https://example.com/x>
            </head></html>"#;
        let r = inspect_html(html);
        assert_eq!(r.meta_description.as_deref(), Some("a>b"));
        assert_eq!(r.canonical_url.as_deref(), Some("https://example.com/x"));
    }

    #[test]
    fn comments_and_script_do_not_spoof_title_or_robots() {
        let html = r#"<html><head>
            <!-- <title>FAKE</title> <meta name="robots" content="noindex"> -->
            <title>Real</title>
            <script>document.write('<h1>nope</h1><title>JS</title>');</script>
            </head><body><h1>Visible</h1></body></html>"#;
        let r = inspect_html(html);
        assert_eq!(r.page_title.as_deref(), Some("Real"));
        assert!(!r.has_noindex);
        assert_eq!(r.h1_content.as_deref(), Some("Visible"));
        assert_eq!(r.h1_count, 1);
    }

    #[test]
    fn json_ld_graph_array_and_multi_type() {
        let html = r#"<html><head><title>T</title>
            <script type="application/ld+json">
            {
              "@context": "https://schema.org",
              "@graph": [
                {"@type": "Organization", "name": "Acme"},
                {"@type": ["NewsArticle", "Article"], "headline": "H"}
              ]
            }
            </script>
            <script type="application/ld+json;charset=utf-8">
            [{"@type": "FAQPage"}, {"@type": "WebPage"}]
            </script>
            </head></html>"#;
        let r = inspect_html(html);
        let types: Vec<_> = r
            .json_ld
            .iter()
            .filter_map(|b| b.schema_type.clone())
            .collect();
        assert!(types.contains(&"Organization".to_string()));
        assert!(types.contains(&"NewsArticle".to_string()) || types.contains(&"Article".to_string()));
        assert!(types.contains(&"FAQPage".to_string()));
        assert!(types.contains(&"WebPage".to_string()));
    }

    #[test]
    fn json_ld_cdata_wrapper() {
        let html = r#"<html><head><title>T</title>
            <script type="application/ld+json">
            //<![CDATA[
            {"@type": "Product", "name": "X"}
            //]]>
            </script></head></html>"#;
        let r = inspect_html(html);
        assert_eq!(r.json_ld[0].schema_type.as_deref(), Some("Product"));
    }

    #[test]
    fn ai_bot_aliases_and_none_directive() {
        let html = r#"<html><head><title>T</title>
            <meta name="GPTBot" content="none" />
            <meta name="anthropic-ai" content="noindex" />
            <meta name="Google-Extended" content="noai" />
            <meta name="perplexitybot" content="index, follow" />
            </head></html>"#;
        let r = inspect_html(html);
        assert!(r.ai_directives.gptbot_blocked);
        assert!(r.ai_directives.claudebot_blocked);
        assert!(r.ai_directives.google_extended_blocked);
        assert!(!r.ai_directives.perplexity_blocked);
    }

    #[test]
    fn entity_one_pass_does_not_double_decode() {
        assert_eq!(decode_basic_entities("Tom &amp; Jerry &#39;Special&#39;"), "Tom & Jerry 'Special'");
        assert_eq!(
            decode_basic_entities("&lt;div&gt;&quot;Hello&quot;&nbsp;World&lt;/div&gt;"),
            "<div>\"Hello\"\u{a0}World</div>"
        );
        assert_eq!(decode_basic_entities("&amp;lt;"), "&lt;");
        assert_eq!(decode_basic_entities("a & b"), "a & b");
        assert_eq!(decode_basic_entities("&mdash;&hellip;&#x1F600;"), "—…😀");
        assert_eq!(decode_basic_entities("&apos;"), "'");
        assert_eq!(decode_basic_entities("&#x41;"), "A");
    }

    #[test]
    fn href_does_not_steal_hreflang() {
        let html = r#"<html><head><title>T</title>
            <link rel="alternate" hreflang="zh-Hans" href="https://example.com/zh" />
            </head></html>"#;
        let r = inspect_html(html);
        assert_eq!(r.hreflangs.len(), 1);
        assert_eq!(r.hreflangs[0].lang, "zh-Hans");
        assert_eq!(r.hreflangs[0].href, "https://example.com/zh");
    }

    #[test]
    fn entity_in_canonical_href() {
        let html = r#"<html><head><title>T</title>
            <link rel="canonical" href="https://example.com/a?b=1&amp;c=2" />
            </head></html>"#;
        let r = inspect_html(html);
        assert_eq!(
            r.canonical_url.as_deref(),
            Some("https://example.com/a?b=1&c=2")
        );
    }

    #[test]
    fn extracts_viewport_lang_and_image_alt() {
        let html = r#"<html data-x="中文" lang="zh-CN"><head>
            <meta content="width=device-width" name="viewport">
            </head><body><img src="a"><img alt="   " src="b"><img alt="图像" src="c"></body></html>"#;
        let r = inspect_html(html);
        assert!(r.has_viewport);
        assert_eq!(r.html_lang.as_deref(), Some("zh-CN"));
        assert_eq!(r.images_missing_alt, 2);
    }

    #[test]
    fn missing_basics_and_malformed_unicode_do_not_panic() {
        let html = "<html><body>中文<img src='x'><img alt='' src='y'>";
        let r = inspect_html(html);
        assert!(!r.has_viewport);
        assert_eq!(r.html_lang, None);
        assert_eq!(r.images_missing_alt, 2);
        assert!(!extract_viewport("<meta name='viewport'"));
    }
}
