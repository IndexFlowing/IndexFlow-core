# indexflow-seo

[![Crates.io](https://img.shields.io/crates/v/indexflow-seo.svg)](https://crates.io/crates/indexflow-seo)
[![Documentation](https://docs.rs/indexflow-seo/badge.svg)](https://docs.rs/indexflow-seo)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/License-MIT%20OR%20Apache--2.0-blue.svg)](LICENSE)

> 基于纯 Rust 实现的高性能、零依赖技术 SEO 质量门禁与 GEO (Generative Engine Optimization) AI 搜索审计库。

[English Documentation](./README.md)

---

## 核心特性

- 🛡️ **技术 SEO 质量门禁**：严格校验 HTTP 状态码、Canonical 规范链接等价性、`noindex`/`nofollow` 阻断指令、`<title>` 缺失及 `<h1>` 主标题。
- 🤖 **GEO 与 AI 搜索引擎审计**：嗅探 `GPTBot` / `ChatGPT-User`、`PerplexityBot`、`ClaudeBot` / `anthropic-ai`、`Google-Extended` 的屏蔽指令，支持按 bot 的 `X-Robots-Tag` 以及 `none` / `noai`。
- 📑 **Schema.org 结构化数据**：提取 `application/ld+json`，展开 `@graph` 与顶层数组，映射字符串或数组形式的 `@type`。
- 🌐 **社交与多语言元数据**：解析 OpenGraph、Twitter Card 标签及 `link rel="alternate" hreflang` 数组。
- ⚡ **纯内存无 I/O 评估**：字符边界安全的 HTML 扫描（中文 / Emoji 不会 panic）。引号感知切标签、无引号属性、多行 meta；可见标签扫描会跳过注释与 `<script>`/`<style>`。
- 🚀 **可选不跟随重定向探针**：内置异步 HTTP 客户端，将 3xx 重定向作为门禁诊断项拦截，响应体上限 5 MiB。

---

## 安装引入

在 `Cargo.toml` 中添加依赖：

```toml
[dependencies]
indexflow-seo = "0.1.2"
```

### Feature Flags 说明

- `probe` *（默认开启）*：启用基于 reqwest（纯 Rustls TLS）的异步网络探测客户端 `SeoProbeClient`。

---

## 快速上手

### 1. 纯内存 HTML 质检评估（零 I/O）

```rust
use indexflow_seo::evaluate_html;

fn main() {
    let page_url = "https://example.com/blog/rust-guide";
    let html = r#"
    <!DOCTYPE html>
    <html lang="zh">
    <head>
      <title>Rust 全栈单体架构实践</title>
      <meta name="description" content="面向独立开发者的 Rust 高性能技术指南。" />
      <link rel="canonical" href="https://example.com/blog/rust-guide" />
      <meta name="robots" content="index, follow" />
      
      <!-- AI 爬虫指令 -->
      <meta name="gptbot" content="index" />
      <meta name="perplexitybot" content="index" />

      <!-- 结构化数据 -->
      <script type="application/ld+json">
      {
        "@context": "https://schema.org",
        "@type": "Article",
        "headline": "Rust 全栈单体架构实践"
      }
      </script>
    </head>
    <body>
      <h1>Rust 全栈单体架构实践指南</h1>
    </body>
    </html>"#;

    let result = evaluate_html(page_url, 200, 25, None, html);

    if result.passed {
        println!("✅ 技术 SEO 门禁: 通过 (PASS)");
        println!("页面标题: {:?}", result.page_title);
        println!("H1 主标题: {:?}", result.h1_content);
        println!("结构化实体: {:?}", result.schema_types());
    } else {
        println!("❌ 技术 SEO 门禁: 拦截 (原因: {:?})", result.block_reason);
    }
}
```

### 2. 异步网络探测客户端（启用 probe Feature）

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

    println!("质检结果: 通过={}, 拦截原因={:?}", result.passed, result.block_reason);
    println!("网络响应耗时: {:?} ms", result.response_time_ms);
    println!("GPTBot 屏蔽状态: {}", result.ai_directives.gptbot_blocked);

    Ok(())
}
```

---

## 🛡️ 质量门禁拦截规则

`indexflow-seo` 在准许 URL 提交给搜索引擎前，会严格按照以下规则执行前置质检：

1. **HTTP 状态码**：必须严格等于 `200 OK`；
2. **Robots 阻断指令**：不得包含 `<meta name="robots" content="noindex">`（或 `none`）或 `X-Robots-Tag: noindex`；
3. **Canonical 规范链接**：声明的 Canonical 必须与实际访问 URL 等价（相对路径、协议相对 URL、`.` / `..`、默认端口 80/443、尾部斜杠、scheme/host 大小写、查询参数顺序、百分号解码；路径本身区分大小写）；
4. **页面标题**：必须包含非空的 `<title>` 元素。

---

## 更新日志

### 0.1.2

- 字符边界安全的 HTML 扫描：中文 / Emoji、引号内 `>`、无引号 URL 属性、多行 meta；可见标签跳过注释与 `<script>`/`<style>`。
- JSON-LD 展开 `@graph`、顶层数组、数组型 `@type`；容忍 CDATA 包装。
- 单次扫描实体解码（HTML Latin-1 命名实体 + `&#N;` / `&#xN;`），`&amp;lt;` 不再二次解码。
- Canonical：查询参数排序、RFC 3986 的 `../`、协议相对 URL、`%7E` ≡ `~`。
- GEO：AI bot 别名、`none` / `noai`、按 bot 的 `X-Robots-Tag`；探针响应体上限 5 MiB。

---

## 开源协议

本项目采用双重授权：

- [Apache License, Version 2.0](LICENSE-APACHE)
- [MIT License](LICENSE-MIT)
