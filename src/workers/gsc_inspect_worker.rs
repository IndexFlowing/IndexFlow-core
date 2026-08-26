use crate::application::GscService;
use crate::config::AppConfig;
use crate::infrastructure::{SiteRepo, UrlRepo};
use std::sync::atomic::{AtomicBool, Ordering};
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
    is_running: Arc<AtomicBool>,
    config: AppConfig,
}

impl GscInspectWorker {
    pub fn new(
        urls: UrlRepo,
        sites: SiteRepo,
        gsc: GscService,
        is_running: Arc<AtomicBool>,
        config: AppConfig,
    ) -> Self {
        Self {
            urls,
            sites,
            gsc,
            is_running,
            config,
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
        if !self.is_running.load(Ordering::Relaxed) {
            return Ok(());
        }

        let pending = self.urls.fetch_pending_gsc(50).await?;
        if pending.is_empty() {
            self.is_running.store(false, Ordering::Relaxed);
            info!("🎉 GSC 增量检测队列已全部处理完毕，Worker 回到待机");
            return Ok(());
        }

        info!(count = pending.len(), "📊 [GSC Worker] 正在以安全平滑速率 (3 并发) 向 Google 查询真实收录状态...");

        // 调优：3 并发，完美规避 Google 瞬时 QPS 防刷限流
        let semaphore = Arc::new(Semaphore::new(3));
        let mut set = JoinSet::new();

        for url in pending {
            if !self.is_running.load(Ordering::Relaxed) {
                break;
            }

            let sem = semaphore.clone();
            let gsc_svc = self.gsc.clone();
            let sites = self.sites.clone();

            set.spawn(async move {
                let _permit = sem.acquire().await.unwrap();
                // 每次请求加入 250ms 平滑间隔，保持在 4 QPS 黄金安全区间
                tokio::time::sleep(Duration::from_millis(250)).await;

                if let Ok(Some(site)) = sites.find_by_id(url.site_id).await {
                    if let Ok(res) = gsc_svc.inspect_one(&site, &url.url).await {
                        let _ = gsc_svc.apply_inspect_result(url.id, &res).await;
                        return res.is_quota_exceeded;
                    }
                }
                false
            });
        }

        while let Some(res) = set.join_next().await {
            if let Ok(is_quota_exceeded) = res {
                if is_quota_exceeded {
                    // 核心熔断器：一旦触发 429，毫秒级终止任务并关闭开关
                    warn!("🛑 [GSC Worker] 触发 Google 429 限流/配额上限，系统已启动自动熔断保护，暂停后续请求");
                    self.is_running.store(false, Ordering::Relaxed);
                    break;
                }
            }
        }
        Ok(())
    }
}