use crate::infrastructure::Site;
use crate::providers::{bing::BingProvider, google::GoogleProvider, SearchProvider, SubmissionResult};

#[derive(Clone)]
pub struct SubmissionService {
    bing: BingProvider,
    google: GoogleProvider,
}

impl SubmissionService {
    pub fn new(bing: BingProvider, google: GoogleProvider) -> Self {
        Self { bing, google }
    }

    pub async fn submit_url_batch_bing(
        &self,
        domain: &str,
        key: &str,
        urls: &[String],
    ) -> anyhow::Result<Vec<SubmissionResult>> {
        self.bing.submit_batch(domain, key, urls).await
    }

    pub async fn submit_url_google(
        &self,
        site: &Site,
        page_url: &str,
    ) -> anyhow::Result<SubmissionResult> {
        let key = site.google_service_account_json.as_deref().unwrap_or("");
        let batch = self
            .google
            .submit_batch(&site.domain, key, &[page_url.to_string()])
            .await?;
        batch
            .into_iter()
            .next()
            .ok_or_else(|| anyhow::anyhow!("Google returned empty result"))
    }
}