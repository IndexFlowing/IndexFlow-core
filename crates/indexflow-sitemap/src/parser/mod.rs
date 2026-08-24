pub mod text;
pub mod xml;

use crate::models::ParsedSitemap;
use text::parse_plain_text;
use xml::parse_xml;

/// 自动探测内容类型（XML 或 PlainText）并解析
pub fn parse_sitemap(content: &str) -> ParsedSitemap {
    let trimmed = content.trim_start();
    if trimmed.starts_with("<?xml") || trimmed.starts_with("<urlset") || trimmed.starts_with("<sitemapindex") {
        parse_xml(content)
    } else {
        let urls = parse_plain_text(content);
        ParsedSitemap::PlainText { urls }
    }
}