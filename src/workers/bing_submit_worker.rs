use crate::application::{PipelineManager, SubmissionService};
use crate::config::AppConfig;
use crate::domain::{PipelineStage, ProviderKind};
use crate::infrastructure::{SiteRepo, SubmissionLogRepo, UrlRepo};
use std::sync::Arc;
use std::time::Duration;
use tracing::{error, info};

#[derive(Clone)]
pub struct BingSubmitWorker {
    urls: UrlRepo,
    sites: SiteRepo,
    logs: SubmissionLogRepo,
    submission: SubmissionService,
    pipeline: PipelineManager,
    config: AppConfig,
}

impl BingSubmitWorker {
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
                .fetch_pending_bing(site_id, self.config.submit_worker_batch)
                .await?;
            if pending.is_empty() {
                let google_left = self.urls.fetch_pending_google(site_id, 1).await?;
                if google_left.is_empty() {
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
                        let message = "Bing submit failed: site not found";
                        error!(site_id, url_id = url.id, url = %url.url, "{message}");
                        if let Err(e) = self
                            .urls
                            .apply_submit_outcome(url.id, Some("FAILED"), Some(message), None, None)
                            .await
                        {
                            error!(error = %e, site_id, url_id = url.id, url = %url.url, "Bing failed outcome persistence failed");
                        }
                        continue;
                    }
                    Err(e) => {
                        let message = format!("Bing submit failed: failed to load site: {e}");
                        error!(error = %e, site_id, url_id = url.id, url = %url.url, "Bing site lookup failed");
                        if let Err(persist_error) = self
                            .urls
                            .apply_submit_outcome(
                                url.id,
                                Some("FAILED"),
                                Some(&message),
                                None,
                                None,
                            )
                            .await
                        {
                            error!(error = %persist_error, site_id, url_id = url.id, url = %url.url, "Bing failed outcome persistence failed");
                        }
                        continue;
                    }
                };

                if !site.bing_ready() {
                    info!(site_id, url_id = url.id, url = %url.url, "Bing submit skipped: Bing credentials are not configured");
                    if let Err(e) = self
                        .urls
                        .apply_submit_outcome(
                            url.id,
                            Some("FAILED"),
                            Some("Bing credentials are not configured"),
                            None,
                            None,
                        )
                        .await
                    {
                        error!(error = %e, site_id, url_id = url.id, "Bing missing credentials failure persistence failed");
                    }
                    continue;
                }

                let key = site.bing_indexnow_key.as_deref().unwrap_or("");
                let results = match self
                    .submission
                    .submit_url_batch_bing(&site.domain, key, &[url.url.clone()])
                    .await
                {
                    Ok(results) => results,
                    Err(e) => {
                        error!(error = %e, site_id, url_id = url.id, url = %url.url, "Bing submission request failed");
                        let message = e.to_string();
                        if let Err(persist_error) = self
                            .urls
                            .apply_submit_outcome(
                                url.id,
                                Some("FAILED"),
                                Some(&message),
                                None,
                                None,
                            )
                            .await
                        {
                            error!(error = %persist_error, site_id, url_id = url.id, url = %url.url, "Bing failed outcome persistence failed");
                        }
                        continue;
                    }
                };

                let Some(res) = results.first() else {
                    let message = "Bing submission returned an empty result";
                    error!(site_id, url_id = url.id, url = %url.url, "{message}");
                    if let Err(e) = self
                        .urls
                        .apply_submit_outcome(url.id, Some("FAILED"), Some(message), None, None)
                        .await
                    {
                        error!(error = %e, site_id, url_id = url.id, url = %url.url, "Bing failed outcome persistence failed");
                    }
                    continue;
                };

                if let Err(e) = self
                    .logs
                    .insert(
                        url.id,
                        ProviderKind::Bing,
                        res.is_success,
                        res.status_code.map(|c| c as i32),
                        res.response_msg.as_deref(),
                    )
                    .await
                {
                    error!(error = %e, site_id, url_id = url.id, url = %url.url, "Bing submission log persistence failed");
                }

                let st = if res.is_success {
                    "SUBMITTED"
                } else {
                    "FAILED"
                };
                if let Err(e) = self
                    .urls
                    .apply_submit_outcome(url.id, Some(st), res.response_msg.as_deref(), None, None)
                    .await
                {
                    error!(error = %e, site_id, url_id = url.id, url = %url.url, "Bing URL outcome persistence failed");
                }

                if res.is_quota_exceeded {
                    info!(
                        site_id,
                        url_id = url.id,
                        "Bing quota exceeded; stopping this site's remaining submissions"
                    );
                    break;
                }
            }
        }
        Ok(())
    }
}
