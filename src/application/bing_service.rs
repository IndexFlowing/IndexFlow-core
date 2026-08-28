use crate::infrastructure::{Site, UrlRepo};
use crate::providers::bing::{BingInspectResult, BingProvider};

#[derive(Clone)]
pub struct BingService {
    bing: BingProvider,
    urls: UrlRepo,
}

impl BingService {
    pub fn new(bing: BingProvider, urls: UrlRepo) -> Self {
        Self { bing, urls }
    }

    /// 执行单条 URL 在 Bing 官方的深度收录检测
    pub async fn inspect_one(
        &self,
        site: &Site,
        page_url: &str,
    ) -> anyhow::Result<BingInspectResult> {
        let api_key = site
            .bing_webmaster_api_key
            .as_deref()
            .map(str::trim)
            .filter(|k| !k.is_empty())
            .ok_or_else(|| anyhow::anyhow!("当前站点尚未在【站点设置】中配置 Bing Webmaster API Key"))?;

        // 自动解析 Bing 官方登记的确切站点 URL (如 https://inkvilion.com/)
        let site_url = self.bing.resolve_site_url(api_key, &site.domain).await?;

        self.bing.inspect_url(api_key, &site_url, page_url).await
    }

    pub async fn apply_inspect_result(
        &self,
        url_id: i64,
        result: &BingInspectResult,
    ) -> anyhow::Result<()> {
        self.urls
            .apply_bing_inspection(
                url_id,
                &result.index_status,
                result.coverage_state.as_deref(),
                result.last_crawl_time,
            )
            .await?;
        Ok(())
    }
}