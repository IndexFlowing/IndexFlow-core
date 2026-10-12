# Sitemap Soft-Demote Absent URLs Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix the issue where sitemap updates do not demote URLs removed from the sitemap, causing the dashboard and sitemap asset totals to stay at historical counts (e.g., 260k instead of 5,008). Implement batch timestamp-based soft demotion (marking absent URLs as `gsc_orphan` while preserving history and allowing revival on future syncs), ensure `/sitemap` filters only active sitemap assets, and clean up existing dirty data.

**Architecture:** 
1. `UrlRepo::demote_absent_sitemap_urls` marks URLs for a site whose `discovered_via = 'sitemap'` and `sitemap_synced_at < sync_start_time` as `discovered_via = 'gsc_orphan'`.
2. `UrlRepo::batch_upsert_discovered` sets `discovered_via = 'sitemap'` on `ON CONFLICT` so demoted URLs can revive if they reappear in a future sitemap.
3. `SyncWorker` captures `sync_start_time = chrono::Utc::now()` before fetching entries and calls `demote_absent_sitemap_urls` after upserting all chunks.
4. `UrlRepo::list_filtered` and `UrlService::list_filtered` support a `discovered_via` filter parameter (`Option<&str>`), which `/sitemap` sets to `Some("sitemap")`.
5. Run migration SQL to demote stale sitemap URLs for site 2 that have not been synced in the last 30 minutes.

**Tech Stack:** Rust 2021, Axum 0.7, SQLx 0.8 (SQLite), Chrono, Tracing.

**Spec:** User task requirement document `任务需求说明书（面向 Antigravity）`.

## Global Constraints
- Compile with `cargo check` and `cargo test` with zero warnings and zero errors.
- Never delete rows from `urls`; soft-demote to `discovered_via = 'gsc_orphan'`.
- Ensure datetime comparisons in SQLite handle ISO-8601 vs SQLite datetime formats using `datetime(...)`.
- Allow previously demoted URLs to revive as `sitemap` on subsequent sitemap fetches.
- Ensure the `/sitemap` page shows the exact current sitemap asset count (e.g. 5,008).

---

### Task 1: Add `demote_absent_sitemap_urls` and Revival Logic to `UrlRepo`

**Files:**
- Modify: `src/infrastructure/repository/url_repo/discovery.rs`
- Test: `src/infrastructure/repository/url_repo/discovery.rs` (or `queries.rs`)

**Interfaces:**
- Consumes: `site_id: i64`, `sync_started_at: chrono::DateTime<chrono::Utc>`, `self.pool: SqlitePool`
- Produces: `pub async fn demote_absent_sitemap_urls(&self, site_id: i64, sync_started_at: DateTime<Utc>) -> anyhow::Result<u64>`

- [ ] **Step 1: Write tests for sitemap soft demotion and revival**

Add unit tests in `src/infrastructure/repository/url_repo/discovery.rs`:
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::SitemapUrlEntry;
    use sqlx::SqlitePool;

    #[tokio::test]
    async fn demote_absent_sitemap_urls_and_revival() -> anyhow::Result<()> {
        let pool = SqlitePool::connect("sqlite::memory:").await?;
        sqlx::migrate!("./migrations").run(&pool).await?;
        let repo = UrlRepo::new(pool);

        let entry1 = SitemapUrlEntry {
            loc: "https://example.com/keep".into(),
            lastmod: None,
            priority: None,
            locale: None,
            path_prefix: None,
        };
        let entry2 = SitemapUrlEntry {
            loc: "https://example.com/drop".into(),
            lastmod: None,
            priority: None,
            locale: None,
            path_prefix: None,
        };

        // First batch: both inserted as sitemap
        repo.batch_upsert_discovered(1, &[entry1.clone(), entry2.clone()]).await?;

        // Simulate time passing: manually set drop's sitemap_synced_at to an older time
        sqlx::query(
            "UPDATE urls SET sitemap_synced_at = '2026-01-01 00:00:00' WHERE url = 'https://example.com/drop'"
        )
        .execute(&repo.pool)
        .await?;

        let sync_start = Utc::now();

        // entry1 is refreshed
        repo.batch_upsert_discovered(1, &[entry1.clone()]).await?;

        // Demote absent URLs
        let demoted = repo.demote_absent_sitemap_urls(1, sync_start).await?;
        assert_eq!(demoted, 1);

        let drop_url = repo.find_by_id(2).await?.unwrap();
        assert_eq!(drop_url.discovered_via, "gsc_orphan");

        let keep_url = repo.find_by_id(1).await?.unwrap();
        assert_eq!(keep_url.discovered_via, "sitemap");

        // Revival test: drop reappears in sitemap
        repo.batch_upsert_discovered(1, &[entry2.clone()]).await?;
        let revived_url = repo.find_by_id(2).await?.unwrap();
        assert_eq!(revived_url.discovered_via, "sitemap");

        Ok(())
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test demote_absent_sitemap_urls_and_revival`
Expected: FAIL with method not found.

- [ ] **Step 3: Implement `demote_absent_sitemap_urls` and revival in `discovery.rs`**

In `src/infrastructure/repository/url_repo/discovery.rs`:
1. In `batch_upsert_discovered`, update `ON CONFLICT(url_hash) DO UPDATE SET`:
```rust
                ON CONFLICT(url_hash) DO UPDATE SET
                    sitemap_lastmod = EXCLUDED.sitemap_lastmod,
                    sitemap_synced_at = CURRENT_TIMESTAMP,
                    discovered_via = 'sitemap',
                    priority = EXCLUDED.priority,
                    updated_at = CURRENT_TIMESTAMP
```
2. Add `demote_absent_sitemap_urls`:
```rust
impl UrlRepo {
    /// 将本次同步中未出现的 Sitemap URL 降级为孤岛状态 (gsc_orphan)
    /// 既不破坏历史诊断/收录历史，又让 Sitemap 视图精准匹配当前最新版本
    pub async fn demote_absent_sitemap_urls(
        &self,
        site_id: i64,
        sync_started_at: chrono::DateTime<chrono::Utc>,
    ) -> anyhow::Result<u64> {
        let res = sqlx::query(
            r#"
            UPDATE urls 
            SET discovered_via = 'gsc_orphan',
                updated_at = CURRENT_TIMESTAMP
            WHERE site_id = $1 
              AND discovered_via = 'sitemap'
              AND (sitemap_synced_at IS NULL OR datetime(sitemap_synced_at) < datetime($2))
            "#,
        )
        .bind(site_id)
        .bind(sync_started_at)
        .execute(&self.pool)
        .await?;

        Ok(res.rows_affected())
    }
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test demote_absent_sitemap_urls_and_revival`
Expected: PASS.

- [ ] **Step 5: Commit changes**

Commit changes for `discovery.rs`.

---

### Task 2: Update `SyncWorker` to Trigger Demotion on Sync Completion

**Files:**
- Modify: `src/workers/sync_worker.rs`

**Interfaces:**
- Consumes: `self.urls.demote_absent_sitemap_urls(site.id, sync_start_time)`
- Produces: Log output: `📦 [SyncWorker] 已将本次 Sitemap 未包含的陈旧 URL 降级为 gsc_orphan (孤岛资产)`

- [ ] **Step 1: Record `sync_start_time` and call `demote_absent_sitemap_urls` in `sync_worker.rs`**

In `src/workers/sync_worker.rs`:
1. Before `expand_to_page_entries`:
```rust
            let started = std::time::Instant::now();
            let sync_start_time = chrono::Utc::now();
            let (sitemap_type, entries) = match self
```
2. After the `entries.chunks(chunk_size)` loop and before stopping the pipeline:
```rust
            if !entries.is_empty() {
                match self.urls.demote_absent_sitemap_urls(site.id, sync_start_time).await {
                    Ok(demoted_count) => {
                        if demoted_count > 0 {
                            info!(
                                site_id = site.id,
                                demoted_count,
                                "📦 [SyncWorker] 已将本次 Sitemap 未包含的陈旧 URL 降级为 gsc_orphan (孤岛资产)"
                            );
                        }
                    }
                    Err(e) => {
                        error!(error = %e, site_id = site.id, "降级陈旧 Sitemap URL 失败");
                    }
                }
            }
```

- [ ] **Step 2: Run `cargo check` and verify compilation**

Run: `cargo check`
Expected: Success with no errors or warnings.

- [ ] **Step 3: Commit changes**

Commit changes for `sync_worker.rs`.

---

### Task 3: Support `discovered_via` Filter in Queries and Update `/sitemap` Route

**Files:**
- Modify: `src/infrastructure/repository/url_repo/queries.rs`
- Modify: `src/application/url_service.rs`
- Modify: `src/api/handlers/sitemap.rs`
- Modify: `src/api/handlers/indexing.rs`
- Modify: `src/api/handlers/submit.rs`
- Modify: `src/api/handlers/seo.rs`
- Modify: `src/api/handlers/monitoring.rs`

**Interfaces:**
- Consumes: `discovered_via: Option<&str>` in `list_filtered`
- Produces: Accurate counts and listing filtered by `discovered_via` (specifically `Some("sitemap")` for `/sitemap`)

- [ ] **Step 1: Update `UrlRepo::list_filtered` and `dashboard_stats` in `queries.rs`**

In `src/infrastructure/repository/url_repo/queries.rs`:
1. Update `dashboard_stats`:
```sql
COUNT(CASE WHEN discovered_via = 'sitemap' THEN 1 END) AS url_total,
```
2. Update `list_filtered` signature from `orphan_only: bool` to `discovered_via: Option<&str>`:
```rust
    pub async fn list_filtered(
        &self,
        site_id: i64,
        page: i64,
        limit: i64,
        query_str: Option<&str>,
        seo_filter: Option<&str>,
        gsc_status_filter: Option<&str>,
        bing_status_filter: Option<&str>,
        discovered_via: Option<&str>,
        bing_filter: Option<&str>,
        google_filter: Option<&str>,
    ) -> anyhow::Result<(Vec<Url>, i64)>
```
3. Update SQL in `list_filtered` for both COUNT and SELECT:
Replace `AND (NOT $6 OR discovered_via = 'gsc_orphan')` with:
`AND ($6 IS NULL OR discovered_via = $6)`
and bind `discovered_via`.

- [ ] **Step 2: Update `UrlService::list_filtered` in `src/application/url_service.rs`**

Update `list_filtered` in `src/application/url_service.rs`:
```rust
    pub async fn list_filtered(
        &self,
        site_id: i64,
        page: i64,
        limit: i64,
        query_str: Option<&str>,
        seo_filter: Option<&str>,
        gsc_status_filter: Option<&str>,
        bing_status_filter: Option<&str>,
        discovered_via: Option<&str>,
        bing_filter: Option<&str>,
        google_filter: Option<&str>,
    ) -> anyhow::Result<(Vec<Url>, i64)> {
        self.urls
            .list_filtered(
                site_id,
                page,
                limit,
                query_str,
                seo_filter,
                gsc_status_filter,
                bing_status_filter,
                discovered_via,
                bing_filter,
                google_filter,
            )
            .await
    }
```
And `list`:
```rust
    pub async fn list(
        &self,
        site_id: i64,
        page: i64,
        limit: i64,
    ) -> anyhow::Result<(Vec<Url>, i64)> {
        self.urls
            .list_filtered(
                site_id, page, limit, None, None, None, None, None, None, None,
            )
            .await
    }
```

- [ ] **Step 3: Update call sites in web handlers**

1. In `src/api/handlers/sitemap.rs`:
Pass `Some("sitemap")` for `discovered_via`:
```rust
    let (items, total) = state
        .url_service
        .list_filtered(
            current_site_id,
            page,
            limit,
            q.q.as_deref(),
            None,
            None,
            None,
            Some("sitemap"),
            None,
            None,
        )
        .await
        .unwrap_or_default();
```
2. In `src/api/handlers/indexing.rs`:
Map orphan filter:
```rust
    let orphan_only = q.orphan.as_deref() == Some("1") || q.orphan.as_deref() == Some("true");
    let discovered_via_filter = if orphan_only { Some("gsc_orphan") } else { None };
    let (items, total) = state
        .url_service
        .list_filtered(
            current_site_id,
            page,
            limit,
            q.q.as_deref(),
            None,
            q.gsc_status.as_deref(),
            q.bing_status.as_deref(),
            discovered_via_filter,
            None,
            None,
        )
        .await
        .unwrap_or_default();
```
3. In `src/api/handlers/submit.rs`, `src/api/handlers/seo.rs`, `src/api/handlers/monitoring.rs`:
Pass `None` for `discovered_via`.

- [ ] **Step 4: Update tests in `queries.rs`**

Update `list_filtered_applies_filters_and_pagination` to use `Option<&str>` and add assertions for `Some("sitemap")`.

- [ ] **Step 5: Run tests and verify**

Run: `cargo test`
Expected: All 13+ tests pass.

- [ ] **Step 6: Commit changes**

Commit changes for `queries.rs`, `url_service.rs`, and handlers.

---

### Task 4: Execute Database Migration and Clean Up Dirty Data

**Files:**
- Create: `migrations/009_demote_stale_sitemap_urls.sql`

**Interfaces:**
- Consumes: Existing SQLite database `indexflow.db`
- Produces: Site 2 `discovered_via = 'sitemap'` count drops from 260,311 to exactly 5,008.

- [ ] **Step 1: Create migration file `migrations/009_demote_stale_sitemap_urls.sql`**

```sql
-- Demote historical stale sitemap URLs that were removed from the sitemap
UPDATE urls 
SET discovered_via = 'gsc_orphan',
    updated_at = CURRENT_TIMESTAMP
WHERE site_id = 2 
  AND discovered_via = 'sitemap' 
  AND sitemap_synced_at < datetime('now', '-30 minutes');
```

- [ ] **Step 2: Execute migration on `indexflow.db`**

Run:
```bash
sqlite3 indexflow.db "UPDATE urls SET discovered_via = 'gsc_orphan', updated_at = CURRENT_TIMESTAMP WHERE site_id = 2 AND discovered_via = 'sitemap' AND sitemap_synced_at < datetime('now', '-30 minutes');"
```

- [ ] **Step 3: Verify URL counts**

Verify:
```bash
sqlite3 indexflow.db "SELECT discovered_via, COUNT(*) FROM urls WHERE site_id = 2 GROUP BY discovered_via;"
```
Expected:
`sitemap|5008`
`gsc_orphan|255303`

- [ ] **Step 4: Commit migration**

Commit `migrations/009_demote_stale_sitemap_urls.sql`.

---

### Task 5: End-to-End Verification

**Files:**
- Verify all modified files

- [ ] **Step 1: Run `cargo check` and `cargo test`**

Run: `cargo check`
Run: `cargo test`
Verify zero warnings, zero errors.

- [ ] **Step 2: Verify git status and diff**

Run: `git status`
Run: `git diff`
Ensure clean working tree without unexpected edits.
