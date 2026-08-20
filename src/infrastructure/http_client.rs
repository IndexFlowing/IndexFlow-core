use reqwest::Client;
use std::time::Duration;

/// Internal crawler UA — Cloudflare WAF allowlist bypass for on-demand and batch fetches.
pub const INTERNAL_CRAWLER_UA: &str = "MandarinClips-Internal-SeoBot-Secret888";

/// Shared HTTP client for workers and providers.
pub fn build_http_client() -> Client {
    Client::builder()
        .timeout(Duration::from_secs(30))
        .user_agent(INTERNAL_CRAWLER_UA)
        .redirect(reqwest::redirect::Policy::limited(5))
        .build()
        .expect("failed to build HTTP client")
}
