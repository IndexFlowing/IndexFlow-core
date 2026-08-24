use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, SqlitePool};

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct SiteConfig {
    pub id: i64,
    pub domain: String,
    pub sitemap_url: Option<String>,
    pub bing_indexnow_key: Option<String>,
    pub google_service_account_json: Option<String>,
    pub gsc_property_url: Option<String>,
    pub gsc_daily_quota: i64,
    pub google_daily_quota: i64,
    pub google_quota_paused_until: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl SiteConfig {
    pub fn has_bing_credentials(&self) -> bool {
        self.bing_indexnow_key
            .as_ref()
            .map(|k| !k.trim().is_empty())
            .unwrap_or(false)
    }

    pub fn has_google_credentials(&self) -> bool {
        self.google_service_account_json
            .as_ref()
            .map(|k| !k.trim().is_empty())
            .unwrap_or(false)
    }

    pub fn bing_ready(&self) -> bool {
        self.has_bing_credentials()
    }

    pub fn google_verified(&self) -> bool {
        self.has_google_credentials()
    }

    pub fn google_ready(&self) -> bool {
        self.has_google_credentials() && !self.google_quota_paused()
    }

    pub fn google_quota_paused(&self) -> bool {
        self.google_quota_paused_until
            .map(|until| until > Utc::now())
            .unwrap_or(false)
    }

    pub fn has_any_credentials_filled(&self) -> bool {
        self.has_bing_credentials() || self.has_google_credentials()
    }
}

#[derive(Clone)]
pub struct SiteRepo {
    pool: SqlitePool,
}

impl SiteRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn get(&self) -> anyhow::Result<Option<SiteConfig>> {
        let site = sqlx::query_as::<_, SiteConfig>(r#"SELECT * FROM site_config WHERE id = 1"#)
            .fetch_optional(&self.pool)
            .await?;
        Ok(site)
    }

    pub async fn save_or_update(
        &self,
        domain: &str,
        sitemap_url: Option<&str>,
        bing_indexnow_key: Option<&str>,
        google_service_account_json: Option<&str>,
    ) -> anyhow::Result<SiteConfig> {
        let site = sqlx::query_as::<_, SiteConfig>(
            r#"
            INSERT INTO site_config (
                id, domain, sitemap_url, bing_indexnow_key, google_service_account_json, updated_at
            )
            VALUES (1, $1, $2, $3, $4, CURRENT_TIMESTAMP)
            ON CONFLICT(id) DO UPDATE SET
                domain = EXCLUDED.domain,
                sitemap_url = COALESCE(EXCLUDED.sitemap_url, site_config.sitemap_url),
                bing_indexnow_key = COALESCE(EXCLUDED.bing_indexnow_key, site_config.bing_indexnow_key),
                google_service_account_json = COALESCE(EXCLUDED.google_service_account_json, site_config.google_service_account_json),
                updated_at = CURRENT_TIMESTAMP
            RETURNING *
            "#,
        )
        .bind(domain)
        .bind(sitemap_url)
        .bind(bing_indexnow_key)
        .bind(google_service_account_json)
        .fetch_one(&self.pool)
        .await?;
        Ok(site)
    }

    pub async fn set_google_quota_paused_until(
        &self,
        until: DateTime<Utc>,
    ) -> anyhow::Result<()> {
        sqlx::query(
            r#"
            UPDATE site_config
            SET google_quota_paused_until = $1, updated_at = CURRENT_TIMESTAMP
            WHERE id = 1
            "#,
        )
        .bind(until)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn set_gsc_property(&self, property_url: &str) -> anyhow::Result<()> {
        sqlx::query(
            r#"
            UPDATE site_config
            SET gsc_property_url = $1, updated_at = CURRENT_TIMESTAMP
            WHERE id = 1
            "#,
        )
        .bind(property_url)
        .execute(&self.pool)
        .await?;
        Ok(())
    }
}