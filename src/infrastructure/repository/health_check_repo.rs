use crate::domain::QualityGateResult;
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, SqlitePool};

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct HealthCheck {
    pub id: i64,
    pub url_id: i64,
    pub http_status: Option<i32>,
    pub response_time: Option<i32>,
    pub has_noindex: bool,
    pub has_canonical: bool,
    pub meta_description: Option<String>,
    pub h1_content: Option<String>,
    pub robots_directive: Option<String>,
    pub payload_bytes: Option<i32>,
    pub hreflang: Option<String>,
    pub checked_at: chrono::DateTime<chrono::Utc>,
}

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
}