use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, SqlitePool};

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct AdminUser {
    pub id: i64,
    pub username: String,
    pub password_hash: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone)]
pub struct AdminRepo {
    pool: SqlitePool,
}

impl AdminRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn count(&self) -> anyhow::Result<i64> {
        let row: (i64,) = sqlx::query_as(r#"SELECT COUNT(*) FROM admin_users"#)
            .fetch_one(&self.pool)
            .await?;
        Ok(row.0)
    }

    pub async fn find_by_username(&self, username: &str) -> anyhow::Result<Option<AdminUser>> {
        let row = sqlx::query_as::<_, AdminUser>(
            r#"SELECT * FROM admin_users WHERE username = $1"#,
        )
        .bind(username)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row)
    }

    pub async fn create(&self, username: &str, password_hash: &str) -> anyhow::Result<AdminUser> {
        let row = sqlx::query_as::<_, AdminUser>(
            r#"
            INSERT INTO admin_users (username, password_hash, created_at, updated_at)
            VALUES ($1, $2, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)
            RETURNING *
            "#,
        )
        .bind(username)
        .bind(password_hash)
        .fetch_one(&self.pool)
        .await?;
        Ok(row)
    }
}