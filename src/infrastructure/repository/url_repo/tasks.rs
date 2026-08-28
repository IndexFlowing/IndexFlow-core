use super::UrlRepo;
use crate::domain::Url;

impl UrlRepo {
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

    pub async fn fetch_pending_bing_inspect(&self, limit: i64) -> anyhow::Result<Vec<Url>> {
        let rows = sqlx::query_as::<_, Url>(
            r#"
            SELECT * FROM urls
            WHERE bing_index_status = 'UNKNOWN'
            ORDER BY priority ASC, id ASC
            LIMIT $1
            "#,
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    /// Bing 推送队列：仅抓取「门禁通过 + Bing 尚未收录」的 URL，已收录的直接豁免！
    pub async fn fetch_pending_bing(&self, limit: i64) -> anyhow::Result<Vec<Url>> {
        let rows = sqlx::query_as::<_, Url>(
            r#"
            SELECT * FROM urls
            WHERE seo_status != 'FAIL'
              AND bing_status IN ('NONE', 'FAILED')
              AND bing_index_status != 'INDEXED'
            ORDER BY priority ASC, id ASC
            LIMIT $1
            "#,
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    /// Google 推送队列：仅抓取「门禁通过 + Google 尚未收录」的 URL，已收录的直接豁免！
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
}