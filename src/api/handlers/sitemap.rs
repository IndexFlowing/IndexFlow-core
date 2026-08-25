use super::{check_auth_or_redirect, AppState, HtmlTemplate, QueryParams};
use crate::domain::Url;
use crate::i18n::{detect_language, get_translations, Translations};
use crate::infrastructure::Site;
use askama::Template;
use axum::{
    extract::{Query, State},
    http::HeaderMap,
    response::{IntoResponse, Response},
};
use std::sync::atomic::Ordering;
use tracing::info;

#[derive(Template)]
#[template(path = "sitemap.html")]
pub struct SitemapTemplate {
    pub lang: &'static str,
    pub t: &'static Translations,
    pub domain: String,
    pub sitemap_url: String,
    pub items: Vec<Url>,
    pub total: i64,
    pub page: i64,
    pub limit: i64,
    pub query_str: Option<String>,
    pub all_sites: Vec<Site>,
    pub current_site_id: i64,
    pub is_sync_running: bool,
}

#[derive(Template)]
#[template(path = "partials/sitemap_action.html")]
pub struct SitemapActionTemplate {
    pub is_sync_running: bool,
    pub current_site_id: i64,
}

pub async fn render_sitemap(
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

    let page = q.page.unwrap_or(1).max(1);
    let limit = q.limit.unwrap_or(50).clamp(1, 200);
    let (items, total) = state.url_service.list_filtered(
        current_site_id, page, limit, q.q.as_deref(), None, None, None, None
    ).await.unwrap_or_default();

    HtmlTemplate(SitemapTemplate {
        lang: lang.as_str(),
        t,
        domain: site.as_ref().map(|s| s.domain.clone()).unwrap_or_default(),
        sitemap_url: site.as_ref().and_then(|s| s.sitemap_url.clone()).unwrap_or_default(),
        items,
        total,
        page,
        limit,
        query_str: q.q,
        all_sites,
        current_site_id,
        is_sync_running: state.site_service.is_sync_running.load(Ordering::Relaxed),
    }, set_cookie).into_response()
}

pub async fn render_sitemap_action(
    State(state): State<AppState>,
    Query(q): Query<QueryParams>,
) -> Response {
    let is_sync_running = state.site_service.is_sync_running.load(Ordering::Relaxed);
    HtmlTemplate(SitemapActionTemplate {
        is_sync_running,
        current_site_id: q.site_id.unwrap_or(1),
    }, None).into_response()
}

pub async fn action_sync_sitemap(
    State(state): State<AppState>,
    Query(q): Query<QueryParams>,
) -> Response {
    info!("🚀 [Action] 用户触发【同步 Sitemap】");
    let _ = state.site_service.trigger_sync_sitemap().await;
    render_sitemap_action(State(state), Query(q)).await
}

pub async fn action_cancel_sync(
    State(state): State<AppState>,
    Query(q): Query<QueryParams>,
) -> Response {
    let _ = state.site_service.cancel_sync().await;
    render_sitemap_action(State(state), Query(q)).await
}