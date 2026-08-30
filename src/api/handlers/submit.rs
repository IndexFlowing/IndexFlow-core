use super::{check_auth_or_redirect, AppState, HtmlTemplate, QueryParams};
use crate::domain::{PipelineStage, Url};
use crate::i18n::{detect_language, get_translations, Translations};
use crate::infrastructure::{DashboardStats, Site};
use askama::Template;
use axum::{
    extract::{Query, State},
    http::HeaderMap,
    response::{IntoResponse, Response},
};

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
    pub is_running: bool,
    pub dry_run: bool,
}

pub async fn render_submit(
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
        SubmitTemplate {
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
            is_running: state
                .site_service
                .pipeline
                .is_running(PipelineStage::PushSubmit),
            dry_run: state.dry_run,
        },
        set_cookie,
    )
    .into_response()
}
