use super::{SearchProvider, SubmissionResult};
use async_trait::async_trait;
use chrono::{DateTime, TimeZone, Utc};
use serde::{Deserialize, Serialize};
use tracing::{info, warn};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BingInspectResult {
    pub ok: bool,
    pub is_indexed: bool,
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
}

impl BingProvider {
    pub fn new(client: reqwest::Client, dry_run: bool) -> Self {
        Self { client, dry_run }
    }

    /// 查询单条 URL 在 Bing 索引库的真实收录与抓取状态 (不受 dry_run 影响，始终允许检测)
    pub async fn inspect_url(
        &self,
        bwt_api_key: &str,
        site_url: &str,
        inspection_url: &str,
    ) -> anyhow::Result<BingInspectResult> {
        let endpoint = format!(
            "https://ssl.bing.com/webmaster/api.json/GetUrlInspection?apikey={}",
            bwt_api_key.trim()
        );

        let payload = serde_json::json!({
            "siteUrl": site_url,
            "url": inspection_url
        });

        let res = self
            .client
            .post(&endpoint)
            .json(&payload)
            .send()
            .await?;

        let status = res.status();
        let body_text = res.text().await.unwrap_or_default();

        if !status.is_success() {
            warn!(
                url = %inspection_url,
                status = %status,
                body = %body_text,
                "Bing Webmaster URL Inspection returned non-200"
            );
            return Ok(BingInspectResult {
                ok: false,
                is_indexed: false,
                index_status: "UNKNOWN".into(),
                coverage_state: Some(format!("HTTP Error {}", status.as_u16())),
                last_crawl_time: None,
                http_status: None,
                raw_response: Some(body_text),
            });
        }

        let parsed: serde_json::Value = serde_json::from_str(&body_text)?;
        let result_obj = parsed.get("d").unwrap_or(&parsed);

        let index_status_raw = result_obj
            .get("IndexingStatus")
            .and_then(|s| s.as_str())
            .unwrap_or("UNKNOWN");

        let is_indexed = index_status_raw.eq_ignore_ascii_case("Indexed");
        let index_status = if is_indexed {
            "INDEXED".to_string()
        } else if index_status_raw.eq_ignore_ascii_case("UNKNOWN") {
            "UNKNOWN".to_string()
        } else {
            "NOT_INDEXED".to_string()
        };

        let last_crawl_time = result_obj
            .get("LastCrawlDate")
            .and_then(|d| d.as_str())
            .and_then(parse_ms_date);

        let http_status = result_obj
            .get("HttpStatus")
            .and_then(|s| s.as_i64())
            .map(|s| s as i32);

        let coverage_state = format!(
            "Status: {}, RobotsTxt: {}, NoIndex: {}",
            index_status_raw,
            result_obj.get("RobotsTxtStatus").and_then(|s| s.as_str()).unwrap_or("-"),
            result_obj.get("NoIndexStatus").and_then(|s| s.as_str()).unwrap_or("-")
        );

        Ok(BingInspectResult {
            ok: true,
            is_indexed,
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

        // ==========================================
        // 1. Dry-Run 演练模式分支
        // ==========================================
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

        // ==========================================
        // 2. Live 生产真实推送分支 (IndexNow 官方协议)
        // ==========================================
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
                // IndexNow 规范: 200 (OK) 与 202 (Accepted) 均为成功状态
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