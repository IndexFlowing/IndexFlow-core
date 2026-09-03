pub mod auth;
pub mod dashboard;
pub mod indexing;
pub mod monitoring;
pub mod pipeline;
pub mod seo;
pub mod settings;
pub mod sitemap;
pub mod submit;
pub mod url_detail;

use crate::application::{SiteService, UrlService};
use crate::infrastructure::{AdminRepo, ViewBoostRegistry};
use askama::Template;
use axum::{
    http::{
        header::{COOKIE, SET_COOKIE},
        HeaderMap, StatusCode,
    },
    response::{Html, IntoResponse, Redirect, Response},
    Json,
};
use jsonwebtoken::{decode, DecodingKey, Validation};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Clone)]
pub struct AppState {
    pub site_service: Arc<SiteService>,
    pub url_service: Arc<UrlService>,
    pub admin_repo: Arc<AdminRepo>,
    pub jwt_secret: String,
    pub dry_run: bool,
    pub view_boost: ViewBoostRegistry,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,
    pub exp: usize,
}

fn empty_string_as_none<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value: Option<String> = Option::deserialize(deserializer)?;
    Ok(value.filter(|value| !value.is_empty()))
}

#[derive(Deserialize, Default)]
pub struct QueryParams {
    pub site_id: Option<i64>,
    pub page: Option<i64>,
    pub limit: Option<i64>,
    #[serde(default, deserialize_with = "empty_string_as_none")]
    pub lang: Option<String>,
    #[serde(default, deserialize_with = "empty_string_as_none")]
    pub q: Option<String>,
    #[serde(default, deserialize_with = "empty_string_as_none")]
    pub status: Option<String>,
    #[serde(default, deserialize_with = "empty_string_as_none")]
    pub gsc_status: Option<String>,
    #[serde(default, deserialize_with = "empty_string_as_none")]
    pub bing_status: Option<String>,
    #[serde(default, deserialize_with = "empty_string_as_none")]
    pub orphan: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::QueryParams;

    #[test]
    fn query_params_empty_string_is_none() {
        let params: QueryParams = serde_urlencoded::from_str("q=").unwrap();

        assert_eq!(params.q, None);
    }

    #[test]
    fn query_params_non_empty_string_is_preserved() {
        let params: QueryParams = serde_urlencoded::from_str("q=example").unwrap();

        assert_eq!(params.q.as_deref(), Some("example"));
    }

    #[test]
    fn query_params_missing_string_is_none() {
        let params: QueryParams = serde_urlencoded::from_str("").unwrap();

        assert_eq!(params.q, None);
    }
}

pub struct HtmlTemplate<T>(pub T, pub Option<String>);

impl<T> IntoResponse for HtmlTemplate<T>
where
    T: Template,
{
    fn into_response(self) -> Response {
        match self.0.render() {
            Ok(html) => {
                let mut res = Html(html).into_response();
                if let Some(cookie) = self.1 {
                    res.headers_mut()
                        .insert(SET_COOKIE, cookie.parse().unwrap());
                }
                res
            }
            Err(err) => {
                tracing::error!(error = %err, "template render failed");
                StatusCode::INTERNAL_SERVER_ERROR.into_response()
            }
        }
    }
}

pub async fn check_auth_or_redirect(state: &AppState, headers: &HeaderMap) -> Option<Response> {
    let count = state.admin_repo.count().await.unwrap_or(0);
    if count == 0 {
        return Some(Redirect::to("/setup").into_response());
    }

    let cookie_header = headers
        .get(COOKIE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    let token = cookie_header.split(';').find_map(|cookie| {
        let mut parts = cookie.trim().splitn(2, '=');
        if parts.next()? == "if_token" {
            parts.next()
        } else {
            None
        }
    });

    if let Some(t) = token {
        let key = DecodingKey::from_secret(state.jwt_secret.as_bytes());
        if decode::<Claims>(t, &key, &Validation::default()).is_ok() {
            return None;
        }
    }

    Some(Redirect::to("/login").into_response())
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
