use url::Url;

/// 解析纯文本格式的 Sitemap（每行一个 URL）
pub fn parse_plain_text(content: &str) -> Vec<String> {
    content
        .lines()
        .map(|line| line.trim())
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .filter_map(|line| {
            if Url::parse(line).is_ok() {
                Some(line.to_string())
            } else {
                None
            }
        })
        .collect()
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
}