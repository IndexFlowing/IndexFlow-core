use crate::config::AppConfig;
use crate::domain::{
    ProviderCredentialStatus, Site, SiteStatus, SitemapType, TaskType, priority,
};
use crate::infrastructure::SiteUrlStats;
use crate::infrastructure::{
    GoogleQuotaWindow, SiteRepo, SitemapRepo, SubmissionLogRepo, TaskQueueCount, TaskRepo, UrlRepo,
};
use crate::providers::{bing::BingProvider, google::GoogleProvider, SearchProvider};
use chrono::Utc;
use serde::Serialize;

#[derive(Clone)]
pub struct SiteService {
    sites: SiteRepo,
    sitemaps: SitemapRepo,
    tasks: TaskRepo,
    urls: UrlRepo,
    submissions: SubmissionLogRepo,
    config: AppConfig,
    bing: BingProvider,
    google: GoogleProvider,
}

#[derive(Debug, Clone, Serialize)]
pub struct SiteActivity {
    pub running: bool,
    pub phase: String,
    pub label: String,
    pub sync_pending: i64,
    pub sync_processing: i64,
    pub submit_pending: i64,
    pub submit_processing: i64,
}

#[derive(Debug, Serialize)]
pub struct SiteDetail {
    pub site: Site,
    pub url_total: i64,
    pub pending: i64,
    pub submitted: i64,
    pub blocked: i64,
    pub bing_submitted_count: i64,
    pub bing_pending_count: i64,
    pub google_submitted_count: i64,
    pub google_pending_count: i64,
    pub activity: SiteActivity,
    pub google_quota_used: i64,
    pub google_quota_total: u32,
    pub google_quota_remaining: i64,
    pub google_quota_next_free_at: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Debug, Serialize)]
pub struct SiteWorkbenchSummary {
    pub site: Site,
    pub url_total: i64,
    pub pending: i64,
    pub submitted: i64,
    pub blocked: i64,
    pub bing_submitted_count: i64,
    pub bing_pending_count: i64,
    pub google_submitted_count: i64,
    pub google_pending_count: i64,
    pub activity: SiteActivity,
    pub google_quota_used: i64,
    pub google_quota_total: u32,
    pub google_quota_remaining: i64,
    pub google_quota_next_free_at: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Debug, Serialize)]
pub struct ConfigInfo {
    pub submit_worker_interval_secs: u64,
    pub scheduler_interval_secs: u64,
    pub worker_poll_interval_secs: u64,
    pub submit_worker_batch: i64,
    pub google_daily_quota: u32,
}

#[derive(Debug, Serialize)]
pub struct DashboardResponse {
    pub sites: Vec<SiteWorkbenchSummary>,
    pub config_info: ConfigInfo,
}

#[derive(Debug, Serialize)]
pub struct WorkflowResult {
    pub success: bool,
    pub tasks_created: u64,
    pub message: String,
}

#[derive(Debug, Serialize)]
pub struct ChannelTestResult {
    pub success: bool,
    pub provider: String,
    /// UNSET | SAVED | VERIFIED | FAILED after this test
    pub credential_status: String,
    pub message: String,
    pub status_code: Option<u16>,
}

impl SiteService {
    pub fn new(
        sites: SiteRepo,
        sitemaps: SitemapRepo,
        tasks: TaskRepo,
        urls: UrlRepo,
        submissions: SubmissionLogRepo,
        config: AppConfig,
        bing: BingProvider,
        google: GoogleProvider,
    ) -> Self {
        Self {
            sites,
            sitemaps,
            tasks,
            urls,
            submissions,
            config,
            bing,
            google,
        }
    }

    pub async fn list(&self) -> anyhow::Result<Vec<Site>> {
        self.sites.list_all().await
    }

    pub async fn get(&self, id: i64) -> anyhow::Result<Option<SiteDetail>> {
        let Some(site) = self.sites.find_by_id(id).await? else {
            return Ok(None);
        };
        let stats = self.urls.site_three_state(id, None, None).await?;
        let activity = build_activity(&self.tasks.site_queue_counts(id).await?);
        let quota = self
            .submissions
            .google_quota_window(id, self.config.google_daily_quota)
            .await?;
        Ok(Some(site_detail(site, &stats, activity, &quota)))
    }

    /// Update Bing / Google credentials for an existing site.
    /// Only fields with `set_* = true` are written (empty string clears).
    pub async fn update_credentials(
        &self,
        site_id: i64,
        set_indexnow_key: bool,
        indexnow_key: Option<&str>,
        set_google_json: bool,
        google_service_account_json: Option<&str>,
    ) -> anyhow::Result<Site> {
        if set_google_json {
            if let Some(sa) = google_service_account_json.map(str::trim).filter(|s| !s.is_empty()) {
                if serde_json::from_str::<serde_json::Value>(sa).is_err() {
                    anyhow::bail!("Invalid Google Service Account JSON");
                }
            }
        }

        self.sites
            .update_credentials(
                site_id,
                set_indexnow_key,
                indexnow_key,
                set_google_json,
                google_service_account_json,
            )
            .await?
            .ok_or_else(|| anyhow::anyhow!("site not found"))
    }

    pub async fn create(
        &self,
        domain: &str,
        sitemap_url: Option<&str>,
        indexnow_key: Option<&str>,
        google_service_account_json: Option<&str>,
    ) -> anyhow::Result<Site> {
        let domain = normalize_domain(domain);
        let site = self
            .sites
            .create(&domain, indexnow_key, google_service_account_json)
            .await?;

        if let Some(sm_url) = sitemap_url.filter(|s| !s.trim().is_empty()) {
            let sitemap = self
                .sitemaps
                .create(site.id, sm_url, SitemapType::UrlSet)
                .await?;

            self.tasks
                .create(
                    site.id,
                    None,
                    Some(sitemap.id),
                    TaskType::SyncSitemap,
                    priority::SYNC_SITEMAP,
                    Utc::now(),
                )
                .await?;

            self.sites
                .update_status(site.id, SiteStatus::Scanning)
                .await?;
        }

        self.sites
            .find_by_id(site.id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("site disappeared after create"))
    }

    pub async fn dashboard(&self) -> anyhow::Result<DashboardResponse> {
        let sites = self.sites.list_all().await?;
        let grouped = self.urls.stats_grouped_by_site().await?;
        let stats_map: std::collections::HashMap<i64, SiteUrlStats> =
            grouped.into_iter().map(|s| (s.site_id, s)).collect();
        let queues = self.tasks.all_sites_queue_counts().await?;
        let mut queue_by_site: std::collections::HashMap<i64, Vec<TaskQueueCount>> =
            std::collections::HashMap::new();
        for q in queues {
            queue_by_site.entry(q.site_id).or_default().push(TaskQueueCount {
                task_type: q.task_type,
                status: q.status,
                count: q.count,
            });
        }

        let quota_total = self.config.google_daily_quota;
        let quota_rows = self.submissions.google_quota_windows_by_site(quota_total).await?;
        let quota_map: std::collections::HashMap<i64, GoogleQuotaWindow> =
            quota_rows.into_iter().map(|q| (q.site_id, q)).collect();

        let summaries = sites
            .into_iter()
            .map(|site| {
                let stats = stats_map.get(&site.id);
                let activity = build_activity(queue_by_site.get(&site.id).map(Vec::as_slice).unwrap_or(&[]));
                let quota = quota_map.get(&site.id).cloned().unwrap_or_else(|| {
                    GoogleQuotaWindow::new(site.id, 0, quota_total, None)
                });
                SiteWorkbenchSummary {
                    site,
                    url_total: stats.map(|s| s.url_total).unwrap_or(0),
                    pending: stats.map(|s| s.pending).unwrap_or(0),
                    submitted: stats.map(|s| s.submitted).unwrap_or(0),
                    blocked: stats.map(|s| s.blocked).unwrap_or(0),
                    bing_submitted_count: stats.map(|s| s.bing_submitted_count).unwrap_or(0),
                    bing_pending_count: stats.map(|s| s.bing_pending_count).unwrap_or(0),
                    google_submitted_count: stats.map(|s| s.google_submitted_count).unwrap_or(0),
                    google_pending_count: stats.map(|s| s.google_pending_count).unwrap_or(0),
                    activity,
                    google_quota_used: quota.used,
                    google_quota_total: quota.total,
                    google_quota_remaining: quota.remaining,
                    google_quota_next_free_at: quota.next_free_at,
                }
            })
            .collect();

        Ok(DashboardResponse {
            sites: summaries,
            config_info: ConfigInfo {
                submit_worker_interval_secs: self.config.submit_worker_interval_secs,
                scheduler_interval_secs: self.config.scheduler_interval_secs,
                worker_poll_interval_secs: self.config.worker_poll_interval_secs,
                submit_worker_batch: self.config.submit_worker_batch,
                google_daily_quota: self.config.google_daily_quota,
            },
        })
    }

    /// Enqueue SUBMIT_BING + SUBMIT_GOOGLE tasks for URLs that still need each engine.
    /// Each engine gets its own independent task so the pipelines run concurrently.
    /// SubmitWorker (legacy SUBMIT_URL) is no longer used for new submissions.
    pub async fn start_submit(&self, site_id: i64) -> anyhow::Result<WorkflowResult> {
        let site = self
            .sites
            .find_by_id(site_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("site not found"))?;

        if !site.has_any_verified_provider() {
            if site.has_any_credentials_filled() {
                anyhow::bail!(
                    "Credentials are saved but not yet verified. Click Test Bing / Test Google on the site workbench and wait until the channel status is Verified before submitting."
                );
            }
            anyhow::bail!("Configure and verify an IndexNow key or Google Service Account first");
        }

        let has_bing = site.bing_ready();
        // Include Google even when quota-paused — the worker will wait for a slot.
        let has_google = site.google_verified();

        let mut total_created = 0u64;
        let mut parts: Vec<String> = Vec::new();

        // --- Bing channel ---
        if has_bing {
            let ids = self
                .urls
                .list_pending_submit_ids(site_id, true, false, 100_000)
                .await?;
            if !ids.is_empty() {
                let n = self
                    .tasks
                    .create_bing_tasks_batch(site_id, &ids, priority::SUBMIT_URL)
                    .await?;
                total_created += n;
                if n > 0 {
                    parts.push(format!("Bing: created {n} task(s)"));
                } else {
                    parts.push(format!("Bing: {} already queued", ids.len()));
                }
            }
        }

        // --- Google channel ---
        if has_google {
            let ids = self
                .urls
                .list_pending_submit_ids(site_id, false, true, 100_000)
                .await?;
            if !ids.is_empty() {
                let n = self
                    .tasks
                    .create_google_tasks_batch(site_id, &ids, priority::SUBMIT_URL)
                    .await?;
                total_created += n;
                if n > 0 {
                    parts.push(format!("Google: created {n} task(s)"));
                } else {
                    parts.push(format!("Google: {} already queued", ids.len()));
                }
            }
        }

        if parts.is_empty() {
            return Ok(WorkflowResult {
                success: true,
                tasks_created: 0,
                message: "No URLs pending submission for enabled search engines".into(),
            });
        }

        Ok(WorkflowResult {
            success: true,
            tasks_created: total_created,
            message: parts.join("; "),
        })
    }

    pub async fn test_bing(&self, site_id: i64) -> anyhow::Result<ChannelTestResult> {
        let site = self
            .sites
            .find_by_id(site_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("site not found"))?;

        let key = site
            .indexnow_key
            .as_deref()
            .filter(|k| !k.trim().is_empty())
            .ok_or_else(|| anyhow::anyhow!("Site has no IndexNow key configured"))?;

        let test_url = format!("https://{}/", site.domain);
        let result = match self
            .bing
            .submit_batch(&site.domain, key, &[test_url.clone()])
            .await
        {
            Ok(results) => match results.into_iter().next() {
                Some(res) => {
                    let success = res.is_success;
                    let message = if success {
                        "IndexNow channel test successful".into()
                    } else {
                        format!(
                            "IndexNow test failed: {}",
                            res.response_msg.unwrap_or_else(|| "unknown error".into())
                        )
                    };
                    (success, message, res.status_code)
                }
                None => (false, "No response from provider".into(), None),
            },
            Err(e) => (false, format!("IndexNow request failed: {e}"), None),
        };

        let (success, message, status_code) = result;
        let status = if success {
            ProviderCredentialStatus::Verified
        } else {
            ProviderCredentialStatus::Failed
        };
        let err = if success { None } else { Some(message.as_str()) };
        self.sites
            .set_provider_verify(site_id, "bing", status, err)
            .await?;

        Ok(ChannelTestResult {
            success,
            provider: "bing".into(),
            credential_status: status.as_str().into(),
            message,
            status_code,
        })
    }

    pub async fn test_google(&self, site_id: i64) -> anyhow::Result<ChannelTestResult> {
        let site = self
            .sites
            .find_by_id(site_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("site not found"))?;

        let sa = site
            .google_service_account_json
            .as_deref()
            .filter(|k| !k.trim().is_empty())
            .ok_or_else(|| anyhow::anyhow!("Site has no Google Service Account JSON configured"))?;

        // Validate JSON structure first
        if serde_json::from_str::<serde_json::Value>(sa).is_err() {
            let message = "Invalid Service Account JSON".to_string();
            self.sites
                .set_provider_verify(
                    site_id,
                    "google",
                    ProviderCredentialStatus::Failed,
                    Some(&message),
                )
                .await?;
            return Ok(ChannelTestResult {
                success: false,
                provider: "google".into(),
                credential_status: ProviderCredentialStatus::Failed.as_str().into(),
                message,
                status_code: None,
            });
        }

        // Full publish test: success only when API accepts the URL.
        // 403 ownership / 401 auth / invalid key → FAILED (filled but not usable).
        let test_url = format!("https://{}/", site.domain);
        let (success, message, status_code) = match self
            .google
            .submit_batch(&site.domain, sa, &[test_url])
            .await
        {
            Ok(results) => match results.into_iter().next() {
                Some(res) => {
                    let code = res.status_code;
                    let raw = res.response_msg.unwrap_or_default();
                    if res.is_success {
                        (
                            true,
                            "Google Indexing API test successful; channel is ready".into(),
                            code,
                        )
                    } else if code == Some(403)
                        || raw.contains("PERMISSION_DENIED")
                        || raw.to_lowercase().contains("ownership")
                    {
                        (
                            false,
                            format!(
                                "Insufficient permission: site ownership could not be verified. Verify the domain in Google Search Console and add the Service Account email as an owner. Details: {raw}"
                            ),
                            code,
                        )
                    } else if code == Some(401) {
                        (
                            false,
                            format!("Authentication failed (401). Check that the Service Account JSON is correct. Details: {raw}"),
                            code,
                        )
                    } else if code == Some(429) {
                        // Quota exceeded but credentials & ownership are OK enough to mark verified
                        (
                            true,
                            format!("Credentials are valid, but the rolling 24-hour quota is exhausted (429). Details: {raw}"),
                            code,
                        )
                    } else {
                        (
                            false,
                            format!("Google Indexing API test failed: {raw}"),
                            code,
                        )
                    }
                }
                None => (false, "No response from provider".into(), None),
            },
            Err(e) => (false, format!("Google credential test failed: {e}"), None),
        };

        let status = if success {
            ProviderCredentialStatus::Verified
        } else {
            ProviderCredentialStatus::Failed
        };
        let err = if success { None } else { Some(message.as_str()) };
        self.sites
            .set_provider_verify(site_id, "google", status, err)
            .await?;

        Ok(ChannelTestResult {
            success,
            provider: "google".into(),
            credential_status: status.as_str().into(),
            message,
            status_code,
        })
    }
}

fn site_detail(
    site: Site,
    stats: &SiteUrlStats,
    activity: SiteActivity,
    quota: &GoogleQuotaWindow,
) -> SiteDetail {
    SiteDetail {
        site,
        url_total: stats.url_total,
        pending: stats.pending,
        submitted: stats.submitted,
        blocked: stats.blocked,
        bing_submitted_count: stats.bing_submitted_count,
        bing_pending_count: stats.bing_pending_count,
        google_submitted_count: stats.google_submitted_count,
        google_pending_count: stats.google_pending_count,
        activity,
        google_quota_used: quota.used,
        google_quota_total: quota.total,
        google_quota_remaining: quota.remaining,
        google_quota_next_free_at: quota.next_free_at,
    }
}

fn build_activity(rows: &[TaskQueueCount]) -> SiteActivity {
    let mut sync_pending = 0i64;
    let mut sync_processing = 0i64;
    let mut submit_pending = 0i64;
    let mut submit_processing = 0i64;

    for r in rows {
        match (r.task_type.as_str(), r.status.as_str()) {
            ("SYNC_SITEMAP", "PENDING") => sync_pending += r.count,
            ("SYNC_SITEMAP", "PROCESSING") => sync_processing += r.count,
            (
                "SUBMIT_URL" | "SUBMIT_BING" | "SUBMIT_GOOGLE" | "RETRY_SUBMISSION",
                "PENDING",
            ) => submit_pending += r.count,
            (
                "SUBMIT_URL" | "SUBMIT_BING" | "SUBMIT_GOOGLE" | "RETRY_SUBMISSION",
                "PROCESSING",
            ) => submit_processing += r.count,
            _ => {}
        }
    }

    let syncing = sync_pending + sync_processing > 0;
    let submitting = submit_pending + submit_processing > 0;
    let running = syncing || submitting;

    let (phase, label) = match (syncing, submitting) {
        (true, true) => (
            "sync_and_submit".to_string(),
            format!(
                "Syncing sitemap and submitting URLs (Queue {} · Running {})",
                submit_pending, submit_processing
            ),
        ),
        (true, false) => (
            "syncing".to_string(),
            if sync_processing > 0 {
                "Syncing sitemap".to_string()
            } else {
                "Sitemap sync queued".to_string()
            },
        ),
        (false, true) => (
            "submitting".to_string(),
            format!(
                "Inspecting SEO & submitting URLs (Queue {} · Running {})",
                submit_pending, submit_processing
            ),
        ),
        (false, false) => ("idle".to_string(), "No active tasks running".to_string()),
    };

    SiteActivity {
        running,
        phase,
        label,
        sync_pending,
        sync_processing,
        submit_pending,
        submit_processing,
    }
}

fn normalize_domain(domain: &str) -> String {
    domain
        .trim()
        .trim_start_matches("https://")
        .trim_start_matches("http://")
        .trim_end_matches('/')
        .to_lowercase()
}
