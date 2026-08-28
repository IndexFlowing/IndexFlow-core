use crate::models::MAX_URL_LEN;
use url::Url;

/// Parse a line-oriented text sitemap (`Sitemap: URL` is *not* robots.txt).
///
/// Rules (fault-tolerant, never panics):
/// * UTF-8 BOM and surrounding whitespace are ignored.
/// * `#` starts a comment (full-line only).
/// * Only absolute `http` / `https` URLs of length ≤ [`MAX_URL_LEN`] are kept.
/// * Empty lines and unparseable tokens are skipped.
pub fn parse_plain_text(content: &str) -> Vec<String> {
    let content = content.trim_start_matches('\u{feff}');
    let mut urls = Vec::new();
    for raw in content.lines() {
        let line = raw.trim().trim_end_matches('\r');
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        // Tolerate a trailing inline comment: `https://x.com/  # home`
        let token = line
            .split_once('#')
            .map(|(url, _)| url.trim())
            .unwrap_or(line);
        if token.is_empty() || token.len() > MAX_URL_LEN {
            continue;
        }
        if is_http_url(token) {
            urls.push(token.to_string());
        }
    }
    urls
}

fn is_http_url(s: &str) -> bool {
    match Url::parse(s) {
        Ok(u) => matches!(u.scheme(), "http" | "https") && u.host_str().is_some(),
        Err(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_plain_text() {
        let txt = "
        https://example.com/page1
        # this is a comment
        https://example.com/page2
        not-a-valid-url
        ";
        let urls = parse_plain_text(txt);
        assert_eq!(urls.len(), 2);
        assert_eq!(urls[0], "https://example.com/page1");
        assert_eq!(urls[1], "https://example.com/page2");
    }

    #[test]
    fn bom_ftp_javascript_and_inline_comments_are_filtered() {
        let txt = "\u{feff}https://example.com/a\n\
                   ftp://files.example.com/x\n\
                   javascript:alert(1)\n\
                   https://example.com/b  # trailing comment\n\
                   http://localhost/ok\n";
        let urls = parse_plain_text(txt);
        assert_eq!(
            urls,
            vec![
                "https://example.com/a".to_string(),
                "https://example.com/b".to_string(),
                "http://localhost/ok".to_string(),
            ]
        );
    }

    #[test]
    fn oversized_line_is_dropped() {
        let long = format!("https://example.com/{}", "a".repeat(3000));
        assert!(parse_plain_text(&long).is_empty());
    }
}
