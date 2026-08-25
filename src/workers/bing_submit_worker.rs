use crate::application::SubmissionService;
use crate::config::AppConfig;
use crate::domain::ProviderKind;
use crate::infrastructure::{SiteRepo, SubmissionLogRepo, UrlRepo};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tracing::{error, info};

#[derive(Clone)]
pub struct BingSubmitWorker {
    urls: UrlRepo,
    sites: SiteRepo,
    logs: SubmissionLogRepo,
    submission: SubmissionService,
    is_running: Arc<AtomicBool>,
    config: AppConfig,
}

impl BingSubmitWorker {
    pub fn new(
        urls: UrlRepo,
        sites: SiteRepo,
        logs: SubmissionLogRepo,
        submission: SubmissionService,
        is_running: Arc<AtomicBool>,
        config: AppConfig,
    ) -> Self {
        Self {
            urls,
            sites,
            logs,
            submission,
            is_running,
            config,
        }
    }

    pub fn start(self: Arc<Self>) {
        let interval = Duration::from_secs(self.config.submit_worker_interval_secs);
        tokio::spawn(async move {
            info!("Bing Submit Worker 待机就绪");
            loop {
                if let Err(e) = self.tick().await {
                    error!(error = %e, "Bing submit worker tick failed");
                }
                tokio::time::sleep(interval).await;
            }
        });
    }

    async fn tick(&self) -> anyhow::Result<()> {
        if !self.is_running.load(Ordering::Relaxed) {
            return Ok(());
        }

        let pending = self.urls.fetch_pending_bing(self.config.submit_worker_batch).await?;
        if pending.is_empty() {
            return Ok(());
        }

        for url in pending {
            if let Ok(Some(site)) = self.sites.find_by_id(url.site_id).await {
                if !site.bing_ready() { continue; }
                let key = site.bing_indexnow_key.as_deref().unwrap_or("");
                if let Ok(results) = self.submission.submit_url_batch_bing(&site.domain, key, &[url.url.clone()]).await {
                    if let Some(res) = results.first() {
                        let _ = self.logs.insert(
                            url.id,
                            ProviderKind::Bing,
                            res.is_success,
                            res.status_code.map(|c| c as i32),
                            res.response_msg.as_deref(),
                        ).await;

                        let st = if res.is_success { "SUBMITTED" } else { "FAILED" };
                        let _ = self.urls.apply_submit_outcome(url.id, Some(st), res.response_msg.as_deref(), None, None).await;
                    }
                }
            }
        }
        Ok(())
    }
}