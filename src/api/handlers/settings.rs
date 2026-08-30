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

#[derive(Deserialize)]
pub struct TestGoogleRequest { pub service_account_json: String, pub domain: String }
#[derive(Deserialize)]
pub struct TestBingWebmasterRequest { pub bing_webmaster_api_key: String, pub domain: String }
#[derive(Deserialize)]
pub struct TestIndexNowRequest { pub bing_indexnow_key: String, pub domain: String }

fn test_result(ok: bool, message: String) -> (StatusCode, Json<serde_json::Value>) {
    (StatusCode::OK, Json(serde_json::json!({ "ok": ok, "message": message })))
}

pub async fn handle_test_google(
    State(state): State<AppState>, Json(request): Json<TestGoogleRequest>,
) -> (StatusCode, Json<serde_json::Value>) {
    match state.url_service.test_google_credentials(&request.service_account_json, &request.domain).await {
        Ok(property) => test_result(true, format!("验证成功，已识别到站点: {property}")),
        Err(error) => test_result(false, format!("连接失败：{error}")),
    }
}

pub async fn handle_test_bing_webmaster(
    State(state): State<AppState>, Json(request): Json<TestBingWebmasterRequest>,
) -> (StatusCode, Json<serde_json::Value>) {
    match state.url_service.test_bing_webmaster_key(&request.bing_webmaster_api_key, &request.domain).await {
        Ok(sites) => test_result(true, format!("验证成功，识别到 {} 个可访问站点", sites.len())),
        Err(error) => test_result(false, format!("连接失败：{error}")),
    }
}

pub async fn handle_test_indexnow(
    Json(request): Json<TestIndexNowRequest>,
) -> (StatusCode, Json<serde_json::Value>) {
    let key = request.bing_indexnow_key.trim();
    if key.len() != 32 || !key.chars().all(|c| c.is_ascii_hexdigit()) {
        return test_result(false, "格式不合法，应为 32 位十六进制字符串".to_string());
    }
    let domain = request.domain.trim().trim_start_matches("https://")
        .trim_start_matches("http://").trim_end_matches('/');
    let url = format!("https://{domain}/{key}.txt");
    let fallback = || format!("未检测到有效的密钥文件（{domain}/{key}.txt），若未采用文件验证方式可忽略此项检测结果");
    match reqwest::Client::builder().timeout(std::time::Duration::from_secs(5)).build() {
        Ok(client) => match client.get(&url).send().await {
            Ok(response) => {
                let success = response.status().is_success();
                match response.text().await {
                    Ok(body) if success && body.contains(key) => test_result(true, "密钥文件验证通过".to_string()),
                    _ => test_result(false, fallback()),
                }
            }
            Err(_) => test_result(false, fallback()),
        },
        Err(_) => test_result(false, fallback()),
    }
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
        dry_run: state.dry_run,
    }, set_cookie).into_response()
}

pub async fn handle_save_settings(
    State(state): State<AppState>,
    Form(form): Form<SettingsForm>,
) -> Response {
    info!(domain = %form.domain, sitemap = ?form.sitemap_url, "💾 [Settings] 保存站点配置");
    let edit_id = form.site_id.filter(|&id| id > 0);
    let saved = state.site_service.save_site(
        edit_id,
        &form.domain,
        form.sitemap_url.as_deref().filter(|s| !s.trim().is_empty()),
        form.bing_indexnow_key.as_deref().filter(|s| !s.trim().is_empty()),
        form.bing_webmaster_api_key.as_deref().filter(|s| !s.trim().is_empty()),
        form.google_service_account_json.as_deref().filter(|s| !s.trim().is_empty()),
    ).await;

    match saved {
        Ok(site) => Redirect::to(&format!("/settings?site_id={}", site.id)).into_response(),
        Err(_) => Redirect::to("/settings").into_response(),
    }
}

pub async fn handle_delete_site(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> (StatusCode, Json<serde_json::Value>) {
    info!(site_id = id, "🗑️ [Settings] 删除站点资产");
    let ok = state.site_service.delete_site(id).await.is_ok();
    (StatusCode::OK, Json(serde_json::json!({ "success": ok })))
}
