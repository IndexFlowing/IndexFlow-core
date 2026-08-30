use super::{SearchProvider, SubmissionResult};
use async_trait::async_trait;
use chrono::{DateTime, TimeZone, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, warn};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BingInspectResult {
    pub ok: bool,
    pub is_indexed: bool,
    pub is_throttled: bool, // 核心新增：是否触发频控
    pub index_status: String,
    pub coverage_state: Option<String>,
    pub last_crawl_time: Option<DateTime<Utc>>,
    pub http_status: Option<i32>,
    pub raw_response: Option<String>,
}

#[derive(Clone)]
pub struct BingProvider {
    client: reqwest::Client,
    dry_run: bool,
    site_cache: Arc<RwLock<HashMap<String, String>>>, // 核心新增：站点 URL 内存缓存
}

impl BingProvider {
    pub fn new(client: reqwest::Client, dry_run: bool) -> Self {
        Self {
            client,
            dry_run,
            site_cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn test_api_key(&self, key: &str, _domain: &str) -> anyhow::Result<Vec<String>> {
        let endpoint = format!(
            "https://ssl.bing.com/webmaster/api.svc/json/GetUserSites?apikey={}",
            key.trim()
        );
        let response = self.client.get(&endpoint).send().await?;
        let status = response.status();
        let body = response.text().await?;
        if !status.is_success() {
            anyhow::bail!("Bing API 返回 HTTP {}: {}", status.as_u16(), body);
        }
        let parsed: serde_json::Value = serde_json::from_str(&body)
            .map_err(|error| anyhow::anyhow!("解析 Bing API 响应失败: {error}"))?;
        let sites = parsed.get("d").and_then(serde_json::Value::as_array)
            .ok_or_else(|| anyhow::anyhow!("Bing API 响应缺少 d 数组"))?;
        sites.iter().map(|site| {
            site.get("Url").and_then(serde_json::Value::as_str).map(str::to_owned)
                .ok_or_else(|| anyhow::anyhow!("Bing API 响应中的站点缺少 Url 字段"))
        }).collect()
    }

    /// 从内存缓存中获取已解析的 Bing 站点前缀，未命中时才调用 GetUserSites (单站仅查 1 次)
    pub async fn resolve_site_url(&self, bwt_api_key: &str, domain: &str) -> anyhow::Result<String> {
        let cache_key = format!("{}:{}", bwt_api_key.trim(), domain.trim());

        // 1. 读锁检查缓存
        {
            let reader = self.site_cache.read().await;
            if let Some(cached) = reader.get(&cache_key) {
                return Ok(cached.clone());
            }
        }

        // 2. 缓存未命中时向 Bing 查询
        let endpoint = format!(
            "https://ssl.bing.com/webmaster/api.svc/json/GetUserSites?apikey={}",
            bwt_api_key.trim()
        );

        let mut resolved_url = None;
        if let Ok(res) = self.client.get(&endpoint).send().await {
            if res.status().is_success() {
                if let Ok(body) = res.json::<serde_json::Value>().await {
                    if let Some(sites) = body.get("d").and_then(|d| d.as_array()) {
                        let clean_target = domain.trim().trim_start_matches("www.").trim_end_matches('/');
                        for site in sites {
                            if let Some(url_str) = site.get("Url").and_then(|u| u.as_str()) {
                                if url_str.contains(clean_target) {
                                    info!(site_url = %url_str, "✅ [Bing Cache] 成功匹配并缓存 Bing 官方站点前缀");
                                    resolved_url = Some(url_str.to_string());
                                    break;
                                }
                            }
                        }
                    }
                }
            }
        }

        let final_url = resolved_url.unwrap_or_else(|| {
            if domain.starts_with("http://") || domain.starts_with("https://") {
                format!("{}/", domain.trim_end_matches('/'))
            } else {
                format!("https://{}/", domain.trim_start_matches("www.").trim_end_matches('/'))
            }
        });

        // 3. 写锁写入缓存
        {
            let mut writer = self.site_cache.write().await;
            writer.insert(cache_key, final_url.clone());
        }

        Ok(final_url)
    }

    /// 使用 Bing 官方 GetUrlInfo 接口查询收录
    pub async fn inspect_url(
        &self,
        bwt_api_key: &str,
        site_url: &str,
        inspection_url: &str,
    ) -> anyhow::Result<BingInspectResult> {
        let endpoint = "https://ssl.bing.com/webmaster/api.svc/json/GetUrlInfo";

        let params = [
            ("siteUrl", site_url.trim()),
            ("url", inspection_url.trim()),
            ("apikey", bwt_api_key.trim()),
        ];

        let res = match self.client.get(endpoint).query(&params).send().await {
            Ok(r) => r,
            Err(e) => {
                warn!(url = %inspection_url, error = %e, "Bing GetUrlInfo 网络请求失败");
                return Ok(BingInspectResult {
                    ok: false,
                    is_indexed: false,
                    is_throttled: false,
                    index_status: "FAILED".into(),
                    coverage_state: Some(format!("网络请求失败: {e}")),
                    last_crawl_time: None,
                    http_status: None,
                    raw_response: None,
                });
            }
        };

        let status = res.status();
        let body_text = res.text().await.unwrap_or_default();

        // 捕获 Bing ErrorCode:5 / ThrottleHost 频控，退避而非永久失败
        let is_throttled = body_text.contains("ThrottleHost")
            || body_text.contains("\"ErrorCode\":5")
            || status.as_u16() == 429;
        if is_throttled {
            warn!(url = %inspection_url, "⏳ Bing API 触发频率限制 (ThrottleHost)，将自动平滑退避");
            return Ok(BingInspectResult {
                ok: false,
                is_indexed: false,
                is_throttled: true,
                index_status: "UNKNOWN".into(), // 保留在待测队列供退避后重试
                coverage_state: Some("Bing 频控保护 (自动排队中)".into()),
                last_crawl_time: None,
                http_status: Some(status.as_u16() as i32),
                raw_response: Some(body_text),
            });
        }

        // 官方库无此 URL 时按未收录处理，避免把 404 当成检测异常
        if status.as_u16() == 404 {
            return Ok(BingInspectResult {
                ok: true,
                is_indexed: false,
                is_throttled: false,
                index_status: "NOT_INDEXED".into(),
                coverage_state: Some("Bing 索引库暂无此页面抓取记录".into()),
                last_crawl_time: None,
                http_status: Some(404),
                raw_response: Some(body_text),
            });
        }

        if !status.is_success() {
            warn!(
                url = %inspection_url,
                status = %status,
                body = %body_text,
                "⚠️ Bing API 返回非 200 响应"
            );
            return Ok(BingInspectResult {
                ok: false,
                is_indexed: false,
                is_throttled: false,
                index_status: "FAILED".into(),
                coverage_state: Some(format!("Bing API 报错 (HTTP {}): {}", status.as_u16(), body_text)),
                last_crawl_time: None,
                http_status: Some(status.as_u16() as i32),
                raw_response: Some(body_text),
            });
        }

        let parsed: serde_json::Value = match serde_json::from_str(&body_text) {
            Ok(v) => v,
            Err(_) => {
                return Ok(BingInspectResult {
                    ok: false,
                    is_indexed: false,
                    is_throttled: false,
                    index_status: "FAILED".into(),
                    coverage_state: Some(format!("解析 Bing 返回内容失败: {body_text}")),
                    last_crawl_time: None,
                    http_status: Some(200),
                    raw_response: Some(body_text),
                });
            }
        };

        let obj = parsed.get("d");

        if obj.is_none() || obj.unwrap().is_null() {
            return Ok(BingInspectResult {
                ok: true,
                is_indexed: false,
                is_throttled: false,
                index_status: "NOT_INDEXED".to_string(),
                coverage_state: Some("Bing 索引库暂无此页面抓取记录".into()),
                last_crawl_time: None,
                http_status: None,
                raw_response: Some(body_text),
            });
        }

        let data = obj.unwrap();

        let last_crawl_time = data
            .get("LastCrawledDate")
            .and_then(|d| d.as_str())
            .and_then(parse_ms_date);

        let is_page = data.get("IsPage").and_then(|p| p.as_bool()).unwrap_or(true);
        let http_status = data.get("HttpStatus").and_then(|s| s.as_i64()).map(|s| s as i32);

        // 核心裁决：只要有最后抓取时间，即为已收录！
        let is_indexed = last_crawl_time.is_some();
        let index_status = if is_indexed {
            "INDEXED".to_string()
        } else {
            "NOT_INDEXED".to_string()
        };

        let coverage_state = if let Some(t) = last_crawl_time {
            format!("Bingbot 已抓取 ({})", t.format("%Y-%m-%d %H:%M"))
        } else {
            "Bing 尚未抓取该页面".to_string()
        };

        info!(
            url = %inspection_url,
            index_status = %index_status,
            is_page,
            "✅ [Bing GetUrlInfo] 成功获取 Bing 收录状态"
        );

        Ok(BingInspectResult {
            ok: true,
            is_indexed,
            is_throttled: false,
            index_status,
            coverage_state: Some(coverage_state),
            last_crawl_time,
            http_status,
            raw_response: Some(body_text),
        })
    }
}

#[async_trait]
impl SearchProvider for BingProvider {
    fn name(&self) -> &'static str {
        "bing"
    }

    async fn submit_batch(
        &self,
        domain: &str,
        key: &str,
        urls: &[String],
    ) -> anyhow::Result<Vec<SubmissionResult>> {
        if urls.is_empty() {
            return Ok(vec![]);
        }

        if self.dry_run {
            info!(
                mode = "DRY_RUN (演练模式)",
                domain = %domain,
                count = urls.len(),
                "🧪【Bing IndexNow 模拟提交】拦截真实请求，记录日志"
            );
            return Ok(urls
                .iter()
                .map(|url| SubmissionResult {
                    url: url.clone(),
                    is_success: true,
                    status_code: Some(200),
                    response_msg: Some("DRY_RUN_MOCK: 已演练并记录日志，未向 IndexNow 发起网络请求".into()),
                    is_quota_exceeded: false,
                })
                .collect());
        }

        let clean_domain = domain
            .trim()
            .trim_start_matches("https://")
            .trim_start_matches("http://")
            .trim_end_matches('/');

        let payload = serde_json::json!({
            "host": clean_domain,
            "key": key.trim(),
            "urlList": urls
        });

        info!(
            domain = %clean_domain,
            count = urls.len(),
            "🚀 [Bing IndexNow] 正在向官方 API 发送真实批量推送..."
        );

        let res = self
            .client
            .post("https://api.indexnow.org/indexnow")
            .json(&payload)
            .send()
            .await;

        match res {
            Ok(resp) => {
                let status = resp.status().as_u16();
                let body = resp.text().await.unwrap_or_default();
                let is_success = status == 200 || status == 202;
                let is_quota = status == 429;

                if !is_success {
                    warn!(status, body = %body, "Bing IndexNow API returned non-success");
                }

                Ok(urls
                    .iter()
                    .map(|url| SubmissionResult {
                        url: url.clone(),
                        is_success,
                        status_code: Some(status),
                        response_msg: Some(body.clone()),
                        is_quota_exceeded: is_quota,
                    })
                    .collect())
            }
            Err(e) => {
                warn!(error = %e, "Bing IndexNow network error");
                Ok(urls
                    .iter()
                    .map(|url| SubmissionResult::failure(url.clone(), None, e.to_string()))
                    .collect())
            }
        }
    }
}

fn parse_ms_date(raw: &str) -> Option<DateTime<Utc>> {
    let start = raw.find('(')? + 1;
    let end = raw.find(')')?;
    let inner = &raw[start..end];
    let digits = inner.split(['+', '-']).next()?;
    let millis = digits.parse::<i64>().ok()?;
    Utc.timestamp_millis_opt(millis).single()
}
