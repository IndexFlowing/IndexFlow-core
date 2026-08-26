use super::{check_auth_or_redirect, AppState, HtmlTemplate, QueryParams};
use crate::domain::Url;
use crate::i18n::{detect_language, get_translations, Translations};
use crate::infrastructure::{DashboardStats, Site};
use askama::Template;
use axum::{
    extract::{Path, Query, State},
    http::HeaderMap,
    response::{IntoResponse, Response},
};
use std::sync::atomic::Ordering;
use tracing::info;

#[derive(Template)]
#[template(path = "gsc.html")]
pub struct GscTemplate {
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
    pub is_gsc_running: bool,
    pub is_quota_exhausted: bool, // 核心新增：是否配额耗尽
}

#[derive(Template)]
#[template(path = "partials/gsc_action.html")]
pub struct GscActionTemplate {
    pub is_gsc_running: bool,
    pub is_quota_exhausted: bool, // 核心新增：是否配额耗尽
    pub current_site_id: i64,
}

pub async fn render_gsc(
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
    let is_quota_exhausted = stats.gsc_remaining_quota() == 0;

    let page = q.page.unwrap_or(1).max(1);
    let limit = q.limit.unwrap_or(50).clamp(1, 200);
    let (items, total) = state.url_service.list_filtered(
        current_site_id, page, limit, q.q.as_deref(), None, q.status.as_deref(), None, None
    ).await.unwrap_or_default();

    HtmlTemplate(GscTemplate {
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
        is_gsc_running: state.site_service.is_gsc_running.load(Ordering::Relaxed),
        is_quota_exhausted,
    }, set_cookie).into_response()
}

pub async fn render_gsc_action(
    State(state): State<AppState>,
    Query(q): Query<QueryParams>,
) -> Response {
    let site_id = q.site_id.unwrap_or(1);
    let stats = state.site_service.dashboard_stats(site_id).await.unwrap_or_default();
    let is_gsc_running = state.site_service.is_gsc_running.load(Ordering::Relaxed);
    let is_quota_exhausted = stats.gsc_remaining_quota() == 0;

    HtmlTemplate(GscActionTemplate {
        is_gsc_running,
        is_quota_exhausted,
        current_site_id: site_id,
    }, None).into_response()
}

pub async fn action_inspect_gsc(
    State(state): State<AppState>,
    Query(q): Query<QueryParams>,
) -> Response {
    info!("🔍 [Action] 用户触发【GSC 增量检测】");
    let _ = state.site_service.trigger_gsc_inspect().await;
    render_gsc_action(State(state), Query(q)).await
}

pub async fn action_sync_gsc_analytics(
    State(state): State<AppState>,
    Query(q): Query<QueryParams>,
) -> Response {
    let site_id = q.site_id.unwrap_or(1);
    info!(site_id, "⚡ [Action] 用户触发【一键同步 Google 曝光收录池】");
    let _ = state.url_service.sync_gsc_analytics(site_id).await;
    render_gsc_action(State(state), Query(q)).await
}

pub async fn action_inspect_url_gsc(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Response {
    info!(url_id = id, "🔍 [Action] 单个 URL GSC 查询");
    let _ = state.url_service.inspect_gsc_now(id).await;
    super::url_detail::render_url_detail_modal(State(state), Path(id)).await
}

pub async fn action_cancel_gsc(
    State(state): State<AppState>,
    Query(q): Query<QueryParams>,
) -> Response {
    let _ = state.site_service.cancel_gsc().await;
    render_gsc_action(State(state), Query(q)).await
}