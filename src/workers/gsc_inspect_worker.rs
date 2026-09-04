use crate::application::{GscService, PipelineManager};
use crate::config::AppConfig;
use crate::domain::PipelineStage;
use crate::infrastructure::{SiteRepo, UrlRepo, ViewBoostRegistry};
use std::sync::Arc;
use std::time::Duration;
use tracing::{error, info, warn};

#[derive(Clone)]
pub struct GscInspectWorker {
    urls: UrlRepo,
    sites: SiteRepo,
    gsc: GscService,
    pipeline: PipelineManager,
    config: AppConfig,
    view_boost: ViewBoostRegistry,
}

impl GscInspectWorker {
    pub fn new(
        urls: UrlRepo,
        sites: SiteRepo,
        gsc: GscService,
        pipeline: PipelineManager,
        config: AppConfig,
        view_boost: ViewBoostRegistry,
    ) -> Self {
        Self {
            urls,
            sites,
            gsc,
            pipeline,
            config,
            view_boost,
        }
    }

    pub fn start(self: Arc<Self>) {
        let interval = Duration::from_secs(self.config.submit_worker_interval_secs);
        tokio::spawn(async move {
            info!("GSC Inspect Worker 待机就绪 (串行平滑限流版)");
            loop {
                if let Err(e) = self.tick().await {
                    error!(error = %e, "GSC inspect worker tick failed");
                }
                tokio::time::sleep(interval).await;
            }
        });
    }

    async fn tick(&self) -> anyhow::Result<()> {
        let running_sites = self
            .pipeline
            .running_sites_for_stage(PipelineStage::GscInspect);
        if running_sites.is_empty() {
            return Ok(());
        }

        for site_id in running_sites {
            if !self.pipeline.is_running(site_id, PipelineStage::GscInspect) {
                continue;
            }

            let boosted = self.view_boost.current_ids(Duration::from_secs(30));
            let pending = self.urls.fetch_pending_gsc(site_id, 50, &boosted).await?;
            if pending.is_empty() {
                self.pipeline.stop(site_id, PipelineStage::GscInspect);
                info!(
                    site_id,
                    "🎉 GSC 增量检测队列已全部处理完毕，Worker 回到待机"
                );
                continue;
            }

            info!(
                site_id,
                count = pending.len(),
                "📊 [GSC Worker] 正在以安全平滑速率串行查询 Google 真实收录状态..."
            );

            for url in pending {
                if !self.pipeline.is_running(site_id, PipelineStage::GscInspect) {
                    break;
                }

                tokio::time::sleep(Duration::from_millis(250)).await;

                let site = match self.sites.find_by_id(url.site_id).await {
                    Ok(Some(site)) => site,
                    Ok(None) => {
                        error!(site_id, url_id = url.id, url = %url.url, "GSC inspect skipped: site not found");
                        if let Err(e) = self
                            .urls
                            .apply_gsc_inspection(url.id, "FAILED", Some("站点不存在"), None)
                            .await
                        {
                            error!(error = %e, site_id, url_id = url.id, "GSC missing site failure persistence failed");
                        }
                        continue;
                    }
                    Err(e) => {
                        error!(error = %e, site_id, url_id = url.id, url = %url.url, "GSC inspect skipped: failed to load site");
                        continue;
                    }
                };

                if !site.has_google_credentials() {
                    let reason = "尚未配置 Google Service Account 密钥";
                    error!(site_id, url_id = url.id, url = %url.url, "GSC inspect skipped: Google credentials are not configured");
                    if let Err(e) = self
                        .urls
                        .apply_gsc_inspection(url.id, "FAILED", Some(reason), None)
                        .await
                    {
                        error!(error = %e, site_id, url_id = url.id, "GSC missing credentials failure persistence failed");
                    }
                    continue;
                }

                let result = match self.gsc.inspect_one(&site, &url.url).await {
                    Ok(result) => result,
                    Err(e) => {
                        error!(error = %e, site_id, url_id = url.id, url = %url.url, "GSC inspect request failed");
                        continue;
                    }
                };

                if let Err(e) = self
                    .gsc
                    .apply_inspect_result(url.id, &result, url.is_watched)
                    .await
                {
                    error!(error = %e, site_id, url_id = url.id, url = %url.url, "GSC inspect result persistence failed");
                }

                if !result.ok && !result.is_quota_exceeded {
                    let reason = result
                        .raw_response
                        .as_deref()
                        .unwrap_or("Google Inspection API 返回失败");
                    if let Err(e) = self
                        .urls
                        .apply_gsc_inspection(url.id, "FAILED", Some(reason), None)
                        .await
                    {
                        error!(error = %e, site_id, url_id = url.id, "GSC failed result persistence failed");
                    }
                }

                if result.is_quota_exceeded {
                    warn!(site_id, "🛑 [GSC Worker] 触发 Google 429 限流/配额上限，系统已启动自动熔断保护，暂停后续请求");
                    self.pipeline.stop(site_id, PipelineStage::GscInspect);
                    break;
                }
            }
        }
        Ok(())
    }
}
