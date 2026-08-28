//! Async sitemap download + recursive index expansion.
//!
//! Safety properties
//! -----------------
//! * Wire body is streamed in chunks and aborted at [`MAX_DOWNLOAD_BYTES`].
//! * Gzip inflate is capped at [`MAX_UNCOMPRESSED_BYTES`] (bomb defence).
//! * Index cycles are detected via a **normalized** identity key (scheme/host
//!   case, default port, fragment, trailing slash) — not a raw string compare.
//! * Child fetch / parse failures are isolated: they are logged and skipped
//!   rather than failing the whole `expand_all` call.
//! * Recursion is bounded by the caller-supplied `max_depth`.

use crate::compression::decode_if_gzipped;
use crate::error::SitemapError;
use crate::models::{
    ParsedSitemap, SitemapUrlEntry, MAX_DOWNLOAD_BYTES, MAX_EXPAND_URLS, MAX_UNCOMPRESSED_BYTES,
};
use crate::parser::parse_sitemap;
use std::collections::HashSet;
use std::future::Future;
use std::pin::Pin;
use tracing::{info, warn};
use url::Url;

/// HTTP client wrapper that fetches and recursively expands sitemaps.
#[derive(Clone)]
pub struct SitemapFetcher {
    client: reqwest::Client,
}

impl SitemapFetcher {
    /// Wrap an existing `reqwest::Client` (timeouts / UA are the caller's concern).
    pub fn new(client: reqwest::Client) -> Self {
        Self { client }
    }

    /// Download and parse a single sitemap (gzip magic-sniffed, size-capped).
    pub async fn fetch(&self, sitemap_url: &str) -> Result<ParsedSitemap, SitemapError> {
        info!(url = %sitemap_url, "fetching sitemap");
        let (final_url, raw_bytes) = self.download_capped(sitemap_url).await?;
        let _ = final_url;
        let text_content = decode_if_gzipped(&raw_bytes)?;
        Ok(parse_sitemap(&text_content))
    }

    /// Recursively expand a sitemap index into a flat URL list.
    ///
    /// Returns `(is_index, entries)` where `is_index` is `true` when the *root*
    /// document was a `<sitemapindex>`. Cycles, max-depth, and per-child HTTP
    /// failures never abort the whole expansion; they skip that branch.
    pub async fn expand_all(
        &self,
        sitemap_url: &str,
        max_depth: u8,
    ) -> Result<(bool, Vec<SitemapUrlEntry>), SitemapError> {
        let mut visited = HashSet::new();
        let mut total = 0usize;
        self.expand_inner(sitemap_url, max_depth, 0, &mut visited, &mut total)
            .await
    }

    fn expand_inner<'a>(
        &'a self,
        sitemap_url: &'a str,
        max_depth: u8,
        depth: u8,
        visited: &'a mut HashSet<String>,
        total: &'a mut usize,
    ) -> Pin<Box<dyn Future<Output = Result<(bool, Vec<SitemapUrlEntry>), SitemapError>> + Send + 'a>>
    {
        Box::pin(async move {
            if depth > max_depth {
                warn!(
                    url = %sitemap_url,
                    max_depth,
                    depth,
                    "max recursive sitemap depth reached; skipping branch"
                );
                return Ok((false, vec![]));
            }

            let key = sitemap_identity_key(sitemap_url);
            if !visited.insert(key) {
                warn!(
                    url = %sitemap_url,
                    "circular sitemap index reference detected; skipping"
                );
                return Ok((false, vec![]));
            }

            if *total >= MAX_EXPAND_URLS {
                warn!(
                    cap = MAX_EXPAND_URLS,
                    "expand_all accumulated URL cap reached; skipping further fetches"
                );
                return Ok((false, vec![]));
            }

            let (final_url, raw_bytes) = match self.download_capped(sitemap_url).await {
                Ok(v) => v,
                Err(e) if depth == 0 => return Err(e),
                Err(e) => {
                    warn!(url = %sitemap_url, error = %e, "child sitemap fetch failed, skipping");
                    return Ok((false, vec![]));
                }
            };

            // Redirects can close a cycle via a different spelling of the same resource.
            if final_url != sitemap_url {
                let redirected_key = sitemap_identity_key(&final_url);
                if !visited.insert(redirected_key) {
                    warn!(
                        from = %sitemap_url,
                        to = %final_url,
                        "redirect closed a sitemap cycle; skipping"
                    );
                    return Ok((false, vec![]));
                }
            }

            let text_content = match decode_if_gzipped(&raw_bytes) {
                Ok(t) => t,
                Err(e) if depth == 0 => return Err(e),
                Err(e) => {
                    warn!(url = %sitemap_url, error = %e, "child sitemap decode failed, skipping");
                    return Ok((false, vec![]));
                }
            };
            drop(raw_bytes);

            match parse_sitemap(&text_content) {
                ParsedSitemap::UrlSet { mut entries } => {
                    cap_entries(&mut entries, total);
                    Ok((false, entries))
                }
                ParsedSitemap::PlainText { urls } => {
                    let mut entries: Vec<SitemapUrlEntry> = urls
                        .into_iter()
                        .map(|loc| SitemapUrlEntry {
                            loc,
                            ..Default::default()
                        })
                        .collect();
                    cap_entries(&mut entries, total);
                    Ok((false, entries))
                }
                ParsedSitemap::Index { child_urls } => {
                    let mut all_entries = Vec::new();
                    for child in child_urls {
                        match self
                            .expand_inner(&child, max_depth, depth + 1, visited, total)
                            .await
                        {
                            Ok((_, entries)) => all_entries.extend(entries),
                            Err(e) => {
                                warn!(url = %child, error = %e, "child sitemap fetch failed, skipping");
                            }
                        }
                        if *total >= MAX_EXPAND_URLS {
                            break;
                        }
                    }
                    Ok((true, all_entries))
                }
            }
        })
    }

    async fn download_capped(&self, sitemap_url: &str) -> Result<(String, Vec<u8>), SitemapError> {
        let response = self.client.get(sitemap_url).send().await?;

        let status = response.status();
        if !status.is_success() {
            return Err(SitemapError::HttpStatus(status.as_u16()));
        }

        if let Some(len) = response.content_length() {
            if len > MAX_DOWNLOAD_BYTES as u64 {
                return Err(SitemapError::PayloadTooLarge {
                    size: len,
                    limit: MAX_DOWNLOAD_BYTES as u64,
                });
            }
        }

        let final_url = response.url().to_string();
        let mut raw = Vec::new();
        let mut response = response;
        while let Some(chunk) = response.chunk().await? {
            let added = chunk.len();
            if raw.len().saturating_add(added) > MAX_DOWNLOAD_BYTES {
                return Err(SitemapError::PayloadTooLarge {
                    size: (raw.len() + added) as u64,
                    limit: MAX_DOWNLOAD_BYTES as u64,
                });
            }
            raw.extend_from_slice(&chunk);
            // A gzip bomb is caught at inflate time; still refuse an uncompressed
            // body that is already over the Google 50 MiB document cap.
            if raw.len() > MAX_UNCOMPRESSED_BYTES && !is_gzip_magic(&raw) {
                return Err(SitemapError::PayloadTooLarge {
                    size: raw.len() as u64,
                    limit: MAX_UNCOMPRESSED_BYTES as u64,
                });
            }
        }
        Ok((final_url, raw))
    }
}

fn is_gzip_magic(bytes: &[u8]) -> bool {
    bytes.len() >= 2 && bytes[0] == 0x1F && bytes[1] == 0x8B
}

fn cap_entries(entries: &mut Vec<SitemapUrlEntry>, total: &mut usize) {
    let room = MAX_EXPAND_URLS.saturating_sub(*total);
    if entries.len() > room {
        warn!(
            dropped = entries.len() - room,
            cap = MAX_EXPAND_URLS,
            "truncating sitemap entries to honour expand_all cap"
        );
        entries.truncate(room);
    }
    *total = total.saturating_add(entries.len());
}

/// Canonical identity used for cycle detection.
///
/// Lowercases scheme + host, drops default ports / fragments / trailing slash
/// (except `/`), so `HTTPS://Example.com:443/s.xml#x` and
/// `https://example.com/s.xml` collide.
pub(crate) fn sitemap_identity_key(url: &str) -> String {
    let Ok(parsed) = Url::parse(url.trim()) else {
        return url.trim().trim_end_matches('/').to_ascii_lowercase();
    };

    let scheme = parsed.scheme().to_ascii_lowercase();
    let host = parsed.host_str().unwrap_or("").to_ascii_lowercase();
    let mut path = parsed.path().to_string();
    if path.len() > 1 && path.ends_with('/') {
        path.pop();
    }

    let mut out = String::with_capacity(scheme.len() + host.len() + path.len() + 8);
    out.push_str(&scheme);
    out.push_str("://");
    out.push_str(&host);
    if let Some(port) = parsed.port() {
        let default = matches!((scheme.as_str(), port), ("http", 80) | ("https", 443));
        if !default {
            out.push(':');
            out.push_str(&port.to_string());
        }
    }
    out.push_str(&path);
    if let Some(q) = parsed.query() {
        out.push('?');
        out.push_str(q);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_key_collapses_default_port_case_fragment_slash() {
        assert_eq!(
            sitemap_identity_key("HTTPS://Example.com:443/s.xml/"),
            sitemap_identity_key("https://example.com/s.xml")
        );
        assert_eq!(
            sitemap_identity_key("https://example.com/s.xml#frag"),
            sitemap_identity_key("https://example.com/s.xml")
        );
        assert_ne!(
            sitemap_identity_key("https://example.com/a.xml"),
            sitemap_identity_key("https://example.com/b.xml")
        );
    }

    #[test]
    fn identity_key_keeps_non_default_port() {
        assert_ne!(
            sitemap_identity_key("https://example.com:4443/s.xml"),
            sitemap_identity_key("https://example.com/s.xml")
        );
    }

    #[test]
    fn identity_key_garbage_is_still_stable() {
        let k = sitemap_identity_key("  NOT A URL/ ");
        assert_eq!(k, sitemap_identity_key("not a url"));
    }
}
