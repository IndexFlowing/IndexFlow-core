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
    pub meta_description: Option<String>,
    pub h1_content: Option<String>,
    pub robots_directive: Option<String>,
    pub payload_bytes: Option<i32>,
    pub hreflang: Option<String>,
    pub checked_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HreflangAlt {
    pub lang: String,
    pub href: String,
}

/// Result of the inline SEO quality gate run immediately before submit.
#[derive(Debug, Clone, Default)]
pub struct QualityGateResult {
    pub http_status: Option<i32>,
    pub response_time_ms: Option<i32>,
    pub has_noindex: bool,
    pub has_canonical: bool,
    pub page_title: Option<String>,
    pub canonical_url: Option<String>,
    pub meta_description: Option<String>,
    pub h1_content: Option<String>,
    pub robots_directive: Option<String>,
    pub hreflang: Vec<HreflangAlt>,
    pub payload_bytes: Option<i32>,
    /// True only when every intercept rule passed — safe to call search APIs.
    pub passed: bool,
    pub block_reason: Option<String>,
}

impl QualityGateResult {
    pub fn hreflang_json(&self) -> Option<String> {
        if self.hreflang.is_empty() {
            None
        } else {
            serde_json::to_string(&self.hreflang).ok()
        }
    }
}
