use crate::domain::{HealthCheck, QualityGateResult};
use sqlx::PgPool;

#[derive(Clone)]
pub struct HealthCheckRepo {
    pool: PgPool,
}

impl HealthCheckRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    #[allow(dead_code)]
    pub async fn insert(
        &self,
        url_id: i64,
        http_status: Option<i32>,
        response_time: Option<i32>,
        has_noindex: bool,
        has_canonical: bool,
    ) -> anyhow::Result<HealthCheck> {
        self.insert_from_gate(
            url_id,
            &QualityGateResult {
                http_status,
                response_time_ms: response_time,
                has_noindex,
                has_canonical,
                ..QualityGateResult::default()
            },
        )
        .await
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
                meta_description, h1_content, robots_directive, payload_bytes, hreflang
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
            RETURNING *
            "#,
        )
        .bind(url_id)
        .bind(gate.http_status)
        .bind(gate.response_time_ms)
        .bind(gate.has_noindex)
        .bind(gate.has_canonical)
        .bind(gate.meta_description.as_deref())
        .bind(gate.h1_content.as_deref())
        .bind(gate.robots_directive.as_deref())
        .bind(gate.payload_bytes)
        .bind(hreflang.as_deref())
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
