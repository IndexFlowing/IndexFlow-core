use super::{check_auth_or_redirect, AppState, HtmlTemplate, QueryParams};
use crate::domain::{MonitoringTimeline, TimelineEntry, Url};
use crate::i18n::{detect_language, get_translations, Translations};
use crate::infrastructure::Site;
use askama::Template;
use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use serde::Deserialize;

#[derive(Template)]
#[template(path = "monitoring.html")]
pub struct MonitoringTemplate {
    pub lang: &'static str,
    pub t: &'static Translations,
    pub watched: Vec<Url>,
    pub candidates: Vec<Url>,
    pub all_sites: Vec<Site>,
    pub current_site_id: i64,
    pub dry_run: bool,
}

#[derive(Template)]
#[template(path = "monitoring_detail.html")]
pub struct MonitoringDetailTemplate {
    pub lang: &'static str,
    pub t: &'static Translations,
    pub timeline: MonitoringTimeline,
    pub all_sites: Vec<Site>,
    pub current_site_id: i64,
    pub dry_run: bool,
}

pub async fn render_monitoring_list(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(q): Query<QueryParams>,
) -> Response {
    if let Some(redirect) = check_auth_or_redirect(&state, &headers).await {
        return redirect;
    }
    let (lang, cookie) = detect_language(&headers, q.lang.as_deref());
    let sites = state.site_service.list_sites().await.unwrap_or_default();
    let site = state
        .site_service
        .get_site_or_default(q.site_id)
        .await
        .ok()
        .flatten();
    let site_id = site.as_ref().map(|s| s.id).unwrap_or(1);
    let watched = state
        .url_service
        .list_watched(site_id)
        .await
        .unwrap_or_default();
    let (candidates, _) = state
        .url_service
        .list_filtered(
            site_id,
            1,
            500,
            q.q.as_deref(),
            None,
            None,
            None,
            false,
            None,
            None,
        )
        .await
        .unwrap_or_default();
    HtmlTemplate(
        MonitoringTemplate {
            lang: lang.as_str(),
            t: get_translations(lang),
            watched,
            candidates,
            all_sites: sites,
            current_site_id: site_id,
            dry_run: state.dry_run,
        },
        cookie,
    )
    .into_response()
}

pub async fn render_monitoring_detail(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Query(q): Query<QueryParams>,
) -> Response {
    if let Some(redirect) = check_auth_or_redirect(&state, &headers).await {
        return redirect;
    }
    let (lang, cookie) = detect_language(&headers, q.lang.as_deref());
    let Some(timeline) = state.url_service.timeline(id).await.ok().flatten() else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let sites = state.site_service.list_sites().await.unwrap_or_default();
    HtmlTemplate(
        MonitoringDetailTemplate {
            lang: lang.as_str(),
            t: get_translations(lang),
            current_site_id: timeline.url.site_id,
            timeline,
            all_sites: sites,
            dry_run: state.dry_run,
        },
        cookie,
    )
    .into_response()
}

#[derive(Deserialize)]
pub struct WatchForm {
    pub watched: bool,
}

#[derive(Deserialize)]
pub struct BatchWatchForm {
    #[serde(default)]
    pub selected_ids: Vec<i64>,
    pub watched: bool,
}

pub async fn action_toggle_watch(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Json(form): Json<WatchForm>,
) -> (StatusCode, Json<serde_json::Value>) {
    let ok = state
        .url_service
        .toggle_watch(id, form.watched)
        .await
        .unwrap_or(false);
    (
        if ok {
            StatusCode::OK
        } else {
            StatusCode::NOT_FOUND
        },
        Json(serde_json::json!({ "success": ok })),
    )
}

pub async fn action_batch_watch(
    State(state): State<AppState>,
    Json(form): Json<BatchWatchForm>,
) -> (StatusCode, Json<serde_json::Value>) {
    let count = state
        .url_service
        .batch_toggle_watch(&form.selected_ids, form.watched)
        .await
        .unwrap_or(0);
    (
        StatusCode::OK,
        Json(serde_json::json!({ "success": true, "updated_count": count })),
    )
}

impl TimelineEntry {
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Sitemap { .. } => "sitemap",
            Self::SeoCheck { .. } => "seo",
            Self::Submission { .. } => "submission",
            Self::IndexStatus { .. } => "index_status",
        }
    }
    pub fn provider(&self) -> Option<&str> {
        match self {
            Self::IndexStatus { history, .. } => Some(&history.provider),
            Self::Submission { log, .. } => Some(&log.provider),
            _ => None,
        }
    }
    pub fn summary(&self) -> String {
        match self {
            Self::Sitemap { lastmod, .. } => format!(
                "lastmod: {}",
                lastmod.map(|v| v.to_string()).unwrap_or_else(|| "-".into())
            ),
            Self::SeoCheck { check, .. } => format!(
                "HTTP {}{}",
                check
                    .http_status
                    .map(|v| v.to_string())
                    .unwrap_or_else(|| "-".into()),
                if check.has_noindex { ", noindex" } else { "" }
            ),
            Self::Submission { log, .. } => format!(
                "{} ({})",
                if log.success == 1 {
                    "success"
                } else {
                    "failed"
                },
                log.response_code
                    .map(|v| v.to_string())
                    .unwrap_or_else(|| "-".into())
            ),
            Self::IndexStatus { history, .. } => format!(
                "{}{}",
                history.index_status,
                history
                    .coverage_state
                    .as_deref()
                    .map(|v| format!(" - {v}"))
                    .unwrap_or_default()
            ),
        }
    }
}
