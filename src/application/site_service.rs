use crate::infrastructure::{DashboardStats, Site, SiteRepo, UrlRepo};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

#[derive(Clone)]
pub struct SiteService {
    sites: SiteRepo,
    urls: UrlRepo,
    pub is_sync_running: Arc<AtomicBool>,
    pub is_seo_running: Arc<AtomicBool>,
    pub is_gsc_running: Arc<AtomicBool>,
    pub is_bing_inspect_running: Arc<AtomicBool>, // 核心新增
    pub is_submit_running: Arc<AtomicBool>,
}

impl SiteService {
    pub fn new(
        sites: SiteRepo,
        urls: UrlRepo,
        is_sync_running: Arc<AtomicBool>,
        is_seo_running: Arc<AtomicBool>,
        is_gsc_running: Arc<AtomicBool>,
        is_bing_inspect_running: Arc<AtomicBool>,
        is_submit_running: Arc<AtomicBool>,
    ) -> Self {
        Self {
            sites,
            urls,
            is_sync_running,
            is_seo_running,
            is_gsc_running,
            is_bing_inspect_running,
            is_submit_running,
        }
    }

    pub async fn list_sites(&self) -> anyhow::Result<Vec<Site>> {
        self.sites.list_all().await
    }

    pub async fn get_site_or_default(&self, site_id: Option<i64>) -> anyhow::Result<Option<Site>> {
        if let Some(id) = site_id {
            if let Some(site) = self.sites.find_by_id(id).await? {
                return Ok(Some(site));
            }
        }
        self.sites.get_default().await
    }

    pub async fn save_site(
        &self,
        id: Option<i64>,
        domain: &str,
        sitemap_url: Option<&str>,
        bing_key: Option<&str>,
        bing_webmaster_key: Option<&str>,
        google_json: Option<&str>,
    ) -> anyhow::Result<Site> {
        self.sites.save_or_update(id, domain, sitemap_url, bing_key, bing_webmaster_key, google_json).await
    }

    pub async fn delete_site(&self, id: i64) -> anyhow::Result<()> {
        self.sites.delete_site(id).await
    }

    pub async fn dashboard_stats(&self, site_id: i64) -> anyhow::Result<DashboardStats> {
        self.urls.dashboard_stats(site_id).await
    }

    pub async fn trigger_sync_sitemap(&self) -> anyhow::Result<bool> {
        self.is_sync_running.store(true, Ordering::Relaxed);
        Ok(true)
    }

    pub async fn trigger_seo_audit(&self) -> anyhow::Result<bool> {
        self.is_seo_running.store(true, Ordering::Relaxed);
        Ok(true)
    }

    pub async fn trigger_gsc_inspect(&self) -> anyhow::Result<bool> {
        self.is_gsc_running.store(true, Ordering::Relaxed);
        Ok(true)
    }

    pub async fn trigger_bing_inspect(&self) -> anyhow::Result<bool> {
        self.is_bing_inspect_running.store(true, Ordering::Relaxed);
        Ok(true)
    }

    pub async fn trigger_submit_all(&self) -> anyhow::Result<bool> {
        self.is_submit_running.store(true, Ordering::Relaxed);
        Ok(true)
    }

    pub async fn cancel_sync(&self) -> anyhow::Result<u64> {
        self.is_sync_running.store(false, Ordering::Relaxed);
        Ok(0)
    }

    pub async fn cancel_seo(&self) -> anyhow::Result<u64> {
        self.is_seo_running.store(false, Ordering::Relaxed);
        Ok(0)
    }

    pub async fn cancel_gsc(&self) -> anyhow::Result<u64> {
        self.is_gsc_running.store(false, Ordering::Relaxed);
        Ok(0)
    }

    pub async fn cancel_bing_inspect(&self) -> anyhow::Result<u64> {
        self.is_bing_inspect_running.store(false, Ordering::Relaxed);
        Ok(0)
    }

    pub async fn cancel_submit(&self) -> anyhow::Result<u64> {
        self.is_submit_running.store(false, Ordering::Relaxed);
        Ok(0)
    }
}