# indexflow-sitemap

[![Crates.io](https://img.shields.io/crates/v/indexflow-sitemap.svg)](https://crates.io/crates/indexflow-sitemap)
[![Documentation](https://docs.rs/indexflow-sitemap/badge.svg)](https://docs.rs/indexflow-sitemap)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/License-MIT%20OR%20Apache--2.0-blue.svg)](LICENSE)

> A blazing-fast, fault-tolerant, streaming Google-compliant Sitemap parser and crawler written in pure Rust.

[中文文档 (Chinese Documentation)](./README_zh.md)

---

## Features

- 📑 **Standard XML Sitemaps**: Full support for `<urlset>` (URL collections) and `<sitemapindex>` (recursive sitemap index trees).
- 🔍 **Google Extensions**: Native extraction of Google Images (`<image:image>`), Google Videos (`<video:video>`), Google News (`<news:news>`), and alternate multilingual links (`<xhtml:link rel="alternate" hreflang="...">`).
- 📄 **Plain Text Support**: Automatic detection and parsing of `.txt` sitemap files (one URL per line).
- 🗜️ **Transparent Gzip Decompression**: Automatic sniffing of gzip magic headers (`0x1F, 0x8B`) and instant decompression of `.xml.gz` files.
- 🛡️ **Depth & Circular Reference Protection**: Guard against infinite loops, sitemap recursion traps, and depth overflow.
- ⚡ **Fault-Tolerant Parsing**: Resilient against UTF-8 BOM headers, unescaped entities, and nested `<![CDATA[...]]>` tags.

---

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
indexflow-sitemap = { version = "0.1", features = ["fetch", "gzip"] }
```

---

## Quick Start

### 1. In-Memory XML String Parsing

```rust
use indexflow_sitemap::{parse_sitemap, ParsedSitemap};

fn main() {
    let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
    <urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9"
            xmlns:xhtml="http://www.w3.org/1999/xhtml">
      <url>
        <loc>https://example.com/blog/rust-guide</loc>
        <lastmod>2026-03-30T10:00:00Z</lastmod>
        <changefreq>daily</changefreq>
        <priority>0.8</priority>
        <xhtml:link rel="alternate" hreflang="zh" href="https://example.com/zh/blog/rust-guide"/>
      </url>
    </urlset>"#;

    match parse_sitemap(xml) {
        ParsedSitemap::UrlSet { entries } => {
            for entry in entries {
                println!("URL: {}", entry.loc);
                println!("Lastmod: {:?}", entry.lastmod);
                println!("Priority: {:?}", entry.priority);
                println!("Hreflang count: {}", entry.hreflangs.len());
            }
        }
        ParsedSitemap::Index { child_urls } => {
            println!("Sitemap Index detected with {} child sitemaps.", child_urls.len());
        }
        ParsedSitemap::PlainText { urls } => {
            println!("Plain text sitemap with {} URLs.", urls.len());
        }
    }
}
```

### 2. Recursive Async Fetch & Index Tree Expansion

```rust
use indexflow_sitemap::SitemapFetcher;
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(15))
        .user_agent("Mozilla/5.0 (compatible; IndexFlowBot/1.0)")
        .build()?;

    let fetcher = SitemapFetcher::new(client);

    // Recursively expand all sitemap indexes up to 3 levels deep
    let target = "https://www.example.com/sitemap_index.xml";
    let (is_index_tree, all_page_entries) = fetcher.expand_all(target, 3).await?;

    println!("Is Sitemap Index: {}", is_index_tree);
    println!("Total discovered page URLs: {}", all_page_entries.len());

    Ok(())
}
```

---

## License

Dual-licensed under either of:
- [Apache License, Version 2.0](LICENSE-APACHE)
- [MIT License](LICENSE-MIT)

