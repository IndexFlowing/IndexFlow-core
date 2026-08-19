pub mod bing;
pub mod google;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// Result of submitting a single URL to a search engine.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubmissionResult {
    pub url: String,
    pub is_success: bool,
    pub status_code: Option<u16>,
    pub response_msg: Option<String>,
    /// True when provider hit rate/quota limits (e.g. Google 429) — callers should stop further submits.
    #[serde(default)]
    pub is_quota_exceeded: bool,
}

impl SubmissionResult {
    pub fn failure(url: String, status_code: Option<u16>, msg: impl Into<String>) -> Self {
        Self {
            url,
            is_success: false,
            status_code,
            response_msg: Some(msg.into()),
            is_quota_exceeded: false,
        }
    }
}

/// Search engine provider abstraction (Google, Bing, future engines).
#[async_trait]
pub trait SearchProvider: Send + Sync {
    #[allow(dead_code)]
    fn name(&self) -> &'static str;

    async fn submit_batch(
        &self,
        domain: &str,
        key: &str,
        urls: &[String],
    ) -> anyhow::Result<Vec<SubmissionResult>>;
}
