use crate::config::AppConfig;
use crate::infrastructure::{SiteRepo, TaskRepo, UrlRepo};
use chrono::{Duration, Utc};
use std::sync::Arc;
use tracing::{error, info};

/// Scheduler in controlled mode:
/// - Does NOT auto-create SUBMIT_URL (user triggers via site workbench)
/// - Only requeues failed tasks that were already created by user actions
#[derive(Clone)]
pub struct Scheduler {
    #[allow(dead_code)]
    urls: UrlRepo,
    tasks: TaskRepo,
    #[allow(dead_code)]
    sites: SiteRepo,
    config: AppConfig,
}

impl Scheduler {
    pub fn new(urls: UrlRepo, tasks: TaskRepo, sites: SiteRepo, config: AppConfig) -> Self {
        Self {
            urls,
            tasks,
            sites,
            config,
        }
    }

    pub fn start(self: Arc<Self>) {
        let interval_secs = self.config.scheduler_interval_secs;
        tokio::spawn(async move {
            info!(
                interval_secs,
                mode = "controlled",
                "scheduler started (retry-only; no auto check/submit)"
            );
            let mut ticker = tokio::time::interval(std::time::Duration::from_secs(interval_secs));
            loop {
                ticker.tick().await;
                if let Err(e) = self.run_once().await {
                    error!(error = %e, "scheduler tick failed");
                }
            }
        });
    }

    pub async fn run_once(&self) -> anyhow::Result<()> {
        // 1. Recover stale PROCESSING tasks older than 5 minutes (crash/restart deadlock).
        let recovered = self.tasks.recover_stale_processing(5).await?;
        if recovered > 0 {
            info!(recovered, "recovered stale PROCESSING tasks");
        }

        // 2. Requeue failed tasks eligible for retry.
        let retries = self.schedule_retries().await?;
        if retries > 0 {
            info!(retries, "scheduler requeued failed tasks");
        }
        Ok(())
    }

    async fn schedule_retries(&self) -> anyhow::Result<u64> {
        let failed = self
            .tasks
            .find_failed_for_retry(
                self.config.max_task_retries,
                self.config.scheduler_batch_size,
            )
            .await?;
        let mut requeued = 0u64;

        for task in failed {
            if let Some(err) = &task.last_error {
                let lower = err.to_lowercase();
                if lower.contains("404")
                    || lower.contains("410")
                    || lower.contains("permission")
                    || lower.contains("auth")
                    || lower.contains("noindex")
                    || lower.contains("no provider")
                    || lower.contains("credential")
                {
                    continue;
                }
            }

            let minutes = (5i64 * (1i64 << task.retry_count.min(6))).min(24 * 60);
            let when = Utc::now() + Duration::minutes(minutes);
            self.tasks
                .requeue(task.id, when, task.last_error.as_deref())
                .await?;
            requeued += 1;
        }
        Ok(requeued)
    }
}
