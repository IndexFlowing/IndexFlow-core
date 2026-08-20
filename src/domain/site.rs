use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

use super::error::{DomainError, DomainResult};

/// Site lifecycle status (PRD / DB design).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[allow(dead_code)]
pub enum SiteStatus {
    Created,
    Scanning,
    Ready,
    NeedAttention,
    Failed,
}

#[allow(dead_code)]
impl SiteStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Created => "CREATED",
            Self::Scanning => "SCANNING",
            Self::Ready => "READY",
            Self::NeedAttention => "NEED_ATTENTION",
            Self::Failed => "FAILED",
        }
    }

    pub fn parse(s: &str) -> DomainResult<Self> {
        match s {
            "CREATED" => Ok(Self::Created),
            "SCANNING" => Ok(Self::Scanning),
            "READY" => Ok(Self::Ready),
            "NEED_ATTENTION" => Ok(Self::NeedAttention),
            "FAILED" => Ok(Self::Failed),
            other => Err(DomainError::InvalidStatus(other.to_string())),
        }
    }

    pub fn can_transition_to(self, next: Self) -> bool {
        matches!(
            (self, next),
            (Self::Created, Self::Scanning)
                | (Self::Scanning, Self::Ready)
                | (Self::Scanning, Self::Failed)
                | (Self::Scanning, Self::NeedAttention)
                | (Self::Ready, Self::Scanning)
                | (Self::Ready, Self::NeedAttention)
                | (Self::NeedAttention, Self::Scanning)
                | (Self::NeedAttention, Self::Ready)
                | (Self::NeedAttention, Self::Failed)
                | (Self::Failed, Self::Scanning)
                | (Self::Failed, Self::Created)
        ) || self == next
    }
}

/// Provider credential usability status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ProviderCredentialStatus {
    /// Not filled in.
    Unset,
    /// Filled / saved, but not successfully verified yet.
    Saved,
    /// Last channel test succeeded — usable for submit.
    Verified,
    /// Last channel test failed — filled but not usable.
    Failed,
}

impl ProviderCredentialStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Unset => "UNSET",
            Self::Saved => "SAVED",
            Self::Verified => "VERIFIED",
            Self::Failed => "FAILED",
        }
    }

    pub fn parse(s: &str) -> Self {
        match s {
            "SAVED" => Self::Saved,
            "VERIFIED" => Self::Verified,
            "FAILED" => Self::Failed,
            _ => Self::Unset,
        }
    }

    /// Initial status after saving a non-empty credential value.
    pub fn from_filled(filled: bool) -> Self {
        if filled {
            Self::Saved
        } else {
            Self::Unset
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Site {
    pub id: i64,
    pub domain: String,
    pub status: String,
    pub indexnow_key: Option<String>,
    pub google_service_account_json: Option<String>,
    pub indexnow_status: String,
    pub indexnow_last_error: Option<String>,
    pub indexnow_verified_at: Option<DateTime<Utc>>,
    pub google_status: String,
    pub google_last_error: Option<String>,
    pub google_verified_at: Option<DateTime<Utc>>,
    /// When set and in the future, skip Google submits (daily quota exhausted).
    pub google_quota_paused_until: Option<DateTime<Utc>>,
    /// Last successful GSC Search Analytics bulk harvest.
    pub gsc_analytics_synced_at: Option<DateTime<Utc>>,
    /// Cached Search Console property URL (`sc-domain:` or `https://…/`).
    pub gsc_property_url: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Site {
    #[allow(dead_code)]
    pub fn status_enum(&self) -> DomainResult<SiteStatus> {
        SiteStatus::parse(&self.status)
    }

    pub fn indexnow_status_enum(&self) -> ProviderCredentialStatus {
        ProviderCredentialStatus::parse(&self.indexnow_status)
    }

    pub fn google_status_enum(&self) -> ProviderCredentialStatus {
        ProviderCredentialStatus::parse(&self.google_status)
    }

    /// Credential value is present (not necessarily verified).
    pub fn has_bing_credentials(&self) -> bool {
        self.indexnow_key
            .as_ref()
            .map(|k| !k.trim().is_empty())
            .unwrap_or(false)
    }

    pub fn has_google_credentials(&self) -> bool {
        self.google_service_account_json
            .as_ref()
            .map(|k| !k.trim().is_empty())
            .unwrap_or(false)
    }

    /// Provider is verified and ready for production submit.
    pub fn bing_ready(&self) -> bool {
        self.indexnow_status_enum() == ProviderCredentialStatus::Verified
            && self.has_bing_credentials()
    }

    pub fn google_ready(&self) -> bool {
        self.google_status_enum() == ProviderCredentialStatus::Verified
            && self.has_google_credentials()
            && !self.google_quota_paused()
    }

    /// True if Google daily quota pause is still active.
    pub fn google_quota_paused(&self) -> bool {
        self.google_quota_paused_until
            .map(|until| until > Utc::now())
            .unwrap_or(false)
    }

    /// Verified Google credentials exist (ignores quota pause).
    pub fn google_verified(&self) -> bool {
        self.google_status_enum() == ProviderCredentialStatus::Verified
            && self.has_google_credentials()
    }

    #[allow(dead_code)]
    pub fn has_any_provider(&self) -> bool {
        self.bing_ready() || self.google_ready()
    }

    /// Verified engines exist (Google quota pause does not hide the channel).
    pub fn has_any_verified_provider(&self) -> bool {
        self.bing_ready() || self.google_verified()
    }

    /// Has any filled credential (for UI / prompts).
    pub fn has_any_credentials_filled(&self) -> bool {
        self.has_bing_credentials() || self.has_google_credentials()
    }
}