use crate::application::SubmissionService;
use crate::config::AppConfig;
use crate::domain::ProviderKind;
use crate::infrastructure::{SiteRepo, SubmissionLogRepo, UrlRepo};
use chrono::{Duration as ChronoDuration, Utc};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tracing::{error, info};

#[derive(Clone)]
pub struct GoogleSubmitWorker {
    urls: UrlRepo,
    sites: SiteRepo,
    logs: SubmissionLogRepo,
    submission: SubmissionService,
    is_running: Arc<AtomicBool>,
    config: AppConfig,
}

impl GoogleSubmitWorker {
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
            info!("Google Submit Worker 待机就绪");
            loop {
                if let Err(e) = self.tick().await {
                    error!(error = %e, "Google submit worker tick failed");
                }
                tokio::time::sleep(interval).await;
            }
        });
    }

    async fn tick(&self) -> anyhow::Result<()> {
        if !self.is_running.load(Ordering::Relaxed) {
            return Ok(());
        }

        let Some(site) = self.sites.get().await? else { return Ok(()); };
        if !site.google_ready() { return Ok(()); }

        let quota = self.logs.google_quota_window(self.config.google_daily_quota).await?;
        if quota.exhausted() {
            let until = quota.next_free_at.unwrap_or_else(|| Utc::now() + ChronoDuration::hours(24));
            self.sites.set_google_quota_paused_until(until).await?;
            return Ok(());
        }

        let pending = self.urls.fetch_pending_google(self.config.submit_worker_batch).await?;
        if pending.is_empty() {
            return Ok(());
        }

        for url in pending {
            if !self.is_running.load(Ordering::Relaxed) {
                break;
            }

            match self.submission.submit_url_google(&site, &url.url).await {
                Ok(res) => {
                    self.logs.insert(
                        url.id,
                        ProviderKind::Google,
                        res.is_success,
                        res.status_code.map(|c| c as i32),
                        res.response_msg.as_deref(),
                    ).await?;

                    let st = if res.is_success { "SUBMITTED" } else { "FAILED" };
                    self.urls.apply_submit_outcome(url.id, None, None, Some(st), res.response_msg.as_deref()).await?;
                }
                Err(e) => {
                    let _ = self.urls.apply_submit_outcome(url.id, None, None, Some("FAILED"), Some(&e.to_string())).await;
                }
            }
        }
        Ok(())
    }
}