use crate::evaluator::evaluate_html;
use crate::models::SeoAuditResult;
use std::time::{Duration, Instant};

#[derive(Clone)]
pub struct SeoProbeClient {
    client: reqwest::Client,
}

impl SeoProbeClient {
    pub fn new(user_agent: &str, timeout: Duration) -> Result<Self, reqwest::Error> {
        let client = reqwest::Client::builder()
            .timeout(timeout)
            .user_agent(user_agent)
            .redirect(reqwest::redirect::Policy::none()) // 不跟随重定向，将 3xx 视为拦截诊断项
            .build()?;
        Ok(Self { client })
    }

    pub async fn check_url(&self, url: &str) -> SeoAuditResult {
        let start = Instant::now();

        let response = match self.client.get(url).send().await {
            Ok(r) => r,
            Err(e) => {
                return SeoAuditResult {
                    http_status: None,
                    response_time_ms: Some(start.elapsed().as_millis() as i32),
                    passed: false,
                    block_reason: Some(format!("request failed: {e}")),
                    ..SeoAuditResult::default()
                };
            }
        };

        let status_code = response.status().as_u16() as i32;
        let x_robots = response
            .headers()
            .get("x-robots-tag")
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string());
        let body = response.text().await.unwrap_or_default();
        let elapsed = start.elapsed().as_millis() as i32;

        evaluate_html(url, status_code, elapsed, x_robots.as_deref(), &body)
    }
}