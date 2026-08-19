use super::{SearchProvider, SubmissionResult};
use async_trait::async_trait;
use serde::Serialize;
use tracing::{error, info};

#[derive(Debug, Serialize)]
struct IndexNowPayload {
    pub host: String,
    pub key: String,
    #[serde(rename = "keyLocation")]
    pub key_location: String,
    #[serde(rename = "urlList")]
    pub url_list: Vec<String>,
}

#[derive(Clone)]
pub struct BingProvider {
    client: reqwest::Client,
}

impl BingProvider {
    pub fn new(client: reqwest::Client) -> Self {
        Self { client }
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

        info!(
            domain = %domain,
            count = urls.len(),
            "submitting batch to IndexNow"
        );

        let payload = IndexNowPayload {
            host: domain.to_string(),
            key: key.to_string(),
            key_location: format!("https://{}/{}.txt", domain, key),
            url_list: urls.to_vec(),
        };

        let response = self
            .client
            .post("https://api.indexnow.org/indexnow")
            .json(&payload)
            .send()
            .await;

        match response {
            Ok(res) => {
                let status_code = res.status().as_u16();
                let is_success = res.status().is_success();
                let response_text = res.text().await.unwrap_or_default();

                if !is_success {
                    error!(
                        status = status_code,
                        body = %response_text,
                        "IndexNow submit failed"
                    );
                }

                Ok(urls
                    .iter()
                    .map(|url| SubmissionResult {
                        url: url.clone(),
                        is_success,
                        status_code: Some(status_code),
                        response_msg: Some(response_text.clone()),
                        is_quota_exceeded: status_code == 429,
                    })
                    .collect())
            }
            Err(e) => {
                error!(error = %e, "IndexNow network error");
                Err(anyhow::anyhow!("IndexNow request failed: {e}"))
            }
        }
    }
}
