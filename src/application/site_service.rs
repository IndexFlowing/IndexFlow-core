use crate::application::PipelineManager;
use crate::infrastructure::{DashboardStats, Site, SiteRepo, UrlRepo};

#[derive(Clone)]
pub struct SiteService {
    sites: SiteRepo,
    urls: UrlRepo,
    pub pipeline: PipelineManager,
}

impl SiteService {
    pub fn new(sites: SiteRepo, urls: UrlRepo, pipeline: PipelineManager) -> Self {
        Self {
            sites,
            urls,
            pipeline,
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
        custom_user_agent: Option<&str>, // 核心新增
    ) -> anyhow::Result<Site> {
        self.sites
            .save_or_update(
                id,
                domain,
                sitemap_url,
                bing_key,
                bing_webmaster_key,
                google_json,
                custom_user_agent,
            )
            .await
    }

    pub async fn delete_site(&self, id: i64) -> anyhow::Result<()> {
        self.sites.delete_site(id).await
    }

    pub async fn dashboard_stats(&self, site_id: i64) -> anyhow::Result<DashboardStats> {
        self.urls.dashboard_stats(site_id).await
    }
}