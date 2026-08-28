//! # indexflow-sitemap
//!
//! A high-performance, fault-tolerant, Google-compliant Sitemap parser and crawler in Rust.
//!
//! ## Features
//! - Standard XML (`<urlset>`, `<sitemapindex>`)
//! - Google extensions (Images, Videos, News, and XHTML Hreflang)
//! - Plain text (`.txt`) sitemap support
//! - Transparent Gzip decompression (`.xml.gz`) with decompression-bomb caps
//! - Circular reference detection and depth protection

pub mod compression;
pub mod error;
pub mod models;
pub mod parser;

#[cfg(feature = "fetch")]
pub mod fetcher;

pub use compression::{decode_if_gzipped, decode_if_gzipped_with_limit};
pub use error::SitemapError;
pub use models::*;
pub use parser::{parse_datetime, parse_priority, parse_sitemap};

#[cfg(feature = "fetch")]
pub use fetcher::SitemapFetcher;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_standard_urlset() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
        <urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
          <url>
            <loc>https://example.com/page-1</loc>
            <lastmod>2026-03-30T10:00:00Z</lastmod>
            <changefreq>weekly</changefreq>
            <priority>0.8</priority>
          </url>
          <url>
            <loc>https://example.com/page-2</loc>
            <lastmod>2026-03-29</lastmod>
            <changefreq>monthly</changefreq>
            <priority>0.5</priority>
          </url>
        </urlset>"#;

        match parse_sitemap(xml) {
            ParsedSitemap::UrlSet { entries } => {
                assert_eq!(entries.len(), 2);

                let first = &entries[0];
                assert_eq!(first.loc, "https://example.com/page-1");
                assert_eq!(first.changefreq, Some(ChangeFreq::Weekly));
                assert_eq!(first.priority, Some(0.8));
                assert!(first.lastmod.is_some());

                let second = &entries[1];
                assert_eq!(second.loc, "https://example.com/page-2");
                assert_eq!(second.changefreq, Some(ChangeFreq::Monthly));
                assert_eq!(second.priority, Some(0.5));
            }
            _ => panic!("Expected ParsedSitemap::UrlSet"),
        }
    }

    #[test]
    fn test_parse_sitemap_index() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
        <sitemapindex xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
          <sitemap>
            <loc>https://example.com/sitemap-posts.xml</loc>
            <lastmod>2026-03-30</lastmod>
          </sitemap>
          <sitemap>
            <loc>https://example.com/sitemap-tags.xml</loc>
            <lastmod>2026-03-29</lastmod>
          </sitemap>
        </sitemapindex>"#;

        match parse_sitemap(xml) {
            ParsedSitemap::Index { child_urls } => {
                assert_eq!(child_urls.len(), 2);
                assert_eq!(child_urls[0], "https://example.com/sitemap-posts.xml");
                assert_eq!(child_urls[1], "https://example.com/sitemap-tags.xml");
            }
            _ => panic!("Expected ParsedSitemap::Index"),
        }
    }

    #[test]
    fn test_parse_google_extensions() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
        <urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9"
                xmlns:image="http://www.google.com/schemas/sitemap-image/1.1"
                xmlns:video="http://www.google.com/schemas/sitemap-video/1.1"
                xmlns:news="http://www.google.com/schemas/sitemap-news/0.9"
                xmlns:xhtml="http://www.w3.org/1999/xhtml">
          <url>
            <loc>https://example.com/post-1</loc>
            <lastmod>2026-03-30T10:00:00Z</lastmod>
            <changefreq>daily</changefreq>
            <priority>0.8</priority>
            <xhtml:link rel="alternate" hreflang="zh" href="https://example.com/zh/post-1"/>
            <image:image>
              <image:loc>https://example.com/cover.jpg</image:loc>
              <image:title>Post Cover</image:title>
              <image:caption>A descriptive caption</image:caption>
            </image:image>
            <video:video>
              <video:thumbnail_loc>https://example.com/thumb.jpg</video:thumbnail_loc>
              <video:title>Tutorial Video</video:title>
              <video:description>How to build with Rust</video:description>
              <video:duration>600</video:duration>
              <video:family_friendly>yes</video:family_friendly>
            </video:video>
            <news:news>
              <news:publication>
                <news:name>Tech Daily</news:name>
                <news:language>en</news:language>
              </news:publication>
              <news:publication_date>2026-03-30</news:publication_date>
              <news:title>Rust Monolith in 2026</news:title>
            </news:news>
          </url>
        </urlset>"#;

        match parse_sitemap(xml) {
            ParsedSitemap::UrlSet { entries } => {
                assert_eq!(entries.len(), 1);
                let entry = &entries[0];
                assert_eq!(entry.loc, "https://example.com/post-1");

                assert_eq!(entry.hreflangs.len(), 1);
                assert_eq!(entry.hreflangs[0].lang, "zh");
                assert_eq!(entry.hreflangs[0].href, "https://example.com/zh/post-1");

                assert_eq!(entry.images.len(), 1);
                assert_eq!(entry.images[0].loc, "https://example.com/cover.jpg");
                assert_eq!(entry.images[0].title.as_deref(), Some("Post Cover"));

                assert_eq!(entry.videos.len(), 1);
                assert_eq!(entry.videos[0].title, "Tutorial Video");
                assert_eq!(entry.videos[0].duration_seconds, Some(600));
                assert_eq!(entry.videos[0].family_friendly, Some(true));

                assert!(entry.news.is_some());
                let news = entry.news.as_ref().unwrap();
                assert_eq!(news.publication_name, "Tech Daily");
                assert_eq!(news.title, "Rust Monolith in 2026");
            }
            _ => panic!("Expected ParsedSitemap::UrlSet"),
        }
    }

    #[test]
    fn test_parse_cdata_and_bom_tolerance() {
        let xml_cdata = "\u{feff}<?xml version=\"1.0\" encoding=\"UTF-8\"?>
        <urlset xmlns=\"http://www.sitemaps.org/schemas/sitemap/0.9\">
          <url>
            <loc><![CDATA[https://example.com/article?id=100&type=tech]]></loc>
            <lastmod><![CDATA[2026-03-30T12:00:00Z]]></lastmod>
          </url>
        </urlset>";

        match parse_sitemap(xml_cdata) {
            ParsedSitemap::UrlSet { entries } => {
                assert_eq!(entries.len(), 1);
                assert_eq!(
                    entries[0].loc,
                    "https://example.com/article?id=100&type=tech"
                );
                assert!(entries[0].lastmod.is_some());
            }
            _ => panic!("Expected ParsedSitemap::UrlSet from CDATA"),
        }
    }

    #[test]
    fn test_parse_plain_text_sitemap() {
        let text_content = r#"
        # Main Sitemap URLs
        https://example.com/home
        https://example.com/about
        
        # Invalid Line Ignored
        not-a-valid-url
        https://example.com/contact
        "#;

        match parse_sitemap(text_content) {
            ParsedSitemap::PlainText { urls } => {
                assert_eq!(urls.len(), 3);
                assert_eq!(urls[0], "https://example.com/home");
                assert_eq!(urls[1], "https://example.com/about");
                assert_eq!(urls[2], "https://example.com/contact");
            }
            _ => panic!("Expected ParsedSitemap::PlainText"),
        }
    }

    #[test]
    fn test_empty_sitemap_detection() {
        let empty_urlset = r#"<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9"></urlset>"#;
        let parsed = parse_sitemap(empty_urlset);
        assert!(parsed.is_empty());

        let empty_index =
            r#"<sitemapindex xmlns="http://www.sitemaps.org/schemas/sitemap/0.9"></sitemapindex>"#;
        let parsed_idx = parse_sitemap(empty_index);
        assert!(parsed_idx.is_empty());
    }
}
