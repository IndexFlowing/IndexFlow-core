use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePool, SqlitePoolOptions, SqliteSynchronous};
use std::str::FromStr;
use std::time::Duration;
use tracing::info;

pub async fn connect(database_url: &str, max_connections: u32) -> anyhow::Result<SqlitePool> {
    // 自动配置 SQLite WAL 模式与并发超时
    let options = SqliteConnectOptions::from_str(database_url)?
        .create_if_missing(true)
        .journal_mode(SqliteJournalMode::Wal)
        .synchronous(SqliteSynchronous::Normal)
        .busy_timeout(Duration::from_secs(5));

    let pool = SqlitePoolOptions::new()
        .max_connections(max_connections)
        .acquire_timeout(Duration::from_secs(5))
        .connect_with(options)
        .await?;

    info!("SQLite database connected with WAL mode (max_connections={})", max_connections);
    Ok(pool)
}

pub async fn migrate(pool: &SqlitePool) -> anyhow::Result<()> {
    info!("applying SQLite database migrations...");
    let started = std::time::Instant::now();
    sqlx::migrate!("./migrations").run(pool).await?;
    info!(elapsed_ms = started.elapsed().as_millis() as u64, "SQLite migrations applied");
    Ok(())
}