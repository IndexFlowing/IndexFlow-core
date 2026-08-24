//! # indexflow-seo
//!
//! A high-performance, lightweight Technical SEO quality gate & GEO (Generative Engine Optimization) auditor.
//!
//! ## Features
//! - Zero-dependency HTML evaluation (Status, Canonical, noindex, Title, H1)
//! - JSON-LD (Schema.org) structured data extraction
//! - OpenGraph & Twitter Card social meta parsing
//! - AI Search Engine Bot directive auditing (GPTBot, Perplexity, Claude, Google-Extended)
//! - Non-redirecting fast HTTP prober client

pub mod canonical;
pub mod evaluator;
pub mod extractor;
pub mod models;

#[cfg(feature = "probe")]
pub mod probe;

pub use canonical::canonical_matches_page;
pub use evaluator::evaluate_html;
pub use models::*;

#[cfg(feature = "probe")]
pub use probe::SeoProbeClient;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_full_geo_and_seo_inspection() {
        let html = r#"<!DOCTYPE html>
        <html>
        <head>
          <title>Rust Monolith Guide &amp; Tips</title>
          <meta name="description" content="A complete guide to Rust Monolith." />
          <link rel="canonical" href="https://example.com/guide" />
          <meta property="og:title" content="Rust Monolith Guide" />
          <meta property="og:type" content="article" />
          <meta name="twitter:card" content="summary_large_image" />
          <script type="application/ld+json">
          {
            "@context": "https://schema.org",
            "@type": "Article",
            "headline": "Rust Monolith Guide"
          }
          </script>
        </head>
        <body>
          <h1>Complete Rust Guide</h1>
        </body>
        </html>"#;

        let res = evaluate_html("https://example.com/guide", 200, 25, None, html);

        assert!(res.passed);
        assert_eq!(res.page_title.as_deref(), Some("Rust Monolith Guide & Tips"));
        assert_eq!(res.h1_content.as_deref(), Some("Complete Rust Guide"));
        assert_eq!(res.h1_count, 1);
        assert!(res.has_canonical);
        assert_eq!(res.opengraph.title.as_deref(), Some("Rust Monolith Guide"));
        assert_eq!(res.opengraph.og_type.as_deref(), Some("article"));
        assert_eq!(res.twitter_card.card.as_deref(), Some("summary_large_image"));
        assert_eq!(res.schema_types(), vec!["Article".to_string()]);
    }
}