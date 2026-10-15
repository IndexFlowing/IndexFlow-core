use super::oauth::GoogleAuthClient;
use chrono::{Duration, Utc};
use serde::{Deserialize, Serialize};
use tracing::{info, warn};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GscInspectResult {
    pub ok: bool,
    pub is_quota_exceeded: bool,
    pub verdict: Option<String>,
    pub coverage_state: Option<String>,
    pub robots_txt_state: Option<String>,
    pub indexing_state: Option<String>,
    pub last_crawl_time: Option<String>,
    pub page_fetch_state: Option<String>,
    pub crawled_as: Option<String>,
    pub user_canonical: Option<String>,
    pub google_canonical: Option<String>,
    pub referring_urls: Vec<String>,
    pub raw_response: Option<String>,
}

#[derive(Clone)]
pub struct GscClient {
    http: reqwest::Client,
    auth: GoogleAuthClient,
    log_raw_payload: bool,
}

impl GscClient {
    pub fn new(http: reqwest::Client, auth: GoogleAuthClient, log_raw_payload: bool) -> Self {
        Self {
            http,
            auth,
            log_raw_payload,
        }
    }

    /// 自动且智能解析站点对应的 GSC Property 资源标识
    /// 优先级策略：
    /// 1. 精确匹配子域名 URL 前缀资源 (如 https://www.mandarinclips.com/)
    /// 2. 匹配管辖该子域名的主网域资源 (如 sc-domain:mandarinclips.com)
    /// 3. 兜底为提取出的 Apex 根域 (sc-domain:mandarinclips.com)
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

        let target_host = domain
            .trim()
            .trim_start_matches("https://")
            .trim_start_matches("http://")
            .trim_end_matches('/')
            .to_ascii_lowercase();

        let apex_domain = extract_apex_domain(&target_host);

        if res.status().is_success() {
            let body: serde_json::Value = res.json().await?;
            if let Some(entries) = body.get("siteEntry").and_then(|v| v.as_array()) {
                let mut matched_prefix_prop = None;
                let mut matched_domain_prop = None;

                for entry in entries {
                    if let Some(site_url) = entry.get("siteUrl").and_then(|s| s.as_str()) {
                        let clean_prop = site_url.trim();

                        // 优先级 1: 检查完全匹配的 URL 前缀 (如 https://www.mandarinclips.com/)
                        if let Ok(parsed) = url::Url::parse(clean_prop) {
                            if let Some(host) = parsed.host_str() {
                                if host.eq_ignore_ascii_case(&target_host) {
                                    matched_prefix_prop = Some(clean_prop.to_string());
                                }
                            }
                        }

                        // 优先级 2: 检查可管辖该域名的 sc-domain 资源 (如 sc-domain:mandarinclips.com)
                        if clean_prop.starts_with("sc-domain:") {
                            let domain_in_prop = clean_prop
                                .trim_start_matches("sc-domain:")
                                .to_ascii_lowercase();
                            if target_host == domain_in_prop
                                || target_host.ends_with(&format!(".{domain_in_prop}"))
                            {
                                matched_domain_prop = Some(clean_prop.to_string());
                            }
                        }
                    }
                }

                if let Some(prop) = matched_prefix_prop {
                    info!(property = %prop, "✅ [GSC] 精确匹配到 URL 前缀站点资源");
                    return Ok(prop);
                }
                if let Some(prop) = matched_domain_prop {
                    info!(property = %prop, "✅ [GSC] 成功匹配到管辖该域名的主网域资源 (sc-domain)");
                    return Ok(prop);
                }
            }
        }

        // 绝不拼接 sc-domain:www.xxx，必须使用标准 Apex Domain
        let fallback_prop = format!("sc-domain:{apex_domain}");
        info!(property = %fallback_prop, "⚠️ [GSC] 未能匹配到列表项，使用合规根域名兜底");
        Ok(fallback_prop)
    }

    /// 查询单条 URL 在 Google 索引库的真实状态
    pub async fn inspect_url(
        &self,
        service_account_json: &str,
        site_url: &str,
        inspection_url: &str,
    ) -> anyhow::Result<GscInspectResult> {
        tracing::info!(url = %inspection_url, site = %site_url, "🔍 [GSC] 正在调用 Inspection API");

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
                raw_response: Some(body_text),
                ..Default::default()
            });
        }

        if self.log_raw_payload {
            tracing::debug!(url = %inspection_url, raw_json = %body_text, "📥 [GSC] 收到 Inspection 原始报文");
        }

        let result = parse_inspect_response(&body_text)?;
        tracing::info!(
            url = %inspection_url,
            verdict = ?result.verdict,
            coverage = ?result.coverage_state,
            last_crawl = ?result.last_crawl_time,
            crawled_as = ?result.crawled_as,
            "✅ [GSC] 检测解析完成"
        );
        Ok(result)
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

/// 解析 Google URL Inspection API 返回的 JSON 报文
pub fn parse_inspect_response(body_text: &str) -> anyhow::Result<GscInspectResult> {
    let parsed: serde_json::Value = serde_json::from_str(body_text)?;
    let is_result = parsed
        .get("inspectionResult")
        .and_then(|ir| ir.get("indexStatusResult"));

    let verdict = is_result
        .and_then(|r| r.get("verdict"))
        .and_then(|v| v.as_str())
        .map(str::to_string);
    let coverage_state = is_result
        .and_then(|r| r.get("coverageState"))
        .and_then(|v| v.as_str())
        .map(str::to_string);
    let robots_txt_state = is_result
        .and_then(|r| r.get("robotsTxtState"))
        .and_then(|v| v.as_str())
        .map(str::to_string);
    let indexing_state = is_result
        .and_then(|r| r.get("indexingState"))
        .and_then(|v| v.as_str())
        .map(str::to_string);
    let last_crawl_time = is_result
        .and_then(|r| r.get("lastCrawlTime"))
        .and_then(|v| v.as_str())
        .map(str::to_string);
    let page_fetch_state = is_result
        .and_then(|r| r.get("pageFetchState"))
        .and_then(|v| v.as_str())
        .map(str::to_string);
    let crawled_as = is_result
        .and_then(|r| r.get("crawledAs"))
        .and_then(|v| v.as_str())
        .map(str::to_string);
    let user_canonical = is_result
        .and_then(|r| r.get("userCanonical"))
        .and_then(|v| v.as_str())
        .map(str::to_string);
    let google_canonical = is_result
        .and_then(|r| r.get("googleCanonical"))
        .and_then(|v| v.as_str())
        .map(str::to_string);
    let referring_urls = is_result
        .and_then(|r| r.get("referringUrls"))
        .and_then(|r| r.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default();

    Ok(GscInspectResult {
        ok: true,
        is_quota_exceeded: false,
        verdict,
        coverage_state,
        robots_txt_state,
        indexing_state,
        last_crawl_time,
        page_fetch_state,
        crawled_as,
        user_canonical,
        google_canonical,
        referring_urls,
        raw_response: Some(body_text.to_string()),
    })
}

/// 提取主机名的根域名 (Apex Domain)，正确剥离 www、app、blog 等二级前缀
fn extract_apex_domain(host: &str) -> String {
    let clean = host.split(':').next().unwrap_or(host);
    let parts: Vec<&str> = clean.split('.').collect();
    if parts.len() >= 2 {
        format!("{}.{}", parts[parts.len() - 2], parts[parts.len() - 1])
    } else {
        clean.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_apex() {
        assert_eq!(extract_apex_domain("www.mandarinclips.com"), "mandarinclips.com");
        assert_eq!(extract_apex_domain("mandarinclips.com"), "mandarinclips.com");
        assert_eq!(extract_apex_domain("blog.mandarinclips.com"), "mandarinclips.com");
    }

    #[test]
    fn test_parse_inspect_response_complete() {
        let json_data = r#"{
            "inspectionResult": {
                "indexStatusResult": {
                    "verdict": "NEUTRAL",
                    "coverageState": "Crawled - currently not indexed",
                    "robotsTxtState": "ALLOWED",
                    "indexingState": "INDEXING_ALLOWED",
                    "lastCrawlTime": "2026-03-30T10:15:30Z",
                    "pageFetchState": "SUCCESSFUL",
                    "crawledAs": "GOOGLEBOT_DESKTOP",
                    "userCanonical": "https://example.com/page",
                    "googleCanonical": "https://example.com/page",
                    "referringUrls": [
                        "https://example.com/ref1",
                        "https://example.com/ref2"
                    ]
                }
            }
        }"#;

        let result = parse_inspect_response(json_data).expect("should parse");
        assert!(result.ok);
        assert!(!result.is_quota_exceeded);
        assert_eq!(result.verdict.as_deref(), Some("NEUTRAL"));
        assert_eq!(result.coverage_state.as_deref(), Some("Crawled - currently not indexed"));
        assert_eq!(result.robots_txt_state.as_deref(), Some("ALLOWED"));
        assert_eq!(result.indexing_state.as_deref(), Some("INDEXING_ALLOWED"));
        assert_eq!(result.last_crawl_time.as_deref(), Some("2026-03-30T10:15:30Z"));
        assert_eq!(result.page_fetch_state.as_deref(), Some("SUCCESSFUL"));
        assert_eq!(result.crawled_as.as_deref(), Some("GOOGLEBOT_DESKTOP"));
        assert_eq!(result.user_canonical.as_deref(), Some("https://example.com/page"));
        assert_eq!(result.google_canonical.as_deref(), Some("https://example.com/page"));
        assert_eq!(
            result.referring_urls,
            vec!["https://example.com/ref1", "https://example.com/ref2"]
        );
    }

    #[test]
    fn test_parse_inspect_response_minimal() {
        let json_data = r#"{ "inspectionResult": {} }"#;
        let result = parse_inspect_response(json_data).expect("should parse");
        assert!(result.ok);
        assert_eq!(result.coverage_state, None);
        assert_eq!(result.referring_urls.len(), 0);
    }
}