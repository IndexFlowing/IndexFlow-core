use crate::config::AppConfig;
use crate::infrastructure::AdminRepo;
use chrono::{Duration, Utc};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Claims {
    pub sub: i64,
    pub username: String,
    pub exp: i64,
    pub iat: i64,
}

#[derive(Clone)]
pub struct AuthService {
    admins: AdminRepo,
    jwt_secret: String,
    jwt_expiry_hours: i64,
}

#[derive(Debug, Serialize)]
pub struct AuthStatus {
    pub setup_required: bool,
    pub authenticated: bool,
    pub username: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct AuthTokenResponse {
    pub token: String,
    pub username: String,
    pub expires_at: String,
}

impl AuthService {
    pub fn new(admins: AdminRepo, config: &AppConfig) -> Self {
        Self {
            admins,
            jwt_secret: config.jwt_secret.clone(),
            jwt_expiry_hours: config.jwt_expiry_hours,
        }
    }

    pub async fn status(&self, bearer: Option<&str>) -> anyhow::Result<AuthStatus> {
        let count = self.admins.count().await?;
        let setup_required = count == 0;
        if let Some(token) = bearer {
            if let Ok(claims) = self.verify_token(token) {
                return Ok(AuthStatus {
                    setup_required,
                    authenticated: true,
                    username: Some(claims.username),
                });
            }
        }
        Ok(AuthStatus {
            setup_required,
            authenticated: false,
            username: None,
        })
    }

    pub async fn setup(&self, username: &str, password: &str) -> anyhow::Result<AuthTokenResponse> {
        let username = username.trim();
        if username.len() < 3 {
            anyhow::bail!("Username must be at least 3 characters long");
        }
        if password.len() < 6 {
            anyhow::bail!("Password must be at least 6 characters long");
        }
        if self.admins.count().await? > 0 {
            anyhow::bail!("Admin user already initialized; please log in");
        }

        let hash = bcrypt::hash(password, bcrypt::DEFAULT_COST)
            .map_err(|e| anyhow::anyhow!("password hash failed: {e}"))?;
        let admin = self.admins.create(username, &hash).await?;
        self.issue_token(admin.id, &admin.username)
    }

    pub async fn login(&self, username: &str, password: &str) -> anyhow::Result<AuthTokenResponse> {
        if self.admins.count().await? == 0 {
            anyhow::bail!("Admin user has not been initialized; please complete setup first");
        }
        let admin = self
            .admins
            .find_by_username(username.trim())
            .await?
            .ok_or_else(|| anyhow::anyhow!("Invalid username or password"))?;

        let ok = bcrypt::verify(password, &admin.password_hash)
            .map_err(|e| anyhow::anyhow!("password verify failed: {e}"))?;
        if !ok {
            anyhow::bail!("Invalid username or password");
        }
        self.issue_token(admin.id, &admin.username)
    }

    pub fn verify_token(&self, token: &str) -> anyhow::Result<Claims> {
        let token = token.trim();
        let token = token
            .strip_prefix("Bearer ")
            .or_else(|| token.strip_prefix("bearer "))
            .unwrap_or(token);

        let data = decode::<Claims>(
            token,
            &DecodingKey::from_secret(self.jwt_secret.as_bytes()),
            &Validation::default(),
        )
        .map_err(|_| anyhow::anyhow!("invalid or expired token"))?;
        Ok(data.claims)
    }

    fn issue_token(&self, user_id: i64, username: &str) -> anyhow::Result<AuthTokenResponse> {
        let now = Utc::now();
        let exp = now + Duration::hours(self.jwt_expiry_hours);
        let claims = Claims {
            sub: user_id,
            username: username.to_string(),
            iat: now.timestamp(),
            exp: exp.timestamp(),
        };
        let token = encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(self.jwt_secret.as_bytes()),
        )?;
        Ok(AuthTokenResponse {
            token,
            username: username.to_string(),
            expires_at: exp.to_rfc3339(),
        })
    }
}
