pub mod gsc;
pub mod indexing;
pub mod oauth;

pub use gsc::{GscClient, GscInspectResult};
pub use indexing::GoogleIndexingClient;
pub use oauth::GoogleAuthClient;

use super::{SearchProvider, SubmissionResult};
use async_trait::async_trait;

/// Google 提供者统一外观门面（向后完全兼容现有所有 Service 调用）
#[derive(Clone)]
pub struct GoogleProvider {
    gsc: GscClient,
    indexing: GoogleIndexingClient,
}

impl GoogleProvider {
    pub fn new(client: reqwest::Client) -> Self {
        let auth = GoogleAuthClient::new(client.clone());
        let gsc = GscClient::new(client.clone(), auth.clone());
        let indexing = GoogleIndexingClient::new(client, auth);

        Self {
            gsc,
            indexing,
        }
    }

    #[inline]
    pub async fn resolve_gsc_property(&self, sa_json: &str, domain: &str) -> anyhow::Result<String> {
        self.gsc.resolve_gsc_property(sa_json, domain).await
    }

    #[inline]
    pub async fn inspect_url(
        &self,
        sa_json: &str,
        site_url: &str,
        inspection_url: &str,
    ) -> anyhow::Result<GscInspectResult> {
        self.gsc.inspect_url(sa_json, site_url, inspection_url).await
    }

    #[inline]
    pub async fn fetch_search_analytics_pages(
        &self,
        sa_json: &str,
        site_url: &str,
    ) -> anyhow::Result<Vec<String>> {
        self.gsc.fetch_search_analytics_pages(sa_json, site_url).await
    }
}

#[async_trait]
impl SearchProvider for GoogleProvider {
    fn name(&self) -> &'static str {
        "google"
    }

    async fn submit_batch(
        &self,
        domain: &str,
        service_account_json: &str,
        urls: &[String],
    ) -> anyhow::Result<Vec<SubmissionResult>> {
        self.indexing
            .submit_batch(domain, service_account_json, urls)
            .await
    }
}