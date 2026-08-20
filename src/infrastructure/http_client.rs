use reqwest::Client;
use std::time::Duration;

/// Internal crawler UA — Cloudflare WAF allowlist bypass for on-demand and batch fetches.
pub const INTERNAL_CRAWLER_UA: &str = "MandarinClips-Internal-SeoBot-Secret888";

/// Shared HTTP client for workers and providers.
///
/// Returns `Err` instead of panicking so callers can propagate startup
/// failures through `anyhow::Result` (consistent with graceful error handling).
pub fn build_http_client() -> anyhow::Result<Client> {
    Client::builder()
        .timeout(Duration::from_secs(30))
        .user_agent(INTERNAL_CRAWLER_UA)
        .redirect(reqwest::redirect::Policy::limited(5))
        .build()
        .map_err(|e| anyhow::anyhow!("failed to build HTTP client: {e}"))
}
