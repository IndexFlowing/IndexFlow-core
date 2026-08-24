use crate::application::SitemapService;
use crate::config::AppConfig;
use crate::infrastructure::{SiteRepo, UrlRepo};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tracing::{error, info, warn};

#[derive(Clone)]
pub struct SyncWorker {
    urls: UrlRepo,
    sites: SiteRepo,
    sitemap_service: SitemapService,
    is_sync_running: Arc<AtomicBool>,
    config: AppConfig,
}

impl SyncWorker {
    pub fn new(
        urls: UrlRepo,
        sites: SiteRepo,
        sitemap_service: SitemapService,
        is_sync_running: Arc<AtomicBool>,
        config: AppConfig,
    ) -> Self {
        Self {
            urls,
            sites,
            sitemap_service,
            is_sync_running,
            config,
        }
    }

    pub fn start(self: Arc<Self>) {
        let interval = Duration::from_secs(self.config.worker_poll_interval_secs);
        tokio::spawn(async move {
            info!("Sitemap Sync Worker 待机就绪");
            loop {
                if let Err(e) = self.tick().await {
                    error!(error = %e, "sync worker tick failed");
                }
                tokio::time::sleep(interval).await;
            }
        });
    }

    async fn tick(&self) -> anyhow::Result<()> {
        if !self.is_sync_running.load(Ordering::Relaxed) {
            return Ok(());
        }

        // 抢占到执行权后立即重置标志
        self.is_sync_running.store(false, Ordering::Relaxed);

        let Some(site) = self.sites.get().await? else {
            warn!("Site configuration not found");
            return Ok(());
        };

        let Some(ref sm_url) = site.sitemap_url else {
            warn!("No sitemap URL configured");
            return Ok(());
        };

        info!(sitemap = %sm_url, "🌐 [SyncWorker] 正在连接目标 Sitemap 并流式解析...");
        let (_is_index, entries) = self.sitemap_service.expand_to_page_entries(sm_url, 3).await?;

        if entries.is_empty() {
            warn!(sitemap = %sm_url, "⚠️ [SyncWorker] Sitemap 解析完毕，但提取到 0 条有效 URL");
            return Ok(());
        }

        info!(total_found = entries.len(), "✨ [SyncWorker] Sitemap 解析成功，正在批量写入 SQLite...");

        let chunk_size = 500usize;
        let mut total_inserted = 0u64;

        for chunk in entries.chunks(chunk_size) {
            let (inserted, _, _) = self.urls.batch_upsert_discovered(chunk).await?;
            total_inserted += inserted;
        }

        info!(
            total_urls = entries.len(),
            new_inserted = total_inserted,
            "🎉 [SyncWorker] Sitemap 数据同步入库完成！"
        );
        Ok(())
    }
}