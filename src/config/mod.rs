use serde::Deserialize;
use std::env;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub server_host: String,
    pub server_port: u16,
    pub database_url: String,
    pub db_max_connections: u32,

    pub log_level: String,
    pub log_raw_payload: bool,

    pub worker_poll_interval_secs: u64,
    pub submit_worker_interval_secs: u64,
    pub submit_worker_batch: i64,
    pub jwt_secret: String,
    pub dry_run: bool,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            server_host: "0.0.0.0".into(),
            server_port: 8010,
            database_url: "sqlite:indexflow.db?mode=rwc".into(),
            db_max_connections: 5,
            log_level: "debug".into(),
            log_raw_payload: true,
            worker_poll_interval_secs: 2,
            submit_worker_interval_secs: 5,
            submit_worker_batch: 10,
            jwt_secret: "indexflow-secret-key-change-in-production".into(),
            dry_run: false,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Default)]
struct TomlConfig {
    server: Option<ServerConfig>,
    database: Option<DatabaseConfig>,
    logging: Option<LoggingConfig>,
    pipeline: Option<PipelineConfig>,
}

#[derive(Debug, Clone, Deserialize, Default)]
struct ServerConfig {
    host: Option<String>,
    port: Option<u16>,
    jwt_secret: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
struct DatabaseConfig {
    url: Option<String>,
    max_connections: Option<u32>,
}

#[derive(Debug, Clone, Deserialize, Default)]
struct LoggingConfig {
    level: Option<String>,
    log_raw_payload: Option<bool>,
}

#[derive(Debug, Clone, Deserialize, Default)]
struct PipelineConfig {
    dry_run: Option<bool>,
    worker_poll_interval_secs: Option<u64>,
    submit_worker_interval_secs: Option<u64>,
    submit_worker_batch: Option<i64>,
}

impl AppConfig {
    /// 优先从 config.toml 读取配置，若不存在则使用内建默认值，并支持环境变量覆写
    pub fn load() -> Self {
        dotenvy::dotenv().ok();

        let config_path = env::var("INDEXFLOW_CONFIG").unwrap_or_else(|_| "config.toml".into());
        let base = if Path::new(&config_path).exists() {
            match fs::read_to_string(&config_path) {
                Ok(content) => Self::from_toml_str(&content).unwrap_or_else(|e| {
                    eprintln!("⚠️ [Config] 解析 {config_path} 失败，使用默认配置: {e}");
                    Self::default()
                }),
                Err(e) => {
                    eprintln!("⚠️ [Config] 读取 {config_path} 失败，使用默认配置: {e}");
                    Self::default()
                }
            }
        } else {
            Self::default()
        };

        base.apply_env_overrides()
    }

    #[allow(dead_code)]
    pub fn from_env() -> Self {
        Self::load()
    }

    /// 从纯 TOML 字符串解析配置
    pub fn from_toml_str(content: &str) -> anyhow::Result<Self> {
        let toml_conf: TomlConfig = toml::from_str(content)?;
        let mut cfg = Self::default();

        if let Some(s) = toml_conf.server {
            if let Some(h) = s.host {
                cfg.server_host = h;
            }
            if let Some(p) = s.port {
                cfg.server_port = p;
            }
            if let Some(j) = s.jwt_secret {
                cfg.jwt_secret = j;
            }
        }
        if let Some(d) = toml_conf.database {
            if let Some(u) = d.url {
                cfg.database_url = u;
            }
            if let Some(m) = d.max_connections {
                cfg.db_max_connections = m;
            }
        }
        if let Some(l) = toml_conf.logging {
            if let Some(lvl) = l.level {
                cfg.log_level = lvl;
            }
            if let Some(raw) = l.log_raw_payload {
                cfg.log_raw_payload = raw;
            }
        }
        if let Some(p) = toml_conf.pipeline {
            if let Some(dry) = p.dry_run {
                cfg.dry_run = dry;
            }
            if let Some(w) = p.worker_poll_interval_secs {
                cfg.worker_poll_interval_secs = w;
            }
            if let Some(s) = p.submit_worker_interval_secs {
                cfg.submit_worker_interval_secs = s;
            }
            if let Some(b) = p.submit_worker_batch {
                cfg.submit_worker_batch = b;
            }
        }

        Ok(cfg)
    }

    /// 允许通过环境变量覆写配置字段 (方便容器化/CI 部署)
    pub fn apply_env_overrides(mut self) -> Self {
        if let Ok(v) = env::var("SERVER_HOST") {
            if !v.trim().is_empty() {
                self.server_host = v;
            }
        }
        if let Ok(v) = env::var("SERVER_PORT").and_then(|s| s.parse().map_err(|_| env::VarError::NotPresent)) {
            self.server_port = v;
        }
        if let Ok(v) = env::var("JWT_SECRET") {
            if !v.trim().is_empty() {
                self.jwt_secret = v;
            }
        }
        if let Ok(v) = env::var("DATABASE_URL") {
            if !v.trim().is_empty() {
                self.database_url = v;
            }
        }
        if let Ok(v) = env::var("DB_MAX_CONNECTIONS").and_then(|s| s.parse().map_err(|_| env::VarError::NotPresent)) {
            self.db_max_connections = v;
        }
        if let Ok(v) = env::var("LOG_LEVEL") {
            if !v.trim().is_empty() {
                self.log_level = v;
            }
        }
        if let Ok(v) = env::var("LOG_RAW_PAYLOAD") {
            self.log_raw_payload = v.eq_ignore_ascii_case("true") || v == "1";
        }
        if let Ok(v) = env::var("DRY_RUN") {
            self.dry_run = v.eq_ignore_ascii_case("true") || v == "1";
        }
        if let Ok(v) = env::var("WORKER_POLL_INTERVAL_SECS").and_then(|s| s.parse().map_err(|_| env::VarError::NotPresent)) {
            self.worker_poll_interval_secs = v;
        }
        if let Ok(v) = env::var("SUBMIT_WORKER_INTERVAL_SECS").and_then(|s| s.parse().map_err(|_| env::VarError::NotPresent)) {
            self.submit_worker_interval_secs = v;
        }
        if let Ok(v) = env::var("SUBMIT_WORKER_BATCH").and_then(|s| s.parse().map_err(|_| env::VarError::NotPresent)) {
            self.submit_worker_batch = v;
        }
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_toml_config() {
        let toml_str = r#"
[server]
host = "127.0.0.1"
port = 9000
jwt_secret = "test-secret"

[database]
url = "sqlite::memory:"
max_connections = 10

[logging]
level = "trace"
log_raw_payload = false

[pipeline]
dry_run = true
worker_poll_interval_secs = 4
submit_worker_interval_secs = 8
submit_worker_batch = 20
"#;

        let config = AppConfig::from_toml_str(toml_str).expect("should parse toml");

        assert_eq!(config.server_host, "127.0.0.1");
        assert_eq!(config.server_port, 9000);
        assert_eq!(config.jwt_secret, "test-secret");
        assert_eq!(config.database_url, "sqlite::memory:");
        assert_eq!(config.db_max_connections, 10);
        assert_eq!(config.log_level, "trace");
        assert!(!config.log_raw_payload);
        assert!(config.dry_run);
        assert_eq!(config.worker_poll_interval_secs, 4);
        assert_eq!(config.submit_worker_interval_secs, 8);
        assert_eq!(config.submit_worker_batch, 20);
    }

    #[test]
    fn test_load_default_config_file() {
        let config = AppConfig::load();
        assert_eq!(config.server_port, 8010);
        assert_eq!(config.log_level, "debug");
        assert!(config.log_raw_payload);
        assert!(!config.dry_run);
    }
}
