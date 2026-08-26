//! # indexflow-seo
//!
//! A high-performance, lightweight Technical SEO quality gate & GEO (Generative Engine Optimization) auditor.
//!
//! ## Features
//! - Zero-dependency HTML evaluation (HTTP Status, Canonical, noindex, Title, H1, Meta Description)
//! - JSON-LD (Schema.org) structured data extraction and entity mapping
//! - OpenGraph & Twitter Card social meta parsing
//! - AI Search Engine Bot directive auditing (GPTBot, Perplexity, Claude, Google-Extended)
//! - Non-redirecting fast HTTP prober client (Redirects treated as gate issues)

pub mod canonical;
pub mod evaluator;
pub mod extractor;
pub mod models;

#[cfg(feature = "probe")]
pub mod probe;

pub use canonical::{canonical_matches_page, normalize_url};
pub use evaluator::evaluate_html;
pub use extractor::{decode_basic_entities, inspect_html};
pub use models::*;

#[cfg(feature = "probe")]
pub use probe::SeoProbeClient;

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_HTML_FULL: &str = r#"<!DOCTYPE html>
    <html lang="en">
    <head>
      <meta charset="UTF-8">
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
        let res = evaluate_html("https://example.com/guide", 200, 32, None, SAMPLE_HTML_FULL);

        assert!(res.passed);
        assert_eq!(res.block_reason, None);
        assert_eq!(res.http_status, Some(200));
        assert_eq!(
            res.page_title.as_deref(),
            Some("Rust Monolith Guide & SEO Best Practices")
        );
        assert_eq!(
            res.meta_description.as_deref(),
            Some("A complete technical SEO guide for modern Rust developers.")
        );
        assert_eq!(res.h1_content.as_deref(), Some("Complete Rust Guide"));
        assert_eq!(res.h1_count, 1);
        assert!(res.has_canonical);
        assert_eq!(res.canonical_url.as_deref(), Some("https://example.com/guide"));
        assert!(!res.has_noindex);
        assert!(!res.has_nofollow);
        assert_eq!(res.hreflang.len(), 2);

        // OpenGraph & Twitter
        assert_eq!(res.opengraph.title.as_deref(), Some("Rust Monolith Guide"));
        assert_eq!(res.opengraph.og_type.as_deref(), Some("article"));
        assert_eq!(res.opengraph.image.as_deref(), Some("https://example.com/cover.jpg"));
        assert_eq!(res.twitter_card.card.as_deref(), Some("summary_large_image"));

        // AI Bot Directives
        assert!(res.ai_directives.gptbot_blocked);
        assert!(!res.ai_directives.perplexity_blocked);

        // JSON-LD Schemas
        assert_eq!(res.json_ld.len(), 2);
        assert_eq!(res.schema_types(), vec!["Article".to_string(), "FAQPage".to_string()]);
    }

    #[test]
    fn test_gate_block_http_non_200() {
        let res = evaluate_html("https://example.com/404", 404, 15, None, "<html><head><title>Not Found</title></head></html>");
        assert!(!res.passed);
        assert_eq!(res.block_reason.as_deref(), Some("HTTP 404"));
    }

    #[test]
    fn test_gate_block_meta_noindex() {
        let html = r#"<html><head><title>Draft Page</title><meta name="robots" content="noindex, nofollow" /></head></html>"#;
        let res = evaluate_html("https://example.com/draft", 200, 20, None, html);
        assert!(!res.passed);
        assert!(res.has_noindex);
        assert!(res.has_nofollow);
        assert_eq!(res.block_reason.as_deref(), Some("noindex directive present"));
    }

    #[test]
    fn test_gate_block_x_robots_tag_header() {
        let html = r#"<html><head><title>Valid Title</title></head></html>"#;
        let res = evaluate_html("https://example.com/page", 200, 20, Some("noindex, noarchive"), html);
        assert!(!res.passed);
        assert!(res.has_noindex);
        assert_eq!(res.block_reason.as_deref(), Some("noindex directive present"));
    }

    #[test]
    fn test_gate_block_missing_title() {
        let html = r#"<html><head><meta name="description" content="No title here" /></head><body><h1>Hello</h1></body></html>"#;
        let res = evaluate_html("https://example.com/no-title", 200, 20, None, html);
        assert!(!res.passed);
        assert_eq!(res.block_reason.as_deref(), Some("Missing <title> tag"));
    }

    #[test]
    fn test_gate_block_canonical_mismatch() {
        let html = r#"<html><head><title>Duplicate Post</title><link rel="canonical" href="https://example.com/original-post" /></head></html>"#;
        let res = evaluate_html("https://example.com/duplicate-post", 200, 20, None, html);
        assert!(!res.passed);
        assert_eq!(
            res.block_reason.as_deref(),
            Some("Canonical URL mismatch: https://example.com/original-post")
        );
    }

    #[test]
    fn test_canonical_fuzzy_match() {
        // Trailing slash loose match
        assert!(canonical_matches_page(
            "https://example.com/blog/post-1/",
            "https://example.com/blog/post-1"
        ));
        // Port loose match
        assert!(canonical_matches_page(
            "https://example.com:443/blog/post-1",
            "https://example.com/blog/post-1"
        ));
        // Relative path resolution
        assert!(canonical_matches_page(
            "https://example.com/blog/post-1",
            "/blog/post-1"
        ));
        // Mismatch
        assert!(!canonical_matches_page(
            "https://example.com/blog/post-1",
            "https://example.com/blog/other"
        ));
    }

    #[test]
    fn test_entity_decoding() {
        assert_eq!(decode_basic_entities("Tom &amp; Jerry &#39;Special&#39;"), "Tom & Jerry 'Special'");
        assert_eq!(decode_basic_entities("&lt;div&gt;&quot;Hello&quot;&nbsp;World&lt;/div&gt;"), "<div>\"Hello\" World</div>");
    }
}