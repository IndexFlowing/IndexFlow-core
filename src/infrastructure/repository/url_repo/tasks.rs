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

    pub async fn fetch_pending_gsc(
        &self,
        limit: i64,
        boosted_ids: &[i64],
    ) -> anyhow::Result<Vec<Url>> {
        let (statement, bind_boosted) = if boosted_ids.is_empty() {
            ("SELECT * FROM urls WHERE gsc_index_status = 'UNKNOWN' ORDER BY priority ASC, id ASC LIMIT $1".to_string(), false)
        } else {
            let placeholders = (1..=boosted_ids.len())
                .map(|i| format!("${i}"))
                .collect::<Vec<_>>()
                .join(", ");
            (format!("SELECT * FROM urls WHERE gsc_index_status = 'UNKNOWN' ORDER BY (CASE WHEN id IN ({placeholders}) THEN 0 ELSE 1 END), priority ASC, id ASC LIMIT ${}", boosted_ids.len() + 1), true)
        };
        let mut query = sqlx::query_as::<_, Url>(&statement);
        if bind_boosted {
            for id in boosted_ids {
                query = query.bind(id);
            }
        }
        let rows = query.bind(limit).fetch_all(&self.pool).await?;
        Ok(rows)
    }

    pub async fn fetch_pending_bing_inspect(
        &self,
        limit: i64,
        boosted_ids: &[i64],
    ) -> anyhow::Result<Vec<Url>> {
        let (statement, bind_boosted) = if boosted_ids.is_empty() {
            ("SELECT * FROM urls WHERE bing_index_status = 'UNKNOWN' ORDER BY priority ASC, id ASC LIMIT $1".to_string(), false)
        } else {
            let placeholders = (1..=boosted_ids.len())
                .map(|i| format!("${i}"))
                .collect::<Vec<_>>()
                .join(", ");
            (format!("SELECT * FROM urls WHERE bing_index_status = 'UNKNOWN' ORDER BY (CASE WHEN id IN ({placeholders}) THEN 0 ELSE 1 END), priority ASC, id ASC LIMIT ${}", boosted_ids.len() + 1), true)
        };
        let mut query = sqlx::query_as::<_, Url>(&statement);
        if bind_boosted {
            for id in boosted_ids {
                query = query.bind(id);
            }
        }
        let rows = query.bind(limit).fetch_all(&self.pool).await?;
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
