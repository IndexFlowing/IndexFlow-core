use crate::application::{HealthService, SubmissionService};
use crate::config::AppConfig;
use crate::domain::{
    engine_is_submitted, engine_needs_submit, fair_site_plan, google_is_indexed,
    resolve_lifecycle_after_submit, ProviderKind, Site, TaskType, Url, UrlStatus,
};
use crate::infrastructure::{
    GoogleQuotaWindow, HealthCheckRepo, SiteRepo, SubmissionLogRepo, TaskRepo, UrlRepo,
};
use chrono::{DateTime, Duration, Utc};
use std::collections::HashMap;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Arc;
use std::time::Duration as StdDuration;
use tracing::{error, info, warn};

/// Worker: SUBMIT_GOOGLE — SEO quality gate then Google Indexing API submit.
/// Respects rolling 24-hour quota; pauses only itself when exhausted.
#[derive(Clone)]
pub struct GoogleSubmitWorker {
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

impl GoogleSubmitWorker {
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
                "google submit worker started (rolling 24h quota)"
            );
            loop {
                if let Err(e) = self.tick().await {
                    error!(error = %e, "google submit worker tick failed");
                }
                tokio::time::sleep(interval).await;
            }
        });
    }

    async fn tick(&self) -> anyhow::Result<()> {
        let pending_sites = self
            .tasks
            .pending_site_ids(TaskType::SubmitGoogle)
            .await?;
        if pending_sites.is_empty() {
            return Ok(());
        }

        let mut eligible = Vec::new();
        for site_id in pending_sites {
            match self.preflight_site(site_id).await {
                Ok(true) => eligible.push(site_id),
                Ok(false) => {}
                Err(e) => {
                    warn!(site_id, error = %e, "google site preflight failed");
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

        info!(sites = selected.len(), per_site, "google fair submit claim");

        for site_id in &selected {
            let claimed = self
                .tasks
                .claim_for_site(*site_id, TaskType::SubmitGoogle, per_site)
                .await?;
            if claimed.is_empty() {
                continue;
            }
            let task_ids: Vec<i64> = claimed.iter().map(|t| t.id).collect();
            if let Err(e) = self.process_site_batch(*site_id, claimed).await {
                error!(site_id, error = %e, "google submit batch failed");
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

    /// Returns true when the site has verified Google credentials and quota remaining.
    /// When quota is exhausted, sleeps all pending tasks for this site and returns false.
    async fn preflight_site(&self, site_id: i64) -> anyhow::Result<bool> {
        let (site, quota) = self.refresh_site_quota(site_id).await?;

        if !site.google_verified() {
            let failed = self
                .tasks
                .fail_pending_for_site(
                    site.id,
                    TaskType::SubmitGoogle,
                    "no Google credentials configured or verified",
                )
                .await?;
            warn!(
                site_id = site.id,
                failed, "google not configured; failing pending google tasks"
            );
            return Ok(false);
        }

        // Quota exhausted: sleep all pending tasks for this site until quota rolls off.
        if !site.google_ready() && site.google_verified() {
            let until = quota_sleep_until(&site, &quota);
            let slept = self
                .tasks
                .sleep_pending_for_site(
                    site.id,
                    TaskType::SubmitGoogle,
                    until,
                    "Google 24h quota exhausted",
                )
                .await?;
            if slept > 0 {
                info!(
                    site_id = site.id,
                    slept,
                    %until,
                    "google quota circuit open; slept tasks"
                );
            }
            return Ok(false);
        }

        Ok(true)
    }

    async fn refresh_site_quota(&self, site_id: i64) -> anyhow::Result<(Site, GoogleQuotaWindow)> {
        let site = self
            .sites
            .find_by_id(site_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("site {site_id} not found"))?;

        let quota = self
            .logs
            .google_quota_window(site.id, self.config.google_daily_quota)
            .await?;

        if quota.remaining > 0 && site.google_quota_paused() {
            let _ = self.sites.clear_google_quota_pause(site.id).await;
        }
        if quota.exhausted() && site.google_verified() {
            let until = quota
                .next_free_at
                .unwrap_or_else(|| Utc::now() + Duration::hours(24));
            let _ = self
                .sites
                .set_google_quota_paused_until(
                    site.id,
                    until,
                    Some("Google rolling 24h quota exhausted"),
                )
                .await;
        }

        // Re-read after potential pause-flag update.
        let site = self
            .sites
            .find_by_id(site_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("site {site_id} not found"))?;
        Ok((site, quota))
    }

    async fn process_site_batch(
        &self,
        site_id: i64,
        tasks: Vec<crate::domain::Task>,
    ) -> anyhow::Result<()> {
        let (site, quota) = self.refresh_site_quota(site_id).await?;
        let sleep_until = quota_sleep_until(&site, &quota);

        if !site.google_ready() {
            // Quota just ran out between preflight and claim — sleep the batch.
            for task in &tasks {
                self.tasks
                    .reschedule(task.id, sleep_until, Some("Google 24h quota exhausted"))
                    .await?;
            }
            info!(
                site_id = site.id,
                n = tasks.len(),
                %sleep_until,
                "google quota circuit open on claimed batch"
            );
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

        let mut google_slots = quota.remaining;

        // Allocate quota slots before the gate to avoid wasting HTTP fetches.
        let mut to_gate: Vec<(crate::domain::Task, Url)> = Vec::new();
        for task in tasks {
            let Some(url_id) = task.url_id else {
                self.tasks.mark_failed(task.id, "SUBMIT_GOOGLE missing url_id").await?;
                continue;
            };
            let Some(url) = url_map.remove(&url_id) else {
                self.tasks.mark_failed(task.id, &format!("url {url_id} not found")).await?;
                continue;
            };

            if google_is_indexed(&url.google_index_status) {
                // GSC Search Analytics already confirmed INDEXED — skip Indexing API quota.
                let overall = resolve_lifecycle_after_submit(
                    site.bing_ready(),
                    true,
                    &url.bing_status,
                    "SUBMITTED",
                );
                self.urls
                    .apply_submit_outcome(
                        url.id,
                        overall,
                        None,
                        None,
                        None,
                        Some("SUBMITTED"),
                        None,
                    )
                    .await?;
                self.tasks.mark_success(task.id).await?;
                info!(url_id = url.id, "google exempt (GSC INDEXED)");
                continue;
            }

            if !engine_needs_submit(&url.google_status) {
                // Already submitted on Google.
                self.finalize_already_done(&site, &task, &url).await?;
                continue;
            }

            if google_slots <= 0 {
                // Quota exhausted mid-batch; sleep the rest.
                self.tasks
                    .reschedule(task.id, sleep_until, Some("Google 24h quota exhausted"))
                    .await?;
                continue;
            }

            google_slots -= 1;
            to_gate.push((task, url));
        }

        // SEO gate.
        let mut passers: Vec<(crate::domain::Task, Url)> = Vec::new();
        for (task, url) in to_gate {
            match self.gate_one(&task, &url).await {
                Ok(true) => passers.push((task, url)),
                Ok(false) => {}
                Err(e) => {
                    error!(task_id = task.id, error = %e, "google quality gate error");
                    let _ = self.tasks.mark_failed(task.id, &e.to_string()).await;
                    if let Some(uid) = task.url_id {
                        let _ = self.urls.mark_blocked(uid, &e.to_string()).await;
                    }
                }
            }
        }

        if passers.is_empty() {
            return Ok(());
        }

        let mut google_quota_hit = false;

        for (task, url) in &passers {
            if google_quota_hit {
                self.tasks
                    .reschedule(task.id, sleep_until, Some("Google 24h quota exhausted"))
                    .await?;
                continue;
            }

            match self.submission.submit_url_google(&site, &url.url).await {
                Ok(result) => {
                    self.logs
                        .insert(
                            url.id,
                            ProviderKind::Google,
                            result.is_success,
                            result.status_code.map(|c| c as i32),
                            result.response_msg.as_deref(),
                        )
                        .await?;

                    if result.is_quota_exceeded {
                        // Record pause; sleep this and future tasks.
                        let refreshed = self
                            .logs
                            .google_quota_window(site.id, self.config.google_daily_quota)
                            .await?;
                        let until = refreshed
                            .next_free_at
                            .or(quota.next_free_at)
                            .unwrap_or_else(|| Utc::now() + Duration::hours(24));
                        self.sites
                            .set_google_quota_paused_until(
                                site.id,
                                until,
                                Some("Google rolling 24h quota exhausted"),
                            )
                            .await?;
                        warn!(site_id = site.id, %until, "google quota exhausted mid-batch");
                        google_quota_hit = true;
                        self.tasks
                            .reschedule(task.id, until, Some("Google 24h quota exhausted"))
                            .await?;
                        continue;
                    }

                    let (google_st, google_msg) = if result.is_success {
                        ("SUBMITTED", None)
                    } else {
                        ("FAILED", result.response_msg.as_deref())
                    };

                    let overall = resolve_lifecycle_after_submit(
                        site.bing_ready(),
                        true, // google enabled (this worker)
                        &url.bing_status,
                        google_st,
                    );

                    self.urls
                        .apply_submit_outcome(
                            url.id,
                            overall,
                            None,
                            None,
                            None,
                            Some(google_st),
                            google_msg,
                        )
                        .await?;

                    if result.is_success {
                        self.tasks.mark_success(task.id).await?;
                        info!(url_id = url.id, url = %url.url, "google submitted");
                    } else {
                        let msg = result
                            .response_msg
                            .clone()
                            .unwrap_or_else(|| "Google submit failed".into());
                        self.tasks.mark_failed(task.id, &msg).await?;
                    }
                }
                Err(e) => {
                    warn!(error = %e, url = %url.url, "google submit error");
                    self.logs
                        .insert(url.id, ProviderKind::Google, false, None, Some(e.to_string().as_str()))
                        .await?;
                    let _ = self
                        .urls
                        .apply_submit_outcome(
                            url.id,
                            UrlStatus::Pending,
                            None,
                            None,
                            None,
                            Some("FAILED"),
                            Some(e.to_string().as_str()),
                        )
                        .await;
                    self.tasks.mark_failed(task.id, &e.to_string()).await?;
                }
            }
        }

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
        if overall == UrlStatus::Submitted && !engine_is_submitted(&url.status) {
            let _ = self
                .urls
                .apply_submit_outcome(url.id, UrlStatus::Submitted, None, None, None, None, None)
                .await;
        }
        self.tasks.mark_success(task.id).await
    }
}

fn quota_sleep_until(site: &Site, quota: &GoogleQuotaWindow) -> DateTime<Utc> {
    site.google_quota_paused_until
        .or(quota.next_free_at)
        .unwrap_or_else(|| Utc::now() + Duration::hours(24))
}
