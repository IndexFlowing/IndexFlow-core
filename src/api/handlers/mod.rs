pub mod web;

use crate::application::{SiteService, UrlService};
use crate::infrastructure::AdminRepo;
use axum::{http::StatusCode, Json};
use std::sync::Arc;

#[derive(Clone)]
pub struct AppState {
    pub site_service: Arc<SiteService>,
    pub url_service: Arc<UrlService>,
    pub admin_repo: Arc<AdminRepo>,
    pub jwt_secret: String,
}

pub async fn health_check() -> (StatusCode, Json<serde_json::Value>) {
    (
        StatusCode::OK,
        Json(serde_json::json!({
            "status": "healthy",
            "service": "indexflow-core",
            "db": "sqlite-wal"
        })),
    )
}