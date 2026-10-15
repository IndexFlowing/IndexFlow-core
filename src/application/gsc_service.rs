// src/application/gsc_service.rs
use crate::domain::coverage_to_index_status;
use crate::infrastructure::{GscInspectionDetails, IndexHistoryRepo, Site, SiteRepo, UrlRepo};
use crate::providers::google::{GoogleProvider, GscInspectResult};
use chrono::Utc;
use tracing::info;

#[derive(Clone)]
pub struct GscService {
    google: GoogleProvider,
    sites: SiteRepo,
    urls: UrlRepo,
    history: IndexHistoryRepo,
}

impl GscService {
    pub fn new(
        google: GoogleProvider,
        sites: SiteRepo,
        urls: UrlRepo,
        history: IndexHistoryRepo,
    ) -> Self {
        Self {
            google,
            sites,
            urls,
            history,
        }
    }

    pub async fn test_credentials(
        &self,
        service_account_json: &str,
        domain: &str,
    ) -> anyhow::Result<String> {
        self.google
            .resolve_gsc_property(service_account_json, domain)
            .await
    }

    /// 单条 URL 深度检测
    pub async fn inspect_one(
        &self,
        site: &Site,
        page_url: &str,
    ) -> anyhow::Result<GscInspectResult> {
        let sa = site.google_service_account_json.as_deref().ok_or_else(|| {
            anyhow::anyhow!("No Google credentials configured for site: {}", site.domain)
        })?;

        // 自动纠正：避免使用数据库中残留的历史错误 sc-domain:www. 缓存
        let property = match site.gsc_property_url.as_deref() {
            Some(p) if !p.starts_with("sc-domain:www.") => p.to_string(),
            _ => {
                let p = self.google.resolve_gsc_property(sa, &site.domain).await?;
                let _ = self.sites.set_gsc_property(site.id, &p).await;
                p
            }
        };

        self.google.inspect_url(sa, &property, page_url).await
    }

    /// 【核心】通过 Search Analytics API 一键批量同步已曝光的已收录 URL 资产
    pub async fn sync_indexed_from_search_analytics(&self, site: &Site) -> anyhow::Result<u64> {
        let sa = site
            .google_service_account_json
            .as_deref()
            .ok_or_else(|| anyhow::anyhow!("当前站点尚未配置 Google Service Account 密钥"))?;

        // 自动纠正：避免使用数据库中残留的历史错误 sc-domain:www. 缓存
        let property = match site.gsc_property_url.as_deref() {
            Some(p) if !p.starts_with("sc-domain:www.") => p.to_string(),
            _ => {
                let p = self.google.resolve_gsc_property(sa, &site.domain).await?;
                let _ = self.sites.set_gsc_property(site.id, &p).await;
                p
            }
        };

        info!(domain = %site.domain, property = %property, "⚡ 正在从 Google Search Analytics 批量同步曝光收录池...");

        let indexed_urls = self
            .google
            .fetch_search_analytics_pages(sa, &property)
            .await?;
        if indexed_urls.is_empty() {
            info!(domain = %site.domain, "Search Analytics 未返回任何曝光 URL");
            return Ok(0);
        }

        let updated_count = self
            .urls
            .batch_mark_gsc_indexed(site.id, &indexed_urls)
            .await?;
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
        is_watched: bool,
    ) -> anyhow::Result<()> {
        if result.ok {
            let coverage = result
                .coverage_state
                .as_deref()
                .unwrap_or("URL is unknown to Google");
            let index_status = coverage_to_index_status(coverage);
            let crawled = result
                .last_crawl_time
                .as_deref()
                .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
                .map(|d| d.with_timezone(&Utc));

            let referring_urls_str = if result.referring_urls.is_empty() {
                None
            } else {
                Some(result.referring_urls.join("\n"))
            };

            let details = GscInspectionDetails {
                verdict: result.verdict.as_deref(),
                crawled_as: result.crawled_as.as_deref(),
                page_fetch_state: result.page_fetch_state.as_deref(),
                robots_txt_state: result.robots_txt_state.as_deref(),
                indexing_state: result.indexing_state.as_deref(),
                user_canonical: result.user_canonical.as_deref(),
                google_canonical: result.google_canonical.as_deref(),
                referring_urls: referring_urls_str.as_deref(),
            };

            self.urls
                .apply_gsc_inspection(
                    url_id,
                    index_status,
                    Some(coverage),
                    crawled,
                    Some(&details),
                )
                .await?;
            if is_watched {
                self.history
                    .insert_if_changed(url_id, "google", index_status, Some(coverage), crawled)
                    .await?;
            }
        } else if !result.is_quota_exceeded {
            let reason = result
                .raw_response
                .as_deref()
                .unwrap_or("Google Inspection API 返回失败");
            self.urls
                .apply_gsc_inspection(url_id, "FAILED", Some(reason), None, None)
                .await?;
        }
        Ok(())
    }
}