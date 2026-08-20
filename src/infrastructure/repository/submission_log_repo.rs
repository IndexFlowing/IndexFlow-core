use crate::domain::{ProviderKind, SubmissionLog};
use chrono::{DateTime, Duration, Utc};
use sqlx::PgPool;

#[allow(dead_code)]
#[derive(Debug, Clone, serde::Serialize, sqlx::FromRow)]
pub struct ProviderSubmitCount {
    pub provider: String,
    pub count: i64,
}

#[derive(Clone)]
pub struct SubmissionLogRepo {
    pool: PgPool,
}

impl SubmissionLogRepo {
    pub fn new(pool: PgPool) -> Self {
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
            INSERT INTO submission_logs (url_id, provider, success, response_code, response_body)
            VALUES ($1, $2, $3, $4, $5)
            RETURNING *
            "#,
        )
        .bind(url_id)
        .bind(provider.as_str())
        .bind(success)
        .bind(response_code)
        .bind(response_body)
        .fetch_one(&self.pool)
        .await?;
        Ok(row)
    }

    pub async fn list_by_url(
        &self,
        url_id: i64,
        limit: i64,
    ) -> anyhow::Result<Vec<SubmissionLog>> {
        let rows = sqlx::query_as::<_, SubmissionLog>(
            r#"
            SELECT * FROM submission_logs
            WHERE url_id = $1
            ORDER BY created_at DESC
            LIMIT $2
            "#,
        )
        .bind(url_id)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    /// Rolling 24h Google Indexing API usage for one site (not UTC midnight).
    pub async fn google_quota_window(
        &self,
        site_id: i64,
        total: u32,
    ) -> anyhow::Result<GoogleQuotaWindow> {
        let row: (i64, Option<DateTime<Utc>>) = sqlx::query_as(
            r#"
            SELECT
                COUNT(*)::bigint,
                MIN(sl.created_at)
            FROM submission_logs sl
            INNER JOIN urls u ON u.id = sl.url_id
            WHERE u.site_id = $1
              AND sl.provider = 'google'
              AND sl.success = true
              AND sl.created_at > NOW() - INTERVAL '24 hours'
            "#,
        )
        .bind(site_id)
        .fetch_one(&self.pool)
        .await?;

        Ok(GoogleQuotaWindow::new(site_id, row.0, total, row.1))
    }

    /// Rolling 24h Google usage for every site that has recent submits.
    pub async fn google_quota_windows_by_site(
        &self,
        total: u32,
    ) -> anyhow::Result<Vec<GoogleQuotaWindow>> {
        let rows: Vec<(i64, i64, Option<DateTime<Utc>>)> = sqlx::query_as(
            r#"
            SELECT
                u.site_id,
                COUNT(*)::bigint,
                MIN(sl.created_at)
            FROM submission_logs sl
            INNER JOIN urls u ON u.id = sl.url_id
            WHERE sl.provider = 'google'
              AND sl.success = true
              AND sl.created_at > NOW() - INTERVAL '24 hours'
            GROUP BY u.site_id
            "#,
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|(site_id, used, oldest)| GoogleQuotaWindow::new(site_id, used, total, oldest))
            .collect())
    }

    /// Distinct successful url_id counts grouped by provider.
    #[allow(dead_code)]
    pub async fn count_submitted_by_provider(&self) -> anyhow::Result<Vec<ProviderSubmitCount>> {
        let rows = sqlx::query_as::<_, ProviderSubmitCount>(
            r#"
            SELECT provider, COUNT(DISTINCT url_id)::bigint AS count
            FROM submission_logs
            WHERE success = true
            GROUP BY provider
            "#,
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }
}

/// Per-site Google Indexing API quota over a rolling 24-hour window.
#[derive(Debug, Clone, serde::Serialize)]
pub struct GoogleQuotaWindow {
    pub site_id: i64,
    pub used: i64,
    pub total: u32,
    pub remaining: i64,
    pub next_free_at: Option<DateTime<Utc>>,
}

impl GoogleQuotaWindow {
    pub fn new(
        site_id: i64,
        used: i64,
        total: u32,
        oldest_in_window: Option<DateTime<Utc>>,
    ) -> Self {
        let remaining = (total as i64 - used).max(0);
        let next_free_at = if remaining == 0 {
            oldest_in_window.map(|t| t + Duration::hours(24))
        } else {
            None
        };
        Self {
            site_id,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rolling_window_next_free_is_oldest_plus_24h() {
        let oldest = Utc::now() - Duration::hours(2);
        let q = GoogleQuotaWindow::new(1, 200, 200, Some(oldest));
        assert!(q.exhausted());
        assert_eq!(q.remaining, 0);
        assert_eq!(q.next_free_at, Some(oldest + Duration::hours(24)));
    }

    #[test]
    fn unused_quota_has_no_next_free() {
        let oldest = Utc::now() - Duration::hours(1);
        let q = GoogleQuotaWindow::new(1, 12, 200, Some(oldest));
        assert!(!q.exhausted());
        assert_eq!(q.remaining, 188);
        assert!(q.next_free_at.is_none());
    }

    #[test]
    fn per_site_window_is_independent() {
        let a = GoogleQuotaWindow::new(1, 200, 200, None);
        let b = GoogleQuotaWindow::new(2, 3, 200, None);
        assert!(a.exhausted());
        assert!(!b.exhausted());
        assert_eq!(b.remaining, 197);
    }

    #[test]
    fn exhausted_without_oldest_yields_none_next_free() {
        // Edge: exhausted but oldest is None (e.g. no recent logs yet counted externally).
        let q = GoogleQuotaWindow::new(1, 200, 200, None);
        assert!(q.exhausted());
        assert!(q.next_free_at.is_none());
    }
}
