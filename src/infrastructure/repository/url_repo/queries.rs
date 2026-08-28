use super::UrlRepo;
use crate::domain::{DashboardStats, Url};
use chrono::{DateTime, Utc};

impl UrlRepo {
    /// 查询站点的 5 问指标看板（单条条件聚合 SQL，<3ms）
    pub async fn dashboard_stats(&self, site_id: i64) -> anyhow::Result<DashboardStats> {
        let row: (
            i64,
            i64,
            i64,
            i64,
            i64,
            i64,
            i64,
            i64,
            i64,
            i64,
            i64,
            i64,
            i64,
            Option<DateTime<Utc>>,
        ) = sqlx::query_as(
            r#"
            SELECT
                COUNT(*) AS url_total,
                COUNT(CASE WHEN gsc_index_status = 'INDEXED' THEN 1 END) AS google_indexed,
                COUNT(CASE WHEN gsc_index_status = 'CRAWLED_NOT_INDEXED' THEN 1 END) AS google_crawled_not_indexed,
                COUNT(CASE WHEN gsc_index_status = 'DISCOVERED_NOT_INDEXED' THEN 1 END) AS google_discovered_not_indexed,
                COUNT(CASE WHEN gsc_index_status IN ('NOT_INDEXED', 'CRAWLED_NOT_INDEXED', 'DISCOVERED_NOT_INDEXED') THEN 1 END) AS google_not_indexed,
                COUNT(CASE WHEN gsc_index_status = 'UNKNOWN' THEN 1 END) AS google_uninspected,
                COUNT(CASE WHEN bing_index_status = 'INDEXED' THEN 1 END) AS bing_indexed,
                COUNT(CASE WHEN bing_index_status = 'NOT_INDEXED' THEN 1 END) AS bing_not_indexed,
                COUNT(CASE WHEN bing_index_status = 'UNKNOWN' THEN 1 END) AS bing_uninspected,
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
            bing_indexed: row.6,
            bing_not_indexed: row.7,
            bing_uninspected: row.8,
            seo_passed: row.9,
            seo_issues: row.10,
            pending_submit: row.11,
            gsc_used_24h: row.12,
            last_seo_scan_at: row.13,
        })
    }

    /// 多维动态条件筛选与分页列表
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
}