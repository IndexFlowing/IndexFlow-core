use crate::application::{PipelineManager, SitemapService};
use crate::config::AppConfig;
use crate::domain::PipelineStage;
use crate::infrastructure::{SiteRepo, UrlRepo};
use std::sync::Arc;
use std::time::Duration;
use tracing::{error, info, warn};

#[derive(Clone)]
pub struct SyncWorker {
    urls: UrlRepo,
    sites: SiteRepo,
    sitemap_service: SitemapService,
    pipeline: PipelineManager,
    config: AppConfig,
}

impl SyncWorker {
    pub fn new(
        urls: UrlRepo,
        sites: SiteRepo,
        sitemap_service: SitemapService,
        pipeline: PipelineManager,
        config: AppConfig,
    ) -> Self {
        Self {
            urls,
            sites,
            sitemap_service,
            pipeline,
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
        let running_sites = self.pipeline.running_sites_for_stage(PipelineStage::Sitemap);
        if running_sites.is_empty() {
            return Ok(());
        }

        for site_id in running_sites {
            if !self.pipeline.is_running(site_id, PipelineStage::Sitemap) {
                continue;
            }

            let site = match self.sites.find_by_id(site_id).await {
                Ok(Some(s)) => s,
                _ => {
                    self.pipeline.stop(site_id, PipelineStage::Sitemap);
                    continue;
                }
            };

            let Some(ref sm_url) = site.sitemap_url else {
                warn!(site_id, domain = %site.domain, "站点未配置 Sitemap URL，跳过同步");
                self.pipeline.stop(site_id, PipelineStage::Sitemap);
                continue;
            };

            let effective_ua = site.effective_crawler_ua();

            info!(sitemap = %sm_url, domain = %site.domain, site_id, ua = ?effective_ua, "🌐 [SyncWorker] 正在连接目标 Sitemap 并流式解析...");
            let (_is_index, entries) = match self.sitemap_service.expand_to_page_entries(sm_url, 3, effective_ua.as_deref()).await {
                Ok(res) => res,
                Err(e) => {
                    warn!(error = %e, site_id, "Sitemap fetch failed");
                    self.pipeline.stop(site_id, PipelineStage::Sitemap);
                    continue;
                }
            };

            let chunk_size = 500usize;
            let mut total_inserted = 0u64;

            for chunk in entries.chunks(chunk_size) {
                if !self.pipeline.is_running(site_id, PipelineStage::Sitemap) {
                    info!(site_id, "Sitemap 同步已被取消，Worker 回到待机");
                    return Ok(());
                }
                let (inserted, _, _) = self.urls.batch_upsert_discovered(site.id, chunk).await?;
                total_inserted += inserted;
            }

            info!(
                domain = %site.domain,
                site_id,
                total_urls = entries.len(),
                new_inserted = total_inserted,
                "🎉 [SyncWorker] 站点 Sitemap 同步入库完成！"
            );

            self.pipeline.stop(site_id, PipelineStage::Sitemap);
        }

        Ok(())
    }
}