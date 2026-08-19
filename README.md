# IndexFlow

**Open-source search index infrastructure for high-volume sites.**

[English] | [中文](README_zh.md)

Engine-decoupled scheduling, an inline SEO quality gate, and a rolling 24-hour Google quota circuit breaker — built for programmatic SEO sites, indie hackers, and SEO engineers who outgrow “paste a sitemap and hope.”

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
- **Engine-decoupled pipelines**
  - **Bing / IndexNow** — high-throughput batch submit
  - **Google Indexing API** — consumes a rolling 24-hour quota window, not a naive UTC-day counter
- **Multi-site fair scheduling** — windowed, partitioned claim so one large site cannot monopolize workers
- **Inline SEO quality gate** — one lightweight GET immediately before submit
  - Blocks non-200 responses, `noindex`, canonical URL mismatch, and missing `<title>`
  - Protects both domain quality and scarce Google quota
- **Quota circuit breaker** — when Google is exhausted and Bing work is done, pending Google work sleeps until the next free slot. No busy-loop HTTP probes.
- **3-state conservation lifecycle** — `PENDING` + `SUBMITTED` + `BLOCKED` = `TOTAL`
  - Per-engine progress is independent
  - Dashboard shows queue depth and quota-release countdown

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

On Windows, `start.ps1` builds `ui/out` if needed and starts the API. The first visit prompts you to create the admin account. After that, add a site, paste an IndexNow key and/or Google Service Account JSON, click **Test Bing** / **Test Google** until the channel is **Verified**, then sync the sitemap and submit.

### Tests

```bash
cargo check
cargo test
```

---

## How it works

```
Site (independent work unit)
        │
        ▼
Sitemap discovery  →  PENDING
        │
        ▼
Inline SEO quality gate  (last millisecond before submit)
        │
   pass ├──────────► Bing / Google pipelines  →  SUBMITTED
        │
   fail └──────────► BLOCKED  (quality-gate intercept)
```

Bing and Google are separate task types (`SUBMIT_BING`, `SUBMIT_GOOGLE`). A URL can be submitted on Bing while still pending on Google. Stale `PROCESSING` tasks are recovered automatically after a timeout so a crash cannot deadlock the queue.

---

## License

Source-available Open Core. Use it, fork it, run it against your own sites.
