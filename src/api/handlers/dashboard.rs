use super::{check_auth_or_redirect, AppState, HtmlTemplate, QueryParams};
use crate::domain::Url;
use crate::i18n::{detect_language, get_translations, Translations};
use crate::infrastructure::{DashboardStats, Site};
use askama::Template;
use axum::{
    extract::{Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};

#[derive(Template)]
#[template(path = "dashboard.html")]
pub struct DashboardTemplate {
    pub lang: &'static str,
    pub t: &'static Translations,
    pub domain: String,
    pub sitemap_url: String,
    pub bing_ready: bool,
    pub google_ready: bool,
    pub stats: DashboardStats,
    pub all_sites: Vec<Site>,
    pub current_site_id: i64,
}

#[derive(Template)]
#[template(path = "partials/stats.html")]
pub struct PartialStatsTemplate {
    pub t: &'static Translations,
    pub stats: DashboardStats,
}

#[derive(Template)]
#[template(path = "partials/recent_table.html")]
pub struct PartialRecentTableTemplate {
    pub t: &'static Translations,
    pub recent_urls: Vec<Url>,
}

pub async fn render_dashboard(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(q): Query<QueryParams>,
) -> Response {
    if let Some(redirect) = check_auth_or_redirect(&state, &headers).await {
        return redirect;
    }

    let (lang, set_cookie) = detect_language(&headers, q.lang.as_deref());
    let t = get_translations(lang);

    let all_sites = state.site_service.list_sites().await.unwrap_or_default();
    let site = state
        .site_service
        .get_site_or_default(q.site_id)
        .await
        .ok()
        .flatten();
    let current_site_id = site.as_ref().map(|s| s.id).unwrap_or(1);

    let domain = site
        .as_ref()
        .map(|s| s.domain.clone())
        .unwrap_or_else(|| t.unconfigured_site.into());
    let sitemap_url = site
        .as_ref()
        .and_then(|s| s.sitemap_url.clone())
        .unwrap_or_else(|| t.unset.into());
    let bing_ready = site.as_ref().map(|s| s.bing_ready()).unwrap_or(false);
    let google_ready = site.as_ref().map(|s| s.google_ready()).unwrap_or(false);

    let stats = state
        .site_service
        .dashboard_stats(current_site_id)
        .await
        .unwrap_or_default();

    HtmlTemplate(
        DashboardTemplate {
            lang: lang.as_str(),
            t,
            domain,
            sitemap_url,
            bing_ready,
            google_ready,
            stats,
            all_sites,
            current_site_id,
        },
        set_cookie,
    )
    .into_response()
}

pub async fn render_partial_stats(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(q): Query<QueryParams>,
) -> Response {
    let (lang, _) = detect_language(&headers, None);
    let t = get_translations(lang);
    let site = state
        .site_service
        .get_site_or_default(q.site_id)
        .await
        .ok()
        .flatten();
    let current_site_id = site.as_ref().map(|s| s.id).unwrap_or(1);
    let stats = state
        .site_service
        .dashboard_stats(current_site_id)
        .await
        .unwrap_or_default();
    HtmlTemplate(PartialStatsTemplate { t, stats }, None).into_response()
}

pub async fn render_partial_recent_urls(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(q): Query<QueryParams>,
) -> Response {
    let (lang, _) = detect_language(&headers, None);
    let t = get_translations(lang);
    let site = state
        .site_service
        .get_site_or_default(q.site_id)
        .await
        .ok()
        .flatten();
    let current_site_id = site.as_ref().map(|s| s.id).unwrap_or(1);
    let (recent_urls, _) = state
        .url_service
        .list(current_site_id, 1, 10)
        .await
        .unwrap_or_default();
    HtmlTemplate(PartialRecentTableTemplate { t, recent_urls }, None).into_response()
}

pub async fn api_get_stats(
    State(state): State<AppState>,
    Query(q): Query<QueryParams>,
) -> (StatusCode, Json<DashboardStats>) {
    let site = state
        .site_service
        .get_site_or_default(q.site_id)
        .await
        .ok()
        .flatten();
    let current_site_id = site.as_ref().map(|s| s.id).unwrap_or(1);
    let stats = state
        .site_service
        .dashboard_stats(current_site_id)
        .await
        .unwrap_or_default();
    (StatusCode::OK, Json(stats))
}