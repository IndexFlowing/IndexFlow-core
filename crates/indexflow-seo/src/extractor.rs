use crate::models::{
    collect_schema_types, AiBotDirectives, HeadingItem, HreflangItem, JsonLdBlock, OpenGraphMeta, TwitterCardMeta,
};

const MAX_SCAN_BYTES: usize = 5 * 1024 * 1024;

#[derive(Debug, Default)]
pub struct RawHtmlInspection {
    pub page_title: Option<String>,
    pub meta_description: Option<String>,
    pub canonical_url: Option<String>,
    pub h1_content: Option<String>,
    pub h1_count: usize,
    pub headings: Vec<HeadingItem>,
    pub word_count: usize,
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

pub fn inspect_html(html: &str) -> RawHtmlInspection {
    let html = clip_html(html);

    let page_title = extract_title(html);
    let headings = extract_headings(html);
    let h1_count = headings.iter().filter(|h| h.level == 1).count();
    let h1_content = headings.iter().find(|h| h.level == 1).map(|h| h.text.clone());
    let word_count = compute_word_count(html);

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
        headings,
        word_count,
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

/// 计算页面正文字数（去除 script/style/tag 后的实际文字数）
fn compute_word_count(html: &str) -> usize {
    let stripped = strip_tags_and_raw_elements(html);
    stripped
        .split_whitespace()
        .map(|token| {
            // 如果是纯 ASCII 单词按 1 个词算，如果是包含 CJK 的东亚文字按字数累加
            let cjk_count = token.chars().filter(|c| is_cjk(*c)).count();
            if cjk_count > 0 {
                cjk_count
            } else {
                1
            }
        })
        .sum()
}

fn is_cjk(c: char) -> bool {
    matches!(c, '\u{4e00}'..='\u{9fff}' | '\u{3400}'..='\u{4dbf}')
}

/// 扫描提取完整 H1 ~ H6 标题大纲树（对齐 Ahrefs 内容大纲）
fn extract_headings(html: &str) -> Vec<HeadingItem> {
    let mut items = Vec::new();
    let tags = ["h1", "h2", "h3", "h4", "h5", "h6"];
    let mut search_from = 0;

    while search_from < html.len() {
        let mut nearest_tag = None;
        let mut nearest_idx = usize::MAX;

        for (level_idx, &t) in tags.iter().enumerate() {
            if let Some(pos) = find_open_tag(html, search_from, t, true) {
                if pos < nearest_idx {
                    nearest_idx = pos;
                    nearest_tag = Some((level_idx as u8 + 1, t));
                }
            }
        }

        let Some((level, tag_name)) = nearest_tag else {
            break;
        };

        let Some(gt) = find_tag_end(html, nearest_idx) else {
            break;
        };
        let after_gt = gt + 1;
        let Some(close_at) = find_close_tag(html, after_gt, tag_name) else {
            search_from = after_gt;
            continue;
        };

        if let Some(raw) = safe_slice(html, after_gt, close_at) {
            let cleaned = normalize_visible_text(&decode_basic_entities(&strip_tags(raw)));
            if !cleaned.is_empty() {
                items.push(HeadingItem {
                    level,
                    text: cleaned,
                });
            }
        }

        search_from = clamp_boundary(html, close_at.saturating_add(tag_name.len() + 3));
    }
    items
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

fn strip_tags_and_raw_elements(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut search = 0;
    while search < s.len() {
        if let Some(lt) = s[search..].find('<') {
            let i = search + lt;
            out.push_str(&s[search..i]);
            let after_lt = &s[i + 1..];
            if tag_name_eq(after_lt, "script") || tag_name_eq(after_lt, "style") {
                let tag_name = if tag_name_eq(after_lt, "script") { "script" } else { "style" };
                if let Some(end_tag) = find_close_tag(s, i, tag_name) {
                    search = end_tag + tag_name.len() + 3;
                    continue;
                }
            }
            if let Some(gt) = find_tag_end(s, i) {
                search = gt + 1;
                out.push(' ');
            } else {
                break;
            }
        } else {
            out.push_str(&s[search..]);
            break;
        }
    }
    out
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

fn safe_slice(html: &str, start: usize, end: usize) -> Option<&str> {
    if start <= end && end <= html.len() && html.is_char_boundary(start) && html.is_char_boundary(end)
    {
        html.get(start..end)
    } else {
        None
    }
}

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
        "copy" | "COPY" => "©",
        "reg" | "REG" => "®",
        _ => return None,
    })
}