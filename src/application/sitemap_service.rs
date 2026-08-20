use crate::domain::{
    extract_locale_and_path_prefix, Sitemap, SitemapType, SitemapUrlEntry, TaskType, priority,
};
use crate::infrastructure::{SiteRepo, SitemapRepo, TaskRepo};
use chrono::{DateTime, NaiveDate, NaiveDateTime, Utc};
use quick_xml::events::Event;
use quick_xml::reader::Reader;
use reqwest::Client;
use tracing::{info, warn};

#[derive(Debug, Clone)]
pub enum ParsedSitemap {
    Index { child_urls: Vec<String> },
    UrlSet { entries: Vec<SitemapUrlEntry> },
}

#[derive(Clone)]
pub struct SitemapService {
    client: Client,
    sitemaps: SitemapRepo,
    sites: SiteRepo,
    tasks: TaskRepo,
}

impl SitemapService {
    pub fn new(client: Client, sitemaps: SitemapRepo, sites: SiteRepo, tasks: TaskRepo) -> Self {
        Self {
            client,
            sitemaps,
            sites,
            tasks,
        }
    }

    pub async fn list_by_site(&self, site_id: i64) -> anyhow::Result<Vec<Sitemap>> {
        self.sitemaps.list_by_site(site_id).await
    }

    /// Manually trigger sitemap sync for a site (all sitemaps or create one).
    pub async fn trigger_sync(
        &self,
        site_id: i64,
        sitemap_url: Option<&str>,
    ) -> anyhow::Result<u64> {
        let site = self
            .sites
            .find_by_id(site_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("site not found"))?;

        let mut created = 0u64;

        if let Some(url) = sitemap_url.filter(|s| !s.trim().is_empty()) {
            let sm = self
                .sitemaps
                .create(site.id, url, SitemapType::UrlSet)
                .await?;
            if self
                .tasks
                .create(
                    site.id,
                    None,
                    Some(sm.id),
                    TaskType::SyncSitemap,
                    priority::SYNC_SITEMAP,
                    Utc::now(),
                )
                .await?
                .is_some()
            {
                created += 1;
            }
        } else {
            let list = self.sitemaps.list_by_site(site_id).await?;
            for sm in list {
                if self
                    .tasks
                    .create(
                        site.id,
                        None,
                        Some(sm.id),
                        TaskType::SyncSitemap,
                        priority::SYNC_SITEMAP,
                        Utc::now(),
                    )
                    .await?
                    .is_some()
                {
                    created += 1;
                }
            }
        }

        Ok(created)
    }

    pub async fn fetch_and_parse(&self, sitemap_url: &str) -> anyhow::Result<ParsedSitemap> {
        info!(url = %sitemap_url, "downloading sitemap");
        let response = self.client.get(sitemap_url).send().await?;
        if !response.status().is_success() {
            anyhow::bail!("sitemap HTTP {}", response.status());
        }
        let body = response.text().await?;
        Ok(parse_sitemap_xml(&body))
    }

    /// Recursively expand sitemap index into page entries (depth-limited).
    pub async fn expand_to_page_entries(
        &self,
        sitemap_url: &str,
        max_depth: u8,
    ) -> anyhow::Result<(SitemapType, Vec<SitemapUrlEntry>)> {
        self.expand_inner(sitemap_url, max_depth, 0).await
    }

    async fn expand_inner(
        &self,
        sitemap_url: &str,
        max_depth: u8,
        depth: u8,
    ) -> anyhow::Result<(SitemapType, Vec<SitemapUrlEntry>)> {
        if depth > max_depth {
            warn!(url = %sitemap_url, "sitemap max depth reached");
            return Ok((SitemapType::UrlSet, vec![]));
        }

        match self.fetch_and_parse(sitemap_url).await? {
            ParsedSitemap::UrlSet { entries } => Ok((SitemapType::UrlSet, entries)),
            ParsedSitemap::Index { child_urls } => {
                let mut all = Vec::new();
                for child in child_urls {
                    match Box::pin(self.expand_inner(&child, max_depth, depth + 1)).await {
                        Ok((_, entries)) => all.extend(entries),
                        Err(e) => {
                            warn!(url = %child, error = %e, "child sitemap failed, keeping historical URLs");
                        }
                    }
                }
                Ok((SitemapType::Index, all))
            }
        }
    }
}

/// Parse sitemap XML: detect sitemapindex vs urlset; capture loc/lastmod/priority/hreflang.
pub fn parse_sitemap_xml(xml: &str) -> ParsedSitemap {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);

    let mut buf = Vec::new();
    let mut is_index = false;
    let mut in_url = false;
    let mut in_sitemap = false;
    let mut current_tag: Option<String> = None;

    let mut loc_buf = String::new();
    let mut lastmod_buf = String::new();
    let mut priority_buf = String::new();
    // (href, hreflang) collected from xhtml:link inside the current <url>.
    let mut alt_links: Vec<(String, String)> = Vec::new();
    let mut entries: Vec<SitemapUrlEntry> = Vec::new();
    let mut index_locs: Vec<String> = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                let local = e.local_name();
                let name = local.as_ref();
                match name {
                    b"sitemapindex" => is_index = true,
                    b"url" => {
                        in_url = true;
                        loc_buf.clear();
                        lastmod_buf.clear();
                        priority_buf.clear();
                        alt_links.clear();
                    }
                    b"sitemap" => {
                        in_sitemap = true;
                        loc_buf.clear();
                    }
                    b"loc" | b"lastmod" | b"priority" => {
                        current_tag = Some(String::from_utf8_lossy(name).into_owned());
                    }
                    b"link" => {
                        if let Some(pair) = read_hreflang_link(e.attributes()) {
                            alt_links.push(pair);
                        }
                    }
                    _ => {}
                }
            }
            Ok(Event::Empty(ref e)) => {
                let local = e.local_name();
                match local.as_ref() {
                    b"sitemapindex" => is_index = true,
                    b"link" if in_url => {
                        if let Some(pair) = read_hreflang_link(e.attributes()) {
                            alt_links.push(pair);
                        }
                    }
                    _ => {}
                }
            }
            Ok(Event::Text(e)) => {
                if let Some(tag) = current_tag.as_deref() {
                    if let Ok(text) = e.unescape() {
                        let s = text.into_owned();
                        match tag {
                            "loc" => loc_buf.push_str(&s),
                            "lastmod" => lastmod_buf.push_str(&s),
                            "priority" => priority_buf.push_str(&s),
                            _ => {}
                        }
                    }
                }
            }
            Ok(Event::End(ref e)) => {
                let name = e.local_name();
                match name.as_ref() {
                    b"loc" | b"lastmod" | b"priority" => {
                        current_tag = None;
                    }
                    b"url" => {
                        if in_url && !loc_buf.trim().is_empty() {
                            let loc = loc_buf.trim().to_string();
                            let hreflang = hreflang_for_loc(&loc, &alt_links);
                            let (locale, path_prefix) =
                                extract_locale_and_path_prefix(&loc, hreflang.as_deref());
                            entries.push(SitemapUrlEntry {
                                loc,
                                lastmod: parse_lastmod(lastmod_buf.trim()),
                                priority: parse_priority(priority_buf.trim()),
                                locale,
                                path_prefix,
                            });
                        }
                        in_url = false;
                        loc_buf.clear();
                        lastmod_buf.clear();
                        priority_buf.clear();
                        alt_links.clear();
                    }
                    b"sitemap" => {
                        if in_sitemap && !loc_buf.trim().is_empty() {
                            index_locs.push(loc_buf.trim().to_string());
                        }
                        in_sitemap = false;
                        loc_buf.clear();
                    }
                    _ => {}
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => {
                tracing::warn!(error = %e, "sitemap XML parse error — returning partial result");
                break;
            }
            _ => {}
        }
        buf.clear();
    }

    if is_index {
        // Fallback: if we only collected locs without <sitemap> wrappers
        if index_locs.is_empty() && !entries.is_empty() {
            index_locs = entries.into_iter().map(|e| e.loc).collect();
        }
        ParsedSitemap::Index {
            child_urls: index_locs,
        }
    } else {
        ParsedSitemap::UrlSet { entries }
    }
}

fn read_hreflang_link(
    attrs: quick_xml::events::attributes::Attributes<'_>,
) -> Option<(String, String)> {
    let mut href = None;
    let mut hreflang = None;
    for attr in attrs.flatten() {
        match attr.key.local_name().as_ref() {
            b"href" => {
                if let Ok(v) = attr.unescape_value() {
                    href = Some(v.into_owned());
                }
            }
            b"hreflang" => {
                if let Ok(v) = attr.unescape_value() {
                    hreflang = Some(v.into_owned());
                }
            }
            _ => {}
        }
    }
    match (href, hreflang) {
        (Some(h), Some(l)) if !h.is_empty() && !l.is_empty() => Some((h, l)),
        _ => None,
    }
}

fn hreflang_for_loc(loc: &str, alts: &[(String, String)]) -> Option<String> {
    let norm = |s: &str| s.trim().trim_end_matches('/').to_ascii_lowercase();
    let target = norm(loc);
    alts.iter()
        .find(|(href, _)| norm(href) == target)
        .map(|(_, hl)| hl.clone())
}

fn parse_priority(s: &str) -> Option<f64> {
    if s.is_empty() {
        return None;
    }
    s.parse::<f64>().ok().map(|v| v.clamp(0.0, 1.0))
}

/// Parse W3C sitemap lastmod: date or datetime.
pub fn parse_lastmod(s: &str) -> Option<DateTime<Utc>> {
    if s.is_empty() {
        return None;
    }
    if let Ok(dt) = DateTime::parse_from_rfc3339(s) {
        return Some(dt.with_timezone(&Utc));
    }
    // 2024-01-15T12:00:00+00:00 already covered; try without zone
    if let Ok(ndt) = NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S") {
        return Some(ndt.and_utc());
    }
    if let Ok(ndt) = NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S") {
        return Some(ndt.and_utc());
    }
    if let Ok(d) = NaiveDate::parse_from_str(s, "%Y-%m-%d") {
        return d.and_hms_opt(0, 0, 0).map(|ndt| ndt.and_utc());
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_urlset_with_priority_lastmod() {
        let xml = r#"<?xml version="1.0"?>
        <urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
          <url>
            <loc>https://example.com/a</loc>
            <lastmod>2024-06-01</lastmod>
            <priority>0.8</priority>
          </url>
          <url>
            <loc>https://example.com/b</loc>
            <priority>0.3</priority>
          </url>
        </urlset>"#;
        match parse_sitemap_xml(xml) {
            ParsedSitemap::UrlSet { entries } => {
                assert_eq!(entries.len(), 2);
                assert_eq!(entries[0].loc, "https://example.com/a");
                assert_eq!(entries[0].priority, Some(0.8));
                assert!(entries[0].lastmod.is_some());
                assert_eq!(entries[0].locale, "default");
                assert_eq!(entries[0].path_prefix, "/a");
                assert_eq!(entries[1].priority, Some(0.3));
            }
            _ => panic!("expected urlset"),
        }
    }

    #[test]
    fn parse_index() {
        let xml = r#"<?xml version="1.0"?>
        <sitemapindex xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
          <sitemap><loc>https://example.com/sitemap-1.xml</loc></sitemap>
        </sitemapindex>"#;
        match parse_sitemap_xml(xml) {
            ParsedSitemap::Index { child_urls } => {
                assert_eq!(child_urls.len(), 1);
            }
            _ => panic!("expected index"),
        }
    }

    #[test]
    fn parse_hreflang_and_path_locale() {
        let xml = r#"<?xml version="1.0"?>
        <urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9"
                xmlns:xhtml="http://www.w3.org/1999/xhtml">
          <url>
            <loc>https://example.com/zh/clips/123</loc>
            <xhtml:link rel="alternate" hreflang="zh" href="https://example.com/zh/clips/123"/>
            <xhtml:link rel="alternate" hreflang="en" href="https://example.com/en/clips/123"/>
          </url>
          <url>
            <loc>https://example.com/tools/x</loc>
          </url>
        </urlset>"#;
        match parse_sitemap_xml(xml) {
            ParsedSitemap::UrlSet { entries } => {
                assert_eq!(entries.len(), 2);
                assert_eq!(entries[0].locale, "zh");
                assert_eq!(entries[0].path_prefix, "/clips");
                assert_eq!(entries[1].locale, "default");
                assert_eq!(entries[1].path_prefix, "/tools");
            }
            _ => panic!("expected urlset"),
        }
    }
}
