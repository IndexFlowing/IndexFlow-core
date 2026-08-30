use super::handlers::{
    auth::{handle_login, handle_setup, render_login, render_setup},
    dashboard::{api_get_stats, render_dashboard, render_partial_recent_urls, render_partial_stats},
    health_check,
    indexing::render_indexing,
    pipeline::{
        action_batch_inspect, action_batch_submit, action_inspect_url, action_pipeline_start,
        action_pipeline_stop, action_pipeline_sync, action_submit_url, render_pipeline_action,
    },
    seo::render_seo,
    settings::{handle_delete_site, handle_save_settings, handle_test_bing_webmaster, handle_test_google, handle_test_indexnow, render_settings},
    sitemap::render_sitemap,
    submit::render_submit,
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
        .route("/seo", get(render_seo))
        .route("/indexing", get(render_indexing))
        .route("/gsc", get(render_indexing))
        .route("/submit", get(render_submit))
        .route("/urls", get(render_dashboard))
        .route("/settings", get(render_settings).post(handle_save_settings))
        .route("/login", get(render_login).post(handle_login))
        .route("/setup", get(render_setup).post(handle_setup))
        .route("/partials/stats", get(render_partial_stats))
        .route("/partials/recent-urls", get(render_partial_recent_urls))
        .route(
            "/partials/pipeline-action/:stage",
            get(render_pipeline_action),
        )
        .route("/partials/url-detail/:id", get(render_url_detail_modal));

    let api_routes = Router::new()
        .route("/stats", get(api_get_stats))
        .route("/pipeline/:stage/start", post(action_pipeline_start))
        .route("/pipeline/:stage/stop", post(action_pipeline_stop))
        .route("/pipeline/:stage/sync", post(action_pipeline_sync))
        .route("/sites/:id/delete", post(handle_delete_site))
        .route("/sites/test-google", post(handle_test_google))
        .route("/sites/test-bing-webmaster", post(handle_test_bing_webmaster))
        .route("/sites/test-indexnow", post(handle_test_indexnow))
        .route("/urls/:id/inspect", post(action_inspect_url))
        .route("/urls/:id/submit", post(action_submit_url))
        .route("/urls/batch-inspect", post(action_batch_inspect))
        .route("/urls/batch-submit", post(action_batch_submit));

    Router::new()
        .merge(web_pages)
        .nest("/api", api_routes.clone())
        .nest("/api/v1", api_routes)
        .nest_service("/static", ServeDir::new("static"))
        .route("/health", get(health_check))
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}
