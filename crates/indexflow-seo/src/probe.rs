// crates/indexflow-seo/src/probe.rs
use crate::evaluator::evaluate_html;
use crate::models::SeoAuditResult;
use std::time::{Duration, Instant};

pub const MAX_HTML_BODY: usize = 5 * 1024 * 1024;

#[derive(Clone)]
pub struct SeoProbeClient {
    client: reqwest::Client,
}

impl SeoProbeClient {
    pub fn new(user_agent: &str, timeout: Duration) -> Result<Self, reqwest::Error> {
        let client = reqwest::Client::builder()
            .timeout(timeout)
            .user_agent(user_agent)
            .redirect(reqwest::redirect::Policy::none()) // 绝对不跟随重定向，暴露出第一跳真实状态
            .build()?;
        Ok(Self { client })
    }

    pub async fn check_url(&self, url: &str) -> SeoAuditResult {
        let start = Instant::now();

        let mut response = match self.client.get(url).send().await {
            Ok(r) => r,
            Err(e) => {
                return SeoAuditResult {
                    http_status: None,
                    response_time_ms: Some(millis_i32(start.elapsed())),
                    passed: false,
                    block_reason: Some(format!("request failed: {e}")),
                    ..SeoAuditResult::default()
                };
            }
        };

        let status_code = response.status().as_u16() as i32;

        // 提取核心网络标头
        let x_robots = join_header_values(response.headers(), "x-robots-tag");
        let location = response
            .headers()
            .get(reqwest::header::LOCATION)
            .and_then(|v| v.to_str().ok())
            .map(str::to_string);
        let link_header = join_header_values(response.headers(), "link");

        if let Some(len) = response.content_length() {
            if len > MAX_HTML_BODY as u64 * 4 {
                let elapsed = millis_i32(start.elapsed());
                return evaluate_html(
                    url,
                    status_code,
                    elapsed,
                    x_robots.as_deref(),
                    location.as_deref(),
                    link_header.as_deref(),
                    "",
                );
            }
        }

        let body = read_body_capped(&mut response).await;
        let elapsed = millis_i32(start.elapsed());

        evaluate_html(
            url,
            status_code,
            elapsed,
            x_robots.as_deref(),
            location.as_deref(),
            link_header.as_deref(),
            &body,
        )
    }
}

fn join_header_values(headers: &reqwest::header::HeaderMap, name: &str) -> Option<String> {
    let mut parts = Vec::new();
    for value in headers.get_all(name) {
        if let Ok(s) = value.to_str() {
            let s = s.trim();
            if !s.is_empty() {
                parts.push(s.to_string());
            }
        }
    }
    if parts.is_empty() {
        None
    } else {
        Some(parts.join(", "))
    }
}

async fn read_body_capped(response: &mut reqwest::Response) -> String {
    let mut buf = Vec::new();
    loop {
        match response.chunk().await {
            Ok(Some(chunk)) => {
                let remaining = MAX_HTML_BODY.saturating_sub(buf.len());
                if remaining == 0 {
                    break;
                }
                if chunk.len() > remaining {
                    buf.extend_from_slice(&chunk[..remaining]);
                    break;
                }
                buf.extend_from_slice(&chunk);
            }
            Ok(None) => break,
            Err(_) => break,
        }
    }
    bytes_to_utf8_clipped(&buf)
}

fn bytes_to_utf8_clipped(buf: &[u8]) -> String {
    match std::str::from_utf8(buf) {
        Ok(s) => s.to_string(),
        Err(e) => {
            let valid = e.valid_up_to();
            match std::str::from_utf8(&buf[..valid]) {
                Ok(s) => s.to_string(),
                Err(_) => String::from_utf8_lossy(buf).into_owned(),
            }
        }
    }
}

fn millis_i32(d: Duration) -> i32 {
    i32::try_from(d.as_millis()).unwrap_or(i32::MAX)
}