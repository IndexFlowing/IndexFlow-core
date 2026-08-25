use crate::application::GscService;
use crate::config::AppConfig;
use crate::infrastructure::{SiteRepo, UrlRepo};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Semaphore;
use tokio::task::JoinSet;
use tracing::{error, info};

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
            info!("GSC Inspect Worker 待机就绪");
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
            return Ok(());
        }

        info!(count = pending.len(), "📊 [GSC Worker] 正在向 Google 查询真实收录状态...");

        let semaphore = Arc::new(Semaphore::new(10));
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
                if let Ok(Some(site)) = sites.find_by_id(url.site_id).await {
                    if let Ok(res) = gsc_svc.inspect_one(&site, &url.url).await {
                        let _ = gsc_svc.apply_inspect_result(url.id, &res).await;
                    }
                }
            });
        }

        while set.join_next().await.is_some() {}
        Ok(())
    }
}