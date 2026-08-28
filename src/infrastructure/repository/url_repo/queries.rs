use super::UrlRepo;
use crate::domain::{DashboardStats, Url};
use chrono::{DateTime, Utc};

impl UrlRepo {
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
                COUNT(CASE WHEN bing_index_status IN ('UNKNOWN', 'FAILED') THEN 1 END) AS bing_uninspected,
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

    /// 【核心升级】：支持雷达全维度组合筛选
    pub async fn list_filtered(
        &self,
        site_id: i64,
        page: i64,
        limit: i64,
        query_str: Option<&str>,
        seo_filter: Option<&str>,
        status_filter: Option<&str>,
        bing_filter: Option<&str>,
        google_filter: Option<&str>,
    ) -> anyhow::Result<(Vec<Url>, i64)> {
        let offset = (page.max(1) - 1) * limit;
        let q_pattern = query_str.map(|s| format!("%{}%", s.trim()));

        // 多引擎状态过滤器
        let (f_g_indexed, f_b_indexed, f_both_indexed, f_neither_indexed, f_not_indexed, f_unknown) = match status_filter {
            Some("G_INDEXED") | Some("INDEXED") => (true, false, false, false, false, false),
            Some("B_INDEXED") => (false, true, false, false, false, false),
            Some("BOTH_INDEXED") => (false, false, true, false, false, false),
            Some("NEITHER_INDEXED") => (false, false, false, true, false, false),
            Some("NOT_INDEXED") => (false, false, false, false, true, false),
            Some("UNKNOWN") => (false, false, false, false, false, true),
            _ => (false, false, false, false, false, false),
        };

        let has_filter = f_g_indexed || f_b_indexed || f_both_indexed || f_neither_indexed || f_not_indexed || f_unknown;

        let total: (i64,) = sqlx::query_as(
            r#"
            SELECT COUNT(*) FROM urls
            WHERE site_id = $1
              AND ($2 IS NULL OR url LIKE $2 OR page_title LIKE $2)
              AND ($3 IS NULL OR seo_status = $3)
              AND (
                  NOT $4
                  OR ($5 AND gsc_index_status = 'INDEXED')
                  OR ($6 AND bing_index_status = 'INDEXED')
                  OR ($7 AND gsc_index_status = 'INDEXED' AND bing_index_status = 'INDEXED')
                  OR ($8 AND gsc_index_status != 'INDEXED' AND bing_index_status != 'INDEXED')
                  OR ($9 AND gsc_index_status IN ('NOT_INDEXED', 'CRAWLED_NOT_INDEXED', 'DISCOVERED_NOT_INDEXED'))
                  OR ($10 AND gsc_index_status = 'UNKNOWN' AND bing_index_status = 'UNKNOWN')
              )
              AND ($11 IS NULL OR bing_status = $11)
              AND ($12 IS NULL OR google_status = $12)
            "#,
        )
        .bind(site_id)
        .bind(&q_pattern)
        .bind(seo_filter)
        .bind(has_filter)
        .bind(f_g_indexed)
        .bind(f_b_indexed)
        .bind(f_both_indexed)
        .bind(f_neither_indexed)
        .bind(f_not_indexed)
        .bind(f_unknown)
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
                  NOT $4
                  OR ($5 AND gsc_index_status = 'INDEXED')
                  OR ($6 AND bing_index_status = 'INDEXED')
                  OR ($7 AND gsc_index_status = 'INDEXED' AND bing_index_status = 'INDEXED')
                  OR ($8 AND gsc_index_status != 'INDEXED' AND bing_index_status != 'INDEXED')
                  OR ($9 AND gsc_index_status IN ('NOT_INDEXED', 'CRAWLED_NOT_INDEXED', 'DISCOVERED_NOT_INDEXED'))
                  OR ($10 AND gsc_index_status = 'UNKNOWN' AND bing_index_status = 'UNKNOWN')
              )
              AND ($11 IS NULL OR bing_status = $11)
              AND ($12 IS NULL OR google_status = $12)
            ORDER BY priority ASC, id DESC
            LIMIT $13 OFFSET $14
            "#,
        )
        .bind(site_id)
        .bind(&q_pattern)
        .bind(seo_filter)
        .bind(has_filter)
        .bind(f_g_indexed)
        .bind(f_b_indexed)
        .bind(f_both_indexed)
        .bind(f_neither_indexed)
        .bind(f_not_indexed)
        .bind(f_unknown)
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