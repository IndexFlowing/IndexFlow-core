use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

pub use indexflow_seo::SeoAuditResult as QualityGateResult;

#[allow(dead_code)]
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
    pub checked_at: DateTime<Utc>,
}