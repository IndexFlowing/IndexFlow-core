use crate::domain::IndexStatusHistory;
use chrono::{DateTime, Utc};
use sqlx::SqlitePool;

#[derive(Clone)]
pub struct IndexHistoryRepo {
    pool: SqlitePool,
}

impl IndexHistoryRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn insert_if_changed(
        &self,
        url_id: i64,
        provider: &str,
        index_status: &str,
        coverage_state: Option<&str>,
        last_crawled_at: Option<DateTime<Utc>>,
    ) -> anyhow::Result<()> {
        let latest: Option<(String,)> = sqlx::query_as(
            "SELECT index_status FROM index_status_history WHERE url_id = $1 AND provider = $2 ORDER BY checked_at DESC, id DESC LIMIT 1",
        )
        .bind(url_id)
        .bind(provider)
        .fetch_optional(&self.pool)
        .await?;
        if latest.as_ref().is_some_and(|row| row.0 == index_status) {
            return Ok(());
        }
        sqlx::query(
            "INSERT INTO index_status_history (url_id, provider, index_status, coverage_state, last_crawled_at) VALUES ($1, $2, $3, $4, $5)",
        )
        .bind(url_id).bind(provider).bind(index_status).bind(coverage_state).bind(last_crawled_at)
        .execute(&self.pool).await?;
        Ok(())
    }

    pub async fn list_by_url(&self, url_id: i64) -> anyhow::Result<Vec<IndexStatusHistory>> {
        Ok(sqlx::query_as::<_, IndexStatusHistory>(
            "SELECT * FROM index_status_history WHERE url_id = $1 ORDER BY checked_at ASC, id ASC",
        )
        .bind(url_id)
        .fetch_all(&self.pool)
        .await?)
    }
}
