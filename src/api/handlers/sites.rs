use super::{bad_request, internal_err, not_found, AppState};
use crate::api::dto::{CreateSiteRequest, UpdateSiteRequest};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};

pub async fn list_sites(
    State(state): State<AppState>,
) -> Result<(StatusCode, Json<serde_json::Value>), (StatusCode, Json<serde_json::Value>)> {
    let sites = state
        .site_service
        .list()
        .await
        .map_err(internal_err)?;
    Ok((
        StatusCode::OK,
        Json(serde_json::json!({ "sites": sites })),
    ))
}

pub async fn get_site(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<(StatusCode, Json<serde_json::Value>), (StatusCode, Json<serde_json::Value>)> {
    match state.site_service.get(id).await {
        Ok(Some(detail)) => Ok((StatusCode::OK, Json(serde_json::json!(detail)))),
        Ok(None) => Err(not_found("site not found")),
        Err(e) => Err(internal_err(e)),
    }
}

pub async fn create_site(
    State(state): State<AppState>,
    Json(payload): Json<CreateSiteRequest>,
) -> Result<(StatusCode, Json<serde_json::Value>), (StatusCode, Json<serde_json::Value>)> {
    if payload.domain.trim().is_empty() {
        return Err(bad_request("domain is required"));
    }

    match state
        .site_service
        .create(
            &payload.domain,
            payload.sitemap_url.as_deref(),
            payload.indexnow_key.as_deref(),
            payload.google_service_account_json.as_deref(),
        )
        .await
    {
        Ok(site) => Ok((
            StatusCode::CREATED,
            Json(serde_json::json!({ "site": site })),
        )),
        Err(e) => Err(internal_err(e)),
    }
}

/// Update Bing IndexNow / Google SA credentials for an existing site.
pub async fn update_site(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Json(payload): Json<UpdateSiteRequest>,
) -> Result<(StatusCode, Json<serde_json::Value>), (StatusCode, Json<serde_json::Value>)> {
    // Write a field if client set the flag OR included the key in JSON body.
    let set_indexnow = payload.set_indexnow_key.unwrap_or(false)
        || payload.indexnow_key.is_some();
    let set_google = payload.set_google_service_account_json.unwrap_or(false)
        || payload.google_service_account_json.is_some();

    if !set_indexnow && !set_google {
        return Err(bad_request(
            "Provide at least one of indexnow_key or google_service_account_json",
        ));
    }

    match state
        .site_service
        .update_credentials(
            id,
            set_indexnow,
            payload.indexnow_key.as_deref(),
            set_google,
            payload.google_service_account_json.as_deref(),
        )
        .await
    {
        Ok(site) => Ok((
            StatusCode::OK,
            Json(serde_json::json!({
                "success": true,
                "site": site,
                "message": "Credentials updated"
            })),
        )),
        Err(e) => {
            let msg = e.to_string();
            if msg.contains("not found") {
                Err(not_found(&msg))
            } else if msg.to_ascii_lowercase().contains("invalid") {
                Err(bad_request(&msg))
            } else {
                Err(internal_err(e))
            }
        }
    }
}

/// Controlled workflow: inline SEO gate + submit for URLs still missing an enabled engine.
pub async fn start_submit(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<(StatusCode, Json<serde_json::Value>), (StatusCode, Json<serde_json::Value>)> {
    match state.site_service.start_submit(id).await {
        Ok(r) => Ok((StatusCode::ACCEPTED, Json(serde_json::json!(r)))),
        Err(e) => {
            let msg = e.to_string();
            if msg.contains("not found") {
                Err(not_found(&msg))
            } else if msg.contains("not yet verified") || msg.contains("Configure and verify") {
                Err(bad_request(&msg))
            } else {
                Err(internal_err(e))
            }
        }
    }
}

pub async fn test_bing(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<(StatusCode, Json<serde_json::Value>), (StatusCode, Json<serde_json::Value>)> {
    match state.site_service.test_bing(id).await {
        Ok(r) => Ok((StatusCode::OK, Json(serde_json::json!(r)))),
        Err(e) => {
            let msg = e.to_string();
            if msg.contains("not found") {
                Err(not_found(&msg))
            } else if msg.contains("configured") {
                Err(bad_request(&msg))
            } else {
                Err(internal_err(e))
            }
        }
    }
}

pub async fn test_google(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<(StatusCode, Json<serde_json::Value>), (StatusCode, Json<serde_json::Value>)> {
    match state.site_service.test_google(id).await {
        Ok(r) => Ok((StatusCode::OK, Json(serde_json::json!(r)))),
        Err(e) => {
            let msg = e.to_string();
            if msg.contains("not found") {
                Err(not_found(&msg))
            } else if msg.contains("configured") {
                Err(bad_request(&msg))
            } else {
                Err(internal_err(e))
            }
        }
    }
}
