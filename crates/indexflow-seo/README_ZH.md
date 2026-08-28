# indexflow-seo

[![Crates.io](https://img.shields.io/crates/v/indexflow-seo.svg)](https://crates.io/crates/indexflow-seo)
[![Documentation](https://docs.rs/indexflow-seo/badge.svg)](https://docs.rs/indexflow-seo)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/License-MIT%20OR%20Apache--2.0-blue.svg)](LICENSE)

> 基于纯 Rust 实现的高性能、零依赖技术 SEO 质量门禁与 GEO (Generative Engine Optimization) AI 搜索审计库。

[English Documentation](./README.md)

---

## 核心特性

- 🛡️ **技术 SEO 质量门禁**：严格校验 HTTP 状态码、Canonical 规范链接等价性、`noindex`/`nofollow` 阻断指令、`<title>` 缺失及 `<h1>` 主标题。
- 🤖 **GEO 与 AI 搜索引擎审计**：原生嗅探主流 AI 爬虫的屏蔽指令（`GPTBot`、`PerplexityBot`、`ClaudeBot`、`Google-Extended`）。
- 📑 **Schema.org 结构化数据**：毫秒级提取 `application/ld+json` 代码块，自动映射 `@type` 实体模型（如 `Article`、`FAQPage`、`Product`）。
- 🌐 **社交与多语言元数据**：解析 OpenGraph、Twitter Card 标签及 `xhtml:link` 多语言 hreflang 数组。
- ⚡ **纯内存无 I/O 评估**：直接传入 HTML 字符串执行评估，适合高并发单元测试与无网络依赖的批处理质检。
- 🚀 **可选不跟随重定向探针**：内置异步 HTTP 客户端，将 3xx 重定向直接作为门禁诊断项拦截。

---

## 安装引入

在 `Cargo.toml` 中添加依赖：

```toml
[dependencies]
indexflow-seo = "0.1"
```

### Feature Flags 说明

- probe *（默认开启）*：启用基于 reqwest (纯 Rustls TLS) 的异步网络探测客户端 SeoProbeClient。

------



## 快速上手

### 1. 纯内存 HTML 质检评估（零 I/O）

```
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

```
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

------



## 🛡️ 质量门禁拦截规则

indexflow-seo 在准许 URL 提交给搜索引擎前，会严格按照以下规则执行前置质检：

1. **HTTP 状态码**：必须严格等于 200 OK；
2. **Robots 阻断指令**：不得包含 <meta name="robots" content="noindex"> 或 X-Robots-Tag: noindex；
3. **Canonical 规范链接**：声明的 Canonical 必须与实际访问 URL 等价（自动处理相对路径、大小写、默认端口 80/443 及尾部斜杠差异）；
4. **页面标题**：必须包含非空的 <title> 元素。

------



## 开源协议

本项目采用双重授权：

- [Apache License, Version 2.0](https://www.google.com/url?sa=E&q=LICENSE-APACHE)
- [MIT License](https://www.google.com/url?sa=E&q=LICENSE-MIT)
