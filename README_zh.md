# IndexFlow

**面向海量页面的开源搜索引擎收录基础设施。**

[English](README.md) | [中文]

内置多引擎独立调度、提交前 SEO 质量门禁、以及 Google 滚动 24 小时配额熔断 —— 专为 Programmatic SEO 站点、独立开发者和 SEO 工程师设计。

---

## 生产环境真实落地

IndexFlow 不是玩具项目，已在真实生产环境承载数十万级页面的高并发 SEO 检查与自动推送：

- **[Mandarin Clips](https://www.mandarinclips.com)** — 海量中文学习与视频语料站，**230,000+** 页面持续调度中
- **[Inkvilion](https://www.inkvilion.com)** — 多语言词典与在线工具站，**10,000+** 页面全自动索引

---

## 为什么选择 IndexFlow？

站点规模到十万、百万 URL 之后，传统 Sitemap 提交会撞上这些墙：

1. **Google Indexing API 配额极紧** — 单项目每天约 200 条，超额会堵死整条队列。
2. **提交低质/死链会反噬整站** — 404、noindex、Canonical 偏离既浪费配额，也伤害抓取信任。
3. **多引擎进度不能混在一起** — Bing（IndexNow）一天可推上万条，Google 只能按日消耗；共用进度条会把两套时钟搅乱。
4. **大站饿死小站** — 单一大队列独占 Worker，其他站点无法推进。

IndexFlow 从这些约束出发设计，而不是从演示级 Sitemap 出发。

---

## 核心特性

- **高性能 Rust 后端** — Axum + Tokio + SQLx。低内存、高并发，轻松承载百万级 URL 元数据与状态机流转。
- **搜索引擎独立解耦流水线**
  - **Bing / IndexNow** — 万级批量瞬时推送
  - **Google Indexing API** — 按滚动 24 小时配额窗口消耗，而不是简单的 UTC 日切计数
- **多站点公平调度** — 窗口分区并发拉取，杜绝单一大站点垄断 Worker
- **提交前实时 SEO 质量门禁** — 正式提交前的最后一毫秒发起轻量 GET
  - 自动拦截非 200、`noindex`、Canonical 规范偏离、空 `<title>`
  - 同时保护整站权重和稀缺的 Google 配额
- **配额熔断** — Google 配额耗尽且 Bing 已完成时，待处理 Google 任务进入智能休眠，直到下一个空闲槽位；零无意义网络探测
- **3 态守恒生命周期** — `PENDING`（待处理）+ `SUBMITTED`（已推送）+ `BLOCKED`（SEO 异常阻断）= `TOTAL`（总数）
  - 各引擎进度独立
  - 看板展示队列深度与配额释放倒计时

凭证状态：**未填写 (Unset)** / **已填写 (Saved)** / **已验证 (Verified)**。只有验证通过的通道才会进入推送。

![控制台首页](./images/首页.png)

---

## 技术栈

| 层次 | 选型 | 说明 |
| :--- | :--- | :--- |
| **后端核心** | Rust 2021, Axum, Tokio, SQLx | 异步 Worker、状态机、API |
| **前端工作台** | Next.js 15 (App Router), Tailwind CSS | 暗色控制台，单站独立工作流 |
| **数据存储** | PostgreSQL 14+ | 生命周期、分区索引、乐观锁 |
| **鉴权** | JWT + Google Service Account OAuth2 | 单租户管理员与凭证隔离 |

---

## 自托管

### 环境要求

- Rust 1.75+
- PostgreSQL 14+
- Node.js 18+（仅在需要重新构建前端时）

### 环境变量

在后端根目录创建 `.env`：

```env
# 服务监听
SERVER_HOST=0.0.0.0
SERVER_PORT=8080

# 数据库
DATABASE_URL=postgres://postgres:password@127.0.0.1:5432/indexflow

# Google 滚动 24 小时配额（默认 200）
GOOGLE_DAILY_QUOTA=200

# 调度与 Worker 节流
SCHEDULER_INTERVAL_SECS=60
SUBMIT_WORKER_BATCH=50
SUBMIT_WORKER_INTERVAL_SECS=2

# JWT 鉴权密钥
JWT_SECRET=your-secure-jwt-secret-key-change-me
```

首次启动时，SQLx 会自动执行 `migrations/`。

### 启动

```bash
# 可选：重新构建静态工作台
cd ui && npm install && npm run build && cd ..

cargo run
```

Windows 可用 `start.ps1`：如有需要会先构建 `ui/out`，再启动 API。首次访问会引导创建管理员账号。之后添加站点，填写 IndexNow Key 和/或 Google Service Account JSON，点击 **测试 Bing** / **测试 Google** 直到通道状态为 **已验证**，再同步 Sitemap 并推送。

### 测试

```bash
cargo check
cargo test
```

---

## 工作原理

```
站点（独立工作单元）
        │
        ▼
Sitemap 发现 URL  →  PENDING（待处理）
        │
        ▼
提交前内联 SEO 质量门禁
        │
   通过 ├──────────► Bing / Google 独立流水线  →  SUBMITTED（已推送）
        │
   拦截 └──────────► BLOCKED（质量门禁拦截）
```

Bing 与 Google 是独立任务类型（`SUBMIT_BING`、`SUBMIT_GOOGLE`）。同一 URL 可以已推送 Bing，同时仍在等待 Google。超过超时仍处于 `PROCESSING` 的僵尸任务会被自动回收，避免服务崩溃后死锁。

---

## 许可

Source-available Open Core。欢迎自用、Fork，并在你自己的站点上运行。
