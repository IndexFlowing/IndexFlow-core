use crate::domain::{ProviderCredentialStatus, Site, SiteStatus};
use chrono::{DateTime, Utc};
use sqlx::PgPool;

#[derive(Clone)]
pub struct SiteRepo {
    pool: PgPool,
}

impl SiteRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn create(
        &self,
        domain: &str,
        indexnow_key: Option<&str>,
        google_service_account_json: Option<&str>,
    ) -> anyhow::Result<Site> {
        let bing_filled = indexnow_key.map(str::trim).filter(|s| !s.is_empty()).is_some();
        let google_filled = google_service_account_json
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .is_some();
        let bing_status = ProviderCredentialStatus::from_filled(bing_filled);
        let google_status = ProviderCredentialStatus::from_filled(google_filled);

        let site = sqlx::query_as::<_, Site>(
            r#"
            INSERT INTO sites (
                domain, status,
                indexnow_key, google_service_account_json,
                indexnow_status, google_status
            )
            VALUES ($1, $2, $3, $4, $5, $6)
            RETURNING *
            "#,
        )
        .bind(domain)
        .bind(SiteStatus::Created.as_str())
        .bind(indexnow_key.map(str::trim).filter(|s| !s.is_empty()))
        .bind(
            google_service_account_json
                .map(str::trim)
                .filter(|s| !s.is_empty()),
        )
        .bind(bing_status.as_str())
        .bind(google_status.as_str())
        .fetch_one(&self.pool)
        .await?;
        Ok(site)
    }

    pub async fn find_by_id(&self, id: i64) -> anyhow::Result<Option<Site>> {
        let site = sqlx::query_as::<_, Site>(r#"SELECT * FROM sites WHERE id = $1"#)
            .bind(id)
            .fetch_optional(&self.pool)
            .await?;
        Ok(site)
    }

    pub async fn list_all(&self) -> anyhow::Result<Vec<Site>> {
        let sites = sqlx::query_as::<_, Site>(
            r#"SELECT * FROM sites ORDER BY created_at DESC"#,
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(sites)
    }

    pub async fn update_status(&self, id: i64, status: SiteStatus) -> anyhow::Result<()> {
        sqlx::query(
            r#"
            UPDATE sites
            SET status = $1, updated_at = NOW()
            WHERE id = $2
            "#,
        )
        .bind(status.as_str())
        .bind(id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Update provider credentials and reset verify status to SAVED/UNSET.
    pub async fn update_credentials(
        &self,
        id: i64,
        update_indexnow: bool,
        indexnow_key: Option<&str>,
        update_google: bool,
        google_service_account_json: Option<&str>,
    ) -> anyhow::Result<Option<Site>> {
        let indexnow_val = indexnow_key.map(str::trim).filter(|s| !s.is_empty());
        let google_val = google_service_account_json
            .map(str::trim)
            .filter(|s| !s.is_empty());

        let bing_status = ProviderCredentialStatus::from_filled(indexnow_val.is_some()).as_str();
        let google_status = ProviderCredentialStatus::from_filled(google_val.is_some()).as_str();

        let site = sqlx::query_as::<_, Site>(
            r#"
            UPDATE sites
            SET
                indexnow_key = CASE WHEN $2 THEN $3 ELSE indexnow_key END,
                google_service_account_json = CASE WHEN $4 THEN $5 ELSE google_service_account_json END,
                indexnow_status = CASE
                    WHEN $2 THEN $6
                    ELSE indexnow_status
                END,
                indexnow_last_error = CASE WHEN $2 THEN NULL ELSE indexnow_last_error END,
                indexnow_verified_at = CASE WHEN $2 THEN NULL ELSE indexnow_verified_at END,
                google_status = CASE
                    WHEN $4 THEN $7
                    ELSE google_status
                END,
                google_last_error = CASE WHEN $4 THEN NULL ELSE google_last_error END,
                google_verified_at = CASE WHEN $4 THEN NULL ELSE google_verified_at END,
                updated_at = NOW()
            WHERE id = $1
            RETURNING *
            "#,
        )
        .bind(id)
        .bind(update_indexnow)
        .bind(indexnow_val)
        .bind(update_google)
        .bind(google_val)
        .bind(bing_status)
        .bind(google_status)
        .fetch_optional(&self.pool)
        .await?;
        Ok(site)
    }

    /// Pause Google submits until `until` (oldest in-window submit + 24h).
    pub async fn set_google_quota_paused_until(
        &self,
        id: i64,
        until: DateTime<Utc>,
        error: Option<&str>,
    ) -> anyhow::Result<()> {
        sqlx::query(
            r#"
            UPDATE sites
            SET
                google_quota_paused_until = $2,
                google_last_error = COALESCE($3, google_last_error),
                updated_at = NOW()
            WHERE id = $1
            "#,
        )
        .bind(id)
        .bind(until)
        .bind(error)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn clear_google_quota_pause(&self, id: i64) -> anyhow::Result<()> {
        sqlx::query(
            r#"
            UPDATE sites
            SET google_quota_paused_until = NULL, updated_at = NOW()
            WHERE id = $1
            "#,
        )
        .bind(id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Persist channel test result for Bing or Google.
    pub async fn set_provider_verify(
        &self,
        id: i64,
        provider: &str, // "bing" | "google"
        status: ProviderCredentialStatus,
        error: Option<&str>,
    ) -> anyhow::Result<Option<Site>> {
        let site = match provider {
            "bing" => {
                sqlx::query_as::<_, Site>(
                    r#"
                    UPDATE sites
                    SET
                        indexnow_status = $2,
                        indexnow_last_error = $3,
                        indexnow_verified_at = CASE WHEN $2 = 'VERIFIED' THEN NOW() ELSE indexnow_verified_at END,
                        updated_at = NOW()
                    WHERE id = $1
                    RETURNING *
                    "#,
                )
                .bind(id)
                .bind(status.as_str())
                .bind(error)
                .fetch_optional(&self.pool)
                .await?
            }
            "google" => {
                sqlx::query_as::<_, Site>(
                    r#"
                    UPDATE sites
                    SET
                        google_status = $2,
                        google_last_error = $3,
                        google_verified_at = CASE WHEN $2 = 'VERIFIED' THEN NOW() ELSE google_verified_at END,
                        updated_at = NOW()
                    WHERE id = $1
                    RETURNING *
                    "#,
                )
                .bind(id)
                .bind(status.as_str())
                .bind(error)
                .fetch_optional(&self.pool)
                .await?
            }
            _ => anyhow::bail!("unknown provider: {provider}"),
        };
        Ok(site)
    }
}
