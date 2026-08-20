# IndexFlow

**Open-source search index infrastructure for high-volume sites.**

[English] | [中文](README_zh.md)

Four decoupled workspaces (sitemap assets, SEO quality gate, engine push, GSC index monitoring), a rolling 24-hour Google quota circuit, and Search Analytics exemption so already-ranking URLs never burn Indexing API quota.

---

## Production case studies

IndexFlow is not a toy. It already runs in production against hundreds of thousands of pages:

- **[Mandarin Clips](https://www.mandarinclips.com)** — large-scale Mandarin learning and video-corpus site, **230,000+** pages under continuous scheduling
- **[Inkvilion](https://www.inkvilion.com)** — multilingual dictionary and online tools, **10,000+** pages fully automated

---

## Why IndexFlow?

Sitemap submission tools break down once a site reaches tens or hundreds of thousands of URLs:

1. **Google Indexing API quota is tiny** — 200 URLs per project per day. Overflow stalls the entire queue.
2. **Submitting junk hurts the whole domain** — 404s, noindex pages, and canonical mismatches waste quota and can damage crawl trust.
3. **Engines do not move at the same speed** — Bing (IndexNow) can take tens of thousands of URLs in a day; Google cannot. Shared progress bars mix those two clocks.
4. **Large sites starve small ones** — a single fat queue monopolizes the worker.

IndexFlow is built around those constraints, not around a demo sitemap.

---

## Features

- **Rust core** — Axum + Tokio + SQLx. Low memory, high concurrency, millions of URL rows and state transitions.
- **4 independent workspaces** under a persistent site header
  - **Sitemap Assets** — recursive XML / sitemap-index parser; locale, path prefix, lastmod, priority
  - **SEO Quality Gate** — standalone HTTP scanner (200, `<title>`, description, canonical, robots, H1). Does not enqueue submit workers
  - **Engine Submissions** — Bing IndexNow batches vs Google Indexing API (rolling 24h quota) as separate queues
  - **Index Monitoring** — GSC Search Analytics harvest + URL Inspection funnel (2,000/day)
- **GSC quota exemption** — pages with Search Analytics impressions &gt; 0 are tagged `INDEXED` / `google_status=SUBMITTED` and skip the daily 200 Indexing API slots
- **Per-URL diagnostics drawer** — live Re-check SEO, Submit to Bing/Google Now, meta signals, GSC coverage, raw API bodies
- **Cloudflare WAF bypass** — all page crawlers send a custom internal User-Agent
- **Multi-site fair scheduling** — windowed, partitioned claim so one large site cannot monopolize workers
- **Quota circuit breaker** — exhausted Google work sleeps until the next free slot. No busy-loop HTTP probes.
- **3-state conservation lifecycle** — `PENDING` + `SUBMITTED` + `BLOCKED` = `TOTAL`

Credential states: **Unset** / **Saved** / **Verified**. Submit only runs against verified channels.

![Dashboard](./images/首页.png)

---

## Tech stack

| Layer | Stack | Role |
| :--- | :--- | :--- |
| **Core engine** | Rust 2021, Axum, Tokio, SQLx | Async workers, state machine, API |
| **Workbench** | Next.js 15 (App Router), Tailwind CSS | Dark console, per-site workflow |
| **Database** | PostgreSQL 14+ | Lifecycle, partitioned indexes, optimistic locks |
| **Auth** | JWT + Google Service Account OAuth2 | Single-tenant admin + isolated credentials |

---

## Self-hosting

### Requirements

- Rust 1.75+
- PostgreSQL 14+
- Node.js 18+ (only if you rebuild the UI)

### Environment

Create a `.env` in the backend root:

```env
# Listen
SERVER_HOST=0.0.0.0
SERVER_PORT=8080

# Database
DATABASE_URL=postgres://postgres:password@127.0.0.1:5432/indexflow

# Google rolling 24-hour quota (default 200)
GOOGLE_DAILY_QUOTA=200

# GSC URL Inspection API rolling 24-hour quota (default 2000)
GSC_INSPECT_DAILY_QUOTA=2000

# Scheduler and worker throttle
SCHEDULER_INTERVAL_SECS=60
SUBMIT_WORKER_BATCH=50
SUBMIT_WORKER_INTERVAL_SECS=2

# JWT signing secret
JWT_SECRET=your-secure-jwt-secret-key-change-me
```

On first boot, SQLx applies `migrations/` automatically.

### Run

```bash
# Optional: rebuild the static workbench
cd ui && npm install && npm run build && cd ..

cargo run
```

On Windows, `start.ps1` builds `ui/out` if needed and starts the API. Open `http://127.0.0.1:<SERVER_PORT>/` (default in this repo: **8010**). The first visit prompts you to create the admin account. After that, add a site, paste an IndexNow key and/or Google Service Account JSON, click **Test Bing** / **Test Google** until the channel is **Verified**. Add the same service-account email as a user on the Search Console property for GSC sync. Then use the four workspace tabs independently: sync sitemap, run SEO audit, submit Bing/Google, sync indexed URLs from GSC.

### Tests

```bash
cargo check
cargo test
```

---

## How it works

```
[ Module 1: Sitemap Assets ]  ── source of truth (SYNC_SITEMAP only)
            │
            ├──► [ Module 2: SEO Quality Gate ]   CHECK_URL  (standalone)
            ├──► [ Module 3: Push Pipelines ]     SUBMIT_BING / SUBMIT_GOOGLE
            └──► [ Module 4: Index Monitor ]      GSC Analytics + GSC_INSPECT
```

- Sitemap sync never enqueues SEO or submit workers.
- SEO audit never enqueues Bing/Google submit.
- GSC Search Analytics (impressions &gt; 0) marks `google_index_status=INDEXED` and exempts those URLs from the Google Indexing API quota.
- GSC URL Inspection fills the funnel: Indexed / Crawled-not-indexed / Discovered-not-indexed / Unknown (max 2,000/day).
- Click a URL in any table to open the diagnostics drawer (`GET /urls/:id/analysis`, `POST /urls/:id/recheck`, `POST /urls/:id/submit-now`).

Stale `PROCESSING` tasks are recovered automatically after a timeout so a crash cannot deadlock the queue.

---

## License

Source-available Open Core. Use it, fork it, run it against your own sites.
