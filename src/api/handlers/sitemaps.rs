use super::{internal_err, AppState};
use crate::api::dto::SyncSitemapRequest;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};

pub async fn list_sitemaps(
    State(state): State<AppState>,
    Path(site_id): Path<i64>,
) -> Result<(StatusCode, Json<serde_json::Value>), (StatusCode, Json<serde_json::Value>)> {
    let items = state
        .sitemap_service
        .list_by_site(site_id)
        .await
        .map_err(internal_err)?;
    Ok((
        StatusCode::OK,
        Json(serde_json::json!({ "sitemaps": items })),
    ))
}

pub async fn sync_sitemap(
    State(state): State<AppState>,
    Path(site_id): Path<i64>,
    payload: Option<Json<SyncSitemapRequest>>,
) -> Result<(StatusCode, Json<serde_json::Value>), (StatusCode, Json<serde_json::Value>)> {
    let sitemap_url = payload.and_then(|p| p.0.sitemap_url);
    let created = state
        .sitemap_service
        .trigger_sync(site_id, sitemap_url.as_deref())
        .await
        .map_err(internal_err)?;

    Ok((
        StatusCode::ACCEPTED,
        Json(serde_json::json!({
            "success": true,
            "tasks_created": created,
            "message": "SYNC_SITEMAP tasks enqueued"
        })),
    ))
}
