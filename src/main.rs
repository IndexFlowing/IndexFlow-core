mod api;
mod application;
mod config;
mod domain;
mod infrastructure;
mod providers;
mod scheduler;
mod workers;

use crate::api::handlers::AppState;
use crate::api::routes::build_router;
use crate::application::{
    AuthService, GscService, HealthService, SiteService, SitemapService, SubmissionService,
    TaskService, UrlService,
};
use crate::config::AppConfig;
use crate::infrastructure::{
    build_http_client, connect, migrate, AdminRepo, HealthCheckRepo, SiteRepo, SitemapRepo,
    SubmissionLogRepo, TaskRepo, UrlRepo,
};
use crate::providers::{bing::BingProvider, google::GoogleProvider};
use crate::scheduler::Scheduler;
use crate::workers::{
    BingSubmitWorker, GoogleSubmitWorker, GscInspectWorker, SeoAuditWorker, SyncWorker,
};
use std::sync::Arc;
use tracing::info;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG")
                .unwrap_or_else(|_| "indexflow_core=debug,tower_http=info".into()),
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();

    info!("IndexFlow Core starting...");

    let config = AppConfig::from_env();
    let pool = connect(&config.database_url, config.db_max_connections).await?;
    migrate(&pool).await?;

    let http = build_http_client()?;

    // Repositories
    let site_repo = SiteRepo::new(pool.clone());
    let sitemap_repo = SitemapRepo::new(pool.clone());
    let url_repo = UrlRepo::new(pool.clone());
    let task_repo = TaskRepo::new(pool.clone());
    let health_repo = HealthCheckRepo::new(pool.clone());
    let submission_log_repo = SubmissionLogRepo::new(pool.clone());
    let admin_repo = AdminRepo::new(pool.clone());

    let bing_provider = BingProvider::new(http.clone());
    let google_provider = GoogleProvider::new(http.clone());

    let auth_service = Arc::new(AuthService::new(admin_repo, &config));

    // Application services
    let site_service = Arc::new(SiteService::new(
        site_repo.clone(),
        sitemap_repo.clone(),
        task_repo.clone(),
        url_repo.clone(),
        submission_log_repo.clone(),
        config.clone(),
        bing_provider.clone(),
        google_provider.clone(),
    ));
    let sitemap_service = SitemapService::new(
        http.clone(),
        sitemap_repo.clone(),
        site_repo.clone(),
        task_repo.clone(),
    );
    let health_service = HealthService::new(http.clone())?;
    let submission_service =
        SubmissionService::new(bing_provider.clone(), google_provider.clone());
    let url_service = Arc::new(UrlService::new(
        url_repo.clone(),
        health_repo.clone(),
        submission_log_repo.clone(),
        site_repo.clone(),
        health_service.clone(),
        submission_service.clone(),
        config.clone(),
    ));
    let task_service = Arc::new(TaskService::new(task_repo.clone()));
    let gsc_service = GscService::new(
        google_provider.clone(),
        site_repo.clone(),
        url_repo.clone(),
        task_repo.clone(),
        config.clone(),
    );

    // Scheduler
    let scheduler = Arc::new(Scheduler::new(
        url_repo.clone(),
        task_repo.clone(),
        site_repo.clone(),
        config.clone(),
    ));
    scheduler.start();

    // Workers
    Arc::new(SyncWorker::new(
        task_repo.clone(),
        sitemap_repo.clone(),
        url_repo.clone(),
        site_repo.clone(),
        sitemap_service.clone(),
        config.clone(),
    ))
    .start();

    // Bing pipeline: no quota, full-speed batch submit.
    Arc::new(BingSubmitWorker::new(
        task_repo.clone(),
        url_repo.clone(),
        site_repo.clone(),
        submission_log_repo.clone(),
        health_repo.clone(),
        health_service.clone(),
        submission_service.clone(),
        config.clone(),
    ))
    .start();

    // Google pipeline: rolling 24-hour quota circuit.
    Arc::new(GoogleSubmitWorker::new(
        task_repo.clone(),
        url_repo.clone(),
        site_repo.clone(),
        submission_log_repo.clone(),
        health_repo.clone(),
        health_service.clone(),
        submission_service,
        config.clone(),
    ))
    .start();

    // Module 2: standalone SEO quality scanner (does not enqueue submit work).
    Arc::new(SeoAuditWorker::new(
        task_repo.clone(),
        url_repo.clone(),
        health_repo.clone(),
        health_service,
        config.clone(),
    ))
    .start();

    // Module 4 layer 2: GSC URL Inspection API (2,000/day).
    Arc::new(GscInspectWorker::new(
        task_repo.clone(),
        url_repo.clone(),
        site_repo.clone(),
        gsc_service.clone(),
        config.clone(),
    ))
    .start();

    // HTTP API + static UI
    let state = AppState {
        site_service,
        sitemap_service: Arc::new(sitemap_service),
        url_service,
        task_service,
        auth_service,
        gsc_service: Arc::new(gsc_service),
    };

    let app = build_router(state);
    serve_http(app, &config.server_host, config.server_port).await
}

/// Bind IPv4 and IPv6 so both `127.0.0.1` and Windows `localhost` (::1) work.
async fn serve_http(app: axum::Router, host: &str, port: u16) -> anyhow::Result<()> {
    let mut handles = Vec::new();

    for addr in listen_targets(host, port) {
        match tokio::net::TcpListener::bind(&addr).await {
            Ok(listener) => {
                info!(%addr, "API server listening");
                let app = app.clone();
                handles.push(tokio::spawn(async move {
                    if let Err(e) = axum::serve(listener, app).await {
                        tracing::error!(error = %e, "HTTP server stopped");
                    }
                }));
            }
            Err(e) => {
                tracing::warn!(%addr, error = %e, "failed to bind listener");
            }
        }
    }

    if handles.is_empty() {
        anyhow::bail!(
            "could not bind HTTP on port {port} (host={host}); check SERVER_HOST/SERVER_PORT"
        );
    }

    for handle in handles {
        let _ = handle.await;
    }
    Ok(())
}

fn listen_targets(host: &str, port: u16) -> Vec<String> {
    match host {
        "127.0.0.1" | "localhost" => {
            vec![format!("127.0.0.1:{port}"), format!("[::1]:{port}")]
        }
        "0.0.0.0" | "*" | "::" => {
            vec![format!("0.0.0.0:{port}"), format!("[::]:{port}")]
        }
        other => vec![format!("{other}:{port}")],
    }
}
