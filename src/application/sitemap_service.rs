use crate::domain::{
    extract_locale_and_path_prefix, SitemapType, SitemapUrlEntry,
};
use indexflow_sitemap::SitemapFetcher;
use reqwest::Client;
use tracing::info;

#[derive(Clone)]
pub struct SitemapService {
    fetcher: SitemapFetcher,
}

impl SitemapService {
    pub fn new(client: Client) -> Self {
        Self {
            fetcher: SitemapFetcher::new(client),
        }
    }

    pub async fn expand_to_page_entries(
        &self,
        sitemap_url: &str,
        max_depth: u8,
    ) -> anyhow::Result<(SitemapType, Vec<SitemapUrlEntry>)> {
        info!(url = %sitemap_url, "expanding sitemap entries");
        let (is_index, raw_entries) = self.fetcher.expand_all(sitemap_url, max_depth).await?;

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