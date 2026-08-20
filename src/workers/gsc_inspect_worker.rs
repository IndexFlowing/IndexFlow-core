use crate::application::GscService;
use crate::config::AppConfig;
use crate::domain::{fair_site_plan, TaskType};
use crate::infrastructure::{SiteRepo, TaskRepo, UrlRepo};
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Arc;
use std::time::Duration as StdDuration;
use tracing::{error, info, warn};

/// Layer 2: GSC URL Inspection API worker (max 2,000/day).
#[derive(Clone)]
pub struct GscInspectWorker {
    tasks: TaskRepo,
    urls: UrlRepo,
    sites: SiteRepo,
    gsc: GscService,
    config: AppConfig,
    last_site_cursor: Arc<AtomicI64>,
}

impl GscInspectWorker {
    pub fn new(
        tasks: TaskRepo,
        urls: UrlRepo,
        sites: SiteRepo,
        gsc: GscService,
        config: AppConfig,
    ) -> Self {
        Self {
            tasks,
            urls,
            sites,
            gsc,
            config,
            last_site_cursor: Arc::new(AtomicI64::new(0)),
        }
    }

    pub fn start(self: Arc<Self>) {
        let interval = StdDuration::from_secs(self.config.submit_worker_interval_secs.max(2));
        tokio::spawn(async move {
            info!(
                quota = self.config.gsc_inspect_daily_quota,
                "gsc inspect worker started"
            );
            loop {
                if let Err(e) = self.tick().await {
                    error!(error = %e, "gsc inspect worker tick failed");
                }
                tokio::time::sleep(interval).await;
            }
        });
    }

    async fn tick(&self) -> anyhow::Result<()> {
        let pending_sites = self.tasks.pending_site_ids(TaskType::GscInspect).await?;
        if pending_sites.is_empty() {
            return Ok(());
        }
        let after = self.last_site_cursor.load(Ordering::Relaxed);
        // Keep batches small — Inspection API is 1 URL per call.
        let batch = self.config.submit_worker_batch.min(10);
        let (selected, per_site) = fair_site_plan(&pending_sites, batch, after);
        if selected.is_empty() {
            return Ok(());
        }

        for site_id in &selected {
            let claimed = self
                .tasks
                .claim_for_site(*site_id, TaskType::GscInspect, per_site)
                .await?;
            if claimed.is_empty() {
                continue;
            }
            if let Err(e) = self.process_site(*site_id, claimed).await {
                warn!(site_id, error = %e, "gsc inspect batch failed");
            }
        }
        if let Some(&last) = selected.last() {
            self.last_site_cursor.store(last, Ordering::Relaxed);
        }
        Ok(())
    }

    async fn process_site(
        &self,
        site_id: i64,
        tasks: Vec<crate::domain::Task>,
    ) -> anyhow::Result<()> {
        let site = self
            .sites
            .find_by_id(site_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("site {site_id} not found"))?;
        if !site.has_google_credentials() {
            for t in &tasks {
                let _ = self
                    .tasks
                    .mark_failed(t.id, "no Google credentials configured")
                    .await;
            }
            return Ok(());
        }

        let stats = self.urls.index_funnel_stats(site_id).await?;
        let mut remaining =
            (self.config.gsc_inspect_daily_quota as i64 - stats.inspected_24h).max(0);

        for task in tasks {
            if remaining <= 0 {
                let until = chrono::Utc::now() + chrono::Duration::hours(1);
                self.tasks
                    .reschedule(task.id, until, Some("GSC inspection 24h quota exhausted"))
                    .await?;
                continue;
            }
            let Some(url_id) = task.url_id else {
                self.tasks.mark_failed(task.id, "GSC_INSPECT missing url_id").await?;
                continue;
            };
            let Some(url) = self.urls.find_by_id(url_id).await? else {
                self.tasks
                    .mark_failed(task.id, &format!("url {url_id} not found"))
                    .await?;
                continue;
            };

            match self.gsc.inspect_one(&site, &url.url).await {
                Ok(result) => {
                    remaining -= 1;
                    match self
                        .gsc
                        .apply_inspect_result(url.id, site.bing_ready(), &result)
                        .await
                    {
                        Ok(status) => {
                            info!(
                                url_id = url.id,
                                coverage = ?result.coverage_state,
                                status,
                                "gsc inspected"
                            );
                            self.tasks.mark_success(task.id).await?;
                        }
                        Err(e) => {
                            self.tasks.mark_failed(task.id, &e.to_string()).await?;
                        }
                    }
                }
                Err(e) => {
                    warn!(url_id = url.id, error = %e, "gsc inspect call failed");
                    self.tasks.mark_failed(task.id, &e.to_string()).await?;
                }
            }
        }
        Ok(())
    }
}
