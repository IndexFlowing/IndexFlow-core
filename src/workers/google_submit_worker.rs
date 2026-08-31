use crate::application::{PipelineManager, SubmissionService};
use crate::config::AppConfig;
use crate::domain::{PipelineStage, ProviderKind};
use crate::infrastructure::{SiteRepo, SubmissionLogRepo, UrlRepo};
use chrono::{Duration as ChronoDuration, Utc};
use std::sync::Arc;
use std::collections::HashSet;
use std::time::Duration;
use tracing::{error, info};

#[derive(Clone)]
pub struct GoogleSubmitWorker {
    urls: UrlRepo,
    sites: SiteRepo,
    logs: SubmissionLogRepo,
    submission: SubmissionService,
    pipeline: PipelineManager,
    config: AppConfig,
}

impl GoogleSubmitWorker {
    pub fn new(
        urls: UrlRepo,
        sites: SiteRepo,
        logs: SubmissionLogRepo,
        submission: SubmissionService,
        pipeline: PipelineManager,
        config: AppConfig,
    ) -> Self {
        Self {
            urls,
            sites,
            logs,
            submission,
            pipeline,
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
        if !self.pipeline.is_running(PipelineStage::PushSubmit) {
            return Ok(());
        }

        let pending = self.urls.fetch_pending_google(self.config.submit_worker_batch).await?;
        if pending.is_empty() {
            let bing_left = self.urls.fetch_pending_bing(1).await?;
            if bing_left.is_empty() {
                self.pipeline.stop(PipelineStage::PushSubmit);
                info!("🎉 全引擎提交队列已全部处理完毕，Worker 回到待机");
            }
            return Ok(());
        }

        let mut exhausted_sites: HashSet<i64> = Default::default();
        for url in pending {
            if !self.pipeline.is_running(PipelineStage::PushSubmit) {
                break;
            }

            if exhausted_sites.contains(&url.site_id) {
                continue;
            }

            if let Ok(Some(site)) = self.sites.find_by_id(url.site_id).await {
                if !site.google_ready() {
                    continue;
                }

                let quota = self.logs.google_quota_window(site.id, site.google_daily_quota as u32).await?;
                if quota.exhausted() {
                    let until = quota.next_free_at.unwrap_or_else(|| Utc::now() + ChronoDuration::hours(24));
                    let _ = self.sites.set_google_quota_paused_until(site.id, until).await;
                    exhausted_sites.insert(site.id);
                    continue;
                }

                match self.submission.submit_url_google(&site, &url.url).await {
                    Ok(res) => {
                        let _ = self
                            .logs
                            .insert(
                                url.id,
                                ProviderKind::Google,
                                res.is_success,
                                res.status_code.map(|c| c as i32),
                                res.response_msg.as_deref(),
                            )
                            .await;

                        let st = if res.is_success { "SUBMITTED" } else { "FAILED" };
                        let _ = self
                            .urls
                            .apply_submit_outcome(
                                url.id,
                                None,
                                None,
                                Some(st),
                                res.response_msg.as_deref(),
                            )
                            .await;
                    }
                    Err(e) => {
                        let _ = self
                            .urls
                            .apply_submit_outcome(url.id, None, None, Some("FAILED"), Some(&e.to_string()))
                            .await;
                    }
                }
            }
        }
        Ok(())
    }
}
