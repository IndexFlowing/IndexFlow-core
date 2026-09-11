# indexflow-seo

[![Crates.io](https://img.shields.io/crates/v/indexflow-seo.svg)](https://crates.io/crates/indexflow-seo)
[![Documentation](https://docs.rs/indexflow-seo/badge.svg)](https://docs.rs/indexflow-seo)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/License-MIT%20OR%20Apache--2.0-blue.svg)](LICENSE)

> 基于纯 Rust 构建的极速、零依赖技术 SEO 质量门禁与 GEO (Generative Engine Optimization) 深度质检引擎。

[English Documentation](./README.md)

---

## 核心特性

- 🛡️ **技术 SEO 门禁卫士**：全面执行 HTTP 状态码、Canonical 声明一致性、`noindex`/`nofollow` 指令、`<title>` 标题与 `<h1>` 层级的准入检查。
- 🔗 **跨层冲突深度审计**：严格比对 RFC 5988 HTTP `Link: <...>; rel="canonical"` 标头（常见于 CDN / Edge 中间件）与 HTML Body 内部声明，阻断死循环。
- 🔀 **严格重定向语义检测**：直接捕获第一跳原始响应；标黄拦截导致权重流失的 `302/307` 临时重定向，拦截缺少 `Location` 的破损跳转与重定向死锁。
- 🎥 **VideoObject 富媒体语义自环防护**：深度解析 Schema.org `VideoObject`，断言其 `embedUrl` 与 `contentUrl` 绝不指向网页自身 HTML，防止 Googlebot 视频解码器遭遇“无法处理视频”致命崩溃。
- 🤖 **GEO 与 AI 搜索引擎嗅探**：全面审计针对 `GPTBot` / `ChatGPT-User`、`PerplexityBot`、`ClaudeBot` / `anthropic-ai` 和 `Google-Extended` 的抓取封禁指令。
- 📑 **Schema.org 结构化数据**：提取 `application/ld+json` 块，递归展开 `@graph` 树与顶级数组，映射实体 `@type`（字符串或数组）。
- 🌐 **社交元数据与多语言**：解析 OpenGraph（含 `og:video` 与 `og:type` 错配校验）、Twitter Card 与 `link rel="alternate" hreflang` 映射数组。
- ⚡ **纯内存评估引擎**：字符边界安全的高速分词（CJK / Emoji 绝不 panic）。支持属性引号容错、多行元数据提取，自动跳过 `<script>`/`<style>`。
- 🚀 **不跟随重定向的高性能探针**：内置异步 HTTP 探测客户端，将 3xx 跳转作为第一现场判定，并设置 5 MiB 安全内存上限。

---

## 安装与引入

在你的 `Cargo.toml` 中添加：

```toml
[dependencies]
indexflow-seo = "0.1.4"
```

### Feature Flags

- `probe` *(默认开启)*：通过 `reqwest`（纯 Rustls TLS）启用异步 HTTP 质检客户端 `SeoProbeClient`。

---

## 快速上手

### 1. 纯内存中 HTML 评估（零 I/O 开销）

```rust
use indexflow_seo::evaluate_html;

fn main() {
    let page_url = "https://example.com/blog/rust-guide";
    let html = r#"
    <!DOCTYPE html>
    <html lang="zh-CN">
    <head>
      <title>Rust 全栈单体架构与技术 SEO 实践</title>
      <meta name="description" content="面向现代 Rust 开发者的工业级技术 SEO 指南。" />
      <link rel="canonical" href="https://example.com/blog/rust-guide" />
      <meta name="robots" content="index, follow" />
      
      <!-- AI 爬虫抓取策略 -->
      <meta name="gptbot" content="index" />
      <meta name="perplexitybot" content="index" />

      <!-- 结构化数据 -->
      <script type="application/ld+json">
      {
        "@context": "https://schema.org",
        "@type": "Article",
        "headline": "Rust 全栈单体架构"
      }
      </script>
    </head>
    <body>
      <h1>完整指南：Rust 全栈工程实践</h1>
    </body>
    </html>"#;

    let result = evaluate_html(
        page_url,
        200,    // HTTP 状态码
        25,     // 耗时 (毫秒)
        None,   // 可选的 X-Robots-Tag 标头
        None,   // 可选的 Location 标头
        None,   // 可选的 Link 标头
        html,
    );

    if result.passed {
        println!("✅ 技术 SEO 门禁: 通过");
        println!("页面标题: {:?}", result.page_title);
        println!("H1 内容: {:?}", result.h1_content);
        println!("结构化实体: {:?}", result.schema_types());
    } else {
        println!("❌ 技术 SEO 门禁: 拦截 (原因: {:?})", result.block_reason);
    }
}
```

### 2. 真实网络层技术探测（需启用 `probe` 特性）

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

    println!("门禁判定  : passed={}, reason={:?}", result.passed, result.block_reason);
    println!("响应耗时  : {:?} ms", result.response_time_ms);
    println!("GPTBot屏蔽: {}", result.ai_directives.gptbot_blocked);
    println!("优化建议  : {:?}", result.warnings);

    Ok(())
}
```

---

## 🛡️ 门禁裁决规则体系

在准许将 URL 推送至搜索引擎前，`indexflow-seo` 强制执行以下硬性验证：

1. **HTTP 状态码**：必须严格返回 `200 OK`。3xx 重定向必须包含合规 `Location` 标头且不能自环，直接拦截提交以避免消耗引擎推送配额。
2. **跨层 Canonical 一致性**：HTTP 标头 `Link: <...>; rel="canonical"` 必须与 HTML Body 内部声明完全一致，杜绝爬虫死锁。
3. **Robots 索引指令**：禁止包含 `<meta name="robots" content="noindex">` (或 `none`) 及 `X-Robots-Tag: noindex`。
4. **规范 URL 归一化**：声明的 Canonical 必须与实际网页 URL 归一化匹配。智能处理相对路径、`../`、默认端口 (80/443)、尾部斜杠、大小写与查询参数乱序。
5. **VideoObject 自环断言**：Schema.org 中的 `embedUrl` 与 `contentUrl` 绝对不可指向宿主 HTML 本身。
6. **标题标签**：必须包含有效、非空的 `<title>` 元素。

---

## 更新日志

### 0.1.4

- **架构全面解耦**：彻底重构 `evaluator.rs` 与 `extractor.rs`，按单一职责平铺抽象为 `html_utils`、`content`、`schema`、`meta`、`headers`、`gate`、`warnings` 独立模块，杜绝目录嵌套与模板噪音。
- **跨层冲突审计**：新增 RFC 5988 HTTP `Link` 标头解析引擎，并与 HTML Head 标签执行跨层一致性校验。
- **VideoObject 语义自环审计**：新增深度 `VideoObject` 实体提取，拦截误将 HTML 作为视频流的致命配置，并检查媒体后缀 (`.mp4`, `.m3u8` 等)。
- **严格重定向语义**：不跟随重定向探针现可精准捕获 `302/307` 权重流失隐患、缺失 `Location` 标头以及自环跳跃。
- **社交标签一致性**：新增 `og:video` 提取，并在 `og:type="video.*"` 缺失视频流时给出错配警示。

### 0.1.2

- 字符边界安全的高速扫描：支持 CJK / Emoji，容错属性引号，跳过 multiline 注释与 `<script>`/`<style>`。
- 深度展开 JSON-LD `@graph` 与数组类型 `@type`，容错 CDATA 包装。
- 单趟字符实体解码（HTML 常用实体与进制编码），防止二次重复解码。
- 规范化 Canonical 匹配：查询参数自动排序，严格支持 RFC 3986 `../` 路径归一。
- GEO 爬虫规则：适配主流 AI 搜索引擎爬虫别名与限制指令。

---

## 开源许可

本项目遵循以下双重开源协议之一：
- [Apache License, Version 2.0](LICENSE-APACHE)
- [MIT License](LICENSE-MIT)