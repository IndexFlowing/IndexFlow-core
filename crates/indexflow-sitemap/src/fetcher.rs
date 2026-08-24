// crates/indexflow-sitemap/src/fetcher.rs
use crate::compression::decode_if_gzipped;
use crate::error::SitemapError;
use crate::models::{ParsedSitemap, SitemapUrlEntry};
use crate::parser::parse_sitemap;
use std::collections::HashSet;
use std::future::Future;
use std::pin::Pin;
use tracing::{info, warn};

#[derive(Clone)]
pub struct SitemapFetcher {
    client: reqwest::Client,
}

impl SitemapFetcher {
    pub fn new(client: reqwest::Client) -> Self {
        Self { client }
    }

    /// 下载并解析单个 Sitemap（自动支持 Gzip 嗅探与解压）
    pub async fn fetch(&self, sitemap_url: &str) -> Result<ParsedSitemap, SitemapError> {
        info!(url = %sitemap_url, "fetching sitemap");
        let response = self.client.get(sitemap_url).send().await?;

        let status = response.status();
        if !status.is_success() {
            return Err(SitemapError::HttpStatus(status.as_u16()));
        }

        let raw_bytes = response.bytes().await?;
        let text_content = decode_if_gzipped(&raw_bytes)?;

        Ok(parse_sitemap(&text_content))
    }

    /// 递归展开 SitemapIndex，自带最大深度限制与循环引用死锁保护
    pub async fn expand_all(
        &self,
        sitemap_url: &str,
        max_depth: u8,
    ) -> Result<(bool, Vec<SitemapUrlEntry>), SitemapError> {
        let mut visited = HashSet::new();
        self.expand_inner(sitemap_url, max_depth, 0, &mut visited).await
    }

    fn expand_inner<'a>(
        &'a self,
        sitemap_url: &'a str,
        max_depth: u8,
        depth: u8,
        visited: &'a mut HashSet<String>,
    ) -> Pin<Box<dyn Future<Output = Result<(bool, Vec<SitemapUrlEntry>), SitemapError>> + Send + 'a>> {
        Box::pin(async move {
            if depth > max_depth {
                warn!(url = %sitemap_url, max_depth, "max recursive sitemap depth reached");
                return Ok((false, vec![]));
            }

            if !visited.insert(sitemap_url.to_string()) {
                warn!(url = %sitemap_url, "circular sitemap index reference detected, skipping");
                return Ok((false, vec![]));
            }

            match self.fetch(sitemap_url).await? {
                ParsedSitemap::UrlSet { entries } => Ok((false, entries)),
                ParsedSitemap::PlainText { urls } => {
                    let entries = urls
                        .into_iter()
                        .map(|loc| SitemapUrlEntry {
                            loc,
                            ..Default::default()
                        })
                        .collect();
                    Ok((false, entries))
                }
                ParsedSitemap::Index { child_urls } => {
                    let mut all_entries = Vec::new();
                    for child in child_urls {
                        match self.expand_inner(&child, max_depth, depth + 1, visited).await {
                            Ok((_, entries)) => all_entries.extend(entries),
                            Err(e) => {
                                warn!(url = %child, error = %e, "child sitemap fetch failed, skipping");
                            }
                        }
                    }
                    Ok((true, all_entries))
                }
            }
        })
    }
}