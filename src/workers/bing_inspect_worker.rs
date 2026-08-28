use crate::application::{BingService, PipelineManager};
use crate::config::AppConfig;
use crate::domain::PipelineStage;
use crate::infrastructure::{SiteRepo, UrlRepo};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Semaphore;
use tokio::task::JoinSet;
use tracing::{error, info, warn};

#[derive(Clone)]
pub struct BingInspectWorker {
    urls: UrlRepo,
    sites: SiteRepo,
    bing_svc: BingService,
    pipeline: PipelineManager,
    config: AppConfig,
}

impl BingInspectWorker {
    pub fn new(
        urls: UrlRepo,
        sites: SiteRepo,
        bing_svc: BingService,
        pipeline: PipelineManager,
        config: AppConfig,
    ) -> Self {
        Self {
            urls,
            sites,
            bing_svc,
            pipeline,
            config,
        }
    }

    pub fn start(self: Arc<Self>) {
        let interval = Duration::from_secs(self.config.submit_worker_interval_secs);
        tokio::spawn(async move {
            info!("Bing Inspect Worker 待机就绪 (2 并发 / 500ms 间隔，1.5 QPS 安全区)");
            loop {
                if let Err(e) = self.tick().await {
                    error!(error = %e, "Bing inspect worker tick failed");
                }
                tokio::time::sleep(interval).await;
            }
        });
    }

    async fn tick(&self) -> anyhow::Result<()> {
        if !self.pipeline.is_running(PipelineStage::BingInspect) {
            return Ok(());
        }

        let pending = self.urls.fetch_pending_bing_inspect(30).await?;
        if pending.is_empty() {
            self.pipeline.stop(PipelineStage::BingInspect);
            info!("🎉 Bing 官方增量收录检测队列已全部处理完毕，Worker 回到待机");
            return Ok(());
        }

        // 2 并发 + 单次 500ms 间隔，贴近 Bing 1.5 QPS 安全区间
        let semaphore = Arc::new(Semaphore::new(2));
        let mut set = JoinSet::new();

        for url in pending {
            if !self.pipeline.is_running(PipelineStage::BingInspect) {
                break;
            }

            let sem = semaphore.clone();
            let bing_svc = self.bing_svc.clone();
            let sites = self.sites.clone();
            let urls_repo = self.urls.clone();
            let pipeline = self.pipeline.clone();

            set.spawn(async move {
                let _permit = sem.acquire().await.unwrap();
                tokio::time::sleep(Duration::from_millis(500)).await;

                if let Ok(Some(site)) = sites.find_by_id(url.site_id).await {
                    if !site.has_bing_webmaster_key() {
                        warn!(
                            domain = %site.domain,
                            "🛑 站点尚未配置 Bing Webmaster API Key，中止检测"
                        );
                        let _ = urls_repo
                            .apply_bing_inspection(
                                url.id,
                                "FAILED",
                                Some("尚未在站点设置中配置 Bing Webmaster API Key"),
                                None,
                            )
                            .await;
                        pipeline.stop(PipelineStage::BingInspect);
                        return false;
                    }

                    match bing_svc.inspect_one(&site, &url.url).await {
                        Ok(res) => {
                            if res.is_throttled {
                                // ThrottleHost：休眠 2 秒退避，不视为永久失败
                                tokio::time::sleep(Duration::from_secs(2)).await;
                                return true;
                            }
                            let _ = bing_svc.apply_inspect_result(url.id, &res).await;
                        }
                        Err(e) => {
                            let _ = urls_repo
                                .apply_bing_inspection(
                                    url.id,
                                    "FAILED",
                                    Some(&format!("质检异常: {e}")),
                                    None,
                                )
                                .await;
                        }
                    }
                }
                false
            });
        }

        while let Some(res) = set.join_next().await {
            if let Ok(true) = res {
                tokio::time::sleep(Duration::from_secs(2)).await;
            }
        }
        Ok(())
    }
}
