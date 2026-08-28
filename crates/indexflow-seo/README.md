# indexflow-seo

[![Crates.io](https://img.shields.io/crates/v/indexflow-seo.svg)](https://crates.io/crates/indexflow-seo)
[![Documentation](https://docs.rs/indexflow-seo/badge.svg)](https://docs.rs/indexflow-seo)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/License-MIT%20OR%20Apache--2.0-blue.svg)](LICENSE)

> A blazing-fast, zero-dependency Technical SEO Quality Gate & GEO (Generative Engine Optimization) Auditor written in pure Rust.

[中文文档 (Chinese Documentation)](./README_ZH.md)

---

## Key Features

- 🛡️ **Technical SEO Gatekeeper**: Pre-flight validation for HTTP status codes, Canonical declaration equivalence, `noindex`/`nofollow` robots directives, `<title>` tags, and `<h1>` headings.
- 🤖 **GEO & AI Bot Auditing**: Audits crawler exclusion directives for `GPTBot` / `ChatGPT-User`, `PerplexityBot`, `ClaudeBot` / `anthropic-ai`, and `Google-Extended` — including per-bot `X-Robots-Tag` headers and `none` / `noai`.
- 📑 **Schema.org Structured Data**: Extracts `application/ld+json` blocks, expands `@graph` and top-level arrays, and maps `@type` (string or array).
- 🌐 **Social & Multilingual Metadata**: Parses OpenGraph, Twitter Card tags, and `link rel="alternate" hreflang` arrays.
- ⚡ **Pure In-Memory Evaluation**: Char-boundary-safe HTML scanner (CJK / emoji never panic). Quote-aware tags, unquoted attributes, multiline meta, HTML comments and `<script>`/`<style>` skipped for visible tags.
- 🚀 **Optional Non-Redirecting Prober**: Lightweight async HTTP client that treats 3xx redirects as actionable gate issues, with a 5 MiB body cap.

---

## Installation

Add this to your `Cargo.toml`:

```toml
[dependencies]
indexflow-seo = "0.1.2"
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
2. **Robots Directives**: Neither `<meta name="robots" content="noindex">` (or `none`) nor `X-Robots-Tag: noindex` may be present.
3. **Canonical Normalization**: Declared `<link rel="canonical">` must match the page URL. Handles relative and protocol-relative paths, `.` / `..` segments, default ports 80/443, trailing slashes, scheme/host case, query-parameter order, and percent-encoding. Path case is preserved (case-sensitive).
4. **Title Tag**: Must contain a valid, non-empty `<title>` element.

---

## Changelog

### 0.1.2

- Char-boundary-safe HTML scanner: CJK / emoji, quote-aware `>`, unquoted URL attributes, multiline meta, comments and raw `<script>`/`<style>` skipped for visible tags.
- JSON-LD expands `@graph`, top-level arrays, and array-typed `@type`; CDATA wrappers tolerated.
- Single-pass entity decode (HTML named Latin-1 + `&#N;` / `&#xN;`); no double-decode of `&amp;lt;`.
- Canonical matching: sorted query params, `../` per RFC 3986, protocol-relative URLs, `%7E` ≡ `~`.
- GEO: AI-bot aliases, `none` / `noai`, per-bot `X-Robots-Tag`; probe body capped at 5 MiB.

---

## License

Dual-licensed under either of:
- [Apache License, Version 2.0](LICENSE-APACHE)
- [MIT License](LICENSE-MIT)
