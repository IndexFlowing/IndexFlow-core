use super::{SearchProvider, SubmissionResult};
use async_trait::async_trait;
use chrono::Utc;
use jsonwebtoken::{encode, Algorithm, EncodingKey, Header};
use serde::{Deserialize, Serialize};
use tracing::{error, info, warn};

#[derive(Clone)]
pub struct GoogleProvider {
    client: reqwest::Client,
}

#[derive(Debug, Deserialize)]
struct ServiceAccountKey {
    pub client_email: String,
    pub private_key: String,
    #[serde(default = "default_token_uri")]
    pub token_uri: String,
}

fn default_token_uri() -> String {
    "https://oauth2.googleapis.com/token".to_string()
}

#[derive(Debug, Serialize)]
struct GoogleJwtClaims {
    pub iss: String,
    pub scope: String,
    pub aud: String,
    pub exp: i64,
    pub iat: i64,
}

#[derive(Debug, Deserialize)]
struct OAuth2TokenResponse {
    pub access_token: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GscInspectResult {
    pub ok: bool,
    pub coverage_state: Option<String>,
    pub last_crawl_time: Option<String>,
    pub raw_response: Option<String>,
}

impl GoogleProvider {
    pub fn new(client: reqwest::Client) -> Self {
        Self { client }
    }

    /// 使用 Service Account JSON 签名换取 Google Bearer Token
    pub async fn get_access_token(&self, sa_json: &str, scopes: &str) -> anyhow::Result<String> {
        let sa: ServiceAccountKey = serde_json::from_str(sa_json)
            .map_err(|e| anyhow::anyhow!("Invalid Service Account JSON: {e}"))?;

        let now = Utc::now().timestamp();
        let claims = GoogleJwtClaims {
            iss: sa.client_email.clone(),
            scope: scopes.to_string(),
            aud: sa.token_uri.clone(),
            exp: now + 3600,
            iat: now,
        };

        let mut header = Header::new(Algorithm::RS256);
        header.typ = Some("JWT".to_string());

        let key = EncodingKey::from_rsa_pem(sa.private_key.as_bytes())
            .map_err(|e| anyhow::anyhow!("Failed to parse private_key: {e}"))?;

        let assertion = encode(&header, &claims, &key)
            .map_err(|e| anyhow::anyhow!("JWT sign failed: {e}"))?;

        let res = self
            .client
            .post(&sa.token_uri)
            .form(&[
                ("grant_type", "urn:ietf:params:oauth:grant-type:jwt-bearer"),
                ("assertion", &assertion),
            ])
            .send()
            .await?;

        if !res.status().is_success() {
            let body = res.text().await.unwrap_or_default();
            anyhow::bail!("Google OAuth2 failed: {body}");
        }

        let token_resp: OAuth2TokenResponse = res.json().await?;
        Ok(token_resp.access_token)
    }

    /// 自动解析 GSC Property 资源标识
    pub async fn resolve_gsc_property(
        &self,
        service_account_json: &str,
        domain: &str,
    ) -> anyhow::Result<String> {
        let token = self
            .get_access_token(
                service_account_json,
                "https://www.googleapis.com/auth/webmasters.readonly",
            )
            .await?;

        let res = self
            .client
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
            .client
            .post("https://searchconsole.googleapis.com/v1/urlInspection/index:inspect")
            .bearer_auth(&token)
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
                "GSC Inspection API returned non-200"
            );
            return Ok(GscInspectResult {
                ok: false,
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
            coverage_state,
            last_crawl_time,
            raw_response: Some(body_text),
        })
    }
}

#[async_trait]
impl SearchProvider for GoogleProvider {
    fn name(&self) -> &'static str {
        "google"
    }

    async fn submit_batch(
        &self,
        domain: &str,
        service_account_json: &str,
        urls: &[String],
    ) -> anyhow::Result<Vec<SubmissionResult>> {
        if urls.is_empty() {
            return Ok(vec![]);
        }

        let token = match self
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
                .client
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