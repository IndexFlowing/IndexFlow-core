use crate::application::{HealthService, SubmissionService};
use crate::domain::{ProviderKind, Url};
use crate::infrastructure::{HealthCheckRepo, SiteRepo, SubmissionLogRepo, UrlRepo};
use serde::Serialize;

#[derive(Clone)]
pub struct UrlService {
    urls: UrlRepo,
    health: HealthCheckRepo,
    submissions: SubmissionLogRepo,
    sites: SiteRepo,
    health_svc: HealthService,
    submission_svc: SubmissionService,
}

#[derive(Debug, Serialize)]
pub struct RecheckResult {
    pub url: Url,
    pub passed: bool,
    pub block_reason: Option<String>,
}

impl UrlService {
    pub fn new(
        urls: UrlRepo,
        health: HealthCheckRepo,
        submissions: SubmissionLogRepo,
        sites: SiteRepo,
        health_svc: HealthService,
        submission_svc: SubmissionService,
    ) -> Self {
        Self {
            urls,
            health,
            submissions,
            sites,
            health_svc,
            submission_svc,
        }
    }

    pub async fn list(&self, page: i64, limit: i64) -> anyhow::Result<(Vec<Url>, i64)> {
        self.urls.list(page, limit).await
    }

    pub async fn recheck(&self, id: i64) -> anyhow::Result<Option<RecheckResult>> {
        let Some(url) = self.urls.find_by_id(id).await? else {
            return Ok(None);
        };
        let gate = self.health_svc.check_url(&url.url).await;
        self.health.insert_from_gate(url.id, &gate).await?;
        self.urls.persist_seo_scan(url.id, &gate).await?;
        let updated = self.urls.find_by_id(id).await?.unwrap_or(url);
        Ok(Some(RecheckResult {
            passed: gate.passed,
            block_reason: gate.block_reason.clone(),
            url: updated,
        }))
    }

    pub async fn submit_now(&self, id: i64, provider: &str) -> anyhow::Result<bool> {
        let Some(url) = self.urls.find_by_id(id).await? else {
            return Ok(false);
        };
        let Some(site) = self.sites.get().await? else {
            return Ok(false);
        };

        match provider.to_ascii_lowercase().as_str() {
            "bing" => {
                if !site.bing_ready() { anyhow::bail!("Bing IndexNow is not configured"); }
                let key = site.bing_indexnow_key.as_deref().unwrap_or("");
                let results = self.submission_svc.submit_url_batch_bing(&site.domain, key, &[url.url.clone()]).await?;
                if let Some(r) = results.first() {
                    self.submissions.insert(url.id, ProviderKind::Bing, r.is_success, r.status_code.map(|c| c as i32), r.response_msg.as_deref()).await?;
                    let st = if r.is_success { "SUBMITTED" } else { "FAILED" };
                    self.urls.apply_submit_outcome(url.id, Some(st), r.response_msg.as_deref(), None, None).await?;
                    return Ok(r.is_success);
                }
            }
            "google" => {
                if !site.google_ready() { anyhow::bail!("Google API is not ready"); }
                let result = self.submission_svc.submit_url_google(&site, &url.url).await?;
                self.submissions.insert(url.id, ProviderKind::Google, result.is_success, result.status_code.map(|c| c as i32), result.response_msg.as_deref()).await?;
                let st = if result.is_success { "SUBMITTED" } else { "FAILED" };
                self.urls.apply_submit_outcome(url.id, None, None, Some(st), result.response_msg.as_deref()).await?;
                return Ok(result.is_success);
            }
            _ => {}
        }
        Ok(false)
    }
}