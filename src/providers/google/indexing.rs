use super::oauth::GoogleAuthClient;
use crate::providers::SubmissionResult;
use tracing::{error, warn};

#[derive(Clone)]
pub struct GoogleIndexingClient {
    http: reqwest::Client,
    auth: GoogleAuthClient,
}

impl GoogleIndexingClient {
    pub fn new(http: reqwest::Client, auth: GoogleAuthClient) -> Self {
        Self { http, auth }
    }

    pub async fn submit_batch(
        &self,
        domain: &str,
        service_account_json: &str,
        urls: &[String],
    ) -> anyhow::Result<Vec<SubmissionResult>> {
        if urls.is_empty() {
            return Ok(vec![]);
        }

        let token = match self
            .auth
            .get_access_token(
                service_account_json,
                "https://www.googleapis.com/auth/indexing",
            )
            .await
        {
            Ok(t) => t,
            Err(e) => {
                error!(error = %e, "Google OAuth2 failed");
                return Ok(urls
                    .iter()
                    .map(|u| SubmissionResult::failure(u.clone(), Some(401), e.to_string()))
                    .collect());
            }
        };

        let mut results = Vec::with_capacity(urls.len());

        for url in urls {
            let payload = serde_json::json!({
                "url": url,
                "type": "URL_UPDATED"
            });

            let res = self
                .http
                .post("https://indexing.googleapis.com/v3/urlNotifications:publish")
                .bearer_auth(&token)
                .json(&payload)
                .send()
                .await;

            match res {
                Ok(resp) => {
                    let status = resp.status().as_u16();
                    let is_quota = status == 429;
                    let body = resp.text().await.unwrap_or_default();
                    let is_success = status == 200;

                    results.push(SubmissionResult {
                        url: url.clone(),
                        is_success,
                        status_code: Some(status),
                        response_msg: Some(body),
                        is_quota_exceeded: is_quota,
                    });

                    if is_quota {
                        warn!(domain, "Google Indexing API 触发 429 配额限制");
                        break;
                    }
                }
                Err(e) => {
                    results.push(SubmissionResult::failure(
                        url.clone(),
                        None,
                        e.to_string(),
                    ));
                }
            }
        }

        Ok(results)
    }
}