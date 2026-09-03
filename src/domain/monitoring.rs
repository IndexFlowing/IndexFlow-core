use super::{HealthCheck, SubmissionLog, Url};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct IndexStatusHistory {
    pub id: i64,
    pub url_id: i64,
    pub provider: String,
    pub index_status: String,
    pub coverage_state: Option<String>,
    pub last_crawled_at: Option<DateTime<Utc>>,
    pub checked_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize)]
pub enum TimelineEntry {
    Sitemap {
        at: DateTime<Utc>,
        lastmod: Option<DateTime<Utc>>,
    },
    SeoCheck {
        at: DateTime<Utc>,
        check: HealthCheck,
    },
    Submission {
        at: DateTime<Utc>,
        log: SubmissionLog,
    },
    IndexStatus {
        at: DateTime<Utc>,
        history: IndexStatusHistory,
    },
}

impl TimelineEntry {
    pub fn at(&self) -> DateTime<Utc> {
        match self {
            Self::Sitemap { at, .. }
            | Self::SeoCheck { at, .. }
            | Self::Submission { at, .. }
            | Self::IndexStatus { at, .. } => *at,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct MonitoringTimeline {
    pub url: Url,
    pub entries: Vec<TimelineEntry>,
}
