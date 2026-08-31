mod api;
mod application;
mod config;
mod domain;
mod i18n;
mod infrastructure;
mod providers;
mod workers;

use crate::api::handlers::AppState;
use crate::api::routes::build_router;
use crate::application::{
    BingService, GscService, HealthService, PipelineManager, SiteService, SitemapService,
    SubmissionService, UrlService,
};
use crate::config::AppConfig;
use crate::infrastructure::{
    build_http_client, connect, migrate, AdminRepo, HealthCheckRepo, SiteRepo, SubmissionLogRepo,
    UrlRepo, ViewBoostRegistry,
};
use crate::providers::{bing::BingProvider, google::GoogleProvider};
use crate::workers::{
    BingInspectWorker, BingSubmitWorker, GoogleSubmitWorker, GscInspectWorker, SeoAuditWorker,
    SyncWorker,
};
use std::sync::Arc;
use tracing::info;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG")
                .unwrap_or_else(|_| "indexflow_core=info,tower_http=info".into()),
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();

    info!("IndexFlow starting...");

    let config = AppConfig::from_env();
    let pool = connect(&config.database_url, config.db_max_connections).await?;
    migrate(&pool).await?;

    let http = build_http_client()?;
    let pipeline = PipelineManager::new();
    let view_boost = ViewBoostRegistry::new();

    let site_repo = SiteRepo::new(pool.clone());
    let url_repo = UrlRepo::new(pool.clone());
    let health_repo = HealthCheckRepo::new(pool.clone());
    let submission_log_repo = SubmissionLogRepo::new(pool.clone());
    let admin_repo = Arc::new(AdminRepo::new(pool.clone()));

    let bing_provider = BingProvider::new(http.clone(), config.dry_run);
    let google_provider = GoogleProvider::new(http.clone(), config.dry_run);

    let site_service = Arc::new(SiteService::new(
        site_repo.clone(),
        url_repo.clone(),
        pipeline.clone(),
    ));
    let sitemap_service = SitemapService::new(http.clone());
    let health_service = HealthService::new(http.clone())?;
    let submission_service = SubmissionService::new(bing_provider.clone(), google_provider.clone());
    let gsc_service = GscService::new(google_provider.clone(), site_repo.clone(), url_repo.clone());
    let bing_service = BingService::new(bing_provider.clone(), url_repo.clone());

    let url_service = Arc::new(UrlService::new(
        url_repo.clone(),
        health_repo.clone(),
        submission_log_repo.clone(),
        site_repo.clone(),
        health_service.clone(),
        submission_service.clone(),
        gsc_service.clone(),
        bing_service.clone(),
    ));

    Arc::new(SyncWorker::new(
        url_repo.clone(),
        site_repo.clone(),
        sitemap_service.clone(),
        pipeline.clone(),
        config.clone(),
    ))
    .start();

    Arc::new(SeoAuditWorker::new(
        url_repo.clone(),
        health_repo.clone(),
        health_service,
        pipeline.clone(),
        config.clone(),
    ))
    .start();

    Arc::new(GscInspectWorker::new(
        url_repo.clone(),
        site_repo.clone(),
        gsc_service.clone(),
        pipeline.clone(),
        config.clone(),
        view_boost.clone(),
    ))
    .start();

    Arc::new(BingInspectWorker::new(
        url_repo.clone(),
        site_repo.clone(),
        bing_service.clone(),
        pipeline.clone(),
        config.clone(),
        view_boost.clone(),
    ))
    .start();

    Arc::new(BingSubmitWorker::new(
        url_repo.clone(),
        site_repo.clone(),
        submission_log_repo.clone(),
        submission_service.clone(),
        pipeline.clone(),
        config.clone(),
    ))
    .start();

    Arc::new(GoogleSubmitWorker::new(
        url_repo.clone(),
        site_repo.clone(),
        submission_log_repo.clone(),
        submission_service,
        pipeline,
        config.clone(),
    ))
    .start();

    let state = AppState {
        site_service,
        url_service,
        admin_repo,
        jwt_secret: config.jwt_secret.clone(),
        dry_run: config.dry_run,
        view_boost,
    };

    let app = build_router(state);
    let listener =
        tokio::net::TcpListener::bind(format!("{}:{}", config.server_host, config.server_port))
            .await?;
    info!(
        addr = %listener.local_addr()?,
        dry_run = config.dry_run,
        "IndexFlow Web Console is running at http://127.0.0.1:{}",
        config.server_port
    );
    axum::serve(listener, app).await?;
    Ok(())
}
