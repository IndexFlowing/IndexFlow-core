use std::env;
use tracing::info;

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub server_host: String,
    pub server_port: u16,
    pub database_url: String,
    pub db_max_connections: u32,

    pub worker_poll_interval_secs: u64,
    pub submit_worker_interval_secs: u64,
    pub submit_worker_batch: i64,
    pub jwt_secret: String,
    pub dry_run: bool, // 核心新增：演练模式开关
}

impl AppConfig {
    pub fn from_env() -> Self {
        dotenvy::dotenv().ok();

        let dry_run = env::var("DRY_RUN")
            .map(|v| v.eq_ignore_ascii_case("true") || v == "1")
            .unwrap_or(true); // 默认开启安全演练保护

        let config = Self {
            server_host: env::var("SERVER_HOST").unwrap_or_else(|_| "0.0.0.0".into()),
            server_port: env::var("SERVER_PORT")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(8080),
            database_url: env::var("DATABASE_URL")
                .unwrap_or_else(|_| "sqlite:indexflow.db?mode=rwc".into()),
            db_max_connections: env::var("DB_MAX_CONNECTIONS")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(5),
            worker_poll_interval_secs: env_u64("WORKER_POLL_INTERVAL_SECS", 2),
            submit_worker_interval_secs: env_u64("SUBMIT_WORKER_INTERVAL_SECS", 5),
            submit_worker_batch: env_i64("SUBMIT_WORKER_BATCH", 10),
            jwt_secret: env::var("JWT_SECRET")
                .unwrap_or_else(|_| "indexflow-secret-key-change-in-production".into()),
            dry_run,
        };

        info!(
            host = %config.server_host,
            port = config.server_port,
            db = %config.database_url,
            dry_run = config.dry_run,
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