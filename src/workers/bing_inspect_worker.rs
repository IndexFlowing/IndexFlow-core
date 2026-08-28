use crate::application::BingService;
use crate::config::AppConfig;
use crate::infrastructure::{SiteRepo, UrlRepo};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Semaphore;
use tokio::task::JoinSet;
use tracing::{error, info};

#[derive(Clone)]
pub struct BingInspectWorker {
    urls: UrlRepo,
    sites: SiteRepo,
    bing_svc: BingService,
    is_running: Arc<AtomicBool>,
    config: AppConfig,
}

impl BingInspectWorker {
    pub fn new(
        urls: UrlRepo,
        sites: SiteRepo,
        bing_svc: BingService,
        is_running: Arc<AtomicBool>,
        config: AppConfig,
    ) -> Self {
        Self {
            urls,
            sites,
            bing_svc,
            is_running,
            config,
        }
    }

    pub fn start(self: Arc<Self>) {
        let interval = Duration::from_secs(self.config.submit_worker_interval_secs);
        tokio::spawn(async move {
            info!("Bing Inspect Worker 待机就绪 (5 并发)");
            loop {
                if let Err(e) = self.tick().await {
                    error!(error = %e, "Bing inspect worker tick failed");
                }
                tokio::time::sleep(interval).await;
            }
        });
    }

    async fn tick(&self) -> anyhow::Result<()> {
        if !self.is_running.load(Ordering::Relaxed) {
            return Ok(());
        }

        let pending = self.urls.fetch_pending_bing_inspect(50).await?;
        if pending.is_empty() {
            self.is_running.store(false, Ordering::Relaxed);
            info!("🎉 Bing 官方增量收录检测队列已全部处理完毕，Worker 回到待机");
            return Ok(());
        }

        info!(count = pending.len(), "🔍 [Bing Worker] 正在向 Bing Webmaster API 查询真实收录状态...");

        let semaphore = Arc::new(Semaphore::new(5));
        let mut set = JoinSet::new();

        for url in pending {
            if !self.is_running.load(Ordering::Relaxed) {
                break;
            }

            let sem = semaphore.clone();
            let bing_svc = self.bing_svc.clone();
            let sites = self.sites.clone();

            set.spawn(async move {
                let _permit = sem.acquire().await.unwrap();
                tokio::time::sleep(Duration::from_millis(200)).await;

                if let Ok(Some(site)) = sites.find_by_id(url.site_id).await {
                    if site.has_bing_webmaster_key() {
                        if let Ok(res) = bing_svc.inspect_one(&site, &url.url).await {
                            let _ = bing_svc.apply_inspect_result(url.id, &res).await;
                        }
                    }
                }
            });
        }

        while set.join_next().await.is_some() {}
        Ok(())
    }
}