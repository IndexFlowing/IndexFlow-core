use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, SqlitePool};

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Site {
    pub id: i64,
    pub domain: String,
    pub sitemap_url: Option<String>,
    pub bing_indexnow_key: Option<String>,
    pub bing_webmaster_api_key: Option<String>, // 核心字段
    pub google_service_account_json: Option<String>,
    pub gsc_property_url: Option<String>,
    pub gsc_daily_quota: i64,
    pub google_daily_quota: i64,
    pub google_quota_paused_until: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Site {
    pub fn has_bing_credentials(&self) -> bool {
        self.bing_indexnow_key
            .as_ref()
            .map(|k| !k.trim().is_empty())
            .unwrap_or(false)
    }

    pub fn has_bing_webmaster_key(&self) -> bool {
        self.bing_webmaster_api_key
            .as_ref()
            .map(|k| !k.trim().is_empty())
            .unwrap_or(false)
    }

    pub fn has_google_credentials(&self) -> bool {
        self.google_service_account_json
            .as_deref()
            .map(|k| !k.trim().is_empty())
            .unwrap_or(false)
    }

    pub fn bing_ready(&self) -> bool {
        self.has_bing_credentials()
    }

    pub fn google_ready(&self) -> bool {
        self.has_google_credentials() && !self.google_quota_paused()
    }

    pub fn google_quota_paused(&self) -> bool {
        self.google_quota_paused_until
            .map(|until| until > Utc::now())
            .unwrap_or(false)
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

    pub async fn list_all(&self) -> anyhow::Result<Vec<Site>> {
        let sites = sqlx::query_as::<_, Site>(r#"SELECT * FROM sites ORDER BY id ASC"#)
            .fetch_all(&self.pool)
            .await?;
        Ok(sites)
    }

    pub async fn find_by_id(&self, id: i64) -> anyhow::Result<Option<Site>> {
        let site = sqlx::query_as::<_, Site>(r#"SELECT * FROM sites WHERE id = $1"#)
            .bind(id)
            .fetch_optional(&self.pool)
            .await?;
        Ok(site)
    }

    pub async fn get_default(&self) -> anyhow::Result<Option<Site>> {
        let site = sqlx::query_as::<_, Site>(r#"SELECT * FROM sites ORDER BY id ASC LIMIT 1"#)
            .fetch_optional(&self.pool)
            .await?;
        Ok(site)
    }

    pub async fn save_or_update(
        &self,
        id: Option<i64>,
        domain: &str,
        sitemap_url: Option<&str>,
        bing_indexnow_key: Option<&str>,
        bing_webmaster_api_key: Option<&str>,
        google_service_account_json: Option<&str>,
    ) -> anyhow::Result<Site> {
        let site = if let Some(site_id) = id {
            sqlx::query_as::<_, Site>(
                r#"
                UPDATE sites
                SET
                    domain = $1,
                    sitemap_url = $2,
                    bing_indexnow_key = $3,
                    bing_webmaster_api_key = $4,
                    google_service_account_json = $5,
                    updated_at = CURRENT_TIMESTAMP
                WHERE id = $6
                RETURNING *
                "#,
            )
            .bind(domain)
            .bind(sitemap_url)
            .bind(bing_indexnow_key)
            .bind(bing_webmaster_api_key)
            .bind(google_service_account_json)
            .bind(site_id)
            .fetch_one(&self.pool)
            .await?
        } else {
            sqlx::query_as::<_, Site>(
                r#"
                INSERT INTO sites (
                    domain, sitemap_url, bing_indexnow_key, bing_webmaster_api_key, google_service_account_json, updated_at
                )
                VALUES ($1, $2, $3, $4, $5, CURRENT_TIMESTAMP)
                RETURNING *
                "#,
            )
            .bind(domain)
            .bind(sitemap_url)
            .bind(bing_indexnow_key)
            .bind(bing_webmaster_api_key)
            .bind(google_service_account_json)
            .fetch_one(&self.pool)
            .await?
        };

        Ok(site)
    }

    pub async fn delete_site(&self, id: i64) -> anyhow::Result<()> {
        let mut tx = self.pool.begin().await?;
        sqlx::query(r#"DELETE FROM urls WHERE site_id = $1"#).bind(id).execute(&mut *tx).await?;
        sqlx::query(r#"DELETE FROM sites WHERE id = $1"#).bind(id).execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(())
    }

    pub async fn set_google_quota_paused_until(
        &self,
        id: i64,
        until: DateTime<Utc>,
    ) -> anyhow::Result<()> {
        sqlx::query(
            r#"
            UPDATE sites
            SET google_quota_paused_until = $1, updated_at = CURRENT_TIMESTAMP
            WHERE id = $2
            "#,
        )
        .bind(until)
        .bind(id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn set_gsc_property(&self, id: i64, property_url: &str) -> anyhow::Result<()> {
        sqlx::query(
            r#"
            UPDATE sites
            SET gsc_property_url = $1, updated_at = CURRENT_TIMESTAMP
            WHERE id = $2
            "#,
        )
        .bind(property_url)
        .bind(id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }
}