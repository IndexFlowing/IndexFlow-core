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
#[template(path = "seo.html")]
pub struct SeoTemplate {
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
    pub is_seo_running: bool,
    pub dry_run: bool,
}

#[derive(Template)]
#[template(path = "partials/seo_action.html")]
pub struct SeoActionTemplate {
    pub is_seo_running: bool,
    pub current_site_id: i64,
}

#[derive(Deserialize)]
pub struct BatchRecheckForm {
    #[serde(default)]
    pub selected_ids: Vec<i64>,
}

pub async fn render_seo(
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
        current_site_id, page, limit, q.q.as_deref(), q.status.as_deref(), None, None, None
    ).await.unwrap_or_default();

    HtmlTemplate(SeoTemplate {
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
        is_seo_running: state.site_service.is_seo_running.load(Ordering::Relaxed),
        dry_run: state.dry_run,
    }, set_cookie).into_response()
}

pub async fn render_seo_action(
    State(state): State<AppState>,
    Query(q): Query<QueryParams>,
) -> Response {
    let is_seo_running = state.site_service.is_seo_running.load(Ordering::Relaxed);
    HtmlTemplate(SeoActionTemplate {
        is_seo_running,
        current_site_id: q.site_id.unwrap_or(1),
    }, None).into_response()
}

pub async fn action_audit_seo(
    State(state): State<AppState>,
    Query(q): Query<QueryParams>,
) -> Response {
    info!("🛡️ [Action] 用户触发【SEO 增量质检】");
    let _ = state.site_service.trigger_seo_audit().await;
    render_seo_action(State(state), Query(q)).await
}

pub async fn action_batch_recheck_urls(
    State(state): State<AppState>,
    Json(form): Json<BatchRecheckForm>,
) -> (StatusCode, Json<serde_json::Value>) {
    info!(count = form.selected_ids.len(), "🛡️ [Action] 用户触发【批量 URL 重新质检】");
    let success = state.url_service.batch_recheck(&form.selected_ids).await.unwrap_or(0);
    (
        StatusCode::OK,
        Json(serde_json::json!({
            "success": true,
            "rechecked_count": success
        })),
    )
}

pub async fn action_recheck_url(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Response {
    info!(url_id = id, "🔍 [Action] 单个 URL SEO 质检");
    let _ = state.url_service.recheck(id).await;
    super::url_detail::render_url_detail_modal(State(state), Path(id)).await
}

pub async fn action_cancel_seo(
    State(state): State<AppState>,
    Query(q): Query<QueryParams>,
) -> Response {
    let _ = state.site_service.cancel_seo().await;
    render_seo_action(State(state), Query(q)).await
}