use crate::domain::{HealthCheck, QualityGateResult};
use sqlx::SqlitePool;

#[derive(Clone)]
pub struct HealthCheckRepo {
    pool: SqlitePool,
}

impl HealthCheckRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn insert_from_gate(
        &self,
        url_id: i64,
        gate: &QualityGateResult,
    ) -> anyhow::Result<HealthCheck> {
        let hreflang = gate.hreflang_json();
        let row = sqlx::query_as::<_, HealthCheck>(
            r#"
            INSERT INTO health_checks (
                url_id, http_status, response_time, has_noindex, has_canonical,
                meta_description, h1_content, robots_directive, payload_bytes, hreflang, checked_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, CURRENT_TIMESTAMP)
            RETURNING *
            "#,
        )
        .bind(url_id)
        .bind(gate.http_status)
        .bind(gate.response_time_ms)
        .bind(if gate.has_noindex { 1 } else { 0 })
        .bind(if gate.has_canonical { 1 } else { 0 })
        .bind(gate.meta_description.as_deref())
        .bind(gate.h1_content.as_deref())
        .bind(gate.robots_directive.as_deref())
        .bind(gate.payload_bytes)
        .bind(hreflang.as_deref())
        .fetch_one(&self.pool)
        .await?;
        Ok(row)
    }

    pub async fn list_by_url(&self, url_id: i64) -> anyhow::Result<Vec<HealthCheck>> {
        Ok(sqlx::query_as::<_, HealthCheck>(
            "SELECT * FROM health_checks WHERE url_id = $1 ORDER BY checked_at ASC, id ASC",
        )
        .bind(url_id)
        .fetch_all(&self.pool)
        .await?)
    }
}
