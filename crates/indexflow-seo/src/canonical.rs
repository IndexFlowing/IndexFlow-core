//! Canonical URL equivalence.
//!
//! Two URLs compare equal after:
//! * scheme / host lowercasing
//! * dropping default ports (`:80` / `:443`)
//! * resolving `.` / `..` path segments
//! * percent-decoding path segments (UTF-8, lossy)
//! * stripping a trailing slash (except `/`)
//! * sorting query parameters by `(key, value)`
//! * dropping the fragment
//!
//! Relative and protocol-relative (`//host/path`) canonicals are resolved
//! against the page URL. The path is **not** lowercased (it is case-sensitive).

use url::Url;

/// Compare a page URL with its declared canonical (absolute, relative, or
/// protocol-relative). Never panics.
pub fn canonical_matches_page(page_url: &str, canonical: &str) -> bool {
    let page_url = page_url.trim();
    let canonical = canonical.trim();
    if page_url.is_empty() || canonical.is_empty() {
        return false;
    }

    let Ok(page) = Url::parse(page_url) else {
        return normalize_loose(page_url) == normalize_loose(canonical);
    };

    let resolved = match resolve_canonical(&page, canonical) {
        Some(u) => u,
        None => return false,
    };

    normalize_url(&page) == normalize_url(&resolved)
}

fn resolve_canonical(page: &Url, canonical: &str) -> Option<Url> {
    if let Ok(abs) = Url::parse(canonical) {
        return Some(abs);
    }
    // Protocol-relative: `//cdn.example.com/x`
    if canonical.starts_with("//") {
        let filled = format!("{}:{canonical}", page.scheme());
        return Url::parse(&filled).ok();
    }
    page.join(canonical).ok()
}

/// Produce a comparison key for an already-parsed URL.
pub fn normalize_url(u: &Url) -> String {
    let scheme = u.scheme().to_ascii_lowercase();
    let host = u.host_str().unwrap_or("").to_ascii_lowercase();
    let path = normalize_path(u.path());

    let mut out = String::with_capacity(scheme.len() + host.len() + path.len() + 16);
    out.push_str(&scheme);
    out.push_str("://");
    out.push_str(&host);

    if let Some(port) = u.port() {
        let default = matches!((scheme.as_str(), port), ("http", 80) | ("https", 443));
        if !default {
            out.push(':');
            // `itoa`-free: a port fits in 5 decimal digits.
            let mut buf = [0u8; 5];
            let mut n = port;
            let mut i = 5;
            if n == 0 {
                out.push('0');
            } else {
                while n > 0 && i > 0 {
                    i -= 1;
                    buf[i] = b'0' + (n % 10) as u8;
                    n /= 10;
                }
                if let Ok(s) = std::str::from_utf8(&buf[i..]) {
                    out.push_str(s);
                }
            }
        }
    }

    out.push_str(&path);
    if let Some(q) = sorted_query(u) {
        out.push('?');
        out.push_str(&q);
    }
    out
}

fn normalize_path(path: &str) -> String {
    let mut parts: Vec<String> = Vec::new();
    for seg in path.split('/') {
        if seg.is_empty() || seg == "." {
            continue;
        }
        if seg == ".." {
            parts.pop();
            continue;
        }
        parts.push(percent_decode(seg));
    }
    let mut out = String::from("/");
    out.push_str(&parts.join("/"));
    if out.len() > 1 && out.ends_with('/') {
        out.pop();
    }
    out
}

fn percent_decode(seg: &str) -> String {
    let bytes = seg.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let (Some(h), Some(l)) = (from_hex(bytes[i + 1]), from_hex(bytes[i + 2])) {
                out.push((h << 4) | l);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    match String::from_utf8(out) {
        Ok(s) => s,
        Err(e) => String::from_utf8_lossy(e.as_bytes()).into_owned(),
    }
}

fn from_hex(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

fn sorted_query(u: &Url) -> Option<String> {
    let mut pairs: Vec<(String, String)> = u
        .query_pairs()
        .map(|(k, v)| (k.into_owned(), v.into_owned()))
        .collect();
    if pairs.is_empty() {
        return None;
    }
    pairs.sort_unstable();
    let mut s = String::new();
    for (i, (k, v)) in pairs.iter().enumerate() {
        if i > 0 {
            s.push('&');
        }
        s.push_str(k);
        s.push('=');
        s.push_str(v);
    }
    Some(s)
}

fn normalize_loose(s: &str) -> String {
    s.trim().trim_end_matches('/').to_ascii_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_canonical_normalization() {
        assert!(canonical_matches_page(
            "https://example.com/blog/post-1/",
            "https://example.com/blog/post-1"
        ));
        assert!(canonical_matches_page(
            "https://example.com:443/blog/post-1",
            "https://example.com/blog/post-1"
        ));
        assert!(canonical_matches_page(
            "https://example.com/blog/post-1",
            "/blog/post-1"
        ));
        assert!(!canonical_matches_page(
            "https://example.com/blog/post-1",
            "https://example.com/blog/other-post"
        ));
    }

    #[test]
    fn scheme_host_case_and_default_http_port() {
        assert!(canonical_matches_page(
            "HTTPS://EXAMPLE.COM/Foo",
            "https://example.com/Foo"
        ));
        assert!(canonical_matches_page(
            "http://example.com:80/x",
            "http://example.com/x"
        ));
        // Path is case-sensitive.
        assert!(!canonical_matches_page(
            "https://example.com/Foo",
            "https://example.com/foo"
        ));
    }

    #[test]
    fn relative_dotdot_and_protocol_relative() {
        // RFC 3986: a path without a trailing slash treats the last segment as
        // a file, so `../` climbs out of `/blog/`, not out of `/blog/post-1`.
        assert!(canonical_matches_page(
            "https://example.com/blog/post-1",
            "./post-1"
        ));
        assert!(canonical_matches_page(
            "https://example.com/a/b/page",
            "../b/page"
        ));
        assert!(canonical_matches_page(
            "https://example.com/blog/post-1",
            "//example.com/blog/post-1"
        ));
        assert!(canonical_matches_page(
            "https://example.com/a/./b",
            "https://example.com/a/b"
        ));
        // `../post-1` from `/blog/post-1` resolves to `/post-1` — a mismatch.
        assert!(!canonical_matches_page(
            "https://example.com/blog/post-1",
            "../post-1"
        ));
    }

    #[test]
    fn query_param_order_is_insignificant() {
        assert!(canonical_matches_page(
            "https://example.com/search?b=2&a=1",
            "https://example.com/search?a=1&b=2"
        ));
        assert!(!canonical_matches_page(
            "https://example.com/search?a=1",
            "https://example.com/search?a=2"
        ));
    }

    #[test]
    fn percent_encoding_and_tilde() {
        assert!(canonical_matches_page(
            "https://example.com/~user",
            "https://example.com/%7Euser"
        ));
    }

    #[test]
    fn fragment_is_ignored() {
        assert!(canonical_matches_page(
            "https://example.com/p#section",
            "https://example.com/p"
        ));
    }

    #[test]
    fn empty_and_garbage_never_panic() {
        assert!(!canonical_matches_page("", "/x"));
        assert!(!canonical_matches_page("https://example.com/", ""));
        assert!(!canonical_matches_page("not a url", "also not"));
        assert!(canonical_matches_page("not-a-url", "not-a-url"));
    }
}
