mod entity;
mod locale;
mod sitemap;
mod status;

pub use entity::{hash_url, Url};
pub use locale::extract_locale_and_path_prefix;
pub use sitemap::{SitemapType, SitemapUrlEntry};
pub use status::*;

#[cfg(test)]
mod tests {
    use super::Url;
    use chrono::Utc;

    fn url(discovered_via: &str) -> Url {
        let now = Utc::now();
        Url {
            id: 1,
            site_id: 1,
            url: "https://example.com".into(),
            url_hash: "hash".into(),
            seo_status: "PENDING".into(),
            seo_issue: None,
            page_title: None,
            meta_description: None,
            h1_content: None,
            h1_count: None,
            has_nofollow: false,
            ai_blocked_bots: None,
            has_opengraph: false,
            has_twitter_card: false,
            schema_types: None,
            response_time_ms: None,
            payload_bytes: None,
            has_viewport: false,
            html_lang: None,
            images_missing_alt: None,
            seo_warnings: None,
            canonical_url: None,
            http_status: None,
            locale: "default".into(),
            path_prefix: "/".into(),
            gsc_index_status: "UNKNOWN".into(),
            gsc_coverage_state: None,
            gsc_last_crawled_at: None,
            gsc_inspected_at: None,
            gsc_verdict: None,
            gsc_crawled_as: None,
            gsc_page_fetch_state: None,
            gsc_robots_txt_state: None,
            gsc_indexing_state: None,
            gsc_user_canonical: None,
            gsc_google_canonical: None,
            gsc_referring_urls: None,
            bing_index_status: "UNKNOWN".into(),
            bing_coverage_state: None,
            bing_last_crawled_at: None,
            bing_inspected_at: None,
            bing_status: "NONE".into(),
            bing_submitted_at: None,
            bing_error: None,
            google_status: "NONE".into(),
            google_submitted_at: None,
            google_error: None,
            priority: 0,
            sitemap_lastmod: None,
            sitemap_synced_at: None,
            discovered_via: discovered_via.into(),
            last_checked_at: None,
            first_seen_at: now,
            created_at: now,
            updated_at: now,
            is_watched: false,
            watched_at: None,
        }
    }

    #[test]
    fn orphan_status_uses_discovery_source_only() {
        let mut orphan = url("gsc_orphan");
        orphan.gsc_coverage_state = Some("Auto-Discovered".into());
        assert!(orphan.is_orphan());

        let mut sitemap = url("sitemap");
        sitemap.gsc_coverage_state = Some("Auto-Discovered".into());
        assert!(!sitemap.is_orphan());
    }

    #[test]
    fn test_referring_urls_list() {
        let mut u = url("sitemap");
        assert!(u.referring_urls_list().is_empty());

        u.gsc_referring_urls = Some("https://example.com/a\n\n  https://example.com/b  \n".into());
        assert_eq!(
            u.referring_urls_list(),
            vec!["https://example.com/a", "https://example.com/b"]
        );
    }
}