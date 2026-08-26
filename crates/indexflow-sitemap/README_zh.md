# indexflow-sitemap

[![Crates.io](https://img.shields.io/crates/v/indexflow-sitemap.svg)](https://crates.io/crates/indexflow-sitemap)
[![Documentation](https://docs.rs/indexflow-sitemap/badge.svg)](https://docs.rs/indexflow-sitemap)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/License-MIT%20OR%20Apache--2.0-blue.svg)](LICENSE)

> 基于纯 Rust 实现的高性能、高容错、全兼容 Google 官方规范的 Sitemap 流式解析器与递归爬虫库。

[English Documentation](./README.md)

---

## 核心特性

- 📑 **标准 XML Sitemap 支持**：原生解析标准 `<urlset>`（页面集合）与多层级递归的 `<sitemapindex>`（Sitemap 索引树）。
- 🔍 **Google 官方扩展全兼容**：原生提取 Google 图片（`<image:image>`）、视频（`<video:video>`）、新闻（`<news:news>`）以及多语言交替标签（`<xhtml:link rel="alternate" hreflang="...">`）。
- 📄 **纯文本 Sitemap**：自动识别并按行提取 `.txt` 格式的 Sitemap 文件。
- 🗜️ **透明 Gzip 解压**：自动嗅探 Magic Header 字节（`0x1F, 0x8B`），在内存中无缝秒级解压 `.xml.gz` 压缩包。
- 🛡️ **循环引用与死锁防护**：内置已访问集合去重与递归深度上限，杜绝 Sitemap 爬虫陷入无限循环陷阱。
- ⚡ **超强容错解析**：自动剥离 UTF-8 BOM 标头、容忍未转义实体字符及复杂嵌套的 `<![CDATA[...]]>` 语法。

---

## 安装引入

在你的 `Cargo.toml` 中添加依赖：

```toml
[dependencies]
indexflow-sitemap = { version = "0.1", features = ["fetch", "gzip"] }
```

### Feature Flags 说明

- `fetch` *（默认开启）*：启用基于 `reqwest` 的异步网络下载与多层级索引树递归展开功能。
- `gzip` *（默认开启）*：启用基于 `flate2` 的自动 Gzip 嗅探与解压支持。

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
                println!("更新时间: {:?}", entry.lastmod);
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

    // 递归展开最多 3 层深度的 SitemapIndex 树结构
    let target_url = "https://www.example.com/sitemap_index.xml";
    let (is_index_tree, all_page_entries) = fetcher.expand_all(target_url, 3).await?;

    println!("是否为索引树: {}", is_index_tree);
    println!("成功发现的全量页面 URL 总数: {}", all_page_entries.len());

    for entry in all_page_entries.iter().take(5) {
        println!("- {} (优先级: {:?})", entry.loc, entry.priority);
    }

    Ok(())
}
```

---

## 核心数据模型

### `SitemapUrlEntry`

| 字段         | 类型                    | 说明                                                         |
| :----------- | :---------------------- | :----------------------------------------------------------- |
| `loc`        | `String`                | 目标网页的规范访问 URL                                       |
| `lastmod`    | `Option<DateTime<Utc>>` | 页面上次修改时间戳（支持 RFC3339 / ISO8601 / 日期格式）      |
| `changefreq` | `Option<ChangeFreq>`    | 更新频率枚举（`Always`, `Hourly`, `Daily`, `Weekly`, `Monthly`, `Yearly`, `Never`） |
| `priority`   | `Option<f64>`           | 抓取调度优先级权重（自动归一化在 `0.0` ~ `1.0` 之间）        |
| `hreflangs`  | `Vec<HreflangEntry>`    | 提取的多语言 alternate 标签映射列表（`lang`, `href`）        |
| `images`     | `Vec<SitemapImage>`     | Google 图片扩展元数据（`loc`, `title`, `caption`, `license`） |
| `videos`     | `Vec<SitemapVideo>`     | Google 视频扩展元数据（`thumbnail_loc`, `title`, `duration` 等） |
| `news`       | `Option<SitemapNews>`   | Google 新闻扩展元数据（`publication_name`, `title`, `date`） |

---

## 开源协议

本项目采用双重授权：
- [Apache License, Version 2.0](LICENSE-APACHE)
- [MIT License](LICENSE-MIT)