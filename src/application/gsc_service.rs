use crate::domain::coverage_to_index_status;
use crate::infrastructure::{Site, SiteRepo, UrlRepo};
use crate::providers::google::{GoogleProvider, GscInspectResult};
use chrono::Utc;
use tracing::info;

#[derive(Clone)]
pub struct GscService {
    google: GoogleProvider,
    sites: SiteRepo,
    urls: UrlRepo,
}

impl GscService {
    pub fn new(google: GoogleProvider, sites: SiteRepo, urls: UrlRepo) -> Self {
        Self {
            google,
            sites,
            urls,
        }
    }

    pub async fn test_credentials(&self, service_account_json: &str, domain: &str) -> anyhow::Result<String> {
        self.google.resolve_gsc_property(service_account_json, domain).await
    }

    /// 单条 URL 深度检测
    pub async fn inspect_one(
        &self,
        site: &Site,
        page_url: &str,
    ) -> anyhow::Result<GscInspectResult> {
        let sa = site
            .google_service_account_json
            .as_deref()
            .ok_or_else(|| anyhow::anyhow!("No Google credentials configured for site: {}", site.domain))?;

        let property = if let Some(ref p) = site.gsc_property_url {
            p.clone()
        } else {
            let p = self.google.resolve_gsc_property(sa, &site.domain).await?;
            self.sites.set_gsc_property(site.id, &p).await?;
            p
        };

        self.google.inspect_url(sa, &property, page_url).await
    }

    /// 【核心新增】通过 Search Analytics API 一键批量同步已曝光的已收录 URL 资产
    pub async fn sync_indexed_from_search_analytics(&self, site: &Site) -> anyhow::Result<u64> {
        let sa = site
            .google_service_account_json
            .as_deref()
            .ok_or_else(|| anyhow::anyhow!("当前站点尚未配置 Google Service Account 密钥"))?;

        let property = if let Some(ref p) = site.gsc_property_url {
            p.clone()
        } else {
            let p = self.google.resolve_gsc_property(sa, &site.domain).await?;
            self.sites.set_gsc_property(site.id, &p).await?;
            p
        };

        info!(domain = %site.domain, property = %property, "⚡ 正在从 Google Search Analytics 批量同步曝光收录池...");

        let indexed_urls = self.google.fetch_search_analytics_pages(sa, &property).await?;
        if indexed_urls.is_empty() {
            info!(domain = %site.domain, "Search Analytics 未返回任何曝光 URL");
            return Ok(0);
        }

        let updated_count = self.urls.batch_mark_gsc_indexed(site.id, &indexed_urls).await?;
        info!(
            domain = %site.domain,
            pulled_count = indexed_urls.len(),
            updated_count,
            "🎉 [GSC] 批量曝光收录同步完成，已落库更新！"
        );

        Ok(updated_count)
    }

    pub async fn apply_inspect_result(
        &self,
        url_id: i64,
        result: &GscInspectResult,
    ) -> anyhow::Result<()> {
        if result.ok {
            let coverage = result.coverage_state.as_deref().unwrap_or("URL is unknown to Google");
            let index_status = coverage_to_index_status(coverage);
            let crawled = result
                .last_crawl_time
                .as_deref()
                .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
                .map(|d| d.with_timezone(&Utc));

            self.urls
                .apply_gsc_inspection(url_id, index_status, Some(coverage), crawled)
                .await?;
        }
        Ok(())
    }
}
