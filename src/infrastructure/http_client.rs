use reqwest::Client;
use std::time::Duration;

/// Shared HTTP client for workers and providers.
pub fn build_http_client() -> Client {
    Client::builder()
        .timeout(Duration::from_secs(30))
        .user_agent("IndexFlow/0.1 (+https://github.com/indexflow)")
        .redirect(reqwest::redirect::Policy::limited(5))
        .build()
        .expect("failed to build HTTP client")
}
