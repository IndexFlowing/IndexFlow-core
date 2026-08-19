use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct HealthCheck {
    pub id: i64,
    pub url_id: i64,
    pub http_status: Option<i32>,
    pub response_time: Option<i32>,
    pub has_noindex: bool,
    pub has_canonical: bool,
    pub checked_at: DateTime<Utc>,
}

/// Result of the inline SEO quality gate run immediately before submit.
#[derive(Debug, Clone)]
pub struct QualityGateResult {
    pub http_status: Option<i32>,
    pub response_time_ms: Option<i32>,
    pub has_noindex: bool,
    pub has_canonical: bool,
    pub page_title: Option<String>,
    pub canonical_url: Option<String>,
    /// True only when every intercept rule passed — safe to call search APIs.
    pub passed: bool,
    pub block_reason: Option<String>,
}
