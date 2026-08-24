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

    pub async fn google_quota_window(&self, total: u32) -> anyhow::Result<GoogleQuotaWindow> {
        let row: (i64, Option<DateTime<Utc>>) = sqlx::query_as(
            r#"
            SELECT COUNT(*), MIN(created_at)
            FROM submission_logs
            WHERE provider = 'google'
              AND success = 1
              AND created_at > datetime('now', '-24 hours')
            "#,
        )
        .fetch_one(&self.pool)
        .await?;

        Ok(GoogleQuotaWindow::new(row.0, total, row.1))
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