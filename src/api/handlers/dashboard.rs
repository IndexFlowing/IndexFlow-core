use super::{internal_err, AppState};
use axum::{extract::State, http::StatusCode, Json};

pub async fn dashboard(
    State(state): State<AppState>,
) -> Result<(StatusCode, Json<serde_json::Value>), (StatusCode, Json<serde_json::Value>)> {
    let counts = state
        .site_service
        .dashboard()
        .await
        .map_err(internal_err)?;
    Ok((StatusCode::OK, Json(serde_json::json!(counts))))
}
