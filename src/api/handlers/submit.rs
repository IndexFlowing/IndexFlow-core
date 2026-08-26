use super::{check_auth_or_redirect, AppState, HtmlTemplate, QueryParams};
use crate::domain::Url;
use crate::i18n::{detect_language, get_translations, Translations};
use crate::infrastructure::{DashboardStats, Site};
use askama::Template;
use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use serde::Deserialize;
use std::sync::atomic::Ordering;
use tracing::info;

#[derive(Template)]
#[template(path = "submit.html")]
pub struct SubmitTemplate {
    pub lang: &'static str,
    pub t: &'static Translations,
    pub stats: DashboardStats,
    pub items: Vec<Url>,
    pub total: i64,
    pub page: i64,
    pub limit: i64,
    pub filter_status: Option<String>,
    pub query_str: Option<String>,
    pub all_sites: Vec<Site>,
    pub current_site_id: i64,
    pub is_submit_running: bool,
}

#[derive(Template)]
#[template(path = "partials/submit_action.html")]
pub struct SubmitActionTemplate {
    pub is_submit_running: bool,
    pub current_site_id: i64,
}

#[derive(Deserialize)]
pub struct BatchSubmitForm {
    #[serde(default)]
    pub selected_ids: Vec<i64>,
    pub provider: String, // "bing" | "google"
}

pub async fn render_submit(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(q): Query<QueryParams>,
) -> Response {
    if let Some(redirect) = check_auth_or_redirect(&state, &headers).await { return redirect; }
    let (lang, set_cookie) = detect_language(&headers, q.lang.as_deref());
    let t = get_translations(lang);

    let all_sites = state.site_service.list_sites().await.unwrap_or_default();
    let site = state.site_service.get_site_or_default(q.site_id).await.ok().flatten();
    let current_site_id = site.as_ref().map(|s| s.id).unwrap_or(1);

    let stats = state.site_service.dashboard_stats(current_site_id).await.unwrap_or_default();

    let page = q.page.unwrap_or(1).max(1);
    let limit = q.limit.unwrap_or(50).clamp(1, 200);
    let (items, total) = state.url_service.list_filtered(
        current_site_id, page, limit, q.q.as_deref(), None, None, q.status.as_deref(), None
    ).await.unwrap_or_default();

    HtmlTemplate(SubmitTemplate {
        lang: lang.as_str(),
        t,
        stats,
        items,
        total,
        page,
        limit,
        filter_status: q.status,
        query_str: q.q,
        all_sites,
        current_site_id,
        is_submit_running: state.site_service.is_submit_running.load(Ordering::Relaxed),
    }, set_cookie).into_response()
}

pub async fn render_submit_action(
    State(state): State<AppState>,
    Query(q): Query<QueryParams>,
) -> Response {
    let is_submit_running = state.site_service.is_submit_running.load(Ordering::Relaxed);
    HtmlTemplate(SubmitActionTemplate {
        is_submit_running,
        current_site_id: q.site_id.unwrap_or(1),
    }, None).into_response()
}

pub async fn action_submit_all(
    State(state): State<AppState>,
    Query(q): Query<QueryParams>,
) -> Response {
    info!("⚡ [Action] 用户触发【全引擎增量提交】");
    let _ = state.site_service.trigger_submit_all().await;
    render_submit_action(State(state), Query(q)).await
}

/// 【核心新增】批量向 Bing 或 Google 推送选中的 URL
pub async fn action_batch_submit_urls(
    State(state): State<AppState>,
    Json(form): Json<BatchSubmitForm>,
) -> (StatusCode, Json<serde_json::Value>) {
    info!(
        count = form.selected_ids.len(),
        provider = %form.provider,
        "🚀 [Action] 用户触发【批量 URL 搜索引擎推送】"
    );

    let mut success_count = 0;
    for id in &form.selected_ids {
        if state.url_service.submit_now(*id, &form.provider).await.unwrap_or(false) {
            success_count += 1;
        }
    }

    (
        StatusCode::OK,
        Json(serde_json::json!({
            "success": true,
            "submitted_count": success_count
        })),
    )
}

pub async fn action_submit_url_bing(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Response {
    info!(url_id = id, "⚡ [Action] 单个 URL 提交 Bing");
    let _ = state.url_service.submit_now(id, "bing").await;
    super::url_detail::render_url_detail_modal(State(state), Path(id)).await
}

pub async fn action_submit_url_google(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Response {
    info!(url_id = id, "🚀 [Action] 单个 URL 提交 Google");
    let _ = state.url_service.submit_now(id, "google").await;
    super::url_detail::render_url_detail_modal(State(state), Path(id)).await
}

pub async fn action_cancel_submit(
    State(state): State<AppState>,
    Query(q): Query<QueryParams>,
) -> Response {
    let _ = state.site_service.cancel_submit().await;
    render_submit_action(State(state), Query(q)).await
}