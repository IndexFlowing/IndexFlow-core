mod discovery;
mod mutations;
mod queries;
mod tasks;

use sqlx::SqlitePool;

#[derive(Clone)]
pub struct UrlRepo {
    pub(super) pool: SqlitePool,
}

impl UrlRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}