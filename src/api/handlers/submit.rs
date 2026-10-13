use super::{check_auth_or_redirect, AppState, HtmlTemplate, QueryParams};
use crate::domain::{PipelineStage, Url};
use crate::i18n::{detect_language, get_translations, Translations};
use crate::infrastructure::{DashboardStats, Site};
use askama::Template;
use axum::{
    extract::{Query, State},
    http::HeaderMap,
    response::{IntoResponse, Redirect, Response},
};
use chrono::{DateTime, Utc};

#[derive(Template)]
#[template(path = "submit_google.html")]
pub struct SubmitGoogleTemplate {
    pub lang: &'static str,
    pub t: &'static Translations,
    pub stats: DashboardStats,
    pub domain: String,
    pub items: Vec<Url>,
    pub total: i64,
    pub page: i64,
    pub limit: i64,
    pub filter_status: Option<String>,
    pub query_str: Option<String>,
    pub all_sites: Vec<Site>,
    pub current_site_id: i64,
    pub quota_used: i64,
    pub quota_total: u32,
    pub quota_remaining: i64,
    pub quota_paused: bool,
    pub quota_paused_until: Option<DateTime<Utc>>,
    pub has_google_key: bool,
    pub is_running: bool,
    pub dry_run: bool,
}

#[derive(Template)]
#[template(path = "submit_bing.html")]
pub struct SubmitBingTemplate {
    pub lang: &'static str,
    pub t: &'static Translations,
    pub stats: DashboardStats,
    pub domain: String,
    pub items: Vec<Url>,
    pub total: i64,
    pub page: i64,
    pub limit: i64,
    pub filter_status: Option<String>,
    pub query_str: Option<String>,
    pub all_sites: Vec<Site>,
    pub current_site_id: i64,
    pub has_bing_key: bool,
    pub is_running: bool,
    pub dry_run: bool,
}

pub async fn render_submit_redirect(
    Query(q): Query<QueryParams>,
) -> Response {
    let mut target = "/submit/google".to_string();
    if let Some(site_id) = q.site_id {
        target.push_str(&format!("?site_id={site_id}"));
    }
    Redirect::to(&target).into_response()
}

pub async fn render_submit_google(
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

    let stats = state
        .site_service
        .dashboard_stats(current_site_id)
        .await
        .unwrap_or_default();

    let (quota_used, quota_total, quota_remaining, quota_paused, quota_paused_until, has_google_key) =
        if let Some(ref s) = site {
            let daily_quota = s.google_daily_quota as u32;
            let quota_window = state
                .url_service
                .google_quota_window(s.id, daily_quota)
                .await
                .ok();
            (
                quota_window.as_ref().map(|w| w.used).unwrap_or(0),
                daily_quota,
                quota_window
                    .as_ref()
                    .map(|w| w.remaining)
                    .unwrap_or(daily_quota as i64),
                s.google_quota_paused(),
                s.google_quota_paused_until,
                s.has_google_credentials(),
            )
        } else {
            (0, 200, 200, false, None, false)
        };

    let page = q.page.unwrap_or(1).max(1);
    let limit = q.limit.unwrap_or(50).clamp(1, 200);
    let (items, total) = state
        .url_service
        .list_filtered(
            current_site_id,
            page,
            limit,
            q.q.as_deref(),
            None,
            None,
            None,
            false,
            None,
            q.status.as_deref(),
        )
        .await
        .unwrap_or_default();

    HtmlTemplate(
        SubmitGoogleTemplate {
            lang: lang.as_str(),
            t,
            stats,
            domain: site.as_ref().map(|s| s.domain.clone()).unwrap_or_default(),
            items,
            total,
            page,
            limit,
            filter_status: q.status,
            query_str: q.q,
            all_sites,
            current_site_id,
            quota_used,
            quota_total,
            quota_remaining,
            quota_paused,
            quota_paused_until,
            has_google_key,
            is_running: state
                .site_service
                .pipeline
                .is_running(current_site_id, PipelineStage::GoogleSubmit),
            dry_run: state.dry_run,
        },
        set_cookie,
    )
    .into_response()
}

pub async fn render_submit_bing(
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

    let has_bing_key = site
        .as_ref()
        .map(|s| s.has_bing_credentials())
        .unwrap_or(false);

    let stats = state
        .site_service
        .dashboard_stats(current_site_id)
        .await
        .unwrap_or_default();

    let page = q.page.unwrap_or(1).max(1);
    let limit = q.limit.unwrap_or(50).clamp(1, 200);
    let (items, total) = state
        .url_service
        .list_filtered(
            current_site_id,
            page,
            limit,
            q.q.as_deref(),
            None,
            None,
            None,
            false,
            q.status.as_deref(),
            None,
        )
        .await
        .unwrap_or_default();

    HtmlTemplate(
        SubmitBingTemplate {
            lang: lang.as_str(),
            t,
            stats,
            domain: site.as_ref().map(|s| s.domain.clone()).unwrap_or_default(),
            items,
            total,
            page,
            limit,
            filter_status: q.status,
            query_str: q.q,
            all_sites,
            current_site_id,
            has_bing_key,
            is_running: state
                .site_service
                .pipeline
                .is_running(current_site_id, PipelineStage::BingSubmit),
            dry_run: state.dry_run,
        },
        set_cookie,
    )
    .into_response()
}
