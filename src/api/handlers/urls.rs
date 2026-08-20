use super::{bad_request, internal_err, not_found, AppState};
use crate::api::dto::{FacetQuery, ListQuery, PageResponse, SubmitNowRequest};
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};

pub async fn list_urls(
    State(state): State<AppState>,
    Path(site_id): Path<i64>,
    Query(q): Query<ListQuery>,
) -> Result<(StatusCode, Json<serde_json::Value>), (StatusCode, Json<serde_json::Value>)> {
    let page = q.page.unwrap_or(1).max(1);
    let limit = q.limit.unwrap_or(50).clamp(1, 200);

    let (items, total) = state
        .url_service
        .list(
            site_id,
            q.status.as_deref(),
            q.locale.as_deref(),
            q.path_prefix.as_deref(),
            page,
            limit,
        )
        .await
        .map_err(internal_err)?;

    Ok((
        StatusCode::OK,
        Json(serde_json::json!(PageResponse {
            items,
            total,
            page,
            limit
        })),
    ))
}

/// Single-site URL inspection table (locale / path_prefix / status filters).
pub async fn list_url_diagnostics(
    State(state): State<AppState>,
    Path(site_id): Path<i64>,
    Query(q): Query<ListQuery>,
) -> Result<(StatusCode, Json<serde_json::Value>), (StatusCode, Json<serde_json::Value>)> {
    let page = q.page.unwrap_or(1).max(1);
    let limit = q.limit.unwrap_or(50).clamp(1, 200);

    let (items, total) = state
        .url_service
        .list_diagnostics(
            site_id,
            q.status.as_deref(),
            q.locale.as_deref(),
            q.path_prefix.as_deref(),
            page,
            limit,
            q.seo_checked,
            q.google_index_status.as_deref(),
        )
        .await
        .map_err(internal_err)?;

    Ok((
        StatusCode::OK,
        Json(serde_json::json!(PageResponse {
            items,
            total,
            page,
            limit
        })),
    ))
}

pub async fn site_url_stats(
    State(state): State<AppState>,
    Path(site_id): Path<i64>,
    Query(q): Query<FacetQuery>,
) -> Result<(StatusCode, Json<serde_json::Value>), (StatusCode, Json<serde_json::Value>)> {
    let stats = state
        .url_service
        .stats(site_id, q.locale.as_deref(), q.path_prefix.as_deref())
        .await
        .map_err(internal_err)?;
    Ok((StatusCode::OK, Json(serde_json::json!(stats))))
}

pub async fn site_locales(
    State(state): State<AppState>,
    Path(site_id): Path<i64>,
    Query(q): Query<FacetQuery>,
) -> Result<(StatusCode, Json<serde_json::Value>), (StatusCode, Json<serde_json::Value>)> {
    let locales = state
        .url_service
        .locales(site_id, q.path_prefix.as_deref())
        .await
        .map_err(internal_err)?;
    Ok((
        StatusCode::OK,
        Json(serde_json::json!({ "locales": locales })),
    ))
}

pub async fn site_path_prefixes(
    State(state): State<AppState>,
    Path(site_id): Path<i64>,
    Query(q): Query<FacetQuery>,
) -> Result<(StatusCode, Json<serde_json::Value>), (StatusCode, Json<serde_json::Value>)> {
    let path_prefixes = state
        .url_service
        .path_prefixes(site_id, q.locale.as_deref())
        .await
        .map_err(internal_err)?;
    Ok((
        StatusCode::OK,
        Json(serde_json::json!({ "path_prefixes": path_prefixes })),
    ))
}

pub async fn get_url(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<(StatusCode, Json<serde_json::Value>), (StatusCode, Json<serde_json::Value>)> {
    match state.url_service.get_detail(id).await {
        Ok(Some(detail)) => Ok((StatusCode::OK, Json(serde_json::json!(detail)))),
        Ok(None) => Err(not_found("url not found")),
        Err(e) => Err(internal_err(e)),
    }
}

pub async fn url_analysis(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<(StatusCode, Json<serde_json::Value>), (StatusCode, Json<serde_json::Value>)> {
    match state.url_service.analysis(id).await {
        Ok(Some(detail)) => Ok((StatusCode::OK, Json(serde_json::json!(detail)))),
        Ok(None) => Err(not_found("url not found")),
        Err(e) => Err(internal_err(e)),
    }
}

pub async fn url_recheck(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<(StatusCode, Json<serde_json::Value>), (StatusCode, Json<serde_json::Value>)> {
    match state.url_service.recheck(id).await {
        Ok(Some(r)) => Ok((StatusCode::OK, Json(serde_json::json!(r)))),
        Ok(None) => Err(not_found("url not found")),
        Err(e) => Err(internal_err(e)),
    }
}

pub async fn url_submit_now(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Json(payload): Json<SubmitNowRequest>,
) -> Result<(StatusCode, Json<serde_json::Value>), (StatusCode, Json<serde_json::Value>)> {
    match state.url_service.submit_now(id, &payload.provider).await {
        Ok(Some(r)) => Ok((StatusCode::OK, Json(serde_json::json!(r)))),
        Ok(None) => Err(not_found("url not found")),
        Err(e) => {
            let msg = e.to_string();
            if msg.contains("BLOCKED")
                || msg.contains("not configured")
                || msg.contains("not verified")
                || msg.contains("quota")
                || msg.contains("provider must")
            {
                Err(bad_request(&msg))
            } else {
                Err(internal_err(e))
            }
        }
    }
}
