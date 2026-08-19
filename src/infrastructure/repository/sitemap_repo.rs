use crate::domain::{Sitemap, SitemapStatus, SitemapType};
use chrono::{DateTime, Utc};
use sqlx::PgPool;

#[derive(Clone)]
pub struct SitemapRepo {
    pool: PgPool,
}

impl SitemapRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn create(
        &self,
        site_id: i64,
        url: &str,
        sitemap_type: SitemapType,
    ) -> anyhow::Result<Sitemap> {
        let sm = sqlx::query_as::<_, Sitemap>(
            r#"
            INSERT INTO sitemaps (site_id, url, type, status)
            VALUES ($1, $2, $3, $4)
            ON CONFLICT (site_id, url) DO UPDATE
                SET updated_at = NOW()
            RETURNING *
            "#,
        )
        .bind(site_id)
        .bind(url)
        .bind(sitemap_type.as_str())
        .bind(SitemapStatus::Active.as_str())
        .fetch_one(&self.pool)
        .await?;
        Ok(sm)
    }

    pub async fn find_by_id(&self, id: i64) -> anyhow::Result<Option<Sitemap>> {
        let sm = sqlx::query_as::<_, Sitemap>(r#"SELECT * FROM sitemaps WHERE id = $1"#)
            .bind(id)
            .fetch_optional(&self.pool)
            .await?;
        Ok(sm)
    }

    pub async fn list_by_site(&self, site_id: i64) -> anyhow::Result<Vec<Sitemap>> {
        let rows = sqlx::query_as::<_, Sitemap>(
            r#"SELECT * FROM sitemaps WHERE site_id = $1 ORDER BY id ASC"#,
        )
        .bind(site_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    pub async fn mark_synced(
        &self,
        id: i64,
        sitemap_type: SitemapType,
        status: SitemapStatus,
        last_error: Option<&str>,
    ) -> anyhow::Result<()> {
        sqlx::query(
            r#"
            UPDATE sitemaps
            SET
                type = $2,
                status = $3,
                last_sync_at = NOW(),
                last_error = $4,
                updated_at = NOW()
            WHERE id = $1
            "#,
        )
        .bind(id)
        .bind(sitemap_type.as_str())
        .bind(status.as_str())
        .bind(last_error)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn mark_failed(&self, id: i64, error: &str) -> anyhow::Result<()> {
        sqlx::query(
            r#"
            UPDATE sitemaps
            SET status = $2, last_error = $3, updated_at = NOW()
            WHERE id = $1
            "#,
        )
        .bind(id)
        .bind(SitemapStatus::Failed.as_str())
        .bind(error)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    #[allow(dead_code)]
    pub async fn update_last_sync(&self, id: i64, at: DateTime<Utc>) -> anyhow::Result<()> {
        sqlx::query(
            r#"UPDATE sitemaps SET last_sync_at = $2, updated_at = NOW() WHERE id = $1"#,
        )
        .bind(id)
        .bind(at)
        .execute(&self.pool)
        .await?;
        Ok(())
    }
}
