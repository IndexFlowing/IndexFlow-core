use crate::domain::{ProviderKind, Site};
use crate::providers::{bing::BingProvider, google::GoogleProvider, SearchProvider, SubmissionResult};
use tracing::warn;

/// Routes submission to **verified** search providers for a site.
#[derive(Clone)]
pub struct SubmissionService {
    bing: BingProvider,
    google: GoogleProvider,
}

impl SubmissionService {
    pub fn new(bing: BingProvider, google: GoogleProvider) -> Self {
        Self { bing, google }
    }

    /// Submit one URL only to providers with VERIFIED credential status.
    #[allow(dead_code)]
    pub async fn submit_url(
        &self,
        site: &Site,
        page_url: &str,
    ) -> anyhow::Result<Vec<(ProviderKind, SubmissionResult)>> {
        let mut results = Vec::new();
        let urls = vec![page_url.to_string()];

        if site.bing_ready() {
            let key = site.indexnow_key.as_deref().unwrap_or("");
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

        // google_ready() already excludes sites with active quota pause
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
                site_id = site.id,
                "skipping Google submit: site 24h quota pause active until {:?}",
                site.google_quota_paused_until
            );
        }

        if results.is_empty() {
            if site.has_any_credentials_filled() {
                anyhow::bail!(
                    "no verified providers for site {} (credentials filled but not verified)",
                    site.id
                );
            }
            anyhow::bail!("no search provider credentials configured for site {}", site.id);
        }

        Ok(results)
    }

    /// Batch IndexNow submit (gate-passed URLs only).
    pub async fn submit_url_batch_bing(
        &self,
        domain: &str,
        key: &str,
        urls: &[String],
    ) -> anyhow::Result<Vec<SubmissionResult>> {
        self.bing.submit_batch(domain, key, urls).await
    }

    /// Single Google Indexing API submit (gate-passed URL only).
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
