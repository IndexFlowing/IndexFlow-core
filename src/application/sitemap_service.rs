use crate::domain::{
    extract_locale_and_path_prefix, SitemapType, SitemapUrlEntry,
};
use indexflow_sitemap::SitemapFetcher;
use std::time::Duration;
use tracing::info;

#[derive(Clone)]
pub struct SitemapService {
    shared_client: reqwest::Client,
}

impl SitemapService {
    pub fn new(client: reqwest::Client) -> Self {
        Self {
            shared_client: client,
        }
    }

    /// 解析 Sitemap：若站点有专属放行 UA，用定制 Client 发起抓取，否则使用共享 Client
    pub async fn expand_to_page_entries(
        &self,
        sitemap_url: &str,
        max_depth: u8,
        custom_ua: Option<&str>,
    ) -> anyhow::Result<(SitemapType, Vec<SitemapUrlEntry>)> {
        info!(url = %sitemap_url, ua = ?custom_ua, "expanding sitemap entries");

        let client = if let Some(ua) = custom_ua.map(str::trim).filter(|s| !s.is_empty()) {
            reqwest::Client::builder()
                .connect_timeout(Duration::from_secs(15))
                .timeout(Duration::from_secs(60))
                .user_agent(ua)
                .redirect(reqwest::redirect::Policy::limited(5))
                .build()
                .unwrap_or_else(|_| self.shared_client.clone())
        } else {
            self.shared_client.clone()
        };

        let fetcher = SitemapFetcher::new(client);
        let (is_index, raw_entries) = fetcher.expand_all(sitemap_url, max_depth).await?;

        let sm_type = if is_index {
            SitemapType::Index
        } else {
            SitemapType::UrlSet
        };

        let domain_entries: Vec<SitemapUrlEntry> = raw_entries
            .into_iter()
            .map(|raw| {
                let best_hreflang = raw
                    .hreflangs
                    .iter()
                    .find(|h| {
                        let norm = |s: &str| s.trim().trim_end_matches('/').to_ascii_lowercase();
                        norm(&h.href) == norm(&raw.loc)
                    })
                    .map(|h| h.lang.as_str());

                let (locale, path_prefix) =
                    extract_locale_and_path_prefix(&raw.loc, best_hreflang);

                SitemapUrlEntry {
                    loc: raw.loc,
                    lastmod: raw.lastmod,
                    priority: raw.priority,
                    locale,
                    path_prefix,
                }
            })
            .collect();

        Ok((sm_type, domain_entries))
    }
}