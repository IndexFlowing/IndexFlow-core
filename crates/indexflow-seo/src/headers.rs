// crates/indexflow-seo/src/headers.rs
use crate::models::{AiBotDirectives, HttpLinkHeader};

#[derive(Debug, Clone, Copy, Default)]
pub struct RobotsTokens {
    pub noindex: bool,
    pub nofollow: bool,
    pub none: bool,
    pub noai: bool,
}

impl RobotsTokens {
    pub fn parse(content: &str) -> Self {
        let mut t = Self::default();
        for tok in content.split([',', ';']).flat_map(|p| p.split_whitespace()).map(str::trim) {
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

/// 解析 HTTP 响应头中的 Link 标头 (RFC 5988 标准)
pub fn parse_http_link_header(header_val: &str) -> Vec<HttpLinkHeader> {
    let mut links = Vec::new();
    for part in header_val.split(',') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }

        let Some(start) = part.find('<') else { continue };
        let Some(end) = part.find('>') else { continue };
        if start >= end {
            continue;
        }

        let uri = part[start + 1..end].trim().to_string();
        let params_str = &part[end + 1..];

        let mut rel = String::new();
        let mut hreflang = None;

        for param in params_str.split(';') {
            let param = param.trim();
            if let Some((k, v)) = param.split_once('=') {
                let k = k.trim().to_ascii_lowercase();
                let v = v.trim().trim_matches('"').trim_matches('\'').to_string();
                if k == "rel" {
                    rel = v.to_ascii_lowercase();
                } else if k == "hreflang" {
                    hreflang = Some(v);
                }
            }
        }

        if !uri.is_empty() && !rel.is_empty() {
            links.push(HttpLinkHeader { uri, rel, hreflang });
        }
    }
    links
}

/// 解析 HTTP 响应头中的 X-Robots-Tag 标头
pub fn parse_x_robots_header(header: &str) -> (RobotsTokens, AiBotDirectives) {
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