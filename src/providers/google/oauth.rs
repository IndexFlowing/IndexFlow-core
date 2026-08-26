use chrono::Utc;
use jsonwebtoken::{encode, Algorithm, EncodingKey, Header};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::debug;

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

#[derive(Debug, Clone)]
struct CachedToken {
    pub token: String,
    pub expires_at: i64,
}

/// 带内存缓存的 Google OAuth2 认证器
#[derive(Clone)]
pub struct GoogleAuthClient {
    http: reqwest::Client,
    cache: Arc<RwLock<HashMap<String, CachedToken>>>,
}

impl GoogleAuthClient {
    pub fn new(http: reqwest::Client) -> Self {
        Self {
            http,
            cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// 获取 Access Token（优先命中内存缓存，有效期剩余 < 5 分钟时自动刷新）
    pub async fn get_access_token(&self, sa_json: &str, scopes: &str) -> anyhow::Result<String> {
        let cache_key = format!("{scopes}:{}", crate::domain::hash_url(sa_json));
        let now = Utc::now().timestamp();

        // 1. 读锁检查缓存
        {
            let reader = self.cache.read().await;
            if let Some(cached) = reader.get(&cache_key) {
                if cached.expires_at > now + 300 {
                    debug!(scopes, "命中 Google OAuth2 Token 内存缓存");
                    return Ok(cached.token.clone());
                }
            }
        }

        // 2. 缓存未命中或即将过期，重新签名并换取 Token
        let sa: ServiceAccountKey = serde_json::from_str(sa_json)
            .map_err(|e| anyhow::anyhow!("Invalid Service Account JSON: {e}"))?;

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
            .http
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
        let token = token_resp.access_token;

        // 3. 写锁写入缓存（保存 55 分钟）
        {
            let mut writer = self.cache.write().await;
            writer.insert(
                cache_key,
                CachedToken {
                    token: token.clone(),
                    expires_at: now + 3300,
                },
            );
        }

        Ok(token)
    }
}