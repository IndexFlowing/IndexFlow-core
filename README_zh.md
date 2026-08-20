# IndexFlow

**面向海量页面的开源搜索引擎收录基础设施。**

[English](README.md) | [中文]

四大解耦工作台（Sitemap 资产、SEO 质检、引擎推送、GSC 收录监控）、Google 滚动 24 小时配额熔断，以及 Search Analytics 豁免：已有展示的 URL 不再消耗 Indexing API 配额。

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
- **四大独立工作台**（同一站点摘要头下）
  - **Sitemap 资产** — 递归解析 XML / sitemap-index；locale、path prefix、lastmod、priority
  - **SEO 质检** — 独立 HTTP 扫描（200、`<title>`、description、canonical、robots、H1），不触发推送
  - **引擎推送** — Bing IndexNow 批量 vs Google Indexing API（滚动 24h 配额）分队列
  - **收录监控** — GSC Search Analytics 批量收获 + URL Inspection 漏斗（2,000/天）
- **GSC 配额豁免** — 有展示次数的页面标记为 `INDEXED` / `google_status=SUBMITTED`，不再占用每日 200 条 Indexing API
- **单 URL 深度诊断抽屉** — 即时 Recheck SEO、立刻提交 Bing/Google、元标签信号、GSC coverage、原始响应
- **Cloudflare WAF 绕过** — 所有页面爬虫携带内部 User-Agent
- **多站点公平调度** — 窗口分区并发拉取，杜绝单一大站点垄断 Worker
- **配额熔断** — Google 耗尽后休眠到下一个空闲槽位，无空转探测
- **3 态守恒生命周期** — `PENDING` + `SUBMITTED` + `BLOCKED` = `TOTAL`

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

# GSC URL Inspection 滚动 24 小时配额（默认 2000）
GSC_INSPECT_DAILY_QUOTA=2000

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

Windows 可用 `start.ps1`：如有需要会先构建 `ui/out`，再启动 API。浏览器打开 `http://127.0.0.1:<SERVER_PORT>/`（本仓库默认 **8010**）。首次访问会引导创建管理员账号。添加站点后填写 IndexNow Key 和/或 Google Service Account JSON，点击 **测试 Bing** / **测试 Google** 直到通道为 **已验证**。GSC 同步还需把同一服务账号邮箱加为 Search Console 用户。四个选项卡可独立操作：同步 Sitemap、跑 SEO 审计、推送 Bing/Google、从 GSC 同步已收录 URL。

### 测试

```bash
cargo check
cargo test
```

---

## 工作原理

```
[ 模块 1：Sitemap 资产 ]  ── 唯一数据源（仅 SYNC_SITEMAP）
            │
            ├──► [ 模块 2：SEO 质检 ]     CHECK_URL（独立）
            ├──► [ 模块 3：引擎推送 ]     SUBMIT_BING / SUBMIT_GOOGLE
            └──► [ 模块 4：收录监控 ]     GSC Analytics + GSC_INSPECT
```

- 同步 Sitemap 不会排队 SEO 或推送任务。
- SEO 审计不会排队 Bing/Google 推送。
- GSC Search Analytics（impressions &gt; 0）标记 `google_index_status=INDEXED`，豁免 Google Indexing API 配额。
- GSC URL Inspection 填充漏斗：已收录 / 已抓取未收录 / 已发现未收录 / 未知（每天最多 2,000）。
- 点击任意 URL 打开诊断抽屉（`GET /urls/:id/analysis`、`POST /urls/:id/recheck`、`POST /urls/:id/submit-now`）。

超时仍处于 `PROCESSING` 的僵尸任务会被自动回收，避免服务崩溃后死锁。

---

## 许可

Source-available Open Core。欢迎自用、Fork，并在你自己的站点上运行。
