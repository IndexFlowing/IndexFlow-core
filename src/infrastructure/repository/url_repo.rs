use crate::domain::{
    compute_url_priority, hash_url, DashboardStats, QualityGateResult, SitemapUrlEntry, Url,
};
use chrono::{DateTime, Utc};
use sqlx::SqlitePool;

#[derive(Clone)]
pub struct UrlRepo {
    pool: SqlitePool,
}

impl UrlRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    // ==========================================
    // 1. 资产发现与批量更新 (Discovery & Upsert)
    // ==========================================

    pub async fn batch_upsert_discovered(
        &self,
        site_id: i64,
        entries: &[SitemapUrlEntry],
    ) -> anyhow::Result<(u64, Vec<i64>, Vec<i64>)> {
        if entries.is_empty() {
            return Ok((0, Vec::new(), Vec::new()));
        }

        let mut tx = self.pool.begin().await?;
        let now = Utc::now();
        let mut inserted_count = 0u64;
        let mut all_ids = Vec::with_capacity(entries.len());
        let mut new_ids = Vec::new();

        for entry in entries {
            let hash = hash_url(&entry.loc);
            let prev: Option<(Option<DateTime<Utc>>,)> = sqlx::query_as(
                r#"SELECT sitemap_lastmod FROM urls WHERE site_id = $1 AND url_hash = $2"#,
            )
            .bind(site_id)
            .bind(&hash)
            .fetch_optional(&mut *tx)
            .await?;

            let is_new = prev.is_none();
            let previous_lastmod = prev.and_then(|p| p.0);
            let computed = compute_url_priority(
                entry.priority,
                entry.lastmod,
                previous_lastmod,
                is_new,
                now,
            );

            let row = sqlx::query_as::<_, (i64,)>(
                r#"
                INSERT INTO urls (
                    site_id, url, url_hash, seo_status, priority, sitemap_lastmod,
                    locale, path_prefix, first_seen_at, created_at, updated_at
                )
                VALUES ($1, $2, $3, 'PENDING', $4, $5, $6, $7, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)
                ON CONFLICT(url_hash) DO UPDATE SET
                    sitemap_lastmod = EXCLUDED.sitemap_lastmod,
                    priority = EXCLUDED.priority,
                    updated_at = CURRENT_TIMESTAMP
                RETURNING id
                "#,
            )
            .bind(site_id)
            .bind(&entry.loc)
            .bind(&hash)
            .bind(computed)
            .bind(entry.lastmod)
            .bind(&entry.locale)
            .bind(&entry.path_prefix)
            .fetch_one(&mut *tx)
            .await?;

            all_ids.push(row.0);
            if is_new {
                inserted_count += 1;
                new_ids.push(row.0);
            }
        }

        tx.commit().await?;
        Ok((inserted_count, all_ids, new_ids))
    }

    pub async fn batch_mark_gsc_indexed(
        &self,
        site_id: i64,
        urls: &[String],
    ) -> anyhow::Result<u64> {
        if urls.is_empty() {
            return Ok(0);
        }

        let mut tx = self.pool.begin().await?;
        let mut updated_total = 0u64;

        for chunk in urls.chunks(500) {
            for page_url in chunk {
                let hash = hash_url(page_url);
                let result = sqlx::query(
                    r#"
                    UPDATE urls
                    SET
                        gsc_index_status = 'INDEXED',
                        gsc_coverage_state = COALESCE(gsc_coverage_state, 'Indexed (Search Analytics Confirmed)'),
                        gsc_inspected_at = CURRENT_TIMESTAMP,
                        updated_at = CURRENT_TIMESTAMP
                    WHERE site_id = $1 AND url_hash = $2
                    "#,
                )
                .bind(site_id)
                .bind(&hash)
                .execute(&mut *tx)
                .await?;

                updated_total += result.rows_affected();
            }
        }

        tx.commit().await?;
        Ok(updated_total)
    }

    // ==========================================
    // 2. 看板统计与检索 (Queries & Stats)
    // ==========================================

    /// 查询当前站点的 Dashboard 看板统计（单条条件聚合 SQL，<3ms）
    pub async fn dashboard_stats(&self, site_id: i64) -> anyhow::Result<DashboardStats> {
        let row: (i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, Option<DateTime<Utc>>) = sqlx::query_as(
            r#"
            SELECT
                COUNT(*) AS url_total,
                COUNT(CASE WHEN gsc_index_status = 'INDEXED' THEN 1 END) AS google_indexed,
                COUNT(CASE WHEN gsc_index_status = 'CRAWLED_NOT_INDEXED' THEN 1 END) AS google_crawled_not_indexed,
                COUNT(CASE WHEN gsc_index_status = 'DISCOVERED_NOT_INDEXED' THEN 1 END) AS google_discovered_not_indexed,
                COUNT(CASE WHEN gsc_index_status IN ('NOT_INDEXED', 'CRAWLED_NOT_INDEXED', 'DISCOVERED_NOT_INDEXED') THEN 1 END) AS google_not_indexed,
                COUNT(CASE WHEN gsc_index_status = 'UNKNOWN' THEN 1 END) AS google_uninspected,
                COUNT(CASE WHEN seo_status = 'PASS' THEN 1 END) AS seo_passed,
                COUNT(CASE WHEN seo_status IN ('WARN', 'FAIL') THEN 1 END) AS seo_issues,
                COUNT(CASE WHEN (bing_status = 'NONE' OR google_status = 'NONE') AND seo_status != 'FAIL' THEN 1 END) AS pending_submit,
                COUNT(CASE WHEN datetime(gsc_inspected_at) > datetime('now', '-24 hours') THEN 1 END) AS gsc_used_24h,
                MAX(last_checked_at) AS last_seo_scan_at
            FROM urls
            WHERE site_id = $1
            "#,
        )
        .bind(site_id)
        .fetch_one(&self.pool)
        .await?;

        Ok(DashboardStats {
            url_total: row.0,
            google_indexed: row.1,
            google_crawled_not_indexed: row.2,
            google_discovered_not_indexed: row.3,
            google_not_indexed: row.4,
            google_uninspected: row.5,
            seo_passed: row.6,
            seo_issues: row.7,
            pending_submit: row.8,
            gsc_used_24h: row.9,
            last_seo_scan_at: row.10,
        })
    }

    pub async fn list_filtered(
        &self,
        site_id: i64,
        page: i64,
        limit: i64,
        query_str: Option<&str>,
        seo_filter: Option<&str>,
        gsc_filter: Option<&str>,
        bing_filter: Option<&str>,
        google_filter: Option<&str>,
    ) -> anyhow::Result<(Vec<Url>, i64)> {
        let offset = (page.max(1) - 1) * limit;
        let q_pattern = query_str.map(|s| format!("%{}%", s.trim()));

        let (gsc_exact, gsc_is_not_indexed) = match gsc_filter {
            Some("NOT_INDEXED") => (None, true),
            Some(other) if !other.is_empty() => (Some(other), false),
            _ => (None, false),
        };

        let total: (i64,) = sqlx::query_as(
            r#"
            SELECT COUNT(*) FROM urls
            WHERE site_id = $1
              AND ($2 IS NULL OR url LIKE $2 OR page_title LIKE $2)
              AND ($3 IS NULL OR seo_status = $3)
              AND (
                  ($4 IS NULL AND NOT $5)
                  OR ($5 AND gsc_index_status IN ('NOT_INDEXED', 'CRAWLED_NOT_INDEXED', 'DISCOVERED_NOT_INDEXED'))
                  OR ($4 IS NOT NULL AND gsc_index_status = $4)
              )
              AND ($6 IS NULL OR bing_status = $6)
              AND ($7 IS NULL OR google_status = $7)
            "#,
        )
        .bind(site_id)
        .bind(&q_pattern)
        .bind(seo_filter)
        .bind(gsc_exact)
        .bind(gsc_is_not_indexed)
        .bind(bing_filter)
        .bind(google_filter)
        .fetch_one(&self.pool)
        .await?;

        let items = sqlx::query_as::<_, Url>(
            r#"
            SELECT * FROM urls
            WHERE site_id = $1
              AND ($2 IS NULL OR url LIKE $2 OR page_title LIKE $2)
              AND ($3 IS NULL OR seo_status = $3)
              AND (
                  ($4 IS NULL AND NOT $5)
                  OR ($5 AND gsc_index_status IN ('NOT_INDEXED', 'CRAWLED_NOT_INDEXED', 'DISCOVERED_NOT_INDEXED'))
                  OR ($4 IS NOT NULL AND gsc_index_status = $4)
              )
              AND ($6 IS NULL OR bing_status = $6)
              AND ($7 IS NULL OR google_status = $7)
            ORDER BY priority ASC, id DESC
            LIMIT $8 OFFSET $9
            "#,
        )
        .bind(site_id)
        .bind(&q_pattern)
        .bind(seo_filter)
        .bind(gsc_exact)
        .bind(gsc_is_not_indexed)
        .bind(bing_filter)
        .bind(google_filter)
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?;

        Ok((items, total.0))
    }

    pub async fn find_by_id(&self, id: i64) -> anyhow::Result<Option<Url>> {
        let url = sqlx::query_as::<_, Url>(r#"SELECT * FROM urls WHERE id = $1"#)
            .bind(id)
            .fetch_optional(&self.pool)
            .await?;
        Ok(url)
    }

    // ==========================================
    // 3. Worker 待办队列调度 (Task Claiming)
    // ==========================================

    pub async fn fetch_pending_seo(&self, limit: i64) -> anyhow::Result<Vec<Url>> {
        let rows = sqlx::query_as::<_, Url>(
            r#"
            SELECT * FROM urls
            WHERE seo_status = 'PENDING'
            ORDER BY priority ASC, id ASC
            LIMIT $1
            "#,
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    pub async fn fetch_pending_gsc(&self, limit: i64) -> anyhow::Result<Vec<Url>> {
        let rows = sqlx::query_as::<_, Url>(
            r#"
            SELECT * FROM urls
            WHERE gsc_index_status = 'UNKNOWN'
            ORDER BY priority ASC, id ASC
            LIMIT $1
            "#,
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    pub async fn fetch_pending_bing(&self, limit: i64) -> anyhow::Result<Vec<Url>> {
        let rows = sqlx::query_as::<_, Url>(
            r#"
            SELECT * FROM urls
            WHERE seo_status != 'FAIL'
              AND bing_status IN ('NONE', 'FAILED')
            ORDER BY priority ASC, id ASC
            LIMIT $1
            "#,
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    pub async fn fetch_pending_google(&self, limit: i64) -> anyhow::Result<Vec<Url>> {
        let rows = sqlx::query_as::<_, Url>(
            r#"
            SELECT * FROM urls
            WHERE seo_status != 'FAIL'
              AND google_status IN ('NONE', 'FAILED')
              AND gsc_index_status != 'INDEXED'
            ORDER BY priority ASC, id ASC
            LIMIT $1
            "#,
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    // ==========================================
    // 4. 状态持久化回写 (Mutations)
    // ==========================================

    pub async fn persist_seo_scan(&self, id: i64, gate: &QualityGateResult) -> anyhow::Result<()> {
        let status = if gate.passed { "PASS" } else { "FAIL" };
        sqlx::query(
            r#"
            UPDATE urls
            SET
                http_status = $1,
                last_checked_at = CURRENT_TIMESTAMP,
                page_title = $2,
                canonical_url = $3,
                meta_description = $4,
                h1_content = $5,
                seo_status = $6,
                seo_issue = $7,
                updated_at = CURRENT_TIMESTAMP
            WHERE id = $8
            "#,
        )
        .bind(gate.http_status)
        .bind(gate.page_title.as_deref())
        .bind(gate.canonical_url.as_deref())
        .bind(gate.meta_description.as_deref())
        .bind(gate.h1_content.as_deref())
        .bind(status)
        .bind(gate.block_reason.as_deref())
        .bind(id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn apply_gsc_inspection(
        &self,
        id: i64,
        index_status: &str,
        coverage_state: Option<&str>,
        last_crawled_at: Option<DateTime<Utc>>,
    ) -> anyhow::Result<()> {
        sqlx::query(
            r#"
            UPDATE urls
            SET
                gsc_index_status = $2,
                gsc_coverage_state = $3,
                gsc_last_crawled_at = $4,
                gsc_inspected_at = CURRENT_TIMESTAMP,
                updated_at = CURRENT_TIMESTAMP
            WHERE id = $1
            "#,
        )
        .bind(id)
        .bind(index_status)
        .bind(coverage_state)
        .bind(last_crawled_at)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn apply_submit_outcome(
        &self,
        id: i64,
        bing_status: Option<&str>,
        bing_error: Option<&str>,
        google_status: Option<&str>,
        google_error: Option<&str>,
    ) -> anyhow::Result<()> {
        sqlx::query(
            r#"
            UPDATE urls
            SET
                bing_status = COALESCE($2, bing_status),
                bing_submitted_at = CASE WHEN $2 = 'SUBMITTED' THEN CURRENT_TIMESTAMP ELSE bing_submitted_at END,
                bing_error = $3,
                google_status = COALESCE($4, google_status),
                google_submitted_at = CASE WHEN $4 = 'SUBMITTED' THEN CURRENT_TIMESTAMP ELSE google_submitted_at END,
                google_error = $5,
                updated_at = CURRENT_TIMESTAMP
            WHERE id = $1
            "#,
        )
        .bind(id)
        .bind(bing_status)
        .bind(bing_error)
        .bind(google_status)
        .bind(google_error)
        .execute(&self.pool)
        .await?;
        Ok(())
    }
}