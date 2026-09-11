// crates/indexflow-seo/src/html_utils.rs

pub const MAX_SCAN_BYTES: usize = 5 * 1024 * 1024;

pub fn clip_html(html: &str) -> &str {
    if html.len() <= MAX_SCAN_BYTES {
        return html;
    }
    let mut end = MAX_SCAN_BYTES;
    while end > 0 && !html.is_char_boundary(end) {
        end -= 1;
    }
    html.get(..end).unwrap_or(html)
}

pub fn clamp_boundary(s: &str, mut i: usize) -> usize {
    if i >= s.len() {
        return s.len();
    }
    while i > 0 && !s.is_char_boundary(i) {
        i -= 1;
    }
    i
}

pub fn safe_slice(html: &str, start: usize, end: usize) -> Option<&str> {
    if start <= end
        && end <= html.len()
        && html.is_char_boundary(start)
        && html.is_char_boundary(end)
    {
        html.get(start..end)
    } else {
        None
    }
}

pub fn find_open_tag(html: &str, from: usize, name_lower: &str, skip_raw: bool) -> Option<usize> {
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

pub fn find_close_tag(html: &str, from: usize, name_lower: &str) -> Option<usize> {
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

pub fn find_tag_end(html: &str, tag_open: usize) -> Option<usize> {
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

pub fn attr_value(tag: &str, want: &str) -> Option<String> {
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

pub fn inner_text(html: &str, tag: &str, skip_raw: bool) -> Option<String> {
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

pub fn strip_tags_and_raw_elements(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut search = 0;
    while search < s.len() {
        if let Some(lt) = s[search..].find('<') {
            let i = search + lt;
            out.push_str(&s[search..i]);
            let after_lt = &s[i + 1..];
            if tag_name_eq(after_lt, "script") || tag_name_eq(after_lt, "style") {
                let tag_name = if tag_name_eq(after_lt, "script") {
                    "script"
                } else {
                    "style"
                };
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

pub fn strip_tags(s: &str) -> String {
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

pub fn normalize_visible_text(s: &str) -> String {
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
        let decoded = match name {
            "amp" | "AMP" => "&",
            "lt" | "LT" => "<",
            "gt" | "GT" => ">",
            "quot" | "QUOT" => "\"",
            "apos" => "'",
            "nbsp" => "\u{a0}",
            "copy" | "COPY" => "©",
            "reg" | "REG" => "®",
            _ => return None,
        };
        Some((j + 1, decoded.to_string()))
    }
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