use super::UrlRepo;
use crate::domain::Url;

impl UrlRepo {
    /// 认领待执行技术 SEO 门禁质检的任务
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

    /// 认领待向 GSC 查询真实收录状态的任务
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

    /// 认领待向 Bing Webmaster API 查询收录状态的任务
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

    /// 认领待向 Bing IndexNow 推送的任务
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

    /// 认领待向 Google Indexing API 推送的任务
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