use crate::domain::{ProviderKind, Site};
use crate::providers::{bing::BingProvider, google::GoogleProvider, SearchProvider, SubmissionResult};
use tracing::warn;

#[derive(Clone)]
pub struct SubmissionService {
    bing: BingProvider,
    google: GoogleProvider,
}

impl SubmissionService {
    pub fn new(bing: BingProvider, google: GoogleProvider) -> Self {
        Self { bing, google }
    }

    #[allow(dead_code)]
    pub async fn submit_url(
        &self,
        site: &Site,
        page_url: &str,
    ) -> anyhow::Result<Vec<(ProviderKind, SubmissionResult)>> {
        let mut results = Vec::new();
        let urls = vec![page_url.to_string()];

        if site.bing_ready() {
            let key = site.bing_indexnow_key.as_deref().unwrap_or("");
            match self.bing.submit_batch(&site.domain, key, &urls).await {
                Ok(batch) => {
                    if let Some(r) = batch.into_iter().next() {
                        results.push((ProviderKind::Bing, r));
                    }
                }
                Err(e) => {
                    warn!(error = %e, "bing submit failed");
                    results.push((
                        ProviderKind::Bing,
                        SubmissionResult::failure(page_url.to_string(), None, e.to_string()),
                    ));
                }
            }
        }

        if site.google_ready() {
            let key = site.google_service_account_json.as_deref().unwrap_or("");
            match self.google.submit_batch(&site.domain, key, &urls).await {
                Ok(batch) => {
                    if let Some(r) = batch.into_iter().next() {
                        results.push((ProviderKind::Google, r));
                    }
                }
                Err(e) => {
                    warn!(error = %e, "google submit failed");
                    results.push((
                        ProviderKind::Google,
                        SubmissionResult::failure(page_url.to_string(), None, e.to_string()),
                    ));
                }
            }
        } else if site.google_verified() && site.google_quota_paused() {
            warn!(
                "skipping Google submit: 24h quota pause active until {:?}",
                site.google_quota_paused_until
            );
        }

        if results.is_empty() {
            if site.has_any_credentials_filled() {
                anyhow::bail!("credentials filled but not ready");
            }
            anyhow::bail!("no search provider credentials configured");
        }

        Ok(results)
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