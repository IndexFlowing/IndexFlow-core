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

/// Whether a site can do real submit work right now (no HTTP involved).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SitePushability {
    /// At least one engine can accept work, or leftover tasks can be drained.
    Ready,
    /// Enabled engines are unusable: Google 24h quota is locked and Bing has nothing left.
    SleepQuota,
    /// No verified provider at all.
    FailNoProvider,
}

/// Site-level circuit breaker.
///
/// `has_bing_work` is true when a claimable `SUBMIT_URL` still needs Bing.
/// Google readiness already encodes the rolling-quota pause (`google_ready()`).
pub fn decide_site_push(
    bing_ready: bool,
    google_ready: bool,
    google_verified: bool,
    has_bing_work: bool,
) -> SitePushability {
    if (bing_ready && has_bing_work) || google_ready {
        return SitePushability::Ready;
    }
    if google_verified && !google_ready {
        return SitePushability::SleepQuota;
    }
    if !bing_ready && !google_verified {
        return SitePushability::FailNoProvider;
    }
    // Bing is ready but every claimable URL is already on Bing; Google is not a channel.
    SitePushability::Ready
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn site_ready_when_bing_still_has_work_even_if_google_paused() {
        assert_eq!(
            decide_site_push(true, false, true, true),
            SitePushability::Ready
        );
    }

    #[test]
    fn site_sleeps_when_google_paused_and_bing_has_no_work() {
        assert_eq!(
            decide_site_push(true, false, true, false),
            SitePushability::SleepQuota
        );
    }

    #[test]
    fn site_sleeps_when_google_paused_and_bing_not_ready() {
        assert_eq!(
            decide_site_push(false, false, true, false),
            SitePushability::SleepQuota
        );
    }

    #[test]
    fn site_fails_when_no_verified_provider() {
        assert_eq!(
            decide_site_push(false, false, false, false),
            SitePushability::FailNoProvider
        );
    }

    #[test]
    fn site_ready_when_google_has_quota() {
        assert_eq!(
            decide_site_push(false, true, true, false),
            SitePushability::Ready
        );
    }

    #[test]
    fn site_drains_when_bing_done_and_google_not_configured() {
        assert_eq!(
            decide_site_push(true, false, false, false),
            SitePushability::Ready
        );
    }
}
