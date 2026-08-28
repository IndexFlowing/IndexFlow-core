# indexflow-sitemap

[![Crates.io](https://img.shields.io/crates/v/indexflow-sitemap.svg)](https://crates.io/crates/indexflow-sitemap)
[![Documentation](https://docs.rs/indexflow-sitemap/badge.svg)](https://docs.rs/indexflow-sitemap)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/License-MIT%20OR%20Apache--2.0-blue.svg)](LICENSE)

> A blazing-fast, fault-tolerant, Google-compliant Sitemap parser and streaming crawler written in pure Rust.

[中文文档 (Chinese Documentation)](./README_zh.md)

---

## Key Features

- 📑 **Standard XML Sitemaps**: Full specification support for standard `<urlset>` collections and recursive `<sitemapindex>` trees.
- 🔍 **Google Extensions**: Native extraction of Google Images (`<image:image>`), Google Videos (`<video:video>`), Google News (`<news:news>`), and alternate multilingual tags (`<xhtml:link rel="alternate" hreflang="...">`). Namespace prefixes are matched by local-name, so custom prefixes and unprefixed tags both work.
- 📄 **Plain Text Support**: Automatic detection and line-by-line parsing of `.txt` sitemap files (`http` / `https` only).
- 🗜️ **Transparent Gzip Decompression**: Automatic sniffing of gzip magic headers (`0x1F, 0x8B`) with a **50 MiB uncompressed cap** against decompression bombs. UTF-8 / UTF-16 (LE & BE) BOMs are decoded.
- 🛡️ **Cycle Detection & Depth Protection**: Normalized identity keys (scheme/host case, default ports, fragments, trailing slashes), configurable recursion limits, and per-child error isolation.
- ⚡ **Fault-Tolerant Parsing**: Resilient against UTF-8 BOM headers, leading comments, bare `&` in `<loc>`, mixed Text + `<![CDATA[...]]>`, truncated XML (partial results), and W3C / RFC 3339 / ISO 8601 datetimes.

---

## Installation

Add this to your `Cargo.toml`:

```toml
[dependencies]
indexflow-sitemap = { version = "0.1.1", features = ["fetch", "gzip"] }
```

### Feature Flags

- `fetch` *(default)*: Enables async HTTP downloading and recursive index expansion via `reqwest`.
- `gzip` *(default)*: Enables automatic gzip decompression support via `flate2`.

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
                println!("Hreflangs count: {}", entry.hreflangs.len());
            }
        }
        ParsedSitemap::Index { child_urls } => {
            println!("Sitemap Index containing {} child sitemaps.", child_urls.len());
        }
        ParsedSitemap::PlainText { urls } => {
            println!("Plain text sitemap containing {} URLs.", urls.len());
        }
    }
}
```

### 2. Recursive Async Crawler & Sitemap Tree Expansion

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

    // Recursively expand sitemap index up to 3 levels deep
    let target_url = "https://www.example.com/sitemap_index.xml";
    let (is_index_tree, all_page_entries) = fetcher.expand_all(target_url, 3).await?;

    println!("Is Index Tree: {}", is_index_tree);
    println!("Total Discovered Page Entries: {}", all_page_entries.len());

    for entry in all_page_entries.iter().take(5) {
        println!("- {} (priority: {:?})", entry.loc, entry.priority);
    }

    Ok(())
}
```

---

## Data Models

### `SitemapUrlEntry`

| Field        | Type                    | Description                                                  |
| :----------- | :---------------------- | :----------------------------------------------------------- |
| `loc`        | `String`                | Target page URL                                              |
| `lastmod`    | `Option<DateTime<Utc>>` | Last modification timestamp (RFC3339 / ISO8601 / Date)       |
| `changefreq` | `Option<ChangeFreq>`    | Update frequency (`Always`, `Hourly`, `Daily`, `Weekly`, `Monthly`, `Yearly`, `Never`) |
| `priority`   | `Option<f64>`           | Crawl priority score normalized between `0.0` and `1.0`      |
| `hreflangs`  | `Vec<HreflangEntry>`    | Extracted multilingual alternate links (`lang`, `href`)      |
| `images`     | `Vec<SitemapImage>`     | Google Images extension metadata (`loc`, `title`, `caption`) |
| `videos`     | `Vec<SitemapVideo>`     | Google Videos extension metadata (`thumbnail_loc`, `title`, `duration`) |
| `news`       | `Option<SitemapNews>`   | Google News extension metadata (`publication_name`, `title`, `date`) |

Safety caps (Google protocol + bomb defence): 50 MiB uncompressed per document, 50_000 URLs per file, 1_000_000 URLs across a recursive `expand_all`.

---

## Changelog

### 0.1.1

- Gzip / raw payload **decompression-bomb cap** (50 MiB) and streamed HTTP download limits.
- Cycle detection uses a **normalized URL identity** (scheme/host case, default port, fragment, trailing slash); child fetch failures are isolated.
- XML parser concatenates Text + CDATA, rewrites bare `&` outside CDATA, matches tags by **local-name** (any prefix), and parses a wide W3C / RFC 3339 / ISO 8601 datetime matrix.
- UTF-8 BOM, UTF-16 LE/BE, leading comments, truncated XML (partial result), and `http(s)`-only loc filtering.

---

## License

Dual-licensed under either of:
- [Apache License, Version 2.0](LICENSE-APACHE)
- [MIT License](LICENSE-MIT)
