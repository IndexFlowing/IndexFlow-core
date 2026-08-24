use super::handlers::{
    health_check,
    web::{
        action_audit_seo, action_cancel_tasks,action_inspect_gsc, action_recheck_url, action_submit_all,
        action_submit_url, action_sync_sitemap, api_get_stats, api_list_urls, handle_login,
        handle_save_settings, handle_setup, render_dashboard, render_login,
        render_partial_recent_urls, render_partial_stats, render_settings, render_setup,
        render_urls,
    },
    AppState,
};
use axum::{
    routing::{get, post},
    Router,
};
use tower_http::trace::TraceLayer;

pub fn build_router(state: AppState) -> Router {
    let web_pages = Router::new()
        .route("/", get(render_dashboard))
        .route("/urls", get(render_urls))
        .route("/settings", get(render_settings).post(handle_save_settings))
        .route("/login", get(render_login).post(handle_login))
        .route("/setup", get(render_setup).post(handle_setup))
        .route("/partials/stats", get(render_partial_stats))
        .route("/partials/recent-urls", get(render_partial_recent_urls));

    let action_routes = Router::new()
        .route("/stats", get(api_get_stats))
        .route("/urls", get(api_list_urls))
        .route("/actions/sync-sitemap", post(action_sync_sitemap))
        .route("/actions/inspect-gsc", post(action_inspect_gsc))
        .route("/actions/audit-seo", post(action_audit_seo))
        .route("/actions/submit-all", post(action_submit_all))
        .route("/actions/cancel-tasks", post(action_cancel_tasks))
        .route("/urls/:id/recheck", post(action_recheck_url))
        .route("/urls/:id/submit", post(action_submit_url));

    Router::new()
        .merge(web_pages)
        .nest("/api", action_routes.clone())
        .nest("/api/v1", action_routes)
        .route("/health", get(health_check))
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}