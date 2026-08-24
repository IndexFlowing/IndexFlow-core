use crate::config::AppConfig;
use crate::infrastructure::{DashboardStats, SiteConfig, SiteRepo, UrlRepo};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

#[derive(Clone)]
pub struct SiteService {
    sites: SiteRepo,
    urls: UrlRepo,
    pub is_sync_running: Arc<AtomicBool>,
    pub is_seo_running: Arc<AtomicBool>,
    pub is_gsc_running: Arc<AtomicBool>,
    pub is_submit_running: Arc<AtomicBool>,
    #[allow(dead_code)]
    config: AppConfig,
}

impl SiteService {
    pub fn new(
        sites: SiteRepo,
        urls: UrlRepo,
        is_sync_running: Arc<AtomicBool>,
        is_seo_running: Arc<AtomicBool>,
        is_gsc_running: Arc<AtomicBool>,
        is_submit_running: Arc<AtomicBool>,
        config: AppConfig,
    ) -> Self {
        Self {
            sites,
            urls,
            is_sync_running,
            is_seo_running,
            is_gsc_running,
            is_submit_running,
            config,
        }
    }

    pub async fn get_config(&self) -> anyhow::Result<Option<SiteConfig>> {
        self.sites.get().await
    }

    pub async fn save_config(
        &self,
        domain: &str,
        sitemap_url: Option<&str>,
        bing_key: Option<&str>,
        google_json: Option<&str>,
    ) -> anyhow::Result<SiteConfig> {
        let site = self.sites.save_or_update(domain, sitemap_url, bing_key, google_json).await?;
        if sitemap_url.is_some() {
            self.is_sync_running.store(true, Ordering::Relaxed);
        }
        Ok(site)
    }

    pub async fn dashboard_stats(&self) -> anyhow::Result<DashboardStats> {
        self.urls.dashboard_stats().await
    }

    /// 触发同步 Sitemap
    pub async fn trigger_sync_sitemap(&self) -> anyhow::Result<bool> {
        self.is_sync_running.store(true, Ordering::Relaxed);
        Ok(true)
    }

    /// 触发高并发 SEO 质检
    pub async fn trigger_seo_audit(&self) -> anyhow::Result<u64> {
        self.is_seo_running.store(true, Ordering::Relaxed);
        let count = self.urls.reset_all_seo_status().await?;
        Ok(count)
    }

    /// 触发高并发 GSC 检测
    pub async fn trigger_gsc_inspect(&self) -> anyhow::Result<u64> {
        self.is_gsc_running.store(true, Ordering::Relaxed);
        let count = self.urls.reset_all_gsc_status().await?;
        Ok(count)
    }

    /// 触发全引擎提交
    pub async fn trigger_submit_all(&self) -> anyhow::Result<u64> {
        self.is_submit_running.store(true, Ordering::Relaxed);
        let count = self.urls.reset_all_submit_status().await?;
        Ok(count)
    }

    /// 毫秒级停止所有正在运行的 Worker
    pub async fn cancel_tasks(&self) -> anyhow::Result<u64> {
        self.is_sync_running.store(false, Ordering::Relaxed);
        self.is_seo_running.store(false, Ordering::Relaxed);
        self.is_gsc_running.store(false, Ordering::Relaxed);
        self.is_submit_running.store(false, Ordering::Relaxed);
        Ok(0)
    }
}