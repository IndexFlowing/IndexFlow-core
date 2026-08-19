use std::env;
use tracing::info;

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub server_host: String,
    pub server_port: u16,
    pub database_url: String,
    pub db_max_connections: u32,

    /// Scheduler tick interval (seconds). Default 60.
    pub scheduler_interval_secs: u64,
    pub scheduler_batch_size: i64,

    /// Worker poll intervals
    pub worker_poll_interval_secs: u64,
    /// Submit worker interval (smooth rate limiting for IndexNow). Default 5s.
    pub submit_worker_interval_secs: u64,

    pub sync_worker_batch: i64,
    pub submit_worker_batch: i64,

    pub max_task_retries: i32,

    /// Google Indexing API daily quota (UTC day). Default 200.
    pub google_daily_quota: u32,

    /// JWT signing secret for admin sessions.
    pub jwt_secret: String,
    /// JWT expiry hours. Default 168 (7 days).
    pub jwt_expiry_hours: i64,
}

impl AppConfig {
    pub fn from_env() -> Self {
        dotenvy::dotenv().ok();

        let config = Self {
            server_host: env::var("SERVER_HOST").unwrap_or_else(|_| "0.0.0.0".into()),
            server_port: env::var("SERVER_PORT")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(8080),
            database_url: env::var("DATABASE_URL")
                .expect("DATABASE_URL must be set"),
            db_max_connections: env::var("DB_MAX_CONNECTIONS")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(10),
            scheduler_interval_secs: env_u64("SCHEDULER_INTERVAL_SECS", 60),
            scheduler_batch_size: env_i64("SCHEDULER_BATCH_SIZE", 200),
            worker_poll_interval_secs: env_u64("WORKER_POLL_INTERVAL_SECS", 2),
            submit_worker_interval_secs: env_u64("SUBMIT_WORKER_INTERVAL_SECS", 5),
            sync_worker_batch: env_i64("SYNC_WORKER_BATCH", 2),
            submit_worker_batch: env_i64("SUBMIT_WORKER_BATCH", 10),
            max_task_retries: env_i64("MAX_TASK_RETRIES", 5) as i32,
            google_daily_quota: env_u64("GOOGLE_DAILY_QUOTA", 200) as u32,
            jwt_secret: env::var("JWT_SECRET")
                .or_else(|_| env::var("API_SECRET_KEY"))
                .unwrap_or_else(|_| "indexflow-dev-secret-change-me".into()),
            jwt_expiry_hours: env_i64("JWT_EXPIRY_HOURS", 168),
        };

        info!(
            host = %config.server_host,
            port = config.server_port,
            "config loaded"
        );
        config
    }
}

fn env_u64(key: &str, default: u64) -> u64 {
    env::var(key)
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(default)
}

fn env_i64(key: &str, default: i64) -> i64 {
    env::var(key)
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(default)
}
