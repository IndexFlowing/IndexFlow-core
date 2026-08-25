use super::handlers::{
    auth::{handle_login, handle_setup, render_login, render_setup},
    dashboard::{api_get_stats, render_dashboard, render_partial_recent_urls, render_partial_stats},
    gsc::{action_cancel_gsc, action_inspect_gsc, action_inspect_url_gsc, render_gsc, render_gsc_action},
    health_check,
    seo::{action_audit_seo, action_cancel_seo, action_recheck_url, render_seo, render_seo_action},
    settings::{handle_delete_site, handle_save_settings, render_settings},
    sitemap::{action_cancel_sync, action_sync_sitemap, render_sitemap, render_sitemap_action},
    submit::{
        action_cancel_submit, action_submit_all, action_submit_url_bing, action_submit_url_google,
        render_submit, render_submit_action,
    },
    url_detail::render_url_detail_modal,
    AppState,
};
use axum::{
    routing::{get, post},
    Router,
};
use tower_http::services::ServeDir;
use tower_http::trace::TraceLayer;

pub fn build_router(state: AppState) -> Router {
    let web_pages = Router::new()
        .route("/", get(render_dashboard))
        .route("/sitemap", get(render_sitemap))
        .route("/gsc", get(render_gsc))
        .route("/seo", get(render_seo))
        .route("/submit", get(render_submit))
        .route("/urls", get(render_dashboard))
        .route("/settings", get(render_settings).post(handle_save_settings))
        .route("/login", get(render_login).post(handle_login))
        .route("/setup", get(render_setup).post(handle_setup))
        .route("/partials/stats", get(render_partial_stats))
        .route("/partials/recent-urls", get(render_partial_recent_urls))
        .route("/partials/actions/sitemap", get(render_sitemap_action))
        .route("/partials/actions/gsc", get(render_gsc_action))
        .route("/partials/actions/seo", get(render_seo_action))
        .route("/partials/actions/submit", get(render_submit_action))
        .route("/partials/url-detail/:id", get(render_url_detail_modal));

    let action_routes = Router::new()
        .route("/stats", get(api_get_stats))
        .route("/actions/sync-sitemap", post(action_sync_sitemap))
        .route("/actions/cancel-sync", post(action_cancel_sync))
        .route("/actions/inspect-gsc", post(action_inspect_gsc))
        .route("/actions/cancel-gsc", post(action_cancel_gsc))
        .route("/actions/audit-seo", post(action_audit_seo))
        .route("/actions/cancel-seo", post(action_cancel_seo))
        .route("/actions/submit-all", post(action_submit_all))
        .route("/actions/cancel-submit", post(action_cancel_submit))
        .route("/sites/:id/delete", post(handle_delete_site))
        .route("/urls/:id/recheck", post(action_recheck_url))
        .route("/urls/:id/inspect-gsc", post(action_inspect_url_gsc))
        .route("/urls/:id/submit-bing", post(action_submit_url_bing))
        .route("/urls/:id/submit-google", post(action_submit_url_google));

    Router::new()
        .merge(web_pages)
        .nest("/api", action_routes.clone())
        .nest("/api/v1", action_routes)
        .nest_service("/static", ServeDir::new("static"))
        .route("/health", get(health_check))
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}