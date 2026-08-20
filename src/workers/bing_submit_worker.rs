use crate::application::{HealthService, SubmissionService};
use crate::config::AppConfig;
use crate::domain::{
    engine_is_submitted, engine_needs_submit, fair_site_plan, resolve_lifecycle_after_submit,
    ProviderKind, Site, TaskType, Url, UrlStatus,
};
use crate::infrastructure::{HealthCheckRepo, SiteRepo, SubmissionLogRepo, TaskRepo, UrlRepo};
use std::collections::HashMap;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Arc;
use std::time::Duration as StdDuration;
use tracing::{error, info, warn};

/// Worker: SUBMIT_BING — SEO quality gate then IndexNow batch submit.
/// Runs at full speed with no quota circuit; only pauses on network failures.
#[derive(Clone)]
pub struct BingSubmitWorker {
    tasks: TaskRepo,
    urls: UrlRepo,
    sites: SiteRepo,
    logs: SubmissionLogRepo,
    health_repo: HealthCheckRepo,
    health: HealthService,
    submission: SubmissionService,
    config: AppConfig,
    last_site_cursor: Arc<AtomicI64>,
}

impl BingSubmitWorker {
    pub fn new(
        tasks: TaskRepo,
        urls: UrlRepo,
        sites: SiteRepo,
        logs: SubmissionLogRepo,
        health_repo: HealthCheckRepo,
        health: HealthService,
        submission: SubmissionService,
        config: AppConfig,
    ) -> Self {
        Self {
            tasks,
            urls,
            sites,
            logs,
            health_repo,
            health,
            submission,
            config,
            last_site_cursor: Arc::new(AtomicI64::new(0)),
        }
    }

    pub fn start(self: Arc<Self>) {
        let interval = StdDuration::from_secs(self.config.submit_worker_interval_secs);
        tokio::spawn(async move {
            info!(
                interval_secs = self.config.submit_worker_interval_secs,
                "bing submit worker started (no-quota, full-speed)"
            );
            loop {
                if let Err(e) = self.tick().await {
                    error!(error = %e, "bing submit worker tick failed");
                }
                tokio::time::sleep(interval).await;
            }
        });
    }

    async fn tick(&self) -> anyhow::Result<()> {
        let pending_sites = self
            .tasks
            .pending_site_ids(TaskType::SubmitBing)
            .await?;
        if pending_sites.is_empty() {
            return Ok(());
        }

        // Only process sites that have Bing credentials verified.
        let mut eligible = Vec::new();
        for site_id in pending_sites {
            match self.preflight_site(site_id).await {
                Ok(true) => eligible.push(site_id),
                Ok(false) => {}
                Err(e) => {
                    warn!(site_id, error = %e, "bing site preflight failed");
                }
            }
        }
        if eligible.is_empty() {
            return Ok(());
        }

        let after = self.last_site_cursor.load(Ordering::Relaxed);
        let (selected, per_site) =
            fair_site_plan(&eligible, self.config.submit_worker_batch, after);
        if selected.is_empty() {
            return Ok(());
        }

        info!(sites = selected.len(), per_site, "bing fair submit claim");

        for site_id in &selected {
            let claimed = self
                .tasks
                .claim_for_site(*site_id, TaskType::SubmitBing, per_site)
                .await?;
            if claimed.is_empty() {
                continue;
            }
            let task_ids: Vec<i64> = claimed.iter().map(|t| t.id).collect();
            if let Err(e) = self.process_site_batch(*site_id, claimed).await {
                error!(site_id, error = %e, "bing submit batch failed");
                for tid in task_ids {
                    let _ = self
                        .tasks
                        .mark_failed(tid, &format!("batch aborted: {e}"))
                        .await;
                }
            }
        }

        if let Some(&last) = selected.last() {
            self.last_site_cursor.store(last, Ordering::Relaxed);
        }
        Ok(())
    }

    /// Returns true when the site has a verified Bing credential.
    async fn preflight_site(&self, site_id: i64) -> anyhow::Result<bool> {
        let site = self
            .sites
            .find_by_id(site_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("site {site_id} not found"))?;

        if !site.bing_ready() {
            // Fail all pending tasks for this site — no credential.
            let failed = self
                .tasks
                .fail_pending_for_site(
                    site.id,
                    TaskType::SubmitBing,
                    "no Bing (IndexNow) credentials configured or verified",
                )
                .await?;
            warn!(
                site_id = site.id,
                failed, "bing not ready; failing pending bing tasks"
            );
            return Ok(false);
        }
        Ok(true)
    }

    async fn process_site_batch(
        &self,
        site_id: i64,
        tasks: Vec<crate::domain::Task>,
    ) -> anyhow::Result<()> {
        let site = self
            .sites
            .find_by_id(site_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("site {site_id} not found"))?;

        if !site.bing_ready() {
            for task in &tasks {
                self.tasks
                    .mark_failed(task.id, "bing not ready on claimed batch")
                    .await?;
            }
            return Ok(());
        }

        let url_ids: Vec<i64> = tasks.iter().filter_map(|t| t.url_id).collect();
        let mut url_map: HashMap<i64, Url> = self
            .urls
            .find_by_ids(&url_ids)
            .await?
            .into_iter()
            .map(|u| (u.id, u))
            .collect();

        // 1. Keep only tasks that still need a Bing submit.
        let mut to_gate: Vec<(crate::domain::Task, Url)> = Vec::new();
        for task in tasks {
            let Some(url_id) = task.url_id else {
                self.tasks.mark_failed(task.id, "SUBMIT_BING missing url_id").await?;
                continue;
            };
            let Some(url) = url_map.remove(&url_id) else {
                self.tasks.mark_failed(task.id, &format!("url {url_id} not found")).await?;
                continue;
            };

            if !engine_needs_submit(&url.bing_status) {
                self.finalize_already_done(&site, &task, &url).await?;
                continue;
            }

            to_gate.push((task, url));
        }

        if to_gate.is_empty() {
            return Ok(());
        }

        // 2. Run SEO quality-gate checks concurrently.
        let mut gate_futures = Vec::new();
        for (task, url) in to_gate {
            let worker = self.clone();
            gate_futures.push(tokio::spawn(async move {
                let passed = worker.gate_one(&task, &url).await;
                (task, url, passed)
            }));
        }

        let mut passers: Vec<(crate::domain::Task, Url)> = Vec::new();
        for fut in gate_futures {
            if let Ok((task, url, Ok(true))) = fut.await {
                passers.push((task, url));
            }
        }

        if passers.is_empty() {
            return Ok(());
        }

        // 3. Batch-submit to Bing IndexNow.
        let key = site.indexnow_key.as_deref().unwrap_or("");
        let page_urls: Vec<String> = passers.iter().map(|(_, u)| u.url.clone()).collect();

        let results = match self
            .submission
            .submit_url_batch_bing(&site.domain, key, &page_urls)
            .await
        {
            Ok(r) => r,
            Err(e) => {
                warn!(error = %e, "bing batch http failed; recording failures");
                for (task, url) in &passers {
                    let _ = self
                        .logs
                        .insert(url.id, ProviderKind::Bing, false, None, Some(e.to_string().as_str()))
                        .await;
                    let _ = self
                        .urls
                        .apply_submit_outcome(
                            url.id,
                            UrlStatus::Pending,
                            None,
                            Some("FAILED"),
                            Some(e.to_string().as_str()),
                            None,
                            None,
                        )
                        .await;
                    let _ = self.tasks.mark_failed(task.id, &e.to_string()).await;
                }
                return Ok(());
            }
        };

        // 4. Persist batch results.
        for ((task, url), result) in passers.iter().zip(results.iter()) {
            let _ = self
                .logs
                .insert(
                    url.id,
                    ProviderKind::Bing,
                    result.is_success,
                    result.status_code.map(|c| c as i32),
                    result.response_msg.as_deref(),
                )
                .await;

            let (bing_st, bing_msg) = if result.is_success {
                ("SUBMITTED", None)
            } else {
                ("FAILED", result.response_msg.as_deref())
            };

            let overall = resolve_lifecycle_after_submit(
                true,
                site.google_verified(),
                bing_st,
                &url.google_status,
            );

            let _ = self
                .urls
                .apply_submit_outcome(
                    url.id,
                    overall,
                    None,
                    Some(bing_st),
                    bing_msg,
                    None,
                    None,
                )
                .await;

            if result.is_success {
                self.tasks.mark_success(task.id).await?;
            } else {
                let msg = result
                    .response_msg
                    .clone()
                    .unwrap_or_else(|| "IndexNow failed".into());
                self.tasks.mark_failed(task.id, &msg).await?;
            }
        }

        info!(site_id, count = passers.len(), "bing batch submit completed");
        Ok(())
    }

    /// SEO gate for one URL. Returns true on pass, false on block.
    async fn gate_one(&self, task: &crate::domain::Task, url: &Url) -> anyhow::Result<bool> {
        let result = self.health.check_url(&url.url).await;

        self.health_repo
            .insert_from_gate(url.id, &result)
            .await?;

        self.urls.persist_seo_scan(url.id, &result).await?;

        if !result.passed {
            let reason = result
                .block_reason
                .unwrap_or_else(|| "quality gate failed".into());
            info!(url_id = url.id, url = %url.url, %reason, "blocked by quality gate");
            self.tasks.mark_success(task.id).await?;
            return Ok(false);
        }

        Ok(true)
    }

    async fn finalize_already_done(
        &self,
        site: &Site,
        task: &crate::domain::Task,
        url: &Url,
    ) -> anyhow::Result<()> {
        let overall = resolve_lifecycle_after_submit(
            site.bing_ready(),
            site.google_verified(),
            &url.bing_status,
            &url.google_status,
        );
        if overall == UrlStatus::Submitted
            && !engine_is_submitted(&url.status)
        {
            let _ = self
                .urls
                .apply_submit_outcome(url.id, UrlStatus::Submitted, None, None, None, None, None)
                .await;
        }
        self.tasks.mark_success(task.id).await
    }
}
