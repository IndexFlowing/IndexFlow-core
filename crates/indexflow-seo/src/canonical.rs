use url::Url;

/// 对比目标 URL 与 Canonical 声明是否等价
/// 自动处理协议大小写、默认端口 80/443、尾部斜杠去除、相对路径与绝对路径解析
pub fn canonical_matches_page(page_url: &str, canonical: &str) -> bool {
    let Ok(page) = Url::parse(page_url) else {
        return normalize_loose(page_url) == normalize_loose(canonical);
    };

    let resolved = match Url::parse(canonical) {
        Ok(abs) => abs,
        Err(_) => match page.join(canonical) {
            Ok(joined) => joined,
            Err(_) => return false,
        },
    };

    normalize_url(&page) == normalize_url(&resolved)
}

pub fn normalize_url(u: &Url) -> String {
    let scheme = u.scheme().to_ascii_lowercase();
    let host = u.host_str().unwrap_or("").to_ascii_lowercase();
    let mut path = u.path().to_string();
    if path.len() > 1 && path.ends_with('/') {
        path.pop();
    }

    let mut out = format!("{scheme}://{host}");
    if let Some(port) = u.port() {
        let default = matches!((scheme.as_str(), port), ("http", 80) | ("https", 443));
        if !default {
            out.push(':');
            out.push_str(&port.to_string());
        }
    }

    out.push_str(&path);
    if let Some(q) = u.query() {
        out.push('?');
        out.push_str(q);
    }
    out
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
}