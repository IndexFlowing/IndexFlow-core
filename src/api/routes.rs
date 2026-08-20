use super::handlers::{
    auth::{auth_login, auth_me, auth_setup, auth_status},
    dashboard::dashboard,
    health_check,
    sitemaps::{list_sitemaps, sync_sitemap},
    sites::{
        create_site, get_site, gsc_inspect_batch, gsc_sync_analytics, index_stats, list_sites,
        seo_audit_full, seo_audit_unchecked, seo_stats, start_submit, start_submit_bing,
        start_submit_google, test_bing, test_google, update_site,
    },
    tasks::{list_tasks, retry_task},
    urls::{
        get_url, list_url_diagnostics, list_urls, site_locales, site_path_prefixes, site_url_stats,
        url_analysis, url_recheck, url_submit_now,
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
        .route("/sites/:id/submit-bing", post(start_submit_bing))
        .route("/sites/:id/submit-google", post(start_submit_google))
        .route("/sites/:id/seo/audit", post(seo_audit_full))
        .route("/sites/:id/seo/audit-unchecked", post(seo_audit_unchecked))
        .route("/sites/:id/seo-stats", get(seo_stats))
        .route("/sites/:id/gsc/sync-analytics", post(gsc_sync_analytics))
        .route("/sites/:id/gsc/inspect-batch", post(gsc_inspect_batch))
        .route("/sites/:id/index-stats", get(index_stats))
        .route("/sites/:id/test-bing", post(test_bing))
        .route("/sites/:id/test-google", post(test_google))
        .route("/sites/:id/urls", get(list_urls))
        .route("/sites/:id/url-diagnostics", get(list_url_diagnostics))
        .route("/sites/:id/stats", get(site_url_stats))
        .route("/sites/:id/locales", get(site_locales))
        .route("/sites/:id/path-prefixes", get(site_path_prefixes))
        .route("/urls/:id", get(get_url))
        .route("/urls/:id/analysis", get(url_analysis))
        .route("/urls/:id/recheck", post(url_recheck))
        .route("/urls/:id/submit-now", post(url_submit_now))
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
