use crate::application::{BingService, PipelineManager};
use crate::config::AppConfig;
use crate::domain::PipelineStage;
use crate::infrastructure::{SiteRepo, UrlRepo, ViewBoostRegistry};
use std::sync::Arc;
use std::time::Duration;
use tracing::{error, info, warn};

#[derive(Clone)]
pub struct BingInspectWorker {
    urls: UrlRepo,
    sites: SiteRepo,
    bing_svc: BingService,
    pipeline: PipelineManager,
    config: AppConfig,
    view_boost: ViewBoostRegistry,
}

impl BingInspectWorker {
    pub fn new(
        urls: UrlRepo,
        sites: SiteRepo,
        bing_svc: BingService,
        pipeline: PipelineManager,
        config: AppConfig,
        view_boost: ViewBoostRegistry,
    ) -> Self {
        Self {
            urls,
            sites,
            bing_svc,
            pipeline,
            config,
            view_boost,
        }
    }

    pub fn start(self: Arc<Self>) {
        let interval = Duration::from_secs(self.config.submit_worker_interval_secs);
        tokio::spawn(async move {
            info!("Bing Inspect Worker 待机就绪 (串行处理 / 500ms 间隔)");
            loop {
                if let Err(e) = self.tick().await {
                    error!(error = %e, "Bing inspect worker tick failed");
                }
                tokio::time::sleep(interval).await;
            }
        });
    }

    async fn tick(&self) -> anyhow::Result<()> {
        let running_sites = self
            .pipeline
            .running_sites_for_stage(PipelineStage::BingInspect);
        if running_sites.is_empty() {
            return Ok(());
        }

        for site_id in running_sites {
            if !self
                .pipeline
                .is_running(site_id, PipelineStage::BingInspect)
            {
                continue;
            }

            let boosted = self.view_boost.current_ids(Duration::from_secs(30));
            let pending = self
                .urls
                .fetch_pending_bing_inspect(site_id, 30, &boosted)
                .await?;
            if pending.is_empty() {
                self.pipeline.stop(site_id, PipelineStage::BingInspect);
                info!(
                    site_id,
                    "🎉 Bing 官方增量收录检测队列已全部处理完毕，Worker 回到待机"
                );
                continue;
            }

            for url in pending {
                if !self
                    .pipeline
                    .is_running(site_id, PipelineStage::BingInspect)
                {
                    break;
                }

                tokio::time::sleep(Duration::from_millis(500)).await;

                let site = match self.sites.find_by_id(url.site_id).await {
                    Ok(Some(site)) => site,
                    Ok(None) => {
                        warn!(
                            site_id = url.site_id,
                            url_id = url.id,
                            "Bing 检测跳过：站点不存在"
                        );
                        continue;
                    }
                    Err(e) => {
                        error!(
                            error = %e,
                            site_id = url.site_id,
                            url_id = url.id,
                            "Bing 检测获取站点失败"
                        );
                        continue;
                    }
                };

                if !site.has_bing_webmaster_key() {
                    let reason = "尚未在站点设置中配置 Bing Webmaster API Key";
                    warn!(
                        domain = %site.domain,
                        site_id,
                        url_id = url.id,
                        "🛑 站点尚未配置 Bing Webmaster API Key，中止检测"
                    );
                    if let Err(e) = self
                        .urls
                        .apply_bing_inspection(url.id, "FAILED", Some(reason), None)
                        .await
                    {
                        error!(
                            error = %e,
                            site_id,
                            url_id = url.id,
                            "Bing 检测标记缺少 API Key 的 URL 失败状态时持久化失败"
                        );
                    }
                    self.pipeline.stop(site_id, PipelineStage::BingInspect);
                    break;
                }

                match self.bing_svc.inspect_one(&site, &url.url).await {
                    Ok(res) if res.is_throttled => {
                        warn!(
                            site_id,
                            url_id = url.id,
                            "Bing API 触发频控，立即停止当前站点检测批次"
                        );
                        self.pipeline.stop(site_id, PipelineStage::BingInspect);
                        break;
                    }
                    Ok(res) => {
                        if let Err(e) = self
                            .bing_svc
                            .apply_inspect_result(url.id, &res, url.is_watched)
                            .await
                        {
                            error!(
                                error = %e,
                                site_id,
                                url_id = url.id,
                                "Bing 检测结果持久化失败"
                            );
                        }
                    }
                    Err(e) => {
                        let reason = format!("质检异常: {e}");
                        error!(
                            error = %e,
                            site_id,
                            url_id = url.id,
                            "Bing API 检测失败"
                        );
                        if let Err(persist_error) = self
                            .urls
                            .apply_bing_inspection(url.id, "FAILED", Some(&reason), None)
                            .await
                        {
                            error!(
                                error = %persist_error,
                                site_id,
                                url_id = url.id,
                                "Bing API 失败 URL 的失败状态持久化失败"
                            );
                        }
                    }
                }
            }
        }
        Ok(())
    }
}
