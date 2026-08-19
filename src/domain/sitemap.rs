use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

use super::error::{DomainError, DomainResult};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SitemapType {
    Index,
    UrlSet,
}

#[allow(dead_code)]
impl SitemapType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Index => "INDEX",
            Self::UrlSet => "URL_SET",
        }
    }

    pub fn parse(s: &str) -> DomainResult<Self> {
        match s {
            "INDEX" => Ok(Self::Index),
            "URL_SET" => Ok(Self::UrlSet),
            other => Err(DomainError::InvalidStatus(other.to_string())),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SitemapStatus {
    Active,
    Failed,
    Recovering,
}

#[allow(dead_code)]
impl SitemapStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Active => "ACTIVE",
            Self::Failed => "FAILED",
            Self::Recovering => "RECOVERING",
        }
    }

    pub fn parse(s: &str) -> DomainResult<Self> {
        match s {
            "ACTIVE" => Ok(Self::Active),
            "FAILED" => Ok(Self::Failed),
            "RECOVERING" => Ok(Self::Recovering),
            other => Err(DomainError::InvalidStatus(other.to_string())),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Sitemap {
    pub id: i64,
    pub site_id: i64,
    pub url: String,
    pub r#type: String,
    pub status: String,
    pub last_sync_at: Option<DateTime<Utc>>,
    pub last_error: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
