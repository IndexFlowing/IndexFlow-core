// src/api/handlers/extension.rs
use super::AppState;
use crate::infrastructure::INTERNAL_CRAWLER_UA;
use axum::{
    extract::State,
    http::StatusCode,
    Json,
};
use serde::Deserialize;
use std::time::Duration;
use tracing::info;
use url::Url;

#[derive(Deserialize)]
pub struct ExtensionInspectRequest {
    pub url: String,
    pub custom_ua: Option<String>, // 核心新增：插件传来的临时 WAF 放行密钥
}

/// 浏览器插件专属：实时即时技术 SEO 深度质检端点
pub async fn action_extension_inspect(
    State(state): State<AppState>,
    Json(payload): Json<ExtensionInspectRequest>,
) -> (StatusCode, Json<serde_json::Value>) {
    info!(target_url = %payload.url, "🔌 [Extension] 收到浏览器插件发起的实时技术 SEO 质检请求");

    // 1. 智能决策爬虫 User-Agent（彻底攻克 Cloudflare 403）
    let effective_ua = if let Some(ua) = payload.custom_ua.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        // 通道 A：优先使用插件临时输入的放行密钥
        format_crawler_ua(ua)
    } else {
        // 通道 B：自动从本地 sites 表中匹配该域名的 Cloudflare 放行配置
        find_site_custom_ua(&state, &payload.url).await.unwrap_or_else(|| INTERNAL_CRAWLER_UA.to_string())
    };

    info!(ua = %effective_ua, "🛡️ [Extension] 正在使用定制放行 UA 探测目标网站...");

    let prober = match indexflow_seo::SeoProbeClient::new(&effective_ua, Duration::from_secs(12)) {
        Ok(p) => p,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": format!("初始化探针失败: {e}") })),
            );
        }
    };

    let result = prober.check_url(&payload.url).await;

    (
        StatusCode::OK,
        Json(serde_json::json!({
            "audit": result
        })),
    )
}

/// 自动在站点资产库中通过域名匹配绑定的 Cloudflare 放行密钥
async fn find_site_custom_ua(state: &AppState, target_url: &str) -> Option<String> {
    let parsed = Url::parse(target_url).ok()?;
    let host = parsed.host_str()?;
    let clean_host = host.trim_start_matches("www.");

    let sites = state.site_service.list_sites().await.ok()?;
    for site in sites {
        let site_domain = site.domain.trim().trim_start_matches("www.").trim_start_matches("https://").trim_start_matches("http://").trim_end_matches('/');
        if clean_host == site_domain || host == site_domain {
            if let Some(ua) = site.effective_crawler_ua() {
                return Some(ua);
            }
        }
    }
    None
}

fn format_crawler_ua(token: &str) -> String {
    if token.starts_with("Mozilla/") {
        token.to_string()
    } else {
        format!("{INTERNAL_CRAWLER_UA}; {token}")
    }
}