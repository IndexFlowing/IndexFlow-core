use crate::application::SitemapService;
use crate::config::AppConfig;
use crate::domain::{SiteStatus, SitemapStatus, TaskType};
use crate::infrastructure::{SiteRepo, SitemapRepo, TaskRepo, UrlRepo};
use std::sync::Arc;
use std::time::Duration;
use tracing::{error, info, warn};

/// Worker: SYNC_SITEMAP — download/parse sitemap, upsert URLs.
#[derive(Clone)]
pub struct SyncWorker {
    tasks: TaskRepo,
    sitemaps: SitemapRepo,
    urls: UrlRepo,
    sites: SiteRepo,
    sitemap_service: SitemapService,
    config: AppConfig,
}

impl SyncWorker {
    pub fn new(
        tasks: TaskRepo,
        sitemaps: SitemapRepo,
        urls: UrlRepo,
        sites: SiteRepo,
        sitemap_service: SitemapService,
        config: AppConfig,
    ) -> Self {
        Self {
            tasks,
            sitemaps,
            urls,
            sites,
            sitemap_service,
            config,
        }
    }

    pub fn start(self: Arc<Self>) {
        let interval = Duration::from_secs(self.config.worker_poll_interval_secs);
        tokio::spawn(async move {
            info!("sync worker started");
            loop {
                if let Err(e) = self.tick().await {
                    error!(error = %e, "sync worker tick failed");
                }
                tokio::time::sleep(interval).await;
            }
        });
    }

    async fn tick(&self) -> anyhow::Result<()> {
        let claimed = self
            .tasks
            .claim(TaskType::SyncSitemap, self.config.sync_worker_batch)
            .await?;
        for task in claimed {
            if let Err(e) = self.process_task(&task).await {
                error!(task_id = task.id, error = %e, "SYNC_SITEMAP failed");
                let _ = self.tasks.mark_failed(task.id, &e.to_string()).await;
                if let Some(sm_id) = task.sitemap_id {
                    let _ = self.sitemaps.mark_failed(sm_id, &e.to_string()).await;
                }
                let _ = self
                    .sites
                    .update_status(task.site_id, SiteStatus::NeedAttention)
                    .await;
            }
        }
        Ok(())
    }

    async fn process_task(&self, task: &crate::domain::Task) -> anyhow::Result<()> {
        let sitemap_id = task
            .sitemap_id
            .ok_or_else(|| anyhow::anyhow!("SYNC_SITEMAP missing sitemap_id"))?;
        let sitemap = self
            .sitemaps
            .find_by_id(sitemap_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("sitemap {sitemap_id} not found"))?;

        info!(
            site_id = task.site_id,
            sitemap = %sitemap.url,
            "syncing sitemap"
        );

        let (sm_type, page_entries) = self
            .sitemap_service
            .expand_to_page_entries(&sitemap.url, 3)
            .await?;

        if page_entries.is_empty() {
            warn!(sitemap = %sitemap.url, "sitemap yielded 0 URLs");
        }

        // Process in chunks to avoid huge memory spikes for million-URL sites
        let chunk_size = 500usize;
        let mut total_inserted = 0u64;
        let mut total_urls = 0u64;

        // Controlled workflow: only discover URLs (DISCOVERED). No auto CHECK/SUBMIT.
        // Priority initialized from <priority> + <lastmod> + new-discovery boost.
        for chunk in page_entries.chunks(chunk_size) {
            let (inserted, ids, _new_ids) = self
                .urls
                .batch_upsert_discovered(task.site_id, chunk)
                .await?;
            total_inserted += inserted;
            total_urls += ids.len() as u64;
        }

        self.sitemaps
            .mark_synced(sitemap_id, sm_type, SitemapStatus::Active, None)
            .await?;
        self.sites
            .update_status(task.site_id, SiteStatus::Ready)
            .await?;
        self.tasks.mark_success(task.id).await?;

        info!(
            site_id = task.site_id,
            total_urls,
            total_inserted,
            "sitemap sync complete"
        );
        Ok(())
    }
}
