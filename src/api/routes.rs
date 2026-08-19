use super::handlers::{
    auth::{auth_login, auth_me, auth_setup, auth_status},
    dashboard::dashboard,
    health_check,
    sitemaps::{list_sitemaps, sync_sitemap},
    sites::{
        create_site, get_site, list_sites, start_submit, test_bing, test_google, update_site,
    },
    tasks::{list_tasks, retry_task},
    urls::{
        get_url, list_url_diagnostics, list_urls, site_locales, site_path_prefixes, site_url_stats,
    },
    AppState,
};
use super::middleware::require_auth;
use axum::{
    middleware,
    routing::{get, post},
    Router,
};
use tower_http::services::{ServeDir, ServeFile};
use tower_http::trace::TraceLayer;

pub fn build_router(state: AppState) -> Router {
    // Public: health + auth bootstrap/login
    let public = Router::new()
        .route("/health", get(health_check))
        .route("/auth/status", get(auth_status))
        .route("/auth/setup", post(auth_setup))
        .route("/auth/login", post(auth_login));

    // Protected business APIs
    let protected = Router::new()
        .route("/auth/me", get(auth_me))
        .route("/dashboard", get(dashboard))
        .route("/sites", get(list_sites).post(create_site))
        .route("/sites/:id", get(get_site).put(update_site))
        .route("/sites/:id/sitemaps", get(list_sitemaps))
        .route("/sites/:id/sitemap/sync", post(sync_sitemap))
        .route("/sites/:id/submit", post(start_submit))
        .route("/sites/:id/test-bing", post(test_bing))
        .route("/sites/:id/test-google", post(test_google))
        .route("/sites/:id/urls", get(list_urls))
        .route("/sites/:id/url-diagnostics", get(list_url_diagnostics))
        .route("/sites/:id/stats", get(site_url_stats))
        .route("/sites/:id/locales", get(site_locales))
        .route("/sites/:id/path-prefixes", get(site_path_prefixes))
        .route("/urls/:id", get(get_url))
        .route("/tasks", get(list_tasks))
        .route("/tasks/:id/retry", post(retry_task))
        .layer(middleware::from_fn_with_state(state.clone(), require_auth));

    let api = Router::new().merge(public).merge(protected);

    let static_service =
        ServeDir::new("./ui/out").fallback(ServeFile::new("./ui/out/index.html"));

    Router::new()
        .nest("/api/v1", api)
        .layer(TraceLayer::new_for_http())
        .with_state(state)
        .fallback_service(static_service)
}
