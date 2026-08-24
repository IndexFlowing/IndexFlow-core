use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProviderKind {
    Google,
    Bing,
}

impl ProviderKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Google => "google",
            Self::Bing => "bing",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct SubmissionLog {
    pub id: i64,
    pub url_id: i64,
    pub provider: String,
    pub success: i64, // 0 | 1 in SQLite
    pub response_code: Option<i32>,
    pub response_body: Option<String>,
    pub created_at: DateTime<Utc>,
}