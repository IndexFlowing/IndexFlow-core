use crate::infrastructure::INTERNAL_CRAWLER_UA;
use indexflow_seo::{SeoAuditResult, SeoProbeClient};
use std::time::Duration;

pub type QualityGateResult = SeoAuditResult;

#[derive(Clone)]
pub struct HealthService {
    prober: SeoProbeClient,
}

impl HealthService {
    pub fn new(_shared: reqwest::Client) -> anyhow::Result<Self> {
        let prober = SeoProbeClient::new(INTERNAL_CRAWLER_UA, Duration::from_secs(15))
            .map_err(|e| anyhow::anyhow!("failed to build SEO probe client: {e}"))?;
        Ok(Self { prober })
    }

    pub async fn check_url(&self, url: &str) -> QualityGateResult {
        self.prober.check_url(url).await
    }
}