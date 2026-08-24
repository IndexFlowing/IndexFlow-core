//! # indexflow-sitemap
//!
//! A high-performance, fault-tolerant, Google-compliant Sitemap parser and crawler in Rust.
//!
//! ## Features
//! - Standard XML (`<urlset>`, `<sitemapindex>`)
//! - Google extensions (Images, Videos, News, and XHTML Hreflang)
//! - Plain text (`.txt`) sitemap support
//! - Transparent Gzip decompression (`.xml.gz`)
//! - Circular reference detection and depth protection

pub mod compression;
pub mod error;
pub mod models;
pub mod parser;

#[cfg(feature = "fetch")]
pub mod fetcher;

pub use compression::decode_if_gzipped;
pub use error::SitemapError;
pub use models::*;
pub use parser::parse_sitemap;

#[cfg(feature = "fetch")]
pub use fetcher::SitemapFetcher;

#[cfg(test)]
mod tests {
    use super::*;

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
            <xhtml:link rel="alternate" hreflang="en" href="https://example.com/en/post-1"/>
            <image:image>
              <image:loc>https://example.com/cover.jpg</image:loc>
              <image:title>Post Cover</image:title>
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
                assert_eq!(entry.changefreq, Some(ChangeFreq::Daily));
                assert_eq!(entry.priority, Some(0.8));
                assert_eq!(entry.hreflangs.len(), 1);
                assert_eq!(entry.images.len(), 1);
                assert_eq!(entry.images[0].loc, "https://example.com/cover.jpg");
                assert_eq!(entry.videos.len(), 1);
                assert_eq!(entry.videos[0].title, "Tutorial Video");
                assert_eq!(entry.videos[0].duration_seconds, Some(600));
                assert_eq!(entry.videos[0].family_friendly, Some(true));
                assert!(entry.news.is_some());
                assert_eq!(entry.news.as_ref().unwrap().publication_name, "Tech Daily");
            }
            _ => panic!("Expected UrlSet"),
        }
    }
}