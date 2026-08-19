pub mod sites;
pub mod sitemaps;
pub mod urls;
pub mod tasks;
pub mod dashboard;
pub mod auth;

use crate::application::{
    AuthService, SiteService, SitemapService, TaskService, UrlService,
};
use axum::{http::StatusCode, Json};
use std::sync::Arc;

#[derive(Clone)]
pub struct AppState {
    pub site_service: Arc<SiteService>,
    pub sitemap_service: Arc<SitemapService>,
    pub url_service: Arc<UrlService>,
    pub task_service: Arc<TaskService>,
    pub auth_service: Arc<AuthService>,
}

pub async fn health_check() -> (StatusCode, Json<serde_json::Value>) {
    (
        StatusCode::OK,
        Json(serde_json::json!({
            "status": "healthy",
            "service": "indexflow-core",
            "version": "0.1.0"
        })),
    )
}

pub fn internal_err(e: impl ToString) -> (StatusCode, Json<serde_json::Value>) {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(serde_json::json!({ "error": e.to_string() })),
    )
}

pub fn not_found(msg: &str) -> (StatusCode, Json<serde_json::Value>) {
    (
        StatusCode::NOT_FOUND,
        Json(serde_json::json!({ "error": msg })),
    )
}

pub fn bad_request(msg: &str) -> (StatusCode, Json<serde_json::Value>) {
    (
        StatusCode::BAD_REQUEST,
        Json(serde_json::json!({ "error": msg })),
    )
}
