use super::{SearchProvider, SubmissionResult};
use async_trait::async_trait;
use chrono::Utc;
use jsonwebtoken::{encode, Algorithm, EncodingKey, Header};
use serde::{Deserialize, Serialize};
use tracing::{error, info, warn};

#[derive(Debug, Deserialize)]
struct ServiceAccount {
    pub client_email: String,
    pub private_key: String,
    pub token_uri: String,
}

#[derive(Debug, Serialize)]
struct JwtClaims {
    pub iss: String,
    pub scope: String,
    pub aud: String,
    pub exp: i64,
    pub iat: i64,
}

#[derive(Debug, Deserialize)]
struct TokenResponse {
    pub access_token: String,
}

#[derive(Debug, Serialize)]
struct IndexingBody {
    pub url: String,
    #[serde(rename = "type")]
    pub notify_type: String,
}

#[derive(Clone)]
pub struct GoogleProvider {
    client: reqwest::Client,
}

impl GoogleProvider {
    pub fn new(client: reqwest::Client) -> Self {
        Self { client }
    }

    async fn get_access_token(&self, service_account_json: &str) -> anyhow::Result<String> {
        let sa: ServiceAccount = serde_json::from_str(service_account_json)
            .map_err(|e| anyhow::anyhow!("invalid service account JSON: {e}"))?;

        let now = Utc::now().timestamp();
        let claims = JwtClaims {
            iss: sa.client_email.clone(),
            scope: "https://www.googleapis.com/auth/indexing".to_string(),
            aud: sa.token_uri.clone(),
            iat: now,
            exp: now + 3600,
        };

        let header = Header::new(Algorithm::RS256);
        let encoding_key = EncodingKey::from_rsa_pem(sa.private_key.as_bytes())
            .map_err(|e| anyhow::anyhow!("invalid RSA private key: {e}"))?;
        let jwt_token = encode(&header, &claims, &encoding_key)?;

        let res = self
            .client
            .post(&sa.token_uri)
            .form(&[
                ("grant_type", "urn:ietf:params:oauth:grant-type:jwt-bearer"),
                ("assertion", jwt_token.as_str()),
            ])
            .send()
            .await?;

        if !res.status().is_success() {
            let err_text = res.text().await.unwrap_or_default();
            error!(body = %err_text, "failed to exchange Google access token");
            anyhow::bail!("Google token exchange failed");
        }

        let token_res: TokenResponse = res.json().await?;
        Ok(token_res.access_token)
    }
}

fn looks_like_quota(status_code: u16, body: &str) -> bool {
    if status_code == 429 {
        return true;
    }
    let lower = body.to_lowercase();
    lower.contains("quota")
        || lower.contains("rate limit")
        || lower.contains("resource_exhausted")
        || lower.contains("userexceeded")
}

#[async_trait]
impl SearchProvider for GoogleProvider {
    fn name(&self) -> &'static str {
        "google"
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
            "submitting batch to Google Indexing API"
        );

        let access_token = self.get_access_token(key).await?;
        let api_url = "https://indexing.googleapis.com/v3/urlNotifications:publish";
        let mut results = Vec::with_capacity(urls.len());

        for url in urls {
            let payload = IndexingBody {
                url: url.clone(),
                notify_type: "URL_UPDATED".to_string(),
            };

            match self
                .client
                .post(api_url)
                .bearer_auth(&access_token)
                .json(&payload)
                .send()
                .await
            {
                Ok(response) => {
                    let status_code = response.status().as_u16();
                    let response_text = response.text().await.unwrap_or_default();
                    let is_success = (200..300).contains(&status_code);
                    let quota = !is_success && looks_like_quota(status_code, &response_text);

                    if quota {
                        warn!(
                            url = %url,
                            status = status_code,
                            "Google quota exceeded — stopping remaining submits in this batch"
                        );
                        results.push(SubmissionResult {
                            url: url.clone(),
                            is_success: false,
                            status_code: Some(status_code),
                            response_msg: Some(response_text),
                            is_quota_exceeded: true,
                        });
                        // Do not continue burning quota on remaining URLs
                        break;
                    } else if !is_success {
                        error!(
                            url = %url,
                            status = status_code,
                            body = %response_text,
                            "Google submit failed"
                        );
                        results.push(SubmissionResult {
                            url: url.clone(),
                            is_success: false,
                            status_code: Some(status_code),
                            response_msg: Some(response_text),
                            is_quota_exceeded: false,
                        });
                    } else {
                        results.push(SubmissionResult {
                            url: url.clone(),
                            is_success: true,
                            status_code: Some(status_code),
                            response_msg: Some(response_text),
                            is_quota_exceeded: false,
                        });
                    }
                }
                Err(e) => {
                    error!(url = %url, error = %e, "Google network error");
                    results.push(SubmissionResult::failure(url.clone(), None, e.to_string()));
                }
            }
        }

        Ok(results)
    }
}
