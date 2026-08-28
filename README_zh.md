<div align="center">

<img src="https://raw.githubusercontent.com/IndexFlowing/IndexFlow-core/main/static/logo.png" alt="IndexFlow Logo" width="96" />

# IndexFlow

### 开源 SEO 与搜索索引基础设施

**基于 Rust 构建 • 内存安全 • 开发者优先 • 面向 AI 搜索时代**

[![Crates.io SEO](https://img.shields.io/crates/v/indexflow-seo.svg?label=crates.io%20%7C%20seo)](https://crates.io/crates/indexflow-seo)
[![Crates.io Sitemap](https://img.shields.io/crates/v/indexflow-sitemap.svg?label=crates.io%20%7C%20sitemap)](https://crates.io/crates/indexflow-sitemap)
[![License](https://img.shields.io/badge/License-MIT%20OR%20Apache--2.0-blue.svg)](LICENSE)
[![GitHub Stars](https://img.shields.io/github/stars/IndexFlowing/IndexFlow-core?style=social)](https://github.com/IndexFlowing/IndexFlow-core)

[🌐 官网](https://www.indexflowing.com) ·
[📖 文档](https://docs.rs/indexflow-seo) ·
[English](README.md)

---

</div>

# 🚀 什么是 IndexFlow？

IndexFlow 是一个基于 Rust 构建的开源 **SEO 与搜索索引基础设施项目**。

在现代 Web 时代，一个网站的增长不再只是发布内容。

随着：

- Google Search
- AI Search（ChatGPT Search、Perplexity、Claude）
- GEO（Generative Engine Optimization，生成式引擎优化）

的发展，开发者需要更好的基础设施来理解：

- 搜索引擎如何发现网站
- 网站是否满足技术 SEO 要求
- 内容是否能够被机器正确理解
- 网站是否具备面向未来 AI 搜索的基础能力

IndexFlow 致力于提供轻量、可靠、开发者友好的 Rust 工具链，帮助开发者构建更好的搜索可见性。


---

</div>

# 🎯 为什么需要 IndexFlow？

构建和运营一个网站，往往存在大量隐藏的技术问题。


## 🔍 搜索发现

搜索引擎需要结构化的信息来发现和理解网站内容。

IndexFlow 提供：

- XML Sitemap 处理
- URL 结构分析
- 搜索索引工作流基础能力


## ⚙️ 技术 SEO

很多网站无法获得良好搜索表现，并不是内容问题，而是隐藏的技术问题：

- Canonical 配置错误
- Robots 指令错误
- Meta 信息缺失
- 页面结构问题

IndexFlow 提供工具帮助开发者发现潜在问题。


## 🤖 AI 搜索时代

搜索正在发生变化。

AI 系统越来越依赖：

- 结构化数据
- 清晰的网站架构
- 机器可读的信息

IndexFlow 希望成为下一代网站发现与搜索基础设施的一部分。


---

# 🏗️ 项目架构

IndexFlow 采用模块化 Rust 生态设计。

```
                IndexFlow

                    |
    ---------------------------------
    |                               |

indexflow-sitemap                 indexflow-seo

XML Sitemap 解析器             技术 SEO 分析器
    |                               |

    ---------------------------------

              IndexFlow Platform
              
```

开源组件提供基础能力。

商业平台将在这些组件之上构建更多网站管理、索引管理和智能分析能力。


---

# 📦 开源组件


## indexflow-sitemap

一个轻量级 Rust XML Sitemap 处理库。

主要功能：

- XML Sitemap 解析
- Sitemap Index 支持
- URL 提取
- 简洁易用的 Rust API


Crates.io：

https://crates.io/crates/indexflow-sitemap


---


## indexflow-seo

一个基于 Rust 的技术 SEO 分析库。

主要功能：

- HTML 页面分析
- SEO 元数据检查
- 技术 SEO 验证
- 可扩展分析架构


Crates.io：

https://crates.io/crates/indexflow-seo


---

# 💡 设计理念


## 🦀 Rust 原生

使用 Rust 构建：

- 内存安全
- 高性能
- 可靠的基础设施组件
- 零成本抽象


## 👨‍💻 开发者优先

IndexFlow 面向希望拥有：

- 开源方案
- 自托管能力
- 透明架构
- 可复用组件

的开发者。


## ⚡ 轻量基础设施

相比复杂庞大的 SEO 平台，IndexFlow 更关注：

- 简单部署
- 清晰架构
- 可组合组件


---

# 🛣️ 路线图


## ✅ 已完成

### indexflow-sitemap

- XML Sitemap 解析器
- Rust Library
- 已发布至 crates.io


### indexflow-seo

- 技术 SEO 分析
- Rust Library
- 已发布至 crates.io


---

## 🚧 开发中

### IndexFlow Platform

计划支持：

- 网站管理
- 搜索索引工作流
- Sitemap 监控
- Google Search Console 集成
- IndexNow 集成


---

## 🔮 未来方向

### AI SEO Intelligence

探索：

- AI SEO 分析报告
- GEO 优化建议
- 网站智能分析
- 搜索可见性分析


---

# 🌐 IndexFlow 平台

开源核心将作为未来 IndexFlow 平台的基础。

IndexFlow 希望帮助网站开发者和企业管理：

- 网站健康状态
- 搜索可见性
- 索引流程
- AI 搜索优化


了解更多：

https://www.indexflowing.com


---

# 🤝 参与贡献

IndexFlow 是一个开放建设中的项目。

欢迎：

- 提交 Issue
- 提出建议
- 参与讨论
- 提交 Pull Request


如果 IndexFlow 对你有帮助：

⭐ 欢迎 Star 项目

你的支持会帮助更多开发者发现 IndexFlow。


---

# 📄 开源协议

IndexFlow 使用双许可证：

- MIT License
- Apache License 2.0

你可以根据需要选择其中之一。