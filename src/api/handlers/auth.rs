use super::{bad_request, internal_err, AppState};
use axum::{
    extract::State,
    http::{header::AUTHORIZATION, HeaderMap, StatusCode},
    Json,
};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct AuthCredentials {
    pub username: String,
    pub password: String,
}

fn bearer_from_headers(headers: &HeaderMap) -> Option<&str> {
    headers
        .get(AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
}

pub async fn auth_status(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<(StatusCode, Json<serde_json::Value>), (StatusCode, Json<serde_json::Value>)> {
    let bearer = bearer_from_headers(&headers);
    match state.auth_service.status(bearer).await {
        Ok(s) => Ok((StatusCode::OK, Json(serde_json::json!(s)))),
        Err(e) => Err(internal_err(e)),
    }
}

pub async fn auth_setup(
    State(state): State<AppState>,
    Json(payload): Json<AuthCredentials>,
) -> Result<(StatusCode, Json<serde_json::Value>), (StatusCode, Json<serde_json::Value>)> {
    match state
        .auth_service
        .setup(&payload.username, &payload.password)
        .await
    {
        Ok(token) => Ok((StatusCode::CREATED, Json(serde_json::json!(token)))),
        Err(e) => {
            let msg = e.to_string();
            if msg.contains("already initialized") || msg.contains("at least") {
                Err(bad_request(&msg))
            } else {
                Err(internal_err(e))
            }
        }
    }
}

pub async fn auth_login(
    State(state): State<AppState>,
    Json(payload): Json<AuthCredentials>,
) -> Result<(StatusCode, Json<serde_json::Value>), (StatusCode, Json<serde_json::Value>)> {
    match state
        .auth_service
        .login(&payload.username, &payload.password)
        .await
    {
        Ok(token) => Ok((StatusCode::OK, Json(serde_json::json!(token)))),
        Err(e) => {
            let msg = e.to_string();
            if msg.contains("Invalid username") || msg.contains("not been initialized") {
                Err((
                    StatusCode::UNAUTHORIZED,
                    Json(serde_json::json!({ "error": msg })),
                ))
            } else {
                Err(internal_err(e))
            }
        }
    }
}

pub async fn auth_me(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<(StatusCode, Json<serde_json::Value>), (StatusCode, Json<serde_json::Value>)> {
    let Some(bearer) = bearer_from_headers(&headers) else {
        return Err((
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({ "error": "missing authorization" })),
        ));
    };
    match state.auth_service.verify_token(bearer) {
        Ok(claims) => Ok((
            StatusCode::OK,
            Json(serde_json::json!({
                "id": claims.sub,
                "username": claims.username
            })),
        )),
        Err(_) => Err((
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({ "error": "invalid or expired token" })),
        )),
    }
}
