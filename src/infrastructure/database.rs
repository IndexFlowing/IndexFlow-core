use sqlx::postgres::{PgPool, PgPoolOptions};
use std::time::Duration;
use tracing::info;

pub async fn connect(database_url: &str, max_connections: u32) -> anyhow::Result<PgPool> {
    let pool = PgPoolOptions::new()
        .max_connections(max_connections)
        .acquire_timeout(Duration::from_secs(8))
        .idle_timeout(Duration::from_secs(60))
        .connect(database_url)
        .await?;

    info!("database connected (max_connections={})", max_connections);
    Ok(pool)
}

pub async fn migrate(pool: &PgPool) -> anyhow::Result<()> {
    info!("applying database migrations (first run on a large URL table can take a minute)...");
    let started = std::time::Instant::now();
    sqlx::migrate!("./migrations").run(pool).await?;
    info!(elapsed_ms = started.elapsed().as_millis() as u64, "database migrations applied");
    Ok(())
}
