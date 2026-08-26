# indexflow-sitemap

[![Crates.io](https://img.shields.io/crates/v/indexflow-sitemap.svg)](https://crates.io/crates/indexflow-sitemap)
[![Documentation](https://docs.rs/indexflow-sitemap/badge.svg)](https://docs.rs/indexflow-sitemap)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/License-MIT%20OR%20Apache--2.0-blue.svg)](LICENSE)

> 基于纯 Rust 实现的高性能、高容错、全兼容 Google 官方规范的 Sitemap 流式解析器与递归爬虫库。

[English Documentation](./README.md)

---

## 核心特性

- 📑 **标准 XML Sitemap 支持**：原生支持 `<urlset>`（页面集合）与 `<sitemapindex>`（Sitemap 索引树）。
- 🔍 **Google 官方扩展全支持**：深度提取 Google 图片（`<image:image>`）、视频（`<video:video>`）、新闻（`<news:news>`）以及多语言交替链接（`<xhtml:link rel="alternate" hreflang="...">`）。
- 📄 **纯文本 Sitemap**：自动嗅探并解析 `.txt` 文本格式的 Sitemap 文件（每行一个 URL）。
- 🗜️ **透明 Gzip 解压**：自动嗅探 Magic Header 字节（`0x1F, 0x8B`），无缝秒级解压 `.xml.gz` 压缩文件。
- 🛡️ **防死锁与深度熔断**：内置访问集合去重与递归深度上限，杜绝 Sitemap 循环引用陷入死循环。
- ⚡ **超强容错解析**：自动剥离 UTF-8 BOM 标头、容忍未转义实体字符及复杂嵌套的 `<![CDATA[...]]>` 语法。

---

## 安装引入

在你的 `Cargo.toml` 中添加依赖：

```toml
[dependencies]
indexflow-sitemap = { version = "0.1", features = ["fetch", "gzip"] }
```

---

## 快速上手

### 1. 纯内存 XML 字符串极速解析

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
                println!("页面 URL: {}", entry.loc);
                println!("上次更新时间: {:?}", entry.lastmod);
                println!("调度优先级: {:?}", entry.priority);
                println!("多语言 Alternate 数量: {}", entry.hreflangs.len());
            }
        }
        ParsedSitemap::Index { child_urls } => {
            println!("检测到 SitemapIndex 索引文件，包含 {} 个子 Sitemap", child_urls.len());
        }
        ParsedSitemap::PlainText { urls } => {
            println!("纯文本格式 Sitemap，包含 {} 个 URL", urls.len());
        }
    }
}
```

### 2. 异步流式递归展开 Sitemap 索引树

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

    // 递归展开最多 3 层的 SitemapIndex 树结构
    let target = "https://www.example.com/sitemap_index.xml";
    let (is_index_tree, all_page_entries) = fetcher.expand_all(target, 3).await?;

    println!("是否为索引树: {}", is_index_tree);
    println!("成功发现的全量页面 URL 总数: {}", all_page_entries.len());

    Ok(())
}
```

---

## 开源协议

本项目采用双重授权：
- [Apache License, Version 2.0](LICENSE-APACHE)
- [MIT License](LICENSE-MIT)

