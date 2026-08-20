use crate::application::HealthService;
use crate::config::AppConfig;
use crate::domain::{fair_site_plan, TaskType, Url};
use crate::infrastructure::{HealthCheckRepo, TaskRepo, UrlRepo};
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Arc;
use std::time::Duration as StdDuration;
use tracing::{error, info, warn};

/// Standalone SEO quality scanner (CHECK_URL).
/// Does not enqueue or run Bing/Google submit workers.
#[derive(Clone)]
pub struct SeoAuditWorker {
    tasks: TaskRepo,
    urls: UrlRepo,
    health_repo: HealthCheckRepo,
    health: HealthService,
    config: AppConfig,
    last_site_cursor: Arc<AtomicI64>,
}

impl SeoAuditWorker {
    pub fn new(
        tasks: TaskRepo,
        urls: UrlRepo,
        health_repo: HealthCheckRepo,
        health: HealthService,
        config: AppConfig,
    ) -> Self {
        Self {
            tasks,
            urls,
            health_repo,
            health,
            config,
            last_site_cursor: Arc::new(AtomicI64::new(0)),
        }
    }

    pub fn start(self: Arc<Self>) {
        let interval = StdDuration::from_secs(self.config.worker_poll_interval_secs);
        tokio::spawn(async move {
            info!("seo audit worker started (standalone quality gate)");
            loop {
                if let Err(e) = self.tick().await {
                    error!(error = %e, "seo audit worker tick failed");
                }
                tokio::time::sleep(interval).await;
            }
        });
    }

    async fn tick(&self) -> anyhow::Result<()> {
        let pending_sites = self.tasks.pending_site_ids(TaskType::CheckUrl).await?;
        if pending_sites.is_empty() {
            return Ok(());
        }
        let after = self.last_site_cursor.load(Ordering::Relaxed);
        let (selected, per_site) =
            fair_site_plan(&pending_sites, self.config.submit_worker_batch, after);
        if selected.is_empty() {
            return Ok(());
        }

        for site_id in &selected {
            let claimed = self
                .tasks
                .claim_for_site(*site_id, TaskType::CheckUrl, per_site)
                .await?;
            for task in claimed {
                if let Err(e) = self.process_one(&task).await {
                    warn!(task_id = task.id, error = %e, "CHECK_URL failed");
                    let _ = self.tasks.mark_failed(task.id, &e.to_string()).await;
                }
            }
        }
        if let Some(&last) = selected.last() {
            self.last_site_cursor.store(last, Ordering::Relaxed);
        }
        Ok(())
    }

    async fn process_one(&self, task: &crate::domain::Task) -> anyhow::Result<()> {
        let url_id = task
            .url_id
            .ok_or_else(|| anyhow::anyhow!("CHECK_URL missing url_id"))?;
        let url = self
            .urls
            .find_by_id(url_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("url {url_id} not found"))?;
        self.scan(&url).await?;
        self.tasks.mark_success(task.id).await
    }

    async fn scan(&self, url: &Url) -> anyhow::Result<()> {
        let gate = self.health.check_url(&url.url).await;
        self.health_repo.insert_from_gate(url.id, &gate).await?;
        self.urls.persist_seo_scan(url.id, &gate).await?;
        if gate.passed {
            info!(url_id = url.id, url = %url.url, "SEO audit passed");
        } else {
            info!(
                url_id = url.id,
                url = %url.url,
                reason = ?gate.block_reason,
                "SEO audit blocked"
            );
        }
        Ok(())
    }
}
