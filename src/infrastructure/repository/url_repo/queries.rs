use super::UrlRepo;
use crate::domain::{DashboardStats, Url};
use chrono::{DateTime, Utc};

#[derive(sqlx::FromRow)]
struct DashboardStatsRaw {
    url_total: i64,
    google_indexed: i64,
    google_crawled_not_indexed: i64,
    google_discovered_not_indexed: i64,
    google_not_indexed: i64,
    google_uninspected: i64,
    bing_indexed: i64,
    bing_not_indexed: i64,
    bing_failed: i64,
    bing_uninspected: i64,
    seo_passed: i64,
    seo_issues: i64,
    seo_warnings: i64,
    pending_submit: i64,
    pending_google_submit: i64,
    pending_bing_submit: i64,
    gsc_used_24h: i64,
    last_seo_scan_at: Option<DateTime<Utc>>,
}

impl UrlRepo {
    pub async fn fetch_watched(&self, site_id: i64) -> anyhow::Result<Vec<Url>> {
        Ok(sqlx::query_as::<_, Url>("SELECT * FROM urls WHERE site_id = $1 AND is_watched = 1 ORDER BY watched_at DESC, id DESC")
            .bind(site_id).fetch_all(&self.pool).await?)
    }
    pub async fn dashboard_stats(&self, site_id: i64) -> anyhow::Result<DashboardStats> {
        let row: DashboardStatsRaw = sqlx::query_as(
            r#"
            SELECT
                COUNT(*) AS url_total,
                COUNT(CASE WHEN gsc_index_status = 'INDEXED' THEN 1 END) AS google_indexed,
                COUNT(CASE WHEN gsc_index_status = 'CRAWLED_NOT_INDEXED' THEN 1 END) AS google_crawled_not_indexed,
                COUNT(CASE WHEN gsc_index_status = 'DISCOVERED_NOT_INDEXED' THEN 1 END) AS google_discovered_not_indexed,
                COUNT(CASE WHEN gsc_index_status = 'NOT_INDEXED' THEN 1 END) AS google_not_indexed,
                COUNT(CASE WHEN gsc_index_status = 'UNKNOWN' THEN 1 END) AS google_uninspected,
                COUNT(CASE WHEN bing_index_status = 'INDEXED' THEN 1 END) AS bing_indexed,
                COUNT(CASE WHEN bing_index_status = 'NOT_INDEXED' THEN 1 END) AS bing_not_indexed,
                COUNT(CASE WHEN bing_index_status = 'FAILED' THEN 1 END) AS bing_failed,
                COUNT(CASE WHEN bing_index_status = 'UNKNOWN' THEN 1 END) AS bing_uninspected,
                COUNT(CASE WHEN seo_status = 'PASS' THEN 1 END) AS seo_passed,
                COUNT(CASE WHEN seo_status = 'FAIL' THEN 1 END) AS seo_issues,
                COUNT(CASE WHEN seo_status = 'WARN' THEN 1 END) AS seo_warnings,
                COUNT(CASE WHEN ((bing_status = 'NONE' AND bing_index_status != 'INDEXED') OR (google_status = 'NONE' AND gsc_index_status != 'INDEXED')) AND seo_status != 'FAIL' THEN 1 END) AS pending_submit,
                COUNT(CASE WHEN google_status IN ('NONE', 'FAILED') AND gsc_index_status != 'INDEXED' AND seo_status != 'FAIL' THEN 1 END) AS pending_google_submit,
                COUNT(CASE WHEN bing_status IN ('NONE', 'FAILED') AND bing_index_status != 'INDEXED' AND seo_status != 'FAIL' THEN 1 END) AS pending_bing_submit,
                COUNT(CASE WHEN datetime(gsc_inspected_at) > datetime('now', '-24 hours') THEN 1 END) AS gsc_used_24h,
                MAX(last_checked_at) AS last_seo_scan_at
            FROM urls
            WHERE site_id = $1 AND discovered_via = 'sitemap'
            "#,
        )
        .bind(site_id)
        .fetch_one(&self.pool)
        .await?;

        Ok(DashboardStats {
            url_total: row.url_total,
            google_indexed: row.google_indexed,
            google_crawled_not_indexed: row.google_crawled_not_indexed,
            google_discovered_not_indexed: row.google_discovered_not_indexed,
            google_not_indexed: row.google_not_indexed,
            google_uninspected: row.google_uninspected,
            bing_indexed: row.bing_indexed,
            bing_not_indexed: row.bing_not_indexed,
            bing_failed: row.bing_failed,
            bing_uninspected: row.bing_uninspected,
            seo_passed: row.seo_passed,
            seo_issues: row.seo_issues,
            seo_warnings: row.seo_warnings,
            pending_submit: row.pending_submit,
            pending_google_submit: row.pending_google_submit,
            pending_bing_submit: row.pending_bing_submit,
            gsc_used_24h: row.gsc_used_24h,
            last_seo_scan_at: row.last_seo_scan_at,
        })
    }

    /// 支持全维度条件与「孤岛资产」/「Sitemap 纳管资产」严格隔离过滤
    pub async fn list_filtered(
        &self,
        site_id: i64,
        page: i64,
        limit: i64,
        query_str: Option<&str>,
        seo_filter: Option<&str>,
        gsc_status_filter: Option<&str>,
        bing_status_filter: Option<&str>,
        orphan_only: bool,
        bing_filter: Option<&str>,
        google_filter: Option<&str>,
    ) -> anyhow::Result<(Vec<Url>, i64)> {
        let offset = (page.max(1) - 1) * limit;
        let q_pattern = query_str.map(|s| format!("%{}%", s.trim()));

        let total: (i64,) = sqlx::query_as(
            r#"
            SELECT COUNT(*) FROM urls
            WHERE site_id = $1
              AND ($2 IS NULL OR url LIKE $2 OR page_title LIKE $2)
              AND ($3 IS NULL OR seo_status = $3)
              AND ($4 IS NULL OR gsc_index_status = $4)
              AND ($5 IS NULL OR bing_index_status = $5)
              AND (
                  CASE 
                      WHEN $6 THEN discovered_via = 'gsc_orphan'
                      ELSE discovered_via = 'sitemap'
                  END
              )
              AND ($7 IS NULL OR bing_status = $7)
              AND ($8 IS NULL OR google_status = $8)
            "#,
        )
        .bind(site_id)
        .bind(&q_pattern)
        .bind(seo_filter)
        .bind(gsc_status_filter)
        .bind(bing_status_filter)
        .bind(orphan_only)
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
              AND ($4 IS NULL OR gsc_index_status = $4)
              AND ($5 IS NULL OR bing_index_status = $5)
              AND (
                  CASE 
                      WHEN $6 THEN discovered_via = 'gsc_orphan'
                      ELSE discovered_via = 'sitemap'
                  END
              )
              AND ($7 IS NULL OR bing_status = $7)
              AND ($8 IS NULL OR google_status = $8)
            ORDER BY priority ASC, id DESC
            LIMIT $9 OFFSET $10
            "#,
        )
        .bind(site_id)
        .bind(&q_pattern)
        .bind(seo_filter)
        .bind(gsc_status_filter)
        .bind(bing_status_filter)
        .bind(orphan_only)
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

    pub async fn find_by_ids(&self, ids: &[i64]) -> anyhow::Result<Vec<Url>> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        let placeholders = (1..=ids.len())
            .map(|i| format!("${i}"))
            .collect::<Vec<_>>()
            .join(", ");
        let statement = format!("SELECT * FROM urls WHERE id IN ({placeholders})");
        let mut query = sqlx::query_as::<_, Url>(&statement);
        for id in ids {
            query = query.bind(id);
        }
        Ok(query.fetch_all(&self.pool).await?)
    }
}

#[cfg(test)]
mod tests {
    use super::UrlRepo;
    use sqlx::SqlitePool;

    #[tokio::test]
    async fn list_filtered_applies_filters_and_pagination() -> anyhow::Result<()> {
        let pool = SqlitePool::connect("sqlite::memory:").await?;
        sqlx::migrate!("./migrations").run(&pool).await?;

        sqlx::query(
            "INSERT INTO urls (site_id, url, url_hash, seo_status, page_title, gsc_index_status, bing_index_status, priority, discovered_via)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(1_i64)
        .bind("https://example.com/first")
        .bind("hash-first")
        .bind("PASS")
        .bind("First")
        .bind("INDEXED")
        .bind("INDEXED")
        .bind(10_i32)
        .bind("sitemap")
        .execute(&pool)
        .await?;

        sqlx::query(
            "INSERT INTO urls (site_id, url, url_hash, seo_status, page_title, gsc_index_status, bing_index_status, priority, discovered_via)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(1_i64)
        .bind("https://example.com/orphan")
        .bind("hash-orphan")
        .bind("PASS")
        .bind("Orphan")
        .bind("INDEXED")
        .bind("NOT_INDEXED")
        .bind(20_i32)
        .bind("gsc_orphan")
        .execute(&pool)
        .await?;

        sqlx::query(
            "INSERT INTO urls (site_id, url, url_hash, seo_status, page_title, gsc_index_status, bing_index_status, priority, discovered_via)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(1_i64)
        .bind("https://example.com/third")
        .bind("hash-third")
        .bind("WARN")
        .bind("Third")
        .bind("UNKNOWN")
        .bind("UNKNOWN")
        .bind(30_i32)
        .bind("sitemap")
        .execute(&pool)
        .await?;

        let repo = UrlRepo::new(pool);

        let (page_two, total) = repo
            .list_filtered(1, 2, 1, None, None, None, None, false, None, None)
            .await?;
        assert_eq!(total, 2);
        assert_eq!(page_two.len(), 1);
        assert_eq!(page_two[0].url, "https://example.com/third");

        let (filtered, total) = repo
            .list_filtered(
                1,
                1,
                50,
                None,
                None,
                Some("INDEXED"),
                Some("NOT_INDEXED"),
                true,
                None,
                None,
            )
            .await?;
        assert_eq!(total, 1);
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].url, "https://example.com/orphan");

        // Verify dashboard_stats only aggregates sitemap assets
        let stats = repo.dashboard_stats(1).await?;
        assert_eq!(stats.url_total, 2);
        assert_eq!(stats.seo_passed, 1);
        assert_eq!(stats.seo_issues, 0); // "third" is WARN, not FAIL
        assert_eq!(stats.seo_warnings, 1);
        assert_eq!(stats.audited_total(), 2);
        assert_eq!(stats.pending_seo_audit(), 0);
        assert_eq!(stats.seo_health_score(), 100);
        assert_eq!(stats.pending_google_submit, 1);
        assert_eq!(stats.pending_bing_submit, 1);

        Ok(())
    }
}
