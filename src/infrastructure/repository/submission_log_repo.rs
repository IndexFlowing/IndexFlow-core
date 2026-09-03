use crate::domain::{ProviderKind, SubmissionLog};
use chrono::{DateTime, Duration, Utc};
use sqlx::SqlitePool;

#[derive(Clone)]
pub struct SubmissionLogRepo {
    pool: SqlitePool,
}

impl SubmissionLogRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn insert(
        &self,
        url_id: i64,
        provider: ProviderKind,
        success: bool,
        response_code: Option<i32>,
        response_body: Option<&str>,
    ) -> anyhow::Result<SubmissionLog> {
        let row = sqlx::query_as::<_, SubmissionLog>(
            r#"
            INSERT INTO submission_logs (url_id, provider, success, response_code, response_body, created_at)
            VALUES ($1, $2, $3, $4, $5, CURRENT_TIMESTAMP)
            RETURNING *
            "#,
        )
        .bind(url_id)
        .bind(provider.as_str())
        .bind(if success { 1 } else { 0 })
        .bind(response_code)
        .bind(response_body)
        .fetch_one(&self.pool)
        .await?;
        Ok(row)
    }

    pub async fn google_quota_window(
        &self,
        site_id: i64,
        total: u32,
    ) -> anyhow::Result<GoogleQuotaWindow> {
        let row: (i64, Option<DateTime<Utc>>) = sqlx::query_as(
            r#"
            SELECT COUNT(*), MIN(sl.created_at)
            FROM submission_logs sl
            JOIN urls u ON u.id = sl.url_id
            WHERE sl.provider = 'google'
              AND sl.success = 1
              AND u.site_id = $1
              AND sl.created_at > datetime('now', '-24 hours')
            "#,
        )
        .bind(site_id)
        .fetch_one(&self.pool)
        .await?;

        Ok(GoogleQuotaWindow::new(row.0, total, row.1))
    }

    pub async fn list_by_url(&self, url_id: i64) -> anyhow::Result<Vec<SubmissionLog>> {
        Ok(sqlx::query_as::<_, SubmissionLog>(
            "SELECT * FROM submission_logs WHERE url_id = $1 ORDER BY created_at ASC, id ASC",
        )
        .bind(url_id)
        .fetch_all(&self.pool)
        .await?)
    }
}

#[cfg(test)]
mod tests {
    use super::SubmissionLogRepo;
    use crate::domain::ProviderKind;
    use sqlx::SqlitePool;

    #[tokio::test]
    async fn google_quota_is_isolated_by_site() -> anyhow::Result<()> {
        let pool = SqlitePool::connect("sqlite::memory:").await?;
        sqlx::migrate!("./migrations").run(&pool).await?;
        sqlx::query("INSERT INTO sites (domain) VALUES ('one.example'), ('two.example')")
            .execute(&pool)
            .await?;
        sqlx::query("INSERT INTO urls (site_id, url, url_hash) VALUES (1, 'https://one.example/a', 'one-a'), (2, 'https://two.example/a', 'two-a')")
            .execute(&pool)
            .await?;

        let repo = SubmissionLogRepo::new(pool);
        repo.insert(1, ProviderKind::Google, true, Some(200), None)
            .await?;

        let site_one = repo.google_quota_window(1, 1).await?;
        let site_two = repo.google_quota_window(2, 1).await?;
        assert_eq!(site_one.used, 1);
        assert!(site_one.exhausted());
        assert_eq!(site_two.used, 0);
        assert!(!site_two.exhausted());
        Ok(())
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct GoogleQuotaWindow {
    pub used: i64,
    pub total: u32,
    pub remaining: i64,
    pub next_free_at: Option<DateTime<Utc>>,
}

impl GoogleQuotaWindow {
    pub fn new(used: i64, total: u32, oldest_in_window: Option<DateTime<Utc>>) -> Self {
        let remaining = (total as i64 - used).max(0);
        let next_free_at = if remaining == 0 {
            oldest_in_window.map(|t| t + Duration::hours(24))
        } else {
            None
        };
        Self {
            used,
            total,
            remaining,
            next_free_at,
        }
    }

    pub fn exhausted(&self) -> bool {
        self.remaining == 0 && self.total > 0
    }
}
