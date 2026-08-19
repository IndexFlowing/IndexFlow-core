use super::{internal_err, not_found, AppState};
use crate::api::dto::{ListQuery, PageResponse};
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};

pub async fn list_tasks(
    State(state): State<AppState>,
    Query(q): Query<ListQuery>,
) -> Result<(StatusCode, Json<serde_json::Value>), (StatusCode, Json<serde_json::Value>)> {
    let page = q.page.unwrap_or(1).max(1);
    let limit = q.limit.unwrap_or(50).clamp(1, 200);

    let (items, total) = state
        .task_service
        .list(q.status.as_deref(), page, limit)
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

pub async fn retry_task(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<(StatusCode, Json<serde_json::Value>), (StatusCode, Json<serde_json::Value>)> {
    match state.task_service.retry(id).await {
        Ok(Some(task)) => Ok((
            StatusCode::OK,
            Json(serde_json::json!({
                "success": true,
                "task": task
            })),
        )),
        Ok(None) => Err(not_found("task not found or not retryable")),
        Err(e) => Err(internal_err(e)),
    }
}
