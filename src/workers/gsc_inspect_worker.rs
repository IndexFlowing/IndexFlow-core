use crate::application::{GscService, PipelineManager};
use crate::config::AppConfig;
use crate::domain::PipelineStage;
use crate::infrastructure::{SiteRepo, UrlRepo, ViewBoostRegistry};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Semaphore;
use tokio::task::JoinSet;
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
            info!("GSC Inspect Worker 待机就绪 (3 并发平滑限流版)");
            loop {
                if let Err(e) = self.tick().await {
                    error!(error = %e, "GSC inspect worker tick failed");
                }
                tokio::time::sleep(interval).await;
            }
        });
    }

    async fn tick(&self) -> anyhow::Result<()> {
        let running_sites = self.pipeline.running_sites_for_stage(PipelineStage::GscInspect);
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
                info!(site_id, "🎉 GSC 增量检测队列已全部处理完毕，Worker 回到待机");
                continue;
            }

            info!(
                site_id,
                count = pending.len(),
                "📊 [GSC Worker] 正在以安全平滑速率 (3 并发) 向 Google 查询真实收录状态..."
            );

            let semaphore = Arc::new(Semaphore::new(3));
            let mut set = JoinSet::new();

            for url in pending {
                if !self.pipeline.is_running(site_id, PipelineStage::GscInspect) {
                    break;
                }

                let sem = semaphore.clone();
                let gsc_svc = self.gsc.clone();
                let sites = self.sites.clone();

                set.spawn(async move {
                    let _permit = sem.acquire().await.unwrap();
                    tokio::time::sleep(Duration::from_millis(250)).await;

                    if let Ok(Some(site)) = sites.find_by_id(url.site_id).await {
                        if let Ok(res) = gsc_svc.inspect_one(&site, &url.url).await {
                            let _ = gsc_svc
                                .apply_inspect_result(url.id, &res, url.is_watched)
                                .await;
                            return res.is_quota_exceeded;
                        }
                    }
                    false
                });
            }

            while let Some(res) = set.join_next().await {
                if let Ok(true) = res {
                    warn!(site_id, "🛑 [GSC Worker] 触发 Google 429 限流/配额上限，系统已启动自动熔断保护，暂停后续请求");
                    self.pipeline.stop(site_id, PipelineStage::GscInspect);
                    break;
                }
            }
        }
        Ok(())
    }
}