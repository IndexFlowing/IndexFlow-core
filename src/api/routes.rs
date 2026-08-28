use super::handlers::{
    auth::{handle_login, handle_setup, render_login, render_setup},
    dashboard::{api_get_stats, render_dashboard, render_partial_recent_urls, render_partial_stats},
    gsc::{
        action_cancel_gsc, action_inspect_gsc, action_inspect_url_gsc,
        action_sync_gsc_analytics, render_gsc, render_gsc_action,
    },
    health_check,
    seo::{
        action_audit_seo, action_batch_recheck_urls, action_cancel_seo, action_recheck_url,
        render_seo, render_seo_action,
    },
    settings::{handle_delete_site, handle_save_settings, render_settings},
    sitemap::{action_cancel_sync, action_sync_sitemap, render_sitemap, render_sitemap_action},
    submit::{
        action_batch_submit_urls, action_cancel_submit, action_submit_all,
        action_submit_url_bing, action_submit_url_google, render_submit, render_submit_action,
    },
    url_detail::render_url_detail_modal,
    AppState,
};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use tower_http::services::ServeDir;
use tower_http::trace::TraceLayer;

pub async fn action_inspect_bing(State(state): State<AppState>) -> Response {
    let _ = state.site_service.trigger_bing_inspect().await;
    (StatusCode::OK, Json(serde_json::json!({ "success": true }))).into_response()
}

pub async fn action_cancel_bing(State(state): State<AppState>) -> Response {
    let _ = state.site_service.cancel_bing_inspect().await;
    (StatusCode::OK, Json(serde_json::json!({ "cancelled": true }))).into_response()
}

pub async fn action_inspect_url_bing(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Response {
    let _ = state.url_service.inspect_bing_now(id).await;
    render_url_detail_modal(State(state), Path(id)).await
}

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
        .route("/actions/sync-gsc-analytics", post(action_sync_gsc_analytics))
        .route("/actions/cancel-gsc", post(action_cancel_gsc))
        .route("/actions/inspect-bing", post(action_inspect_bing))
        .route("/actions/cancel-bing", post(action_cancel_bing))
        .route("/actions/audit-seo", post(action_audit_seo))
        .route("/actions/cancel-seo", post(action_cancel_seo))
        .route("/actions/submit-all", post(action_submit_all))
        .route("/actions/cancel-submit", post(action_cancel_submit))
        .route("/sites/:id/delete", post(handle_delete_site))
        .route("/urls/:id/recheck", post(action_recheck_url))
        .route("/urls/batch-recheck", post(action_batch_recheck_urls))
        .route("/urls/batch-submit", post(action_batch_submit_urls))
        .route("/urls/:id/inspect-gsc", post(action_inspect_url_gsc))
        .route("/urls/:id/inspect-bing", post(action_inspect_url_bing))
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