use super::UrlRepo;
use crate::domain::Url;

impl UrlRepo {
    pub async fn fetch_pending_seo(&self, site_id: i64, limit: i64) -> anyhow::Result<Vec<Url>> {
        let rows = sqlx::query_as::<_, Url>(
            r#"
            SELECT * FROM urls
            WHERE site_id = $1 AND seo_status = 'PENDING'
            ORDER BY priority ASC, id ASC
            LIMIT $2
            "#,
        )
        .bind(site_id)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    pub async fn fetch_pending_gsc(
        &self,
        site_id: i64,
        limit: i64,
        boosted_ids: &[i64],
    ) -> anyhow::Result<Vec<Url>> {
        let (statement, bind_boosted) = if boosted_ids.is_empty() {
            (
                "SELECT * FROM urls WHERE site_id = $1 AND gsc_index_status = 'UNKNOWN' ORDER BY priority ASC, id ASC LIMIT $2".to_string(),
                false,
            )
        } else {
            let placeholders = (2..=boosted_ids.len() + 1)
                .map(|i| format!("${i}"))
                .collect::<Vec<_>>()
                .join(", ");
            let limit_idx = boosted_ids.len() + 2;
            (
                format!(
                    "SELECT * FROM urls WHERE site_id = $1 AND gsc_index_status = 'UNKNOWN' ORDER BY (CASE WHEN id IN ({placeholders}) THEN 0 ELSE 1 END), priority ASC, id ASC LIMIT ${limit_idx}"
                ),
                true,
            )
        };
        let mut query = sqlx::query_as::<_, Url>(&statement).bind(site_id);
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
        site_id: i64,
        limit: i64,
        boosted_ids: &[i64],
    ) -> anyhow::Result<Vec<Url>> {
        let (statement, bind_boosted) = if boosted_ids.is_empty() {
            (
                "SELECT * FROM urls WHERE site_id = $1 AND bing_index_status = 'UNKNOWN' ORDER BY priority ASC, id ASC LIMIT $2".to_string(),
                false,
            )
        } else {
            let placeholders = (2..=boosted_ids.len() + 1)
                .map(|i| format!("${i}"))
                .collect::<Vec<_>>()
                .join(", ");
            let limit_idx = boosted_ids.len() + 2;
            (
                format!(
                    "SELECT * FROM urls WHERE site_id = $1 AND bing_index_status = 'UNKNOWN' ORDER BY (CASE WHEN id IN ({placeholders}) THEN 0 ELSE 1 END), priority ASC, id ASC LIMIT ${limit_idx}"
                ),
                true,
            )
        };
        let mut query = sqlx::query_as::<_, Url>(&statement).bind(site_id);
        if bind_boosted {
            for id in boosted_ids {
                query = query.bind(id);
            }
        }
        let rows = query.bind(limit).fetch_all(&self.pool).await?;
        Ok(rows)
    }

    /// Bing 推送队列：仅抓取「指定站点 + 门禁通过 + Bing 尚未收录」的 URL，已收录的直接豁免！
    pub async fn fetch_pending_bing(&self, site_id: i64, limit: i64) -> anyhow::Result<Vec<Url>> {
        let rows = sqlx::query_as::<_, Url>(
            r#"
            SELECT * FROM urls
            WHERE site_id = $1
              AND seo_status != 'FAIL'
              AND bing_status IN ('NONE', 'FAILED')
              AND bing_index_status != 'INDEXED'
            ORDER BY priority ASC, id ASC
            LIMIT $2
            "#,
        )
        .bind(site_id)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    /// Google 推送队列：仅抓取「指定站点 + 门禁通过 + Google 尚未收录」的 URL，已收录的直接豁免！
    pub async fn fetch_pending_google(&self, site_id: i64, limit: i64) -> anyhow::Result<Vec<Url>> {
        let rows = sqlx::query_as::<_, Url>(
            r#"
            SELECT * FROM urls
            WHERE site_id = $1
              AND seo_status != 'FAIL'
              AND google_status IN ('NONE', 'FAILED')
              AND gsc_index_status != 'INDEXED'
            ORDER BY priority ASC, id ASC
            LIMIT $2
            "#,
        )
        .bind(site_id)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }
}