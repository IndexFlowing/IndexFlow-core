use crate::application::{HealthService, SubmissionService};
use crate::config::AppConfig;
use crate::domain::{
    all_enabled_engines_failed, decide_site_push, decide_url_push, engine_is_submitted,
    engine_needs_submit, fair_site_plan, resolve_lifecycle_after_submit, ProviderKind, Site,
    SitePushability, Task, TaskType, Url, UrlPushDecision, UrlStatus,
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

/// Worker: SUBMIT_URL — inline SEO quality gate, then search-engine submit.
#[derive(Clone)]
pub struct SubmitWorker {
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

impl SubmitWorker {
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
                "submit worker started (fair multi-site + quota circuit)"
            );
            loop {
                if let Err(e) = self.tick().await {
                    error!(error = %e, "submit worker tick failed");
                }
                tokio::time::sleep(interval).await;
            }
        });
    }

    async fn tick(&self) -> anyhow::Result<()> {
        let pending_sites = self.tasks.pending_site_ids(TaskType::SubmitUrl).await?;
        if pending_sites.is_empty() {
            return Ok(());
        }

        let mut eligible = Vec::new();
        for site_id in pending_sites {
            match self.preflight_site(site_id).await {
                Ok(true) => eligible.push(site_id),
                Ok(false) => {}
                Err(e) => {
                    warn!(site_id, error = %e, "site preflight failed");
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

        info!(
            sites = selected.len(),
            per_site,
            "fair submit claim"
        );

        for site_id in &selected {
            let claimed = self
                .tasks
                .claim_for_site(*site_id, TaskType::SubmitUrl, per_site)
                .await?;
            if claimed.is_empty() {
                continue;
            }
            let task_ids: Vec<i64> = claimed.iter().map(|t| t.id).collect();
            if let Err(e) = self.process_site_batch(*site_id, claimed).await {
                error!(site_id, error = %e, "submit batch failed, marking claimed tasks as failed");
                // Keep claimed tasks from staying PROCESSING if process_site_batch returns Err.
                for tid in task_ids {
                    let _ = self
                        .tasks
                        .mark_failed(tid, &format!("batch execution aborted: {e}"))
                        .await;
                }
            }
        }

        if let Some(&last) = selected.last() {
            self.last_site_cursor.store(last, Ordering::Relaxed);
        }
        Ok(())
    }

    /// Site-level quota circuit: decide whether this site may be claimed.
    /// Returns `true` when the site should receive a fair share of this tick.
    /// Never issues HTTP. Quota-dead sites have their pending tasks slept until
    /// the next free slot; sites with no provider fail pending work in place.
    async fn preflight_site(&self, site_id: i64) -> anyhow::Result<bool> {
        let (site, quota) = self.refresh_site_quota(site_id).await?;

        // When Google is 24h-locked, immediately sleep every task that cannot use Bing.
        // This covers both "Bing already done" (whole site) and mixed queues (Google-only
        // URLs) so those rows never enter the quality-gate HTTP path.
        if site.google_verified() && !site.google_ready() {
            let until = quota_sleep_until(&site, &quota);
            let slept = self
                .tasks
                .sleep_pending_without_available_engine(
                    site.id,
                    TaskType::SubmitUrl,
                    until,
                    "Google 24h quota exhausted for this site",
                    site.bing_ready(),
                )
                .await?;
            if slept > 0 {
                info!(
                    site_id = site.id,
                    slept,
                    %until,
                    "quota circuit open; slept tasks with no available engine, skip quality-gate HTTP"
                );
            }
        }

        let has_bing_work = if site.bing_ready() {
            self.tasks
                .has_claimable_engine_work(site.id, ProviderKind::Bing)
                .await?
        } else {
            false
        };

        match decide_site_push(
            site.bing_ready(),
            site.google_ready(),
            site.google_verified(),
            has_bing_work,
        ) {
            SitePushability::Ready => Ok(true),
            SitePushability::SleepQuota => {
                let until = quota_sleep_until(&site, &quota);
                // Catch rows the JOIN update missed (e.g. SUBMIT_URL without url_id).
                let _ = self
                    .tasks
                    .sleep_pending_for_site(
                        site.id,
                        TaskType::SubmitUrl,
                        until,
                        "Google 24h quota exhausted and no Bing work remaining",
                    )
                    .await?;
                info!(
                    site_id = site.id,
                    "quota circuit open; skipping site, no quality-gate HTTP"
                );
                Ok(false)
            }
            SitePushability::FailNoProvider => {
                let failed = self
                    .tasks
                    .fail_pending_for_site(
                        site.id,
                        TaskType::SubmitUrl,
                        "no provider credentials configured or ready",
                    )
                    .await?;
                warn!(
                    site_id = site.id,
                    failed,
                    "no usable provider; failing pending submit tasks"
                );
                Ok(false)
            }
        }
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

        // Rolling 24h window: free slots may have expired even if a pause flag remains.
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
                    Some("Google rolling 24h quota exhausted for this site"),
                )
                .await;
        }

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
        tasks: Vec<Task>,
    ) -> anyhow::Result<ProcessOutcome> {
        let (site, quota) = self.refresh_site_quota(site_id).await?;
        let sleep_until = quota_sleep_until(&site, &quota);

        let url_ids: Vec<i64> = tasks.iter().filter_map(|t| t.url_id).collect();
        let mut url_map: HashMap<i64, Url> = self
            .urls
            .find_by_ids(&url_ids)
            .await?
            .into_iter()
            .map(|u| (u.id, u))
            .collect();

        // Safety net: if the site became un-pushable between preflight and claim, sleep
        // the claimed batch without any page fetch.
        let has_bing_work = site.bing_ready()
            && url_map
                .values()
                .any(|u| engine_needs_submit(&u.bing_status));
        match decide_site_push(
            site.bing_ready(),
            site.google_ready(),
            site.google_verified(),
            has_bing_work,
        ) {
            SitePushability::SleepQuota => {
                for task in &tasks {
                    self.tasks
                        .reschedule(
                            task.id,
                            sleep_until,
                            Some("Google 24h quota exhausted and no Bing work remaining"),
                        )
                        .await?;
                }
                info!(
                    site_id = site.id,
                    n = tasks.len(),
                    %sleep_until,
                    "quota circuit open on claimed batch; skip quality-gate HTTP"
                );
                return Ok(ProcessOutcome::StopGoogleQuota);
            }
            SitePushability::FailNoProvider => {
                for task in &tasks {
                    self.tasks
                        .mark_failed(task.id, "no provider credentials configured or ready")
                        .await?;
                }
                return Ok(ProcessOutcome::Continue);
            }
            SitePushability::Ready => {}
        }

        let mut to_gate: Vec<(Task, Url)> = Vec::new();
        let mut google_slots = if site.google_ready() {
            quota.remaining
        } else {
            0
        };

        for task in tasks {
            let Some(url_id) = task.url_id else {
                self.tasks
                    .mark_failed(task.id, "SUBMIT_URL missing url_id")
                    .await?;
                continue;
            };
            let Some(url) = url_map.remove(&url_id) else {
                self.tasks
                    .mark_failed(task.id, &format!("url {url_id} not found"))
                    .await?;
                continue;
            };

            let bing_needed = site.bing_ready() && engine_needs_submit(&url.bing_status);
            let google_needed = site.google_ready() && engine_needs_submit(&url.google_status);
            let decision = decide_url_push(
                site.bing_ready(),
                site.google_ready(),
                site.google_verified(),
                &url.bing_status,
                &url.google_status,
            );

            match decision {
                UrlPushDecision::FetchAndSubmit => {
                    let google_only = google_needed && !bing_needed;
                    if google_only && google_slots <= 0 {
                        self.tasks
                            .reschedule(
                                task.id,
                                sleep_until,
                                Some("Google 24h quota exhausted for this site"),
                            )
                            .await?;
                        continue;
                    }
                    if google_only {
                        google_slots -= 1;
                    }
                    to_gate.push((task, url));
                }
                UrlPushDecision::SleepUntilQuota => {
                    self.tasks
                        .reschedule(
                            task.id,
                            sleep_until,
                            Some("Google 24h quota exhausted for this site"),
                        )
                        .await?;
                }
                UrlPushDecision::AlreadyDone => {
                    let overall = resolve_lifecycle_after_submit(
                        site.bing_ready(),
                        site.google_verified(),
                        &url.bing_status,
                        &url.google_status,
                    );
                    if overall == UrlStatus::Submitted && url.status != UrlStatus::Submitted.as_str()
                    {
                        let _ = self
                            .urls
                            .apply_submit_outcome(
                                url.id,
                                UrlStatus::Submitted,
                                None,
                                None,
                                None,
                                None,
                                None,
                            )
                            .await;
                    }
                    self.tasks.mark_success(task.id).await?;
                }
            }
        }

        let mut passers: Vec<(Task, Url)> = Vec::new();

        for (task, url) in to_gate {
            match self.gate_one(&task, url).await {
                Ok(GateOutcome::Blocked) => {}
                Ok(GateOutcome::Passed(url)) => passers.push((task, url)),
                Err(e) => {
                    error!(task_id = task.id, error = %e, "quality gate failed");
                    let _ = self.tasks.mark_failed(task.id, &e.to_string()).await;
                    if let Some(url_id) = task.url_id {
                        let _ = self.urls.mark_blocked(url_id, &e.to_string()).await;
                    }
                }
            }
        }

        if passers.is_empty() {
            return Ok(ProcessOutcome::Continue);
        }

        // Only call Bing for URLs that are not already SUBMITTED on that engine.
        let mut bing_ok: HashMap<i64, bool> = HashMap::new();
        let mut bing_err: HashMap<i64, String> = HashMap::new();
        if site.bing_ready() {
            let bing_targets: Vec<&Url> = passers
                .iter()
                .map(|(_, u)| u)
                .filter(|u| engine_needs_submit(&u.bing_status))
                .collect();
            if !bing_targets.is_empty() {
                let key = site.indexnow_key.as_deref().unwrap_or("");
                let page_urls: Vec<String> =
                    bing_targets.iter().map(|u| u.url.clone()).collect();
                match self
                    .submission
                    .submit_url_batch_bing(&site.domain, key, &page_urls)
                    .await
                {
                    Ok(results) => {
                        for (url, result) in bing_targets.iter().zip(results.into_iter()) {
                            self.logs
                                .insert(
                                    url.id,
                                    ProviderKind::Bing,
                                    result.is_success,
                                    result.status_code.map(|c| c as i32),
                                    result.response_msg.as_deref(),
                                )
                                .await?;
                            bing_ok.insert(url.id, result.is_success);
                            if !result.is_success {
                                bing_err.insert(
                                    url.id,
                                    result
                                        .response_msg
                                        .unwrap_or_else(|| "IndexNow failed".into()),
                                );
                            }
                        }
                    }
                    Err(e) => {
                        warn!(error = %e, "bing batch submit failed");
                        for url in &bing_targets {
                            self.logs
                                .insert(
                                    url.id,
                                    ProviderKind::Bing,
                                    false,
                                    None,
                                    Some(e.to_string().as_str()),
                                )
                                .await?;
                            bing_ok.insert(url.id, false);
                            bing_err.insert(url.id, e.to_string());
                        }
                    }
                }
            }
        }

        let mut google_quota_hit = false;
        let mut google_ok: HashMap<i64, bool> = HashMap::new();
        let mut google_err: HashMap<i64, String> = HashMap::new();
        let mut google_left = quota.remaining;

        if site.google_ready() {
            for (_, url) in &passers {
                if engine_is_submitted(&url.google_status) {
                    continue;
                }
                if google_left <= 0 {
                    google_quota_hit = true;
                    break;
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
                            // Quota is not a per-URL failure — leave google_status untouched and requeue.
                            google_quota_hit = true;
                            break;
                        }
                        google_ok.insert(url.id, result.is_success);
                        if result.is_success {
                            google_left -= 1;
                        } else {
                            google_err.insert(
                                url.id,
                                result
                                    .response_msg
                                    .unwrap_or_else(|| "Google submit failed".into()),
                            );
                        }
                    }
                    Err(e) => {
                        warn!(error = %e, url = %url.url, "google submit failed");
                        self.logs
                            .insert(
                                url.id,
                                ProviderKind::Google,
                                false,
                                None,
                                Some(e.to_string().as_str()),
                            )
                            .await?;
                        google_ok.insert(url.id, false);
                        google_err.insert(url.id, e.to_string());
                    }
                }
            }
        } else if site.google_verified() && site.google_quota_paused() {
            google_quota_hit = true;
        }

        let mut sleep_until = sleep_until;
        if google_quota_hit {
            let refreshed = self
                .logs
                .google_quota_window(site.id, self.config.google_daily_quota)
                .await?;
            let until = refreshed
                .next_free_at
                .or(quota.next_free_at)
                .unwrap_or_else(|| Utc::now() + Duration::hours(24));
            let msg = "Google rolling 24h quota exhausted for this site";
            self.sites
                .set_google_quota_paused_until(site.id, until, Some(msg))
                .await?;
            sleep_until = until;
            warn!(site_id = site.id, %until, "paused Google submits until a 24h slot rolls off");
        }

        let bing_enabled = site.bing_ready();
        let google_enabled = site.google_verified();

        for (task, url) in passers {
            let bing_already = engine_is_submitted(&url.bing_status);
            let google_already = engine_is_submitted(&url.google_status);
            let bing_tried = bing_ok.contains_key(&url.id);
            let google_tried = google_ok.contains_key(&url.id);
            let bing_success = bing_already || bing_ok.get(&url.id).copied().unwrap_or(false);
            let google_success = google_already || google_ok.get(&url.id).copied().unwrap_or(false);

            let effective_bing = if bing_success {
                "SUBMITTED"
            } else if bing_tried {
                "FAILED"
            } else {
                url.bing_status.as_str()
            };
            let effective_google = if google_success {
                "SUBMITTED"
            } else if google_tried {
                "FAILED"
            } else {
                url.google_status.as_str()
            };

            let (bing_st, bing_msg) = if bing_tried && bing_ok.get(&url.id) == Some(&true) {
                (Some("SUBMITTED"), None)
            } else if bing_tried {
                (Some("FAILED"), bing_err.get(&url.id).map(String::as_str))
            } else {
                (None, None)
            };
            let (google_st, google_msg) = if google_tried && google_ok.get(&url.id) == Some(&true)
            {
                (Some("SUBMITTED"), None)
            } else if google_tried {
                (Some("FAILED"), google_err.get(&url.id).map(String::as_str))
            } else {
                (None, None)
            };

            let google_still_needed = google_enabled && !google_success;
            let waiting_quota = google_still_needed
                && (google_quota_hit || !site.google_ready())
                && !google_tried;

            let overall = if all_enabled_engines_failed(
                bing_enabled,
                google_enabled,
                effective_bing,
                effective_google,
            ) && !waiting_quota
            {
                UrlStatus::Blocked
            } else {
                resolve_lifecycle_after_submit(
                    bing_enabled,
                    google_enabled,
                    effective_bing,
                    effective_google,
                )
            };

            if overall == UrlStatus::Submitted {
                if site.google_quota_paused_until.is_some() && !site.google_quota_paused() {
                    let _ = self.sites.clear_google_quota_pause(site.id).await;
                }
                self.urls
                    .apply_submit_outcome(
                        url.id,
                        UrlStatus::Submitted,
                        None,
                        bing_st,
                        bing_msg,
                        google_st,
                        google_msg,
                    )
                    .await?;
                self.tasks.mark_success(task.id).await?;
                info!(url_id = url.id, url = %url.url, "url submitted to all enabled engines");
            } else if waiting_quota {
                self.urls
                    .apply_submit_outcome(
                        url.id,
                        UrlStatus::Pending,
                        None,
                        bing_st,
                        bing_msg,
                        None,
                        None,
                    )
                    .await?;
                self.tasks
                    .reschedule(
                        task.id,
                        sleep_until,
                        Some("Google 24h quota exhausted for this site"),
                    )
                    .await?;
            } else if overall == UrlStatus::Blocked {
                let reason = google_err
                    .get(&url.id)
                    .cloned()
                    .or_else(|| bing_err.get(&url.id).cloned())
                    .unwrap_or_else(|| "all providers failed".into());
                self.urls
                    .apply_submit_outcome(
                        url.id,
                        UrlStatus::Blocked,
                        Some(&reason),
                        bing_st,
                        bing_msg,
                        google_st,
                        google_msg,
                    )
                    .await?;
                self.tasks.mark_failed(task.id, &reason).await?;
            } else {
                // Partial: at least one enabled engine still NONE/FAILED, but not all-failed.
                self.urls
                    .apply_submit_outcome(
                        url.id,
                        UrlStatus::Pending,
                        None,
                        bing_st,
                        bing_msg,
                        google_st,
                        google_msg,
                    )
                    .await?;
                self.tasks.mark_success(task.id).await?;
                info!(
                    url_id = url.id,
                    url = %url.url,
                    bing = effective_bing,
                    google = effective_google,
                    "url partially submitted; remaining engines left pending"
                );
            }
        }

        if google_quota_hit {
            Ok(ProcessOutcome::StopGoogleQuota)
        } else {
            Ok(ProcessOutcome::Continue)
        }
    }

    async fn gate_one(&self, task: &Task, url_row: Url) -> anyhow::Result<GateOutcome> {
        let url_id = url_row.id;
        let result = self.health.check_url(&url_row.url).await;

        self.health_repo
            .insert(
                url_id,
                result.http_status,
                result.response_time_ms,
                result.has_noindex,
                result.has_canonical,
            )
            .await?;

        self.urls
            .apply_gate_result(
                url_id,
                result.http_status,
                result.page_title.as_deref(),
                result.canonical_url.as_deref(),
            )
            .await?;

        if !result.passed {
            let reason = result
                .block_reason
                .unwrap_or_else(|| "quality gate failed".into());
            info!(url_id, url = %url_row.url, %reason, "blocked by quality gate — not submitting");
            self.urls.mark_blocked(url_id, &reason).await?;
            self.tasks.mark_success(task.id).await?;
            return Ok(GateOutcome::Blocked);
        }

        Ok(GateOutcome::Passed(url_row))
    }
}

enum GateOutcome {
    Blocked,
    Passed(Url),
}

enum ProcessOutcome {
    Continue,
    StopGoogleQuota,
}

fn quota_sleep_until(site: &Site, quota: &GoogleQuotaWindow) -> DateTime<Utc> {
    site.google_quota_paused_until
        .or(quota.next_free_at)
        .unwrap_or_else(|| Utc::now() + Duration::hours(24))
}
