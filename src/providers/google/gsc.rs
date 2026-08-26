use super::oauth::GoogleAuthClient;
use chrono::{Duration, Utc};
use serde::{Deserialize, Serialize};
use tracing::{info, warn};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GscInspectResult {
    pub ok: bool,
    pub is_quota_exceeded: bool,
    pub coverage_state: Option<String>,
    pub last_crawl_time: Option<String>,
    pub raw_response: Option<String>,
}

#[derive(Clone)]
pub struct GscClient {
    http: reqwest::Client,
    auth: GoogleAuthClient,
}

impl GscClient {
    pub fn new(http: reqwest::Client, auth: GoogleAuthClient) -> Self {
        Self { http, auth }
    }

    /// 自动解析 GSC Property 资源标识
    pub async fn resolve_gsc_property(
        &self,
        service_account_json: &str,
        domain: &str,
    ) -> anyhow::Result<String> {
        let token = self
            .auth
            .get_access_token(
                service_account_json,
                "https://www.googleapis.com/auth/webmasters.readonly",
            )
            .await?;

        let res = self
            .http
            .get("https://www.googleapis.com/webmasters/v3/sites")
            .bearer_auth(&token)
            .send()
            .await?;

        if res.status().is_success() {
            let body: serde_json::Value = res.json().await?;
            if let Some(entries) = body.get("siteEntry").and_then(|v| v.as_array()) {
                let clean_domain = domain.trim().trim_start_matches("www.");
                for entry in entries {
                    if let Some(site_url) = entry.get("siteUrl").and_then(|s| s.as_str()) {
                        if site_url == format!("sc-domain:{clean_domain}")
                            || site_url.contains(clean_domain)
                        {
                            info!(site_url, "✅ 成功匹配到 GSC 站点资源");
                            return Ok(site_url.to_string());
                        }
                    }
                }
            }
        }

        Ok(format!("sc-domain:{domain}"))
    }

    /// 查询单条 URL 在 Google 索引库的真实状态
    pub async fn inspect_url(
        &self,
        service_account_json: &str,
        site_url: &str,
        inspection_url: &str,
    ) -> anyhow::Result<GscInspectResult> {
        let token = self
            .auth
            .get_access_token(
                service_account_json,
                "https://www.googleapis.com/auth/webmasters.readonly",
            )
            .await?;

        let payload = serde_json::json!({
            "inspectionUrl": inspection_url,
            "siteUrl": site_url,
            "languageCode": "en-US"
        });

        let res = self
            .http
            .post("https://searchconsole.googleapis.com/v1/urlInspection/index:inspect")
            .bearer_auth(&token)
            .json(&payload)
            .send()
            .await?;

        let status = res.status();
        let is_quota_exceeded = status.as_u16() == 429;
        let body_text = res.text().await.unwrap_or_default();

        if !status.is_success() {
            if is_quota_exceeded {
                warn!(
                    url = %inspection_url,
                    "⚠️ GSC Inspection API 触发 429 限流/配额耗尽"
                );
            } else {
                warn!(
                    url = %inspection_url,
                    status = %status,
                    body = %body_text,
                    "GSC Inspection API returned non-200"
                );
            }
            return Ok(GscInspectResult {
                ok: false,
                is_quota_exceeded,
                coverage_state: None,
                last_crawl_time: None,
                raw_response: Some(body_text),
            });
        }

        let parsed: serde_json::Value = serde_json::from_str(&body_text)?;
        let index_status = parsed
            .get("inspectionResult")
            .and_then(|ir| ir.get("indexStatusResult"));

        let coverage_state = index_status
            .and_then(|is| is.get("coverageState"))
            .and_then(|c| c.as_str())
            .map(|s| s.to_string());

        let last_crawl_time = index_status
            .and_then(|is| is.get("lastCrawlTime"))
            .and_then(|t| t.as_str())
            .map(|s| s.to_string());

        Ok(GscInspectResult {
            ok: true,
            is_quota_exceeded: false,
            coverage_state,
            last_crawl_time,
            raw_response: Some(body_text),
        })
    }

    /// 通过 Search Analytics API 批量拉取过去 30 天内所有有曝光的已收录 URL (单次最多 25,000 条)
    pub async fn fetch_search_analytics_pages(
        &self,
        service_account_json: &str,
        site_url: &str,
    ) -> anyhow::Result<Vec<String>> {
        let token = self
            .auth
            .get_access_token(
                service_account_json,
                "https://www.googleapis.com/auth/webmasters.readonly",
            )
            .await?;

        let end_date = Utc::now().date_naive();
        let start_date = end_date - Duration::days(30);

        let encoded_site: String =
            url::form_urlencoded::byte_serialize(site_url.as_bytes()).collect();

        let api_url = format!(
            "https://www.googleapis.com/webmasters/v3/sites/{}/searchAnalytics/query",
            encoded_site
        );

        let payload = serde_json::json!({
            "startDate": start_date.to_string(),
            "endDate": end_date.to_string(),
            "dimensions": ["page"],
            "rowLimit": 25000
        });

        info!(site_url, %start_date, %end_date, "📊 [GSC] 正在调用 Search Analytics API 批量获取已收录 URL...");

        let res = self
            .http
            .post(&api_url)
            .bearer_auth(&token)
            .json(&payload)
            .send()
            .await?;

        if !res.status().is_success() {
            let body = res.text().await.unwrap_or_default();
            anyhow::bail!("Search Analytics query failed: {body}");
        }

        let body: serde_json::Value = res.json().await?;
        let mut indexed_urls = Vec::new();

        if let Some(rows) = body.get("rows").and_then(|r| r.as_array()) {
            for row in rows {
                if let Some(keys) = row.get("keys").and_then(|k| k.as_array()) {
                    if let Some(first_url) = keys.first().and_then(|u| u.as_str()) {
                        indexed_urls.push(first_url.to_string());
                    }
                }
            }
        }

        info!(
            count = indexed_urls.len(),
            "🎉 [GSC] 成功从 Google 搜索结果中批量提取到已收录 URL"
        );
        Ok(indexed_urls)
    }
}