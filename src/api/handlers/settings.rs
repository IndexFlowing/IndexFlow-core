use super::{check_auth_or_redirect, AppState, HtmlTemplate, QueryParams};
use crate::i18n::{detect_language, get_translations, Translations};
use crate::infrastructure::Site;
use askama::Template;
use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Redirect, Response},
    Form, Json,
};
use serde::Deserialize;
use tracing::info;

#[derive(Template)]
#[template(path = "settings.html")]
pub struct SettingsTemplate {
    pub lang: &'static str,
    pub t: &'static Translations,
    pub all_sites: Vec<Site>,
    pub current_site_id: i64,
    pub editing_site_id: Option<i64>,
    pub domain: String,
    pub sitemap_url: String,
    pub bing_indexnow_key: String,
    pub bing_webmaster_api_key: String,
    pub google_service_account_json: String,
    pub dry_run: bool,
}

#[derive(Deserialize)]
pub struct SettingsForm {
    pub site_id: Option<i64>,
    pub domain: String,
    pub sitemap_url: Option<String>,
    pub bing_indexnow_key: Option<String>,
    pub bing_webmaster_api_key: Option<String>,
    pub google_service_account_json: Option<String>,
}

pub async fn render_settings(
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
    let site = state.site_service.get_site_or_default(q.site_id).await.ok().flatten();
    let current_site_id = site.as_ref().map(|s| s.id).unwrap_or(1);

    HtmlTemplate(SettingsTemplate {
        lang: lang.as_str(),
        t,
        all_sites,
        current_site_id,
        editing_site_id: q.site_id,
        domain: site.as_ref().map(|s| s.domain.clone()).unwrap_or_default(),
        sitemap_url: site.as_ref().and_then(|s| s.sitemap_url.clone()).unwrap_or_default(),
        bing_indexnow_key: site.as_ref().and_then(|s| s.bing_indexnow_key.clone()).unwrap_or_default(),
        bing_webmaster_api_key: site.as_ref().and_then(|s| s.bing_webmaster_api_key.clone()).unwrap_or_default(),
        google_service_account_json: site.as_ref().and_then(|s| s.google_service_account_json.clone()).unwrap_or_default(),
        dry_run: state.dry_run,
    }, set_cookie).into_response()
}

pub async fn handle_save_settings(
    State(state): State<AppState>,
    Form(form): Form<SettingsForm>,
) -> Response {
    info!(domain = %form.domain, sitemap = ?form.sitemap_url, "💾 [Settings] 保存站点配置");
    let edit_id = form.site_id.filter(|&id| id > 0);
    let _ = state.site_service.save_site(
        edit_id,
        &form.domain,
        form.sitemap_url.as_deref().filter(|s| !s.trim().is_empty()),
        form.bing_indexnow_key.as_deref().filter(|s| !s.trim().is_empty()),
        form.bing_webmaster_api_key.as_deref().filter(|s| !s.trim().is_empty()),
        form.google_service_account_json.as_deref().filter(|s| !s.trim().is_empty()),
    ).await;

    Redirect::to("/settings").into_response()
}

pub async fn handle_delete_site(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> (StatusCode, Json<serde_json::Value>) {
    info!(site_id = id, "🗑️ [Settings] 删除站点资产");
    let ok = state.site_service.delete_site(id).await.is_ok();
    (StatusCode::OK, Json(serde_json::json!({ "success": ok })))
}