use crate::domain::HealthCheck;
use sqlx::PgPool;

#[derive(Clone)]
pub struct HealthCheckRepo {
    pool: PgPool,
}

impl HealthCheckRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn insert(
        &self,
        url_id: i64,
        http_status: Option<i32>,
        response_time: Option<i32>,
        has_noindex: bool,
        has_canonical: bool,
    ) -> anyhow::Result<HealthCheck> {
        let row = sqlx::query_as::<_, HealthCheck>(
            r#"
            INSERT INTO health_checks (url_id, http_status, response_time, has_noindex, has_canonical)
            VALUES ($1, $2, $3, $4, $5)
            RETURNING *
            "#,
        )
        .bind(url_id)
        .bind(http_status)
        .bind(response_time)
        .bind(has_noindex)
        .bind(has_canonical)
        .fetch_one(&self.pool)
        .await?;
        Ok(row)
    }

    pub async fn list_by_url(&self, url_id: i64, limit: i64) -> anyhow::Result<Vec<HealthCheck>> {
        let rows = sqlx::query_as::<_, HealthCheck>(
            r#"
            SELECT * FROM health_checks
            WHERE url_id = $1
            ORDER BY checked_at DESC
            LIMIT $2
            "#,
        )
        .bind(url_id)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }
}
