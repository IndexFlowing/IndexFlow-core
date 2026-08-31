use crate::infrastructure::INTERNAL_CRAWLER_UA;
use indexflow_seo::{SeoAuditResult, SeoProbeClient};
use std::time::Duration;

pub type QualityGateResult = SeoAuditResult;

#[derive(Clone)]
pub struct HealthService {
    default_prober: SeoProbeClient,
}

impl HealthService {
    pub fn new(_shared: reqwest::Client) -> anyhow::Result<Self> {
        let default_prober = SeoProbeClient::new(INTERNAL_CRAWLER_UA, Duration::from_secs(15))
            .map_err(|e| anyhow::anyhow!("failed to build default SEO probe client: {e}"))?;
        Ok(Self { default_prober })
    }

    /// 执行页面质检：若站点有专属放行密钥，临时用定制 UA 探测，否则使用默认客户端
    pub async fn check_url(&self, url: &str, custom_ua: Option<&str>) -> QualityGateResult {
        if let Some(ua) = custom_ua.map(str::trim).filter(|s| !s.is_empty()) {
            if let Ok(prober) = SeoProbeClient::new(ua, Duration::from_secs(15)) {
                return prober.check_url(url).await;
            }
        }
        self.default_prober.check_url(url).await
    }
}