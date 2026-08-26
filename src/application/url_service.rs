use crate::application::{GscService, HealthService, SubmissionService};
use crate::domain::{ProviderKind, Url};
use crate::infrastructure::{HealthCheckRepo, SiteRepo, SubmissionLogRepo, UrlRepo};
use serde::Serialize;
use std::sync::Arc;
use tokio::sync::Semaphore;
use tokio::task::JoinSet;
use tracing::info;

#[derive(Clone)]
pub struct UrlService {
    urls: UrlRepo,
    health: HealthCheckRepo,
    submissions: SubmissionLogRepo,
    sites: SiteRepo,
    health_svc: HealthService,
    submission_svc: SubmissionService,
    gsc_svc: GscService,
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
        gsc_svc: GscService,
    ) -> Self {
        Self {
            urls,
            health,
            submissions,
            sites,
            health_svc,
            submission_svc,
            gsc_svc,
        }
    }

    pub async fn list_filtered(
        &self,
        site_id: i64,
        page: i64,
        limit: i64,
        query_str: Option<&str>,
        seo_filter: Option<&str>,
        gsc_filter: Option<&str>,
        bing_filter: Option<&str>,
        google_filter: Option<&str>,
    ) -> anyhow::Result<(Vec<Url>, i64)> {
        self.urls
            .list_filtered(
                site_id,
                page,
                limit,
                query_str,
                seo_filter,
                gsc_filter,
                bing_filter,
                google_filter,
            )
            .await
    }

    pub async fn list(&self, site_id: i64, page: i64, limit: i64) -> anyhow::Result<(Vec<Url>, i64)> {
        self.urls
            .list_filtered(site_id, page, limit, None, None, None, None, None)
            .await
    }

    pub async fn find_by_id(&self, id: i64) -> anyhow::Result<Option<Url>> {
        self.urls.find_by_id(id).await
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

    /// 【核心新增】批量并发重新质检选中的 URL
    pub async fn batch_recheck(&self, ids: &[i64]) -> anyhow::Result<usize> {
        if ids.is_empty() {
            return Ok(0);
        }

        info!(count = ids.len(), "🛡️ [UrlService] 正在并发批量重检选中的 URL...");

        let semaphore = Arc::new(Semaphore::new(10));
        let mut set = JoinSet::new();

        for &id in ids {
            let sem = semaphore.clone();
            let urls_repo = self.urls.clone();
            let health_repo = self.health.clone();
            let health_svc = self.health_svc.clone();

            set.spawn(async move {
                let _permit = sem.acquire().await.unwrap();
                if let Ok(Some(url)) = urls_repo.find_by_id(id).await {
                    let gate = health_svc.check_url(&url.url).await;
                    let _ = health_repo.insert_from_gate(url.id, &gate).await;
                    let _ = urls_repo.persist_seo_scan(url.id, &gate).await;
                    return true;
                }
                false
            });
        }

        let mut success_count = 0;
        while let Some(res) = set.join_next().await {
            if let Ok(true) = res {
                success_count += 1;
            }
        }

        info!(success_count, "✅ [UrlService] 选中的 URL 批量重检完成并落库");
        Ok(success_count)
    }

    pub async fn inspect_gsc_now(&self, id: i64) -> anyhow::Result<bool> {
        let Some(url) = self.urls.find_by_id(id).await? else { return Ok(false); };
        let Some(site) = self.sites.find_by_id(url.site_id).await? else { return Ok(false); };
        let res = self.gsc_svc.inspect_one(&site, &url.url).await?;
        self.gsc_svc.apply_inspect_result(url.id, &res).await?;
        Ok(res.ok)
    }

    pub async fn sync_gsc_analytics(&self, site_id: i64) -> anyhow::Result<u64> {
        let Some(site) = self.sites.find_by_id(site_id).await? else {
            anyhow::bail!("站点资产不存在 (site_id: {})", site_id);
        };
        self.gsc_svc.sync_indexed_from_search_analytics(&site).await
    }

    pub async fn submit_now(&self, id: i64, provider: &str) -> anyhow::Result<bool> {
        let Some(url) = self.urls.find_by_id(id).await? else {
            return Ok(false);
        };
        let Some(site) = self.sites.find_by_id(url.site_id).await? else {
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