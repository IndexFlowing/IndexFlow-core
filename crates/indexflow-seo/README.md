# indexflow-seo

[![Crates.io](https://img.shields.io/crates/v/indexflow-seo.svg)](https://crates.io/crates/indexflow-seo)
[![Documentation](https://docs.rs/indexflow-seo/badge.svg)](https://docs.rs/indexflow-seo)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/License-MIT%20OR%20Apache--2.0-blue.svg)](LICENSE)

> A blazing-fast, zero-dependency Technical SEO Quality Gate & GEO (Generative Engine Optimization) Auditor written in pure Rust.

[中文文档 (Chinese Documentation)](./README_zh.md)

---

## Key Features

- 🛡️ **Technical SEO Gatekeeper**: Pre-flight validation for HTTP status codes, Canonical declaration equivalence, `noindex`/`nofollow` robots directives, `<title>` tags, and `<h1>` headings.
- 🤖 **GEO & AI Bot Auditing**: Automatically audits crawler exclusion directives for mainstream AI search engines (`GPTBot`, `PerplexityBot`, `ClaudeBot`, `Google-Extended`).
- 📑 **Schema.org Structured Data**: Instant extraction of embedded `application/ld+json` blocks and automatic entity mapping (e.g. `Article`, `FAQPage`, `Product`, `Organization`).
- 🌐 **Social & Multilingual Metadata**: Parses OpenGraph, Twitter Card tags, and `xhtml:link` alternate language (`hreflang`) arrays.
- ⚡ **Pure In-Memory Evaluation**: Zero-cost, memory-safe evaluation function that works directly on HTML strings without mandatory network I/O.
- 🚀 **Optional Non-Redirecting Prober**: Lightweight async HTTP client that treats 3xx redirects as actionable gate issues.

---

## Installation

Add this to your `Cargo.toml`:

```toml
[dependencies]
indexflow-seo = "0.1"
```

### Feature Flags

- `probe` *(default)*: Enables the async HTTP `SeoProbeClient` via `reqwest` (with pure Rustls TLS).

---

## Quick Start

### 1. Pure In-Memory HTML Evaluation (Zero-I/O)

```rust
use indexflow_seo::evaluate_html;

fn main() {
    let page_url = "https://example.com/blog/rust-guide";
    let html = r#"
    <!DOCTYPE html>
    <html lang="en">
    <head>
      <title>Rust Monolith Guide &amp; SEO Best Practices</title>
      <meta name="description" content="A complete technical SEO guide for modern Rust developers." />
      <link rel="canonical" href="https://example.com/blog/rust-guide" />
      <meta name="robots" content="index, follow" />
      
      <!-- AI Bot Directives -->
      <meta name="gptbot" content="index" />
      <meta name="perplexitybot" content="index" />

      <!-- Structured Data -->
      <script type="application/ld+json">
      {
        "@context": "https://schema.org",
        "@type": "Article",
        "headline": "Rust Monolith Guide"
      }
      </script>
    </head>
    <body>
      <h1>Complete Guide to Rust Monolith</h1>
    </body>
    </html>"#;

    let result = evaluate_html(page_url, 200, 25, None, html);

    if result.passed {
        println!("✅ SEO Gate: PASSED");
        println!("Page Title: {:?}", result.page_title);
        println!("H1 Content: {:?}", result.h1_content);
        println!("Schema.org Entities: {:?}", result.schema_types());
    } else {
        println!("❌ SEO Gate: BLOCKED (Reason: {:?})", result.block_reason);
    }
}
```

### 2. Async HTTP Quality Prober (with `probe` feature)

```rust
use indexflow_seo::SeoProbeClient;
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let prober = SeoProbeClient::new(
        "Mozilla/5.0 (compatible; IndexFlowBot/1.0)",
        Duration::from_secs(10),
    )?;

    let result = prober.check_url("https://www.example.com").await;

    println!("Gate Result: passed={}, reason={:?}", result.passed, result.block_reason);
    println!("Response Time: {:?} ms", result.response_time_ms);
    println!("GPTBot Blocked: {}", result.ai_directives.gptbot_blocked);

    Ok(())
}
```

---

## 🛡️ Gate Rules & Criteria

`indexflow-seo` enforces the following pre-flight checks before approving a URL for search engine submission:

1. **HTTP Status**: Must strictly return `200 OK`.
2. **Robots Directives**: Neither `<meta name="robots" content="noindex">` nor `X-Robots-Tag: noindex` header may be present.
3. **Canonical Normalization**: Declared `<link rel="canonical">` must match the actual URL (automatically handles relative paths, default ports 80/443, trailing slashes, and case-insensitivity).
4. **Title Tag**: Must contain a valid, non-empty `<title>` element.

---

## License

Dual-licensed under either of:
- [Apache License, Version 2.0](LICENSE-APACHE)
- [MIT License](LICENSE-MIT)
