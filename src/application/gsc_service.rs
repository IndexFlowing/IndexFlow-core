use crate::config::AppConfig;
use crate::domain::{coverage_to_index_status, priority, GINDEX_UNKNOWN, TaskType};
use crate::infrastructure::{IndexFunnelStats, SiteRepo, TaskRepo, UrlRepo};
use crate::providers::google::GoogleProvider;
use chrono::{Duration, Utc};
use serde::Serialize;

#[derive(Clone)]
pub struct GscService {
    google: GoogleProvider,
    sites: SiteRepo,
    urls: UrlRepo,
    tasks: TaskRepo,
    config: AppConfig,
}

#[derive(Debug, Serialize)]
pub struct GscSyncResult {
    pub success: bool,
    pub property_url: String,
    pub pages_from_gsc: u64,
    pub urls_marked_indexed: u64,
    pub message: String,
}

#[derive(Debug, Serialize)]
pub struct GscInspectEnqueueResult {
    pub success: bool,
    pub tasks_created: u64,
    pub quota_used_24h: i64,
    pub quota_remaining: i64,
    pub message: String,
}

#[derive(Debug, Serialize)]
pub struct IndexMonitorStats {
    pub funnel: IndexFunnelStats,
    pub gsc_inspect_quota_total: u32,
    pub gsc_inspect_used_24h: i64,
    pub gsc_inspect_remaining: i64,
    pub gsc_inspect_pending: i64,
    pub gsc_property_url: Option<String>,
    pub gsc_analytics_synced_at: Option<chrono::DateTime<Utc>>,
}

impl GscService {
    pub fn new(
        google: GoogleProvider,
        sites: SiteRepo,
        urls: UrlRepo,
        tasks: TaskRepo,
        config: AppConfig,
    ) -> Self {
        Self {
            google,
            sites,
            urls,
            tasks,
            config,
        }
    }

    async fn require_google_site(&self, site_id: i64) -> anyhow::Result<crate::domain::Site> {
        let site = self
            .sites
            .find_by_id(site_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("site not found"))?;
        if !site.has_google_credentials() {
            anyhow::bail!("Configure a Google Service Account JSON first");
        }
        Ok(site)
    }

    async fn resolve_property(
        &self,
        site: &crate::domain::Site,
    ) -> anyhow::Result<String> {
        if let Some(cached) = site
            .gsc_property_url
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            return Ok(cached.to_string());
        }
        let sa = site
            .google_service_account_json
            .as_deref()
            .unwrap_or("");
        let property = self.google.resolve_gsc_property(sa, &site.domain).await?;
        self.sites.set_gsc_property(site.id, &property).await?;
        Ok(property)
    }

    /// Layer 1: bulk harvest Search Analytics pages with impressions > 0
    /// and tag them INDEXED / google_status=SUBMITTED (quota exemption).
    pub async fn sync_analytics(&self, site_id: i64) -> anyhow::Result<GscSyncResult> {
        let site = self.require_google_site(site_id).await?;
        let sa = site.google_service_account_json.as_deref().unwrap_or("");
        let property = self.resolve_property(&site).await?;

        let end = Utc::now().date_naive();
        let start = end - Duration::days(90);
        let pages = self
            .google
            .search_analytics_pages(
                sa,
                &property,
                &start.format("%Y-%m-%d").to_string(),
                &end.format("%Y-%m-%d").to_string(),
            )
            .await?;

        let n_pages = pages.len() as u64;
        let marked = self
            .urls
            .mark_gsc_indexed_pages(site.id, &pages, site.bing_ready())
            .await?;
        self.sites.mark_gsc_analytics_synced(site.id).await?;

        Ok(GscSyncResult {
            success: true,
            property_url: property,
            pages_from_gsc: n_pages,
            urls_marked_indexed: marked,
            message: format!(
                "GSC Search Analytics: {n_pages} ranking page(s), {marked} URL(s) tagged INDEXED (exempt from Google submit quota)"
            ),
        })
    }

    /// Layer 2: enqueue GSC URL Inspection tasks within the 2,000/day budget.
    pub async fn enqueue_inspect_batch(
        &self,
        site_id: i64,
    ) -> anyhow::Result<GscInspectEnqueueResult> {
        let site = self.require_google_site(site_id).await?;
        // Resolve property up front so the worker has a cached URL.
        let _ = self.resolve_property(&site).await?;

        let funnel = self.urls.index_funnel_stats(site_id).await?;
        let pending = self
            .tasks
            .count_pending_type(site_id, TaskType::GscInspect)
            .await?;
        let total = self.config.gsc_inspect_daily_quota as i64;
        let remaining = (total - funnel.inspected_24h - pending).max(0);
        if remaining <= 0 {
            return Ok(GscInspectEnqueueResult {
                success: true,
                tasks_created: 0,
                quota_used_24h: funnel.inspected_24h,
                quota_remaining: 0,
                message: format!(
                    "GSC inspection quota exhausted ({}/{} used in the last 24h, {pending} already queued)",
                    funnel.inspected_24h, total
                ),
            });
        }

        let ids = self.urls.list_gsc_inspect_ids(site_id, remaining).await?;
        if ids.is_empty() {
            return Ok(GscInspectEnqueueResult {
                success: true,
                tasks_created: 0,
                quota_used_24h: funnel.inspected_24h,
                quota_remaining: remaining,
                message: "No unindexed URLs remaining for GSC inspection".into(),
            });
        }

        let created = self
            .tasks
            .create_gsc_inspect_tasks_batch(site_id, &ids, priority::GSC_INSPECT)
            .await?;

        Ok(GscInspectEnqueueResult {
            success: true,
            tasks_created: created,
            quota_used_24h: funnel.inspected_24h,
            quota_remaining: remaining - created as i64,
            message: format!(
                "Queued {created} GSC URL Inspection task(s) (quota remaining ≈ {})",
                remaining - created as i64
            ),
        })
    }

    pub async fn inspect_one(
        &self,
        site: &crate::domain::Site,
        page_url: &str,
    ) -> anyhow::Result<crate::providers::google::GscInspectResult> {
        let sa = site
            .google_service_account_json
            .as_deref()
            .ok_or_else(|| anyhow::anyhow!("no Google credentials"))?;
        let property = self.resolve_property(site).await?;
        self.google.inspect_url(sa, &property, page_url).await
    }

    pub async fn apply_inspect_result(
        &self,
        url_id: i64,
        bing_enabled: bool,
        result: &crate::providers::google::GscInspectResult,
    ) -> anyhow::Result<&'static str> {
        if !result.ok {
            let msg = format!(
                "GSC inspect HTTP {}: {}",
                result.status_code,
                truncate(&result.raw, 400)
            );
            self.urls.mark_gsc_inspected_error(url_id, &msg).await?;
            return Ok(GINDEX_UNKNOWN);
        }
        let coverage = result
            .coverage_state
            .clone()
            .unwrap_or_else(|| "URL is unknown to Google".into());
        let index_status = coverage_to_index_status(&coverage);
        let crawled = result
            .last_crawl_time
            .as_deref()
            .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
            .map(|d| d.with_timezone(&Utc));
        self.urls
            .apply_gsc_inspection(
                url_id,
                index_status,
                Some(coverage.as_str()),
                crawled,
                bing_enabled,
            )
            .await?;
        Ok(index_status)
    }

    pub async fn monitor_stats(&self, site_id: i64) -> anyhow::Result<IndexMonitorStats> {
        let site = self
            .sites
            .find_by_id(site_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("site not found"))?;
        let funnel = self.urls.index_funnel_stats(site_id).await?;
        let pending = self
            .tasks
            .count_pending_type(site_id, TaskType::GscInspect)
            .await?;
        let total = self.config.gsc_inspect_daily_quota as i64;
        let remaining = (total - funnel.inspected_24h - pending).max(0);
        Ok(IndexMonitorStats {
            gsc_inspect_quota_total: self.config.gsc_inspect_daily_quota,
            gsc_inspect_used_24h: funnel.inspected_24h,
            gsc_inspect_remaining: remaining,
            gsc_inspect_pending: pending,
            gsc_property_url: site.gsc_property_url,
            gsc_analytics_synced_at: site.gsc_analytics_synced_at,
            funnel,
        })
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let t: String = s.chars().take(max).collect();
        format!("{t}…")
    }
}
