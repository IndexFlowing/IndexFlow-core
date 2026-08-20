use crate::domain::{
    compute_url_priority, google_is_indexed, hash_url, QualityGateResult, SitemapUrlEntry, Url,
    UrlStatus, GINDEX_INDEXED,
};
use chrono::{DateTime, Utc};
use sqlx::PgPool;

#[derive(Debug, Clone, serde::Serialize, sqlx::FromRow)]
pub struct SiteUrlStats {
    pub site_id: i64,
    pub url_total: i64,
    pub pending: i64,
    pub submitted: i64,
    pub blocked: i64,
    /// Bing IndexNow accepted (independent of overall 3-state).
    pub bing_submitted_count: i64,
    /// Bing never attempted and not SEO-blocked.
    pub bing_pending_count: i64,
    /// Google Indexing API accepted (independent of overall 3-state).
    pub google_submitted_count: i64,
    /// Google never attempted and not SEO-blocked.
    pub google_pending_count: i64,
}

#[derive(Debug, Clone, serde::Serialize, sqlx::FromRow)]
pub struct LocaleCount {
    pub locale: String,
    pub count: i64,
}

#[derive(Debug, Clone, serde::Serialize, sqlx::FromRow)]
pub struct PathPrefixCount {
    pub path_prefix: String,
    pub count: i64,
}

/// URL row for the single-site workbench table.
#[derive(Debug, Clone, serde::Serialize, sqlx::FromRow)]
pub struct UrlDiagnostic {
    pub id: i64,
    pub site_id: i64,
    pub url: String,
    pub status: String,
    pub locale: String,
    pub path_prefix: String,
    pub page_title: Option<String>,
    pub canonical_url: Option<String>,
    pub block_reason: Option<String>,
    pub bing_status: String,
    pub google_status: String,
    pub bing_submitted_at: Option<DateTime<Utc>>,
    pub google_submitted_at: Option<DateTime<Utc>>,
    pub bing_error: Option<String>,
    pub google_error: Option<String>,
    pub queue_status: Option<String>,
    pub priority: i32,
    pub sitemap_priority: Option<f64>,
    pub sitemap_lastmod: Option<DateTime<Utc>>,
    pub last_http_status: Option<i32>,
    pub last_checked_at: Option<DateTime<Utc>>,
    pub last_submitted_at: Option<DateTime<Utc>>,
    pub meta_description: Option<String>,
    pub h1_content: Option<String>,
    pub google_index_status: String,
    pub google_coverage_state: Option<String>,
    pub google_last_crawled_at: Option<DateTime<Utc>>,
    pub google_inspected_at: Option<DateTime<Utc>>,
    pub bing_index_status: String,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, serde::Serialize, sqlx::FromRow)]
pub struct HttpStatusCount {
    pub http_status: Option<i32>,
    pub count: i64,
}

#[derive(Debug, Clone, serde::Serialize, sqlx::FromRow)]
pub struct BlockReasonCount {
    pub reason: String,
    pub count: i64,
}

#[derive(Debug, Clone, serde::Serialize, sqlx::FromRow)]
pub struct SeoStats {
    pub site_id: i64,
    pub checked: i64,
    pub unchecked: i64,
    pub blocked: i64,
}

#[derive(Debug, Clone, serde::Serialize, sqlx::FromRow)]
pub struct IndexFunnelStats {
    pub site_id: i64,
    pub indexed: i64,
    pub crawled_not_indexed: i64,
    pub discovered_not_indexed: i64,
    pub unknown: i64,
    pub inspected_24h: i64,
}

#[derive(Clone)]
pub struct UrlRepo {
    pool: PgPool,
}

impl UrlRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Batch upsert sitemap entries.
    /// New URLs → PENDING. Existing with a newer `<lastmod>` → reset to PENDING.
    /// Unchanged URLs keep their current status.
    /// Returns (inserted_count, all_ids, newly_inserted_ids).
    pub async fn batch_upsert_discovered(
        &self,
        site_id: i64,
        entries: &[SitemapUrlEntry],
    ) -> anyhow::Result<(u64, Vec<i64>, Vec<i64>)> {
        let mut inserted = 0u64;
        let mut all_ids = Vec::with_capacity(entries.len());
        let mut new_ids = Vec::new();
        let now = Utc::now();

        for entry in entries {
            let url_hash = hash_url(&entry.loc);

            let prev: Option<(Option<DateTime<Utc>>,)> = sqlx::query_as(
                r#"SELECT sitemap_lastmod FROM urls WHERE site_id = $1 AND url_hash = $2"#,
            )
            .bind(site_id)
            .bind(&url_hash)
            .fetch_optional(&self.pool)
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

            let row = sqlx::query_as::<_, (i64, bool)>(
                r#"
                INSERT INTO urls (
                    site_id, url, url_hash, status,
                    priority, sitemap_priority, sitemap_lastmod,
                    locale, path_prefix,
                    first_seen_at, last_seen_at, next_check_at
                )
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, NOW(), NOW(), NULL)
                ON CONFLICT (site_id, url_hash) DO UPDATE
                    SET
                        last_seen_at = NOW(),
                        updated_at = NOW(),
                        sitemap_priority = EXCLUDED.sitemap_priority,
                        sitemap_lastmod = EXCLUDED.sitemap_lastmod,
                        locale = EXCLUDED.locale,
                        path_prefix = EXCLUDED.path_prefix,
                        priority = EXCLUDED.priority,
                        status = CASE
                            WHEN EXCLUDED.sitemap_lastmod IS NOT NULL
                             AND (
                                 urls.sitemap_lastmod IS NULL
                                 OR EXCLUDED.sitemap_lastmod > urls.sitemap_lastmod
                             )
                            THEN 'PENDING'
                            ELSE urls.status
                        END,
                        block_reason = CASE
                            WHEN EXCLUDED.sitemap_lastmod IS NOT NULL
                             AND (
                                 urls.sitemap_lastmod IS NULL
                                 OR EXCLUDED.sitemap_lastmod > urls.sitemap_lastmod
                             )
                            THEN NULL
                            ELSE urls.block_reason
                        END,
                        bing_status = CASE
                            WHEN EXCLUDED.sitemap_lastmod IS NOT NULL
                             AND (
                                 urls.sitemap_lastmod IS NULL
                                 OR EXCLUDED.sitemap_lastmod > urls.sitemap_lastmod
                             )
                            THEN 'NONE'
                            ELSE urls.bing_status
                        END,
                        google_status = CASE
                            WHEN EXCLUDED.sitemap_lastmod IS NOT NULL
                             AND (
                                 urls.sitemap_lastmod IS NULL
                                 OR EXCLUDED.sitemap_lastmod > urls.sitemap_lastmod
                             )
                            THEN 'NONE'
                            ELSE urls.google_status
                        END,
                        bing_error = CASE
                            WHEN EXCLUDED.sitemap_lastmod IS NOT NULL
                             AND (
                                 urls.sitemap_lastmod IS NULL
                                 OR EXCLUDED.sitemap_lastmod > urls.sitemap_lastmod
                             )
                            THEN NULL
                            ELSE urls.bing_error
                        END,
                        google_error = CASE
                            WHEN EXCLUDED.sitemap_lastmod IS NOT NULL
                             AND (
                                 urls.sitemap_lastmod IS NULL
                                 OR EXCLUDED.sitemap_lastmod > urls.sitemap_lastmod
                             )
                            THEN NULL
                            ELSE urls.google_error
                        END
                RETURNING id, (xmax = 0) AS is_insert
                "#,
            )
            .bind(site_id)
            .bind(&entry.loc)
            .bind(&url_hash)
            .bind(UrlStatus::Pending.as_str())
            .bind(computed)
            .bind(entry.priority)
            .bind(entry.lastmod)
            .bind(&entry.locale)
            .bind(&entry.path_prefix)
            .fetch_one(&self.pool)
            .await?;

            all_ids.push(row.0);
            if row.1 {
                inserted += 1;
                new_ids.push(row.0);
            }
        }

        Ok((inserted, all_ids, new_ids))
    }

    pub async fn find_by_id(&self, id: i64) -> anyhow::Result<Option<Url>> {
        let url = sqlx::query_as::<_, Url>(r#"SELECT * FROM urls WHERE id = $1"#)
            .bind(id)
            .fetch_optional(&self.pool)
            .await?;
        Ok(url)
    }

    pub async fn find_by_ids(&self, ids: &[i64]) -> anyhow::Result<Vec<Url>> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        let rows = sqlx::query_as::<_, Url>(r#"SELECT * FROM urls WHERE id = ANY($1)"#)
            .bind(ids)
            .fetch_all(&self.pool)
            .await?;
        Ok(rows)
    }

    pub async fn list_by_site(
        &self,
        site_id: i64,
        status: Option<&str>,
        locale: Option<&str>,
        path_prefix: Option<&str>,
        page: i64,
        limit: i64,
    ) -> anyhow::Result<(Vec<Url>, i64)> {
        let offset = (page.max(1) - 1) * limit;

        let total: (i64,) = sqlx::query_as(
            r#"
            SELECT COUNT(*) FROM urls
            WHERE site_id = $1
              AND ($2::text IS NULL OR status = $2)
              AND ($3::text IS NULL OR locale = $3)
              AND ($4::text IS NULL OR path_prefix = $4)
            "#,
        )
        .bind(site_id)
        .bind(status)
        .bind(locale)
        .bind(path_prefix)
        .fetch_one(&self.pool)
        .await?;

        let rows = sqlx::query_as::<_, Url>(
            r#"
            SELECT * FROM urls
            WHERE site_id = $1
              AND ($2::text IS NULL OR status = $2)
              AND ($3::text IS NULL OR locale = $3)
              AND ($4::text IS NULL OR path_prefix = $4)
            ORDER BY
                CASE status
                    WHEN 'PENDING' THEN 0
                    WHEN 'BLOCKED' THEN 1
                    ELSE 2
                END,
                priority ASC,
                id DESC
            LIMIT $5 OFFSET $6
            "#,
        )
        .bind(site_id)
        .bind(status)
        .bind(locale)
        .bind(path_prefix)
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?;

        Ok((rows, total.0))
    }

    /// Diagnostic list scoped to a single site (workbench table).
    pub async fn list_diagnostics(
        &self,
        site_id: i64,
        status: Option<&str>,
        locale: Option<&str>,
        path_prefix: Option<&str>,
        page: i64,
        limit: i64,
        seo_checked: Option<bool>,
        google_index_status: Option<&str>,
    ) -> anyhow::Result<(Vec<UrlDiagnostic>, i64)> {
        let offset = (page.max(1) - 1) * limit;

        let total: (i64,) = sqlx::query_as(
            r#"
            SELECT COUNT(*) FROM urls
            WHERE site_id = $1
              AND ($2::text IS NULL OR status = $2)
              AND ($3::text IS NULL OR locale = $3)
              AND ($4::text IS NULL OR path_prefix = $4)
              AND ($5::bool IS NULL OR ($5::bool = (last_checked_at IS NOT NULL)))
              AND ($6::text IS NULL OR google_index_status = $6)
            "#,
        )
        .bind(site_id)
        .bind(status)
        .bind(locale)
        .bind(path_prefix)
        .bind(seo_checked)
        .bind(google_index_status)
        .fetch_one(&self.pool)
        .await?;

        let rows = sqlx::query_as::<_, UrlDiagnostic>(
            r#"
            SELECT
                u.id,
                u.site_id,
                u.url,
                u.status,
                u.locale,
                u.path_prefix,
                u.page_title,
                u.canonical_url,
                u.block_reason,
                u.bing_status,
                u.google_status,
                u.bing_submitted_at,
                u.google_submitted_at,
                u.bing_error,
                u.google_error,
                t.queue_status,
                u.priority,
                u.sitemap_priority,
                u.sitemap_lastmod,
                u.last_http_status,
                u.last_checked_at,
                u.last_submitted_at,
                u.meta_description,
                u.h1_content,
                u.google_index_status,
                u.google_coverage_state,
                u.google_last_crawled_at,
                u.google_inspected_at,
                u.bing_index_status,
                u.updated_at
            FROM urls u
            LEFT JOIN LATERAL (
                SELECT status AS queue_status
                FROM tasks
                WHERE url_id = u.id
                  AND task_type IN ('SUBMIT_URL', 'SUBMIT_BING', 'SUBMIT_GOOGLE', 'CHECK_URL', 'GSC_INSPECT')
                  AND status IN ('PENDING', 'PROCESSING')
                ORDER BY CASE status WHEN 'PROCESSING' THEN 0 ELSE 1 END, id DESC
                LIMIT 1
            ) t ON TRUE
            WHERE u.site_id = $1
              AND ($2::text IS NULL OR u.status = $2)
              AND ($3::text IS NULL OR u.locale = $3)
              AND ($4::text IS NULL OR u.path_prefix = $4)
              AND ($7::bool IS NULL OR ($7::bool = (u.last_checked_at IS NOT NULL)))
              AND ($8::text IS NULL OR u.google_index_status = $8)
            ORDER BY
                CASE u.status
                    WHEN 'PENDING' THEN 0
                    WHEN 'BLOCKED' THEN 1
                    ELSE 2
                END,
                u.priority ASC,
                u.updated_at DESC
            LIMIT $5 OFFSET $6
            "#,
        )
        .bind(site_id)
        .bind(status)
        .bind(locale)
        .bind(path_prefix)
        .bind(limit)
        .bind(offset)
        .bind(seo_checked)
        .bind(google_index_status)
        .fetch_all(&self.pool)
        .await?;

        Ok((rows, total.0))
    }

    #[allow(dead_code)]
    pub async fn list_ids_by_status(
        &self,
        site_id: i64,
        status: &str,
        limit: i64,
    ) -> anyhow::Result<Vec<i64>> {
        let rows: Vec<(i64,)> = sqlx::query_as(
            r#"
            SELECT id FROM urls
            WHERE site_id = $1 AND status = $2
            ORDER BY priority ASC, id ASC
            LIMIT $3
            "#,
        )
        .bind(site_id)
        .bind(status)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.into_iter().map(|r| r.0).collect())
    }

    /// IDs that still need a submit on at least one **enabled** engine.
    ///
    /// Excludes SEO-blocked rows. Overall `status` is ignored — a URL marked
    /// `SUBMITTED` after Bing-only success is still returned when Google is
    /// enabled and `google_status` is `NONE` / `FAILED`.
    pub async fn list_pending_submit_ids(
        &self,
        site_id: i64,
        has_bing: bool,
        has_google: bool,
        limit: i64,
    ) -> anyhow::Result<Vec<i64>> {
        if !has_bing && !has_google {
            return Ok(Vec::new());
        }
        let rows: Vec<(i64,)> = sqlx::query_as(
            r#"
            SELECT id FROM urls
            WHERE site_id = $1
              AND status <> 'BLOCKED'
              AND (
                    ($2::bool AND bing_status IN ('NONE', 'FAILED'))
                 OR ($3::bool AND google_status IN ('NONE', 'FAILED')
                     AND google_index_status <> 'INDEXED')
              )
            ORDER BY priority ASC, id ASC
            LIMIT $4
            "#,
        )
        .bind(site_id)
        .bind(has_bing)
        .bind(has_google)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.into_iter().map(|r| r.0).collect())
    }

    #[allow(dead_code)]
    pub async fn apply_gate_result(
        &self,
        id: i64,
        http_status: Option<i32>,
        page_title: Option<&str>,
        canonical_url: Option<&str>,
    ) -> anyhow::Result<()> {
        sqlx::query(
            r#"
            UPDATE urls
            SET
                last_http_status = $1,
                last_checked_at = NOW(),
                page_title = $2,
                canonical_url = $3,
                updated_at = NOW()
            WHERE id = $4
            "#,
        )
        .bind(http_status)
        .bind(page_title)
        .bind(canonical_url)
        .bind(id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Persist a full SEO scan: diagnostic signals + BLOCKED/unblock.
    /// Passing a previously BLOCKED URL returns it to PENDING (does not demote SUBMITTED).
    pub async fn persist_seo_scan(&self, id: i64, gate: &QualityGateResult) -> anyhow::Result<()> {
        sqlx::query(
            r#"
            UPDATE urls
            SET
                last_http_status = $1,
                last_checked_at = NOW(),
                page_title = $2,
                canonical_url = $3,
                meta_description = $4,
                h1_content = $5,
                status = CASE
                    WHEN $6::bool THEN CASE WHEN status = 'BLOCKED' THEN 'PENDING' ELSE status END
                    ELSE 'BLOCKED'
                END,
                block_reason = CASE WHEN $6::bool THEN NULL ELSE $7 END,
                updated_at = NOW()
            WHERE id = $8
            "#,
        )
        .bind(gate.http_status)
        .bind(gate.page_title.as_deref())
        .bind(gate.canonical_url.as_deref())
        .bind(gate.meta_description.as_deref())
        .bind(gate.h1_content.as_deref())
        .bind(gate.passed)
        .bind(gate.block_reason.as_deref())
        .bind(id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// IDs for the standalone SEO scanner.
    pub async fn list_seo_audit_ids(
        &self,
        site_id: i64,
        unchecked_only: bool,
        limit: i64,
    ) -> anyhow::Result<Vec<i64>> {
        let rows: Vec<(i64,)> = if unchecked_only {
            sqlx::query_as(
                r#"
                SELECT id FROM urls
                WHERE site_id = $1 AND last_checked_at IS NULL
                ORDER BY priority ASC, id ASC
                LIMIT $2
                "#,
            )
            .bind(site_id)
            .bind(limit)
            .fetch_all(&self.pool)
            .await?
        } else {
            sqlx::query_as(
                r#"
                SELECT id FROM urls
                WHERE site_id = $1
                ORDER BY priority ASC, id ASC
                LIMIT $2
                "#,
            )
            .bind(site_id)
            .bind(limit)
            .fetch_all(&self.pool)
            .await?
        };
        Ok(rows.into_iter().map(|r| r.0).collect())
    }

    /// Unindexed URLs eligible for GSC URL Inspection (respects 24h inspect cooldown).
    pub async fn list_gsc_inspect_ids(
        &self,
        site_id: i64,
        limit: i64,
    ) -> anyhow::Result<Vec<i64>> {
        let rows: Vec<(i64,)> = sqlx::query_as(
            r#"
            SELECT id FROM urls
            WHERE site_id = $1
              AND google_index_status <> 'INDEXED'
              AND (google_inspected_at IS NULL
                   OR google_inspected_at < NOW() - INTERVAL '24 hours')
            ORDER BY google_inspected_at NULLS FIRST, priority ASC, id ASC
            LIMIT $2
            "#,
        )
        .bind(site_id)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.into_iter().map(|r| r.0).collect())
    }

    /// Bulk-tag ranking pages (Search Analytics impressions > 0) as INDEXED.
    /// Also marks `google_status = SUBMITTED` so they skip the Indexing API quota.
    /// Does not change BLOCKED lifecycle (SEO issues remain visible).
    pub async fn mark_gsc_indexed_pages(
        &self,
        site_id: i64,
        pages: &[String],
        bing_enabled: bool,
    ) -> anyhow::Result<u64> {
        if pages.is_empty() {
            return Ok(0);
        }
        let result = sqlx::query(
            r#"
            UPDATE urls
            SET
                google_index_status = $3,
                google_coverage_state = COALESCE(
                    google_coverage_state,
                    'Indexed (Search Analytics impressions > 0)'
                ),
                google_status = 'SUBMITTED',
                google_submitted_at = COALESCE(google_submitted_at, NOW()),
                google_error = NULL,
                status = CASE
                    WHEN status = 'BLOCKED' THEN status
                    WHEN $4::bool AND bing_status NOT IN ('SUBMITTED') THEN 'PENDING'
                    ELSE 'SUBMITTED'
                END,
                updated_at = NOW()
            WHERE site_id = $1
              AND (
                    url = ANY($2)
                 OR rtrim(url, '/') = ANY(
                        SELECT rtrim(p, '/') FROM unnest($2::text[]) AS p
                    )
              )
            "#,
        )
        .bind(site_id)
        .bind(pages)
        .bind(GINDEX_INDEXED)
        .bind(bing_enabled)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected())
    }

    pub async fn apply_gsc_inspection(
        &self,
        id: i64,
        index_status: &str,
        coverage_state: Option<&str>,
        last_crawled_at: Option<DateTime<Utc>>,
        bing_enabled: bool,
    ) -> anyhow::Result<()> {
        let indexed = google_is_indexed(index_status);
        sqlx::query(
            r#"
            UPDATE urls
            SET
                google_index_status = $2,
                google_coverage_state = $3,
                google_last_crawled_at = COALESCE($4, google_last_crawled_at),
                google_inspected_at = NOW(),
                google_status = CASE WHEN $5::bool THEN 'SUBMITTED' ELSE google_status END,
                google_submitted_at = CASE
                    WHEN $5::bool THEN COALESCE(google_submitted_at, NOW())
                    ELSE google_submitted_at
                END,
                google_error = CASE WHEN $5::bool THEN NULL ELSE google_error END,
                status = CASE
                    WHEN NOT $5::bool THEN status
                    WHEN status = 'BLOCKED' THEN status
                    WHEN $6::bool AND bing_status NOT IN ('SUBMITTED') THEN 'PENDING'
                    ELSE 'SUBMITTED'
                END,
                updated_at = NOW()
            WHERE id = $1
            "#,
        )
        .bind(id)
        .bind(index_status)
        .bind(coverage_state)
        .bind(last_crawled_at)
        .bind(indexed)
        .bind(bing_enabled)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn mark_gsc_inspected_error(&self, id: i64, error: &str) -> anyhow::Result<()> {
        sqlx::query(
            r#"
            UPDATE urls
            SET
                google_inspected_at = NOW(),
                google_coverage_state = $2,
                updated_at = NOW()
            WHERE id = $1
            "#,
        )
        .bind(id)
        .bind(error)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn seo_stats(&self, site_id: i64) -> anyhow::Result<SeoStats> {
        let row = sqlx::query_as::<_, SeoStats>(
            r#"
            SELECT
                $1::bigint AS site_id,
                COUNT(*) FILTER (WHERE last_checked_at IS NOT NULL)::bigint AS checked,
                COUNT(*) FILTER (WHERE last_checked_at IS NULL)::bigint AS unchecked,
                COUNT(*) FILTER (WHERE status = 'BLOCKED')::bigint AS blocked
            FROM urls
            WHERE site_id = $1
            "#,
        )
        .bind(site_id)
        .fetch_one(&self.pool)
        .await?;
        Ok(row)
    }

    pub async fn http_status_breakdown(
        &self,
        site_id: i64,
    ) -> anyhow::Result<Vec<HttpStatusCount>> {
        let rows = sqlx::query_as::<_, HttpStatusCount>(
            r#"
            SELECT last_http_status AS http_status, COUNT(*)::bigint AS count
            FROM urls
            WHERE site_id = $1 AND last_http_status IS NOT NULL
            GROUP BY last_http_status
            ORDER BY count DESC
            "#,
        )
        .bind(site_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    pub async fn block_reason_breakdown(
        &self,
        site_id: i64,
    ) -> anyhow::Result<Vec<BlockReasonCount>> {
        let rows = sqlx::query_as::<_, BlockReasonCount>(
            r#"
            SELECT COALESCE(block_reason, '(unspecified)') AS reason,
                   COUNT(*)::bigint AS count
            FROM urls
            WHERE site_id = $1 AND status = 'BLOCKED'
            GROUP BY COALESCE(block_reason, '(unspecified)')
            ORDER BY count DESC
            LIMIT 20
            "#,
        )
        .bind(site_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    pub async fn index_funnel_stats(&self, site_id: i64) -> anyhow::Result<IndexFunnelStats> {
        let row = sqlx::query_as::<_, IndexFunnelStats>(
            r#"
            SELECT
                $1::bigint AS site_id,
                COUNT(*) FILTER (WHERE google_index_status = 'INDEXED')::bigint AS indexed,
                COUNT(*) FILTER (WHERE google_index_status = 'CRAWLED_NOT_INDEXED')::bigint AS crawled_not_indexed,
                COUNT(*) FILTER (WHERE google_index_status = 'DISCOVERED_NOT_INDEXED')::bigint AS discovered_not_indexed,
                COUNT(*) FILTER (
                    WHERE google_index_status IS NULL
                       OR google_index_status = 'UNKNOWN'
                )::bigint AS unknown,
                COUNT(*) FILTER (
                    WHERE google_inspected_at IS NOT NULL
                      AND google_inspected_at > NOW() - INTERVAL '24 hours'
                )::bigint AS inspected_24h
            FROM urls
            WHERE site_id = $1
            "#,
        )
        .bind(site_id)
        .fetch_one(&self.pool)
        .await?;
        Ok(row)
    }

    pub async fn mark_blocked(&self, id: i64, reason: &str) -> anyhow::Result<()> {
        sqlx::query(
            r#"
            UPDATE urls
            SET status = $1, block_reason = $2, updated_at = NOW()
            WHERE id = $3
            "#,
        )
        .bind(UrlStatus::Blocked.as_str())
        .bind(reason)
        .bind(id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Persist lifecycle + per-engine outcomes after a submit attempt.
    /// `bing_status` / `google_status` are `Some("SUBMITTED"|"FAILED")` when that engine was tried.
    /// Pass `None` to leave an engine unchanged (already SUBMITTED, or not attempted).
    pub async fn apply_submit_outcome(
        &self,
        id: i64,
        overall: UrlStatus,
        block_reason: Option<&str>,
        bing_status: Option<&str>,
        bing_error: Option<&str>,
        google_status: Option<&str>,
        google_error: Option<&str>,
    ) -> anyhow::Result<()> {
        sqlx::query(
            r#"
            UPDATE urls
            SET
                status = $1,
                block_reason = CASE WHEN $1 = 'SUBMITTED' THEN NULL ELSE COALESCE($2, block_reason) END,
                last_submitted_at = CASE
                    WHEN $3 = 'SUBMITTED' OR $5 = 'SUBMITTED' OR $1 = 'SUBMITTED'
                    THEN NOW()
                    ELSE last_submitted_at
                END,
                bing_status = COALESCE($3, bing_status),
                bing_submitted_at = CASE WHEN $3 = 'SUBMITTED' THEN NOW() ELSE bing_submitted_at END,
                bing_error = CASE
                    WHEN $3 = 'SUBMITTED' THEN NULL
                    WHEN $3 = 'FAILED' THEN $4
                    ELSE bing_error
                END,
                google_status = COALESCE($5, google_status),
                google_submitted_at = CASE WHEN $5 = 'SUBMITTED' THEN NOW() ELSE google_submitted_at END,
                google_error = CASE
                    WHEN $5 = 'SUBMITTED' THEN NULL
                    WHEN $5 = 'FAILED' THEN $6
                    ELSE google_error
                END,
                updated_at = NOW()
            WHERE id = $7
            "#,
        )
        .bind(overall.as_str())
        .bind(block_reason)
        .bind(bing_status)
        .bind(bing_error)
        .bind(google_status)
        .bind(google_error)
        .bind(id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// 3-state stats for one site, optionally sliced by locale / path_prefix.
    pub async fn site_three_state(
        &self,
        site_id: i64,
        locale: Option<&str>,
        path_prefix: Option<&str>,
    ) -> anyhow::Result<SiteUrlStats> {
        let row = sqlx::query_as::<_, SiteUrlStats>(
            r#"
            SELECT
                $1::bigint AS site_id,
                COUNT(*)::bigint AS url_total,
                COUNT(*) FILTER (WHERE status = 'PENDING')::bigint AS pending,
                COUNT(*) FILTER (WHERE status = 'SUBMITTED')::bigint AS submitted,
                COUNT(*) FILTER (WHERE status = 'BLOCKED')::bigint AS blocked,
                COUNT(*) FILTER (WHERE bing_status = 'SUBMITTED')::bigint AS bing_submitted_count,
                COUNT(*) FILTER (WHERE bing_status = 'NONE' AND status <> 'BLOCKED')::bigint AS bing_pending_count,
                COUNT(*) FILTER (WHERE google_status = 'SUBMITTED')::bigint AS google_submitted_count,
                COUNT(*) FILTER (WHERE google_status = 'NONE' AND status <> 'BLOCKED')::bigint AS google_pending_count
            FROM urls
            WHERE site_id = $1
              AND ($2::text IS NULL OR locale = $2)
              AND ($3::text IS NULL OR path_prefix = $3)
            "#,
        )
        .bind(site_id)
        .bind(locale)
        .bind(path_prefix)
        .fetch_one(&self.pool)
        .await?;
        Ok(row)
    }

    pub async fn stats_grouped_by_site(&self) -> anyhow::Result<Vec<SiteUrlStats>> {
        let rows = sqlx::query_as::<_, SiteUrlStats>(
            r#"
            SELECT
                site_id,
                COUNT(*)::bigint AS url_total,
                COUNT(*) FILTER (WHERE status = 'PENDING')::bigint AS pending,
                COUNT(*) FILTER (WHERE status = 'SUBMITTED')::bigint AS submitted,
                COUNT(*) FILTER (WHERE status = 'BLOCKED')::bigint AS blocked,
                COUNT(*) FILTER (WHERE bing_status = 'SUBMITTED')::bigint AS bing_submitted_count,
                COUNT(*) FILTER (WHERE bing_status = 'NONE' AND status <> 'BLOCKED')::bigint AS bing_pending_count,
                COUNT(*) FILTER (WHERE google_status = 'SUBMITTED')::bigint AS google_submitted_count,
                COUNT(*) FILTER (WHERE google_status = 'NONE' AND status <> 'BLOCKED')::bigint AS google_pending_count
            FROM urls
            GROUP BY site_id
            "#,
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    pub async fn list_locales(
        &self,
        site_id: i64,
        path_prefix: Option<&str>,
    ) -> anyhow::Result<Vec<LocaleCount>> {
        let rows = sqlx::query_as::<_, LocaleCount>(
            r#"
            SELECT locale, COUNT(*)::bigint AS count
            FROM urls
            WHERE site_id = $1
              AND ($2::text IS NULL OR path_prefix = $2)
            GROUP BY locale
            ORDER BY locale
            "#,
        )
        .bind(site_id)
        .bind(path_prefix)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    pub async fn list_path_prefixes(
        &self,
        site_id: i64,
        locale: Option<&str>,
    ) -> anyhow::Result<Vec<PathPrefixCount>> {
        let rows = sqlx::query_as::<_, PathPrefixCount>(
            r#"
            SELECT path_prefix, COUNT(*)::bigint AS count
            FROM urls
            WHERE site_id = $1
              AND ($2::text IS NULL OR locale = $2)
            GROUP BY path_prefix
            ORDER BY path_prefix
            "#,
        )
        .bind(site_id)
        .bind(locale)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }
}
