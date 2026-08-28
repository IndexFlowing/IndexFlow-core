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
            .ok_or_else(|| anyhow::anyhow!("当前站点尚未配置 Bing Webmaster API Key"))?;

        let site_url = if site.domain.starts_with("http://") || site.domain.starts_with("https://") {
            site.domain.clone()
        } else {
            format!("https://{}", site.domain.trim_end_matches('/'))
        };

        self.bing.inspect_url(api_key, &site_url, page_url).await
    }

    pub async fn apply_inspect_result(
        &self,
        url_id: i64,
        result: &BingInspectResult,
    ) -> anyhow::Result<()> {
        if result.ok {
            self.urls
                .apply_bing_inspection(
                    url_id,
                    &result.index_status,
                    result.coverage_state.as_deref(),
                    result.last_crawl_time,
                )
                .await?;
        }
        Ok(())
    }
}