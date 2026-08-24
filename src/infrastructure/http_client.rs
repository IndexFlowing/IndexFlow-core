use reqwest::Client;
use std::time::Duration;

/// 通用标准化 User-Agent（兼容 Cloudflare 与各大站点防火墙）
pub const INTERNAL_CRAWLER_UA: &str = "Mozilla/5.0 (compatible; IndexFlowBot/1.0; +https://www.indexflowing.com)";

/// 全局共享 HTTP 客户端
pub fn build_http_client() -> anyhow::Result<Client> {
    Client::builder()
        .connect_timeout(Duration::from_secs(15))
        .timeout(Duration::from_secs(60)) // 给大 Sitemap 足够的下载缓冲时间
        .user_agent(INTERNAL_CRAWLER_UA)
        .redirect(reqwest::redirect::Policy::limited(5))
        .build()
        .map_err(|e| anyhow::anyhow!("failed to build HTTP client: {e}"))
}