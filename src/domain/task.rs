use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

use super::error::{DomainError, DomainResult};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TaskType {
    SyncSitemap,
    CheckUrl,
    SubmitUrl,
    RetrySubmission,
    /// Engine-decoupled: Bing-only submit pipeline (no quota limits).
    SubmitBing,
    /// Engine-decoupled: Google-only submit pipeline (rolling 24h quota).
    SubmitGoogle,
}

#[allow(dead_code)]
impl TaskType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::SyncSitemap => "SYNC_SITEMAP",
            Self::CheckUrl => "CHECK_URL",
            Self::SubmitUrl => "SUBMIT_URL",
            Self::RetrySubmission => "RETRY_SUBMISSION",
            Self::SubmitBing => "SUBMIT_BING",
            Self::SubmitGoogle => "SUBMIT_GOOGLE",
        }
    }

    pub fn parse(s: &str) -> DomainResult<Self> {
        match s {
            "SYNC_SITEMAP" => Ok(Self::SyncSitemap),
            "CHECK_URL" => Ok(Self::CheckUrl),
            "SUBMIT_URL" => Ok(Self::SubmitUrl),
            "RETRY_SUBMISSION" => Ok(Self::RetrySubmission),
            "SUBMIT_BING" => Ok(Self::SubmitBing),
            "SUBMIT_GOOGLE" => Ok(Self::SubmitGoogle),
            other => Err(DomainError::InvalidTaskType(other.to_string())),
        }
    }

    /// All task types belonging to the submit pipeline (legacy + decoupled).
    pub fn is_submit_type(self) -> bool {
        matches!(self, Self::SubmitUrl | Self::SubmitBing | Self::SubmitGoogle)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TaskStatus {
    Pending,
    Processing,
    Success,
    Failed,
}

#[allow(dead_code)]
impl TaskStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "PENDING",
            Self::Processing => "PROCESSING",
            Self::Success => "SUCCESS",
            Self::Failed => "FAILED",
        }
    }

    pub fn parse(s: &str) -> DomainResult<Self> {
        match s {
            "PENDING" => Ok(Self::Pending),
            "PROCESSING" => Ok(Self::Processing),
            "SUCCESS" => Ok(Self::Success),
            "FAILED" => Ok(Self::Failed),
            other => Err(DomainError::InvalidStatus(other.to_string())),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Task {
    pub id: i64,
    pub site_id: i64,
    pub url_id: Option<i64>,
    pub sitemap_id: Option<i64>,
    pub task_type: String,
    pub status: String,
    pub priority: i32,
    pub scheduled_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub finished_at: Option<DateTime<Utc>>,
    pub retry_count: i32,
    pub locked_at: Option<DateTime<Utc>>,
    pub last_error: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[allow(dead_code)]
impl Task {
    pub fn task_type_enum(&self) -> DomainResult<TaskType> {
        TaskType::parse(&self.task_type)
    }

    pub fn status_enum(&self) -> DomainResult<TaskStatus> {
        TaskStatus::parse(&self.status)
    }
}

/// Default priorities (lower number = higher priority).
pub mod priority {
    pub const SYNC_SITEMAP: i32 = 10;
    pub const SUBMIT_URL: i32 = 80;
    #[allow(dead_code)]
    pub const RETRY: i32 = 90;
}

/// Fair multi-site claim plan for one worker tick.
///
/// When there are no more sites than `batch`, every site is included and the batch
/// is split evenly (`max(1, batch / n)` each). When there are more sites than
/// `batch`, take `batch` sites (1 task each) after rotating past `after_site_id`
/// so a large tenant cannot starve the others across ticks.
pub fn fair_site_plan(site_ids: &[i64], batch: i64, after_site_id: i64) -> (Vec<i64>, i64) {
    if site_ids.is_empty() || batch <= 0 {
        return (Vec::new(), 0);
    }
    let mut ids = site_ids.to_vec();
    ids.sort_unstable();
    ids.dedup();
    let start = ids.iter().position(|&id| id > after_site_id).unwrap_or(0);
    let rotated: Vec<i64> = ids[start..]
        .iter()
        .copied()
        .chain(ids[..start].iter().copied())
        .collect();
    let n = rotated.len() as i64;
    if n <= batch {
        (rotated, (batch / n).max(1))
    } else {
        (
            rotated.into_iter().take(batch as usize).collect(),
            1,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::fair_site_plan;

    #[test]
    fn two_sites_split_the_batch() {
        let (sites, per) = fair_site_plan(&[10, 20], 10, 0);
        assert_eq!(sites, vec![10, 20]);
        assert_eq!(per, 5);
    }

    #[test]
    fn two_sites_still_both_selected_after_rotation() {
        let (sites, per) = fair_site_plan(&[10, 20], 10, 10);
        assert_eq!(sites, vec![20, 10]);
        assert_eq!(per, 5);
    }

    #[test]
    fn more_sites_than_batch_rotates_and_takes_one_each() {
        let ids: Vec<i64> = (1..=15).collect();
        let (sites, per) = fair_site_plan(&ids, 10, 0);
        assert_eq!(sites, (1..=10).collect::<Vec<_>>());
        assert_eq!(per, 1);

        let (sites, per) = fair_site_plan(&ids, 10, 10);
        assert_eq!(sites, vec![11, 12, 13, 14, 15, 1, 2, 3, 4, 5]);
        assert_eq!(per, 1);
    }

    #[test]
    fn empty_or_non_positive_batch_selects_nothing() {
        assert_eq!(fair_site_plan(&[1], 0, 0), (Vec::new(), 0));
        assert_eq!(fair_site_plan(&[], 10, 0), (Vec::new(), 0));
    }

    #[test]
    fn three_sites_split_uneven_batch() {
        let (sites, per) = fair_site_plan(&[3, 1, 2], 10, 0);
        assert_eq!(sites, vec![1, 2, 3]);
        assert_eq!(per, 3);
    }
}
