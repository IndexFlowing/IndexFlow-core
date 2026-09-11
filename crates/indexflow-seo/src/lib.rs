// crates/indexflow-seo/src/lib.rs
pub mod canonical;
pub mod content;
pub mod evaluator;
pub mod extractor;
pub mod gate;
pub mod headers;
pub mod html_utils;
pub mod meta;
pub mod models;
pub mod schema;
pub mod warnings;

#[cfg(feature = "probe")]
pub mod probe;

pub use canonical::{canonical_matches_page, normalize_url};
pub use content::{compute_word_count, extract_headings};
pub use evaluator::evaluate_html;
pub use extractor::{inspect_html, RawHtmlInspection};
pub use gate::evaluate_gate_block;
pub use headers::{parse_http_link_header, parse_x_robots_header, RobotsTokens};
pub use html_utils::decode_basic_entities;
pub use meta::{count_images_missing_alt, extract_html_lang, extract_viewport};
pub use models::*;
pub use schema::{extract_json_ld, extract_video_objects};
pub use warnings::compute_warnings;

#[cfg(feature = "probe")]
pub use probe::SeoProbeClient;

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_HTML_FULL: &str = r#"<!DOCTYPE html>
    <html lang="en">
    <head>
      <meta charset="UTF-8">
      <meta name="viewport" content="width=device-width, initial-scale=1">
      <title>Rust Monolith Guide &amp; SEO Best Practices</title>
      <meta name="description" content="A complete technical SEO guide for modern Rust developers." />
      <meta name="robots" content="index, follow" />
      <link rel="canonical" href="https://example.com/guide" />
      <link rel="alternate" hreflang="zh" href="https://example.com/zh/guide" />
      <link rel="alternate" hreflang="en" href="https://example.com/guide" />
      
      <!-- Social Metadata -->
      <meta property="og:title" content="Rust Monolith Guide" />
      <meta property="og:description" content="Social preview description." />
      <meta property="og:type" content="article" />
      <meta property="og:image" content="https://example.com/cover.jpg" />
      <meta name="twitter:card" content="summary_large_image" />
      
      <!-- AI Bot Directives -->
      <meta name="gptbot" content="noindex" />
      <meta name="perplexitybot" content="index" />
      
      <!-- JSON-LD Schema.org -->
      <script type="application/ld+json">
      {
        "@context": "https://schema.org",
        "@type": "Article",
        "headline": "Rust Monolith Guide",
        "author": {
          "@type": "Person",
          "name": "IndexFlow Team"
        }
      }
      </script>
      <script type="application/ld+json">
      {
        "@context": "https://schema.org",
        "@type": "FAQPage",
        "mainEntity": []
      }
      </script>
    </head>
    <body>
      <h1>Complete Rust Guide</h1>
      <p>Body paragraph</p>
    </body>
    </html>"#;

    #[test]
    fn test_full_inspection_pass() {
        let res = evaluate_html(
            "https://example.com/guide",
            200,
            32,
            None,
            None,
            None,
            SAMPLE_HTML_FULL,
        );

        assert!(res.passed);
        assert_eq!(res.block_reason, None);
        assert_eq!(res.http_status, Some(200));
        assert_eq!(
            res.page_title.as_deref(),
            Some("Rust Monolith Guide & SEO Best Practices")
        );
        assert_eq!(res.h1_content.as_deref(), Some("Complete Rust Guide"));
        assert_eq!(res.h1_count, 1);
        assert!(res.has_canonical);
        assert_eq!(res.canonical_url.as_deref(), Some("https://example.com/guide"));
        assert!(!res.has_noindex);
        assert!(!res.has_nofollow);
        assert_eq!(res.hreflang.len(), 2);
        assert_eq!(res.json_ld.len(), 2);
    }

    #[test]
    fn test_video_schema_self_reference_block() {
        let html_with_bad_video = r#"<!DOCTYPE html>
        <html>
        <head>
          <title>AI Video Generator - Example</title>
          <link rel="canonical" href="https://example.com/video/123" />
          <script type="application/ld+json">
          {
            "@context": "https://schema.org",
            "@type": "VideoObject",
            "name": "Awesome Video",
            "embedUrl": "https://example.com/video/123",
            "contentUrl": "https://cdn.example.com/stream.mp4"
          }
          </script>
        </head>
        <body><h1>Video Title</h1></body>
        </html>"#;

        let res = evaluate_html(
            "https://example.com/video/123",
            200,
            10,
            None,
            None,
            None,
            html_with_bad_video,
        );

        assert!(!res.passed);
        assert!(res
            .block_reason
            .as_deref()
            .unwrap_or("")
            .contains("VideoObject #0 语义自环"));
    }

    #[test]
    fn test_cross_layer_header_canonical_conflict() {
        let html = r#"<html><head><title>Test</title><link rel="canonical" href="https://example.com/a" /></head></html>"#;
        let link_hdr = r#"<https://example.com/b>; rel="canonical""#;

        let res = evaluate_html(
            "https://example.com/a",
            200,
            10,
            None,
            None,
            Some(link_hdr),
            html,
        );

        assert!(!res.passed);
        assert!(res
            .block_reason
            .as_deref()
            .unwrap_or("")
            .contains("跨层 Canonical 冲突"));
    }

    #[test]
    fn test_redirect_semantics_and_loop() {
        let res_missing_loc = evaluate_html("https://example.com/old", 301, 10, None, None, None, "");
        assert!(!res_missing_loc.passed);
        assert!(res_missing_loc
            .block_reason
            .as_deref()
            .unwrap_or("")
            .contains("FATAL_MALFORMED_REDIRECT"));

        let res_self_loop = evaluate_html(
            "https://example.com/loop",
            301,
            10,
            None,
            Some("https://example.com/loop"),
            None,
            "",
        );
        assert!(!res_self_loop.passed);
        assert!(res_self_loop
            .block_reason
            .as_deref()
            .unwrap_or("")
            .contains("重定向目标指向自身"));

        let res_307 = evaluate_html(
            "https://example.com/temp",
            307,
            10,
            None,
            Some("https://example.com/target"),
            None,
            "",
        );
        assert!(!res_307.passed);
        assert!(res_307.warnings.iter().any(|w| w.contains("WARN_TEMPORARY_REDIRECT_SEO_LEAK")));
    }
}