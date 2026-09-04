use crate::application::{PipelineManager, SubmissionService};
use crate::config::AppConfig;
use crate::domain::{PipelineStage, ProviderKind};
use crate::infrastructure::{SiteRepo, SubmissionLogRepo, UrlRepo};
use chrono::{Duration as ChronoDuration, Utc};
use std::sync::Arc;
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
        let running_sites = self
            .pipeline
            .running_sites_for_stage(PipelineStage::PushSubmit);
        if running_sites.is_empty() {
            return Ok(());
        }

        for site_id in running_sites {
            if !self.pipeline.is_running(site_id, PipelineStage::PushSubmit) {
                continue;
            }

            let pending = self
                .urls
                .fetch_pending_google(site_id, self.config.submit_worker_batch)
                .await?;
            if pending.is_empty() {
                let bing_left = self.urls.fetch_pending_bing(site_id, 1).await?;
                if bing_left.is_empty() {
                    self.pipeline.stop(site_id, PipelineStage::PushSubmit);
                    info!(
                        site_id,
                        "🎉 该站点全引擎提交队列已全部处理完毕，Worker 回到待机"
                    );
                }
                continue;
            }

            for url in pending {
                if !self.pipeline.is_running(site_id, PipelineStage::PushSubmit) {
                    break;
                }

                let site = match self.sites.find_by_id(url.site_id).await {
                    Ok(Some(site)) => site,
                    Ok(None) => {
                        error!(site_id, url_id = url.id, url = %url.url, "Google submit skipped: site not found");
                        if let Err(e) = self
                            .urls
                            .apply_submit_outcome(
                                url.id,
                                None,
                                None,
                                Some("FAILED"),
                                Some("Google submit failed: site not found"),
                            )
                            .await
                        {
                            error!(error = %e, site_id, url_id = url.id, "Google missing site failure persistence failed");
                        }
                        continue;
                    }
                    Err(e) => {
                        error!(error = %e, site_id, url_id = url.id, url = %url.url, "Google submit skipped: failed to load site");
                        let message = format!("Google submit failed: failed to load site: {e}");
                        if let Err(persist_error) = self
                            .urls
                            .apply_submit_outcome(
                                url.id,
                                None,
                                None,
                                Some("FAILED"),
                                Some(&message),
                            )
                            .await
                        {
                            error!(error = %persist_error, site_id, url_id = url.id, "Google site lookup failure persistence failed");
                        }
                        continue;
                    }
                };

                if !site.has_google_credentials() {
                    error!(site_id, url_id = url.id, url = %url.url, "Google submit skipped: Google credentials are not configured");
                    if let Err(e) = self
                        .urls
                        .apply_submit_outcome(
                            url.id,
                            None,
                            None,
                            Some("FAILED"),
                            Some("Google credentials are not configured"),
                        )
                        .await
                    {
                        error!(error = %e, site_id, url_id = url.id, "Google missing credentials failure persistence failed");
                    }
                    continue;
                }
                if site.google_quota_paused() {
                    info!(site_id, url_id = url.id, url = %url.url, "Google submit skipped: Google quota is paused");
                    continue;
                }

                let quota = self
                    .logs
                    .google_quota_window(site.id, site.google_daily_quota as u32)
                    .await?;
                if quota.exhausted() {
                    let until = quota
                        .next_free_at
                        .unwrap_or_else(|| Utc::now() + ChronoDuration::hours(24));
                    if let Err(e) = self
                        .sites
                        .set_google_quota_paused_until(site.id, until)
                        .await
                    {
                        error!(error = %e, site_id, "Google quota pause persistence failed");
                    }
                    info!(site_id, "Google quota exhausted; stopping this batch");
                    break;
                }

                match self.submission.submit_url_google(&site, &url.url).await {
                    Ok(res) => {
                        if let Err(e) = self
                            .logs
                            .insert(
                                url.id,
                                ProviderKind::Google,
                                res.is_success,
                                res.status_code.map(|c| c as i32),
                                res.response_msg.as_deref(),
                            )
                            .await
                        {
                            error!(error = %e, site_id, url_id = url.id, url = %url.url, "Google submission log persistence failed");
                        }

                        let st = if res.is_success {
                            "SUBMITTED"
                        } else {
                            "FAILED"
                        };
                        if let Err(e) = self
                            .urls
                            .apply_submit_outcome(
                                url.id,
                                None,
                                None,
                                Some(st),
                                res.response_msg.as_deref(),
                            )
                            .await
                        {
                            error!(error = %e, site_id, url_id = url.id, url = %url.url, "Google URL outcome persistence failed");
                        }

                        if res.is_quota_exceeded {
                            let until = match self
                                .logs
                                .google_quota_window(site.id, site.google_daily_quota as u32)
                                .await
                            {
                                Ok(quota) => quota
                                    .next_free_at
                                    .unwrap_or_else(|| Utc::now() + ChronoDuration::hours(24)),
                                Err(e) => {
                                    error!(error = %e, site_id, "Google quota window recalculation failed; using 24-hour pause");
                                    Utc::now() + ChronoDuration::hours(24)
                                }
                            };
                            if let Err(e) = self
                                .sites
                                .set_google_quota_paused_until(site.id, until)
                                .await
                            {
                                error!(error = %e, site_id, "Google quota pause persistence failed");
                            }
                            info!(
                                site_id,
                                url_id = url.id,
                                "Google quota exceeded; stopping this site's remaining submissions"
                            );
                            break;
                        }
                    }
                    Err(e) => {
                        error!(error = %e, site_id, url_id = url.id, url = %url.url, "Google submission request failed");
                        let error_message = e.to_string();
                        if let Err(persist_error) = self
                            .urls
                            .apply_submit_outcome(
                                url.id,
                                None,
                                None,
                                Some("FAILED"),
                                Some(&error_message),
                            )
                            .await
                        {
                            error!(error = %persist_error, site_id, url_id = url.id, url = %url.url, "Google failed outcome persistence failed");
                        }
                    }
                }
            }
        }
        Ok(())
    }
}
