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
        self.get_access_token_scoped(
            service_account_json,
            "https://www.googleapis.com/auth/indexing",
        )
        .await
    }

    async fn get_access_token_scoped(
        &self,
        service_account_json: &str,
        scope: &str,
    ) -> anyhow::Result<String> {
        let sa: ServiceAccount = serde_json::from_str(service_account_json)
            .map_err(|e| anyhow::anyhow!("invalid service account JSON: {e}"))?;

        let now = Utc::now().timestamp();
        let claims = JwtClaims {
            iss: sa.client_email.clone(),
            scope: scope.to_string(),
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

    async fn webmaster_token(&self, service_account_json: &str) -> anyhow::Result<String> {
        self.get_access_token_scoped(
            service_account_json,
            "https://www.googleapis.com/auth/webmasters.readonly https://www.googleapis.com/auth/webmasters",
        )
        .await
    }

    /// List Search Console properties visible to this service account.
    pub async fn list_gsc_sites(
        &self,
        service_account_json: &str,
    ) -> anyhow::Result<Vec<GscSiteEntry>> {
        let token = self.webmaster_token(service_account_json).await?;
        let res = self
            .client
            .get("https://www.googleapis.com/webmasters/v3/sites")
            .bearer_auth(&token)
            .send()
            .await?;
        let status = res.status();
        let text = res.text().await.unwrap_or_default();
        if !status.is_success() {
            anyhow::bail!("GSC sites.list failed ({status}): {text}");
        }
        let parsed: GscSitesResponse = serde_json::from_str(&text).unwrap_or_default();
        Ok(parsed.site_entry)
    }

    /// Resolve the Search Console property URL for a site domain.
    pub async fn resolve_gsc_property(
        &self,
        service_account_json: &str,
        domain: &str,
    ) -> anyhow::Result<String> {
        let entries = self.list_gsc_sites(service_account_json).await?;
        if entries.is_empty() {
            anyhow::bail!(
                "Service account has no Search Console properties. Add the SA email as a user (Full) on the GSC property for {domain}."
            );
        }
        pick_gsc_property(domain, &entries).ok_or_else(|| {
            let listed: Vec<&str> = entries.iter().map(|e| e.site_url.as_str()).collect();
            anyhow::anyhow!(
                "No Search Console property matches `{domain}`. Visible properties: {}. Add the service account as a user on the matching property.",
                listed.join(", ")
            )
        })
    }

    /// Harvest pages with impressions > 0 via Search Analytics (paginated).
    pub async fn search_analytics_pages(
        &self,
        service_account_json: &str,
        site_url: &str,
        start_date: &str,
        end_date: &str,
    ) -> anyhow::Result<Vec<String>> {
        let token = self.webmaster_token(service_account_json).await?;
        let encoded = encode_gsc_site_url(site_url);
        let endpoint = format!(
            "https://www.googleapis.com/webmasters/v3/sites/{encoded}/searchAnalytics/query"
        );

        let mut pages = Vec::new();
        let mut start_row: i32 = 0;
        const ROW_LIMIT: i32 = 25000;

        loop {
            let body = SearchAnalyticsRequest {
                start_date: start_date.to_string(),
                end_date: end_date.to_string(),
                dimensions: vec!["page".into()],
                row_limit: ROW_LIMIT,
                start_row,
            };
            let res = self
                .client
                .post(&endpoint)
                .bearer_auth(&token)
                .json(&body)
                .send()
                .await?;
            let status = res.status();
            let text = res.text().await.unwrap_or_default();
            if !status.is_success() {
                anyhow::bail!("GSC searchAnalytics.query failed ({status}): {text}");
            }
            let parsed: SearchAnalyticsResponse =
                serde_json::from_str(&text).unwrap_or_default();
            let n = parsed.rows.len() as i32;
            for row in parsed.rows {
                if row.impressions > 0.0 {
                    if let Some(page) = row.keys.into_iter().next() {
                        if !page.is_empty() {
                            pages.push(page);
                        }
                    }
                }
            }
            if n < ROW_LIMIT {
                break;
            }
            start_row += n;
            if start_row > 1_000_000 {
                break;
            }
        }

        pages.sort();
        pages.dedup();
        Ok(pages)
    }

    /// Inspect a single URL via the GSC URL Inspection API.
    pub async fn inspect_url(
        &self,
        service_account_json: &str,
        site_url: &str,
        inspection_url: &str,
    ) -> anyhow::Result<GscInspectResult> {
        let token = self.webmaster_token(service_account_json).await?;
        let payload = InspectRequest {
            inspection_url: inspection_url.to_string(),
            site_url: site_url.to_string(),
        };
        let res = self
            .client
            .post("https://searchconsole.googleapis.com/v1/urlInspection/index:inspect")
            .bearer_auth(&token)
            .json(&payload)
            .send()
            .await?;
        let status = res.status().as_u16();
        let text = res.text().await.unwrap_or_default();
        if !(200..300).contains(&status) {
            return Ok(GscInspectResult {
                ok: false,
                status_code: status,
                raw: text,
                coverage_state: None,
                last_crawl_time: None,
                verdict: None,
            });
        }
        let parsed: InspectResponse = serde_json::from_str(&text).unwrap_or_default();
        let idx = parsed
            .inspection_result
            .and_then(|r| r.index_status_result);
        Ok(GscInspectResult {
            ok: true,
            status_code: status,
            raw: text,
            coverage_state: idx.as_ref().and_then(|i| i.coverage_state.clone()),
            last_crawl_time: idx.as_ref().and_then(|i| i.last_crawl_time.clone()),
            verdict: idx.as_ref().and_then(|i| i.verdict.clone()),
        })
    }
}

fn encode_gsc_site_url(site_url: &str) -> String {
    site_url
        .replace('%', "%25")
        .replace(':', "%3A")
        .replace('/', "%2F")
}

/// Prefer `sc-domain:` exact match, then https://domain/, then www / http variants.
pub fn pick_gsc_property(domain: &str, entries: &[GscSiteEntry]) -> Option<String> {
    let domain = domain.trim().trim_end_matches('/').to_ascii_lowercase();
    let sc_domain = format!("sc-domain:{domain}");
    if let Some(e) = entries
        .iter()
        .find(|e| e.site_url.eq_ignore_ascii_case(&sc_domain))
    {
        return Some(e.site_url.clone());
    }
    let candidates = [
        format!("https://{domain}/"),
        format!("https://{domain}"),
        format!("http://{domain}/"),
        format!("http://{domain}"),
        format!("https://www.{domain}/"),
        format!("http://www.{domain}/"),
    ];
    for c in &candidates {
        if let Some(e) = entries
            .iter()
            .find(|e| e.site_url.eq_ignore_ascii_case(c))
        {
            return Some(e.site_url.clone());
        }
    }
    // Suffix match: property host equals domain (strip scheme + path).
    entries.iter().find_map(|e| {
        let raw = e.site_url.to_ascii_lowercase();
        if raw.starts_with("sc-domain:") {
            return (raw.trim_start_matches("sc-domain:") == domain).then(|| e.site_url.clone());
        }
        let host = raw
            .trim_start_matches("https://")
            .trim_start_matches("http://")
            .trim_start_matches("www.")
            .split('/')
            .next()
            .unwrap_or("");
        (host == domain).then(|| e.site_url.clone())
    })
}

#[derive(Debug, Default, Deserialize)]
struct GscSitesResponse {
    #[serde(rename = "siteEntry", default)]
    site_entry: Vec<GscSiteEntry>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GscSiteEntry {
    #[serde(rename = "siteUrl")]
    pub site_url: String,
    #[serde(rename = "permissionLevel")]
    #[allow(dead_code)]
    pub permission_level: Option<String>,
}

#[derive(Debug, Serialize)]
struct SearchAnalyticsRequest {
    #[serde(rename = "startDate")]
    start_date: String,
    #[serde(rename = "endDate")]
    end_date: String,
    dimensions: Vec<String>,
    #[serde(rename = "rowLimit")]
    row_limit: i32,
    #[serde(rename = "startRow")]
    start_row: i32,
}

#[derive(Debug, Default, Deserialize)]
struct SearchAnalyticsResponse {
    #[serde(default)]
    rows: Vec<SearchAnalyticsRow>,
}

#[derive(Debug, Deserialize)]
struct SearchAnalyticsRow {
    #[serde(default)]
    keys: Vec<String>,
    #[serde(default)]
    impressions: f64,
}

#[derive(Debug, Serialize)]
struct InspectRequest {
    #[serde(rename = "inspectionUrl")]
    inspection_url: String,
    #[serde(rename = "siteUrl")]
    site_url: String,
}

#[derive(Debug, Default, Deserialize)]
struct InspectResponse {
    #[serde(rename = "inspectionResult")]
    inspection_result: Option<InspectionResult>,
}

#[derive(Debug, Default, Deserialize)]
struct InspectionResult {
    #[serde(rename = "indexStatusResult")]
    index_status_result: Option<IndexStatusResult>,
}

#[derive(Debug, Default, Deserialize)]
struct IndexStatusResult {
    #[serde(rename = "coverageState")]
    coverage_state: Option<String>,
    #[serde(rename = "lastCrawlTime")]
    last_crawl_time: Option<String>,
    verdict: Option<String>,
}

#[derive(Debug, Clone)]
pub struct GscInspectResult {
    pub ok: bool,
    pub status_code: u16,
    pub raw: String,
    pub coverage_state: Option<String>,
    pub last_crawl_time: Option<String>,
    #[allow(dead_code)]
    pub verdict: Option<String>,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn picks_sc_domain_over_url_prefix() {
        let entries = vec![
            GscSiteEntry {
                site_url: "https://example.com/".into(),
                permission_level: None,
            },
            GscSiteEntry {
                site_url: "sc-domain:example.com".into(),
                permission_level: None,
            },
        ];
        assert_eq!(
            pick_gsc_property("example.com", &entries).as_deref(),
            Some("sc-domain:example.com")
        );
    }

    #[test]
    fn picks_https_prefix_when_no_sc_domain() {
        let entries = vec![GscSiteEntry {
            site_url: "https://example.com/".into(),
            permission_level: None,
        }];
        assert_eq!(
            pick_gsc_property("example.com", &entries).as_deref(),
            Some("https://example.com/")
        );
    }
}
