use crate::application::{HealthService, SubmissionService};
use crate::config::AppConfig;
use crate::domain::{
    engine_needs_submit, google_is_indexed, resolve_lifecycle_after_submit, canonical_matches_page,
    HealthCheck, HreflangAlt, ProviderKind, QualityGateResult, SubmissionLog, Url, UrlStatus,
};
use crate::infrastructure::{
    HealthCheckRepo, LocaleCount, PathPrefixCount, SiteRepo, SiteUrlStats, SubmissionLogRepo,
    UrlDiagnostic, UrlRepo,
};
use crate::providers::SubmissionResult;
use serde::Serialize;

#[derive(Clone)]
pub struct UrlService {
    urls: UrlRepo,
    health: HealthCheckRepo,
    submissions: SubmissionLogRepo,
    sites: SiteRepo,
    health_svc: HealthService,
    submission_svc: SubmissionService,
    config: AppConfig,
}

#[derive(Debug, Serialize)]
pub struct UrlDetail {
    pub url: Url,
    pub recent_checks: Vec<HealthCheck>,
    pub recent_submissions: Vec<SubmissionLog>,
}

#[derive(Debug, Serialize)]
pub struct UrlSignals {
    pub title: Option<String>,
    pub title_chars: usize,
    pub meta_description: Option<String>,
    pub meta_description_chars: usize,
    pub h1: Option<String>,
    pub canonical_url: Option<String>,
    pub canonical_matches: Option<bool>,
    pub robots: Option<String>,
    pub hreflang: Vec<HreflangAlt>,
    pub http_status: Option<i32>,
    pub response_time_ms: Option<i32>,
    pub payload_bytes: Option<i32>,
}

#[derive(Debug, Serialize)]
pub struct UrlGscTrail {
    pub index_status: String,
    pub coverage_state: Option<String>,
    pub last_crawled_at: Option<chrono::DateTime<chrono::Utc>>,
    pub inspected_at: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Debug, Serialize)]
pub struct UrlAnalysis {
    pub url: Url,
    pub signals: UrlSignals,
    pub gsc: UrlGscTrail,
    pub recent_checks: Vec<HealthCheck>,
    pub recent_submissions: Vec<SubmissionLog>,
}

#[derive(Debug, Serialize)]
pub struct RecheckResult {
    pub url: Url,
    pub passed: bool,
    pub block_reason: Option<String>,
    pub gate: QualityGateView,
}

#[derive(Debug, Serialize)]
pub struct QualityGateView {
    pub http_status: Option<i32>,
    pub response_time_ms: Option<i32>,
    pub page_title: Option<String>,
    pub meta_description: Option<String>,
    pub h1_content: Option<String>,
    pub canonical_url: Option<String>,
    pub robots_directive: Option<String>,
    pub payload_bytes: Option<i32>,
    pub passed: bool,
    pub block_reason: Option<String>,
}

impl From<&QualityGateResult> for QualityGateView {
    fn from(g: &QualityGateResult) -> Self {
        Self {
            http_status: g.http_status,
            response_time_ms: g.response_time_ms,
            page_title: g.page_title.clone(),
            meta_description: g.meta_description.clone(),
            h1_content: g.h1_content.clone(),
            canonical_url: g.canonical_url.clone(),
            robots_directive: g.robots_directive.clone(),
            payload_bytes: g.payload_bytes,
            passed: g.passed,
            block_reason: g.block_reason.clone(),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct SubmitNowResult {
    pub url: Url,
    pub provider: String,
    pub success: bool,
    pub status_code: Option<u16>,
    pub response_body: Option<String>,
    pub message: String,
    pub quota_exempt: bool,
}

impl UrlService {
    pub fn new(
        urls: UrlRepo,
        health: HealthCheckRepo,
        submissions: SubmissionLogRepo,
        sites: SiteRepo,
        health_svc: HealthService,
        submission_svc: SubmissionService,
        config: AppConfig,
    ) -> Self {
        Self {
            urls,
            health,
            submissions,
            sites,
            health_svc,
            submission_svc,
            config,
        }
    }

    pub async fn list(
        &self,
        site_id: i64,
        status: Option<&str>,
        locale: Option<&str>,
        path_prefix: Option<&str>,
        page: i64,
        limit: i64,
    ) -> anyhow::Result<(Vec<Url>, i64)> {
        self.urls
            .list_by_site(site_id, status, locale, path_prefix, page, limit)
            .await
    }

    pub async fn list_diagnostics(
        &self,
        site_id: i64,
        status: Option<&str>,
        locale: Option<&str>,
        path_prefix: Option<&str>,
        page: i64,
        limit: i64,
        seo_checked: Option<bool>,
        google_index_status: Option<&str>,
    ) -> anyhow::Result<(Vec<UrlDiagnostic>, i64)> {
        self.urls
            .list_diagnostics(
                site_id,
                status,
                locale,
                path_prefix,
                page,
                limit,
                seo_checked,
                google_index_status,
            )
            .await
    }

    pub async fn stats(
        &self,
        site_id: i64,
        locale: Option<&str>,
        path_prefix: Option<&str>,
    ) -> anyhow::Result<SiteUrlStats> {
        self.urls.site_three_state(site_id, locale, path_prefix).await
    }

    pub async fn locales(
        &self,
        site_id: i64,
        path_prefix: Option<&str>,
    ) -> anyhow::Result<Vec<LocaleCount>> {
        self.urls.list_locales(site_id, path_prefix).await
    }

    pub async fn path_prefixes(
        &self,
        site_id: i64,
        locale: Option<&str>,
    ) -> anyhow::Result<Vec<PathPrefixCount>> {
        self.urls.list_path_prefixes(site_id, locale).await
    }

    pub async fn get_detail(&self, id: i64) -> anyhow::Result<Option<UrlDetail>> {
        let Some(url) = self.urls.find_by_id(id).await? else {
            return Ok(None);
        };
        let recent_checks = self.health.list_by_url(id, 20).await?;
        let recent_submissions = self.submissions.list_by_url(id, 20).await?;
        Ok(Some(UrlDetail {
            url,
            recent_checks,
            recent_submissions,
        }))
    }

    pub async fn analysis(&self, id: i64) -> anyhow::Result<Option<UrlAnalysis>> {
        let Some(url) = self.urls.find_by_id(id).await? else {
            return Ok(None);
        };
        let recent_checks = self.health.list_by_url(id, 20).await?;
        let recent_submissions = self.submissions.list_by_url(id, 20).await?;
        let latest = recent_checks.first();
        let hreflang = latest
            .and_then(|c| c.hreflang.as_deref())
            .and_then(|s| serde_json::from_str::<Vec<HreflangAlt>>(s).ok())
            .unwrap_or_default();
        let title = url.page_title.clone();
        let desc = url.meta_description.clone();
        let canonical_matches = url
            .canonical_url
            .as_deref()
            .map(|c| canonical_matches_page(&url.url, c));
        let signals = UrlSignals {
            title_chars: title.as_deref().map(char_len).unwrap_or(0),
            title,
            meta_description_chars: desc.as_deref().map(char_len).unwrap_or(0),
            meta_description: desc,
            h1: url.h1_content.clone(),
            canonical_url: url.canonical_url.clone(),
            canonical_matches,
            robots: latest.and_then(|c| c.robots_directive.clone()),
            hreflang,
            http_status: url.last_http_status,
            response_time_ms: latest.and_then(|c| c.response_time),
            payload_bytes: latest.and_then(|c| c.payload_bytes),
        };
        let gsc = UrlGscTrail {
            index_status: url.google_index_status.clone(),
            coverage_state: url.google_coverage_state.clone(),
            last_crawled_at: url.google_last_crawled_at,
            inspected_at: url.google_inspected_at,
        };
        Ok(Some(UrlAnalysis {
            url,
            signals,
            gsc,
            recent_checks,
            recent_submissions,
        }))
    }

    /// On-demand SEO quality gate. Unblocks the URL when it now passes.
    pub async fn recheck(&self, id: i64) -> anyhow::Result<Option<RecheckResult>> {
        let Some(url) = self.urls.find_by_id(id).await? else {
            return Ok(None);
        };
        let gate = self.health_svc.check_url(&url.url).await;
        self.health.insert_from_gate(url.id, &gate).await?;
        self.urls.persist_seo_scan(url.id, &gate).await?;
        let updated = self
            .urls
            .find_by_id(id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("url disappeared after recheck"))?;
        Ok(Some(RecheckResult {
            passed: gate.passed,
            block_reason: gate.block_reason.clone(),
            gate: QualityGateView::from(&gate),
            url: updated,
        }))
    }

    /// Bypass the task queue and submit one URL to Bing or Google immediately.
    pub async fn submit_now(
        &self,
        id: i64,
        provider: &str,
    ) -> anyhow::Result<Option<SubmitNowResult>> {
        let Some(url) = self.urls.find_by_id(id).await? else {
            return Ok(None);
        };
        let site = self
            .sites
            .find_by_id(url.site_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("site not found"))?;

        if url.status.eq_ignore_ascii_case("BLOCKED") {
            anyhow::bail!(
                "URL is BLOCKED ({}). Re-check SEO first.",
                url.block_reason.as_deref().unwrap_or("quality gate")
            );
        }

        let provider = provider.trim().to_ascii_lowercase();
        match provider.as_str() {
            "bing" => self.submit_now_bing(&site, url).await.map(Some),
            "google" => self.submit_now_google(&site, url).await.map(Some),
            other => anyhow::bail!("provider must be `bing` or `google`, got `{other}`"),
        }
    }

    async fn submit_now_bing(
        &self,
        site: &crate::domain::Site,
        url: Url,
    ) -> anyhow::Result<SubmitNowResult> {
        if !site.bing_ready() {
            anyhow::bail!("Bing IndexNow is not configured or not verified");
        }
        let key = site.indexnow_key.as_deref().unwrap_or("");
        let result = self
            .submission_svc
            .submit_url_batch_bing(&site.domain, key, &[url.url.clone()])
            .await?
            .into_iter()
            .next()
            .unwrap_or_else(|| SubmissionResult::failure(url.url.clone(), None, "empty response"));

        self.persist_submit_outcome(site, &url, "bing", &result)
            .await?;
        let updated = self.urls.find_by_id(url.id).await?.unwrap_or(url);
        Ok(SubmitNowResult {
            provider: "bing".into(),
            success: result.is_success,
            status_code: result.status_code,
            response_body: result.response_msg.clone(),
            message: if result.is_success {
                "Submitted to Bing IndexNow".into()
            } else {
                result
                    .response_msg
                    .clone()
                    .unwrap_or_else(|| "Bing submit failed".into())
            },
            quota_exempt: false,
            url: updated,
        })
    }

    async fn submit_now_google(
        &self,
        site: &crate::domain::Site,
        url: Url,
    ) -> anyhow::Result<SubmitNowResult> {
        if !site.google_verified() {
            anyhow::bail!("Google Indexing API is not configured or not verified");
        }

        // GSC exemption: already indexed — do not burn daily quota.
        if google_is_indexed(&url.google_index_status) {
            if engine_needs_submit(&url.google_status) {
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
            }
            let updated = self.urls.find_by_id(url.id).await?.unwrap_or(url);
            return Ok(SubmitNowResult {
                provider: "google".into(),
                success: true,
                status_code: None,
                response_body: Some(
                    "Exempt: GSC Search Analytics already shows impressions > 0 (INDEXED)".into(),
                ),
                message: "Already indexed in GSC — skipped Google Indexing API (quota saved)".into(),
                quota_exempt: true,
                url: updated,
            });
        }

        if !site.google_ready() {
            anyhow::bail!("Google 24-hour quota is exhausted; try again when a slot frees");
        }

        let quota = self
            .submissions
            .google_quota_window(site.id, self.config.google_daily_quota)
            .await?;
        if quota.remaining <= 0 {
            anyhow::bail!("Google 24-hour quota is exhausted");
        }

        let result = self
            .submission_svc
            .submit_url_google(site, &url.url)
            .await?;
        self.persist_submit_outcome(site, &url, "google", &result)
            .await?;
        let updated = self.urls.find_by_id(url.id).await?.unwrap_or(url);
        Ok(SubmitNowResult {
            provider: "google".into(),
            success: result.is_success,
            status_code: result.status_code,
            response_body: result.response_msg.clone(),
            message: if result.is_success {
                "Submitted to Google Indexing API".into()
            } else if result.is_quota_exceeded {
                "Google quota exceeded".into()
            } else {
                result
                    .response_msg
                    .clone()
                    .unwrap_or_else(|| "Google submit failed".into())
            },
            quota_exempt: false,
            url: updated,
        })
    }

    async fn persist_submit_outcome(
        &self,
        site: &crate::domain::Site,
        url: &Url,
        provider: &str,
        result: &SubmissionResult,
    ) -> anyhow::Result<()> {
        let kind = if provider == "bing" {
            ProviderKind::Bing
        } else {
            ProviderKind::Google
        };
        self.submissions
            .insert(
                url.id,
                kind,
                result.is_success,
                result.status_code.map(|c| c as i32),
                result.response_msg.as_deref(),
            )
            .await?;

        let st = if result.is_success {
            "SUBMITTED"
        } else {
            "FAILED"
        };
        let msg = if result.is_success {
            None
        } else {
            result.response_msg.as_deref()
        };

        let (bing_st, bing_err, google_st, google_err) = if provider == "bing" {
            (Some(st), msg, None, None)
        } else {
            (None, None, Some(st), msg)
        };

        let bing_now = bing_st.unwrap_or(url.bing_status.as_str());
        let google_now = google_st.unwrap_or(url.google_status.as_str());
        let overall = resolve_lifecycle_after_submit(
            site.bing_ready(),
            site.google_verified(),
            bing_now,
            google_now,
        );
        let overall = if overall == UrlStatus::Pending && !result.is_success && url.status == "BLOCKED"
        {
            UrlStatus::Blocked
        } else {
            overall
        };

        self.urls
            .apply_submit_outcome(
                url.id,
                overall,
                None,
                bing_st,
                bing_err,
                google_st,
                google_err,
            )
            .await?;
        Ok(())
    }
}

fn char_len(s: &str) -> usize {
    s.chars().count()
}
