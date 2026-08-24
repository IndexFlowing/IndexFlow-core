use crate::api::handlers::AppState;
use crate::domain::Url;
use crate::i18n::{detect_language, get_translations, Translations};
use crate::infrastructure::DashboardStats;
use askama::Template;
use axum::{
    extract::{Path, Query, State},
    http::{header::{COOKIE, SET_COOKIE}, HeaderMap, StatusCode},
    response::{Html, IntoResponse, Redirect, Response},
    Form, Json,
};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use tracing::info;

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,
    pub exp: usize,
}

// --- Askama 模板参数 ---

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
    pub recent_urls: Vec<Url>,
}

#[derive(Template)]
#[template(path = "urls.html")]
pub struct UrlsTemplate {
    pub lang: &'static str,
    pub t: &'static Translations,
    pub items: Vec<Url>,
    pub total: i64,
    pub page: i64,
}

#[derive(Template)]
#[template(path = "settings.html")]
pub struct SettingsTemplate {
    pub lang: &'static str,
    pub t: &'static Translations,
    pub domain: String,
    pub sitemap_url: String,
    pub bing_indexnow_key: String,
    pub google_service_account_json: String,
}

#[derive(Template)]
#[template(path = "login.html")]
pub struct LoginTemplate {
    pub lang: &'static str,
    pub t: &'static Translations,
    pub setup_required: bool,
    pub error: Option<String>,
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

pub struct HtmlTemplate<T>(pub T, pub Option<String>);

impl<T> IntoResponse for HtmlTemplate<T>
where
    T: Template,
{
    fn into_response(self) -> Response {
        match self.0.render() {
            Ok(html) => {
                let mut res = Html(html).into_response();
                if let Some(cookie) = self.1 {
                    res.headers_mut().insert(SET_COOKIE, cookie.parse().unwrap());
                }
                res
            }
            Err(err) => {
                tracing::error!(error = %err, "template render failed");
                StatusCode::INTERNAL_SERVER_ERROR.into_response()
            }
        }
    }
}

async fn check_auth_or_redirect(state: &AppState, headers: &HeaderMap) -> Option<Response> {
    let count = state.admin_repo.count().await.unwrap_or(0);
    if count == 0 {
        return Some(Redirect::to("/setup").into_response());
    }

    let cookie_header = headers.get(COOKIE).and_then(|v| v.to_str().ok()).unwrap_or("");
    let token = cookie_header
        .split(';')
        .find_map(|cookie| {
            let mut parts = cookie.trim().splitn(2, '=');
            if parts.next()? == "if_token" {
                parts.next()
            } else {
                None
            }
        });

    if let Some(t) = token {
        let key = DecodingKey::from_secret(state.jwt_secret.as_bytes());
        if decode::<Claims>(t, &key, &Validation::default()).is_ok() {
            return None;
        }
    }

    Some(Redirect::to("/login").into_response())
}

#[derive(Deserialize)]
pub struct QueryParams {
    pub page: Option<i64>,
    pub limit: Option<i64>,
    pub lang: Option<String>,
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

    let site = state.site_service.get_config().await.ok().flatten();
    let domain = site.as_ref().map(|s| s.domain.clone()).unwrap_or_else(|| t.unconfigured_site.into());
    let sitemap_url = site.as_ref().and_then(|s| s.sitemap_url.clone()).unwrap_or_else(|| t.unset.into());
    let bing_ready = site.as_ref().map(|s| s.bing_ready()).unwrap_or(false);
    let google_ready = site.as_ref().map(|s| s.google_ready()).unwrap_or(false);

    let stats = state.site_service.dashboard_stats().await.unwrap_or_default();
    let (recent_urls, _) = state.url_service.list(1, 10).await.unwrap_or_default();

    HtmlTemplate(DashboardTemplate {
        lang: lang.as_str(),
        t,
        domain,
        sitemap_url,
        bing_ready,
        google_ready,
        stats,
        recent_urls,
    }, set_cookie).into_response()
}

pub async fn render_urls(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(q): Query<QueryParams>,
) -> Response {
    if let Some(redirect) = check_auth_or_redirect(&state, &headers).await {
        return redirect;
    }

    let (lang, set_cookie) = detect_language(&headers, q.lang.as_deref());
    let t = get_translations(lang);

    let page = q.page.unwrap_or(1).max(1);
    let limit = q.limit.unwrap_or(50).clamp(1, 200);
    let (items, total) = state.url_service.list(page, limit).await.unwrap_or_default();

    HtmlTemplate(UrlsTemplate {
        lang: lang.as_str(),
        t,
        items,
        total,
        page,
    }, set_cookie).into_response()
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

    let site = state.site_service.get_config().await.ok().flatten();

    HtmlTemplate(SettingsTemplate {
        lang: lang.as_str(),
        t,
        domain: site.as_ref().map(|s| s.domain.clone()).unwrap_or_default(),
        sitemap_url: site.as_ref().and_then(|s| s.sitemap_url.clone()).unwrap_or_default(),
        bing_indexnow_key: site.as_ref().and_then(|s| s.bing_indexnow_key.clone()).unwrap_or_default(),
        google_service_account_json: site.as_ref().and_then(|s| s.google_service_account_json.clone()).unwrap_or_default(),
    }, set_cookie).into_response()
}

#[derive(Deserialize)]
pub struct SettingsForm {
    pub domain: String,
    pub sitemap_url: Option<String>,
    pub bing_indexnow_key: Option<String>,
    pub google_service_account_json: Option<String>,
}

pub async fn handle_save_settings(
    State(state): State<AppState>,
    Form(form): Form<SettingsForm>,
) -> Response {
    info!(domain = %form.domain, sitemap = ?form.sitemap_url, "💾 [Settings] 用户更新了站点配置");
    let _ = state.site_service.save_config(
        &form.domain,
        form.sitemap_url.as_deref().filter(|s| !s.trim().is_empty()),
        form.bing_indexnow_key.as_deref().filter(|s| !s.trim().is_empty()),
        form.google_service_account_json.as_deref().filter(|s| !s.trim().is_empty()),
    ).await;

    Redirect::to("/settings").into_response()
}

pub async fn render_partial_stats(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Response {
    let (lang, _) = detect_language(&headers, None);
    let t = get_translations(lang);
    let stats = state.site_service.dashboard_stats().await.unwrap_or_default();
    HtmlTemplate(PartialStatsTemplate { t, stats }, None).into_response()
}

pub async fn render_partial_recent_urls(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Response {
    let (lang, _) = detect_language(&headers, None);
    let t = get_translations(lang);
    let (recent_urls, _) = state.url_service.list(1, 10).await.unwrap_or_default();
    HtmlTemplate(PartialRecentTableTemplate { t, recent_urls }, None).into_response()
}

pub async fn render_login(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(q): Query<QueryParams>,
) -> Response {
    let count = state.admin_repo.count().await.unwrap_or(0);
    if count == 0 {
        return Redirect::to("/setup").into_response();
    }
    let (lang, set_cookie) = detect_language(&headers, q.lang.as_deref());
    let t = get_translations(lang);
    HtmlTemplate(LoginTemplate {
        lang: lang.as_str(),
        t,
        setup_required: false,
        error: None,
    }, set_cookie).into_response()
}

pub async fn render_setup(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(q): Query<QueryParams>,
) -> Response {
    let count = state.admin_repo.count().await.unwrap_or(0);
    if count > 0 {
        return Redirect::to("/login").into_response();
    }
    let (lang, set_cookie) = detect_language(&headers, q.lang.as_deref());
    let t = get_translations(lang);
    HtmlTemplate(LoginTemplate {
        lang: lang.as_str(),
        t,
        setup_required: true,
        error: None,
    }, set_cookie).into_response()
}

#[derive(Deserialize)]
pub struct AuthForm {
    pub username: String,
    pub password: String,
}

pub async fn handle_setup(
    State(state): State<AppState>,
    headers: HeaderMap,
    Form(form): Form<AuthForm>,
) -> Response {
    let (lang, _) = detect_language(&headers, None);
    let t = get_translations(lang);

    let count = state.admin_repo.count().await.unwrap_or(0);
    if count > 0 {
        return Redirect::to("/login").into_response();
    }

    let hash = match bcrypt::hash(&form.password, bcrypt::DEFAULT_COST) {
        Ok(h) => h,
        Err(_) => return HtmlTemplate(LoginTemplate {
            lang: lang.as_str(),
            t,
            setup_required: true,
            error: Some("Password hash failed".into()),
        }, None).into_response(),
    };

    if state.admin_repo.create(&form.username, &hash).await.is_err() {
        return HtmlTemplate(LoginTemplate {
            lang: lang.as_str(),
            t,
            setup_required: true,
            error: Some("Failed to create admin".into()),
        }, None).into_response();
    }

    info!(username = %form.username, "🎉 [Auth] 管理员账号初始化成功！");
    issue_cookie_and_redirect(&state, &form.username, "/settings")
}

pub async fn handle_login(
    State(state): State<AppState>,
    headers: HeaderMap,
    Form(form): Form<AuthForm>,
) -> Response {
    let (lang, _) = detect_language(&headers, None);
    let t = get_translations(lang);

    let user = match state.admin_repo.find_by_username(&form.username).await {
        Ok(Some(u)) => u,
        _ => return HtmlTemplate(LoginTemplate {
            lang: lang.as_str(),
            t,
            setup_required: false,
            error: Some("Invalid username or password".into()),
        }, None).into_response(),
    };

    let ok = bcrypt::verify(&form.password, &user.password_hash).unwrap_or(false);
    if !ok {
        return HtmlTemplate(LoginTemplate {
            lang: lang.as_str(),
            t,
            setup_required: false,
            error: Some("Invalid username or password".into()),
        }, None).into_response();
    }

    info!(username = %form.username, "🔑 [Auth] 管理员登录成功");
    issue_cookie_and_redirect(&state, &form.username, "/")
}

fn issue_cookie_and_redirect(state: &AppState, username: &str, target_path: &str) -> Response {
    let exp = chrono::Utc::now().timestamp() + (7 * 24 * 3600);
    let claims = Claims {
        sub: username.to_string(),
        exp: exp as usize,
    };
    let token = encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(state.jwt_secret.as_bytes()),
    ).unwrap_or_default();

    let cookie = format!("if_token={token}; Path=/; HttpOnly; SameSite=Lax; Max-Age={}", 7 * 24 * 3600);

    let mut response = Redirect::to(target_path).into_response();
    response.headers_mut().insert(SET_COOKIE, cookie.parse().unwrap());
    response
}

// ==========================================
// ⚡ RESTful API 控制器 (返回 JSON)
// ==========================================

pub async fn api_get_stats(State(state): State<AppState>) -> (StatusCode, Json<DashboardStats>) {
    let stats = state.site_service.dashboard_stats().await.unwrap_or_default();
    (StatusCode::OK, Json(stats))
}

pub async fn api_list_urls(
    State(state): State<AppState>,
    Query(q): Query<QueryParams>,
) -> (StatusCode, Json<serde_json::Value>) {
    let page = q.page.unwrap_or(1).max(1);
    let limit = q.limit.unwrap_or(50).clamp(1, 200);
    let (items, total) = state.url_service.list(page, limit).await.unwrap_or_default();
    (
        StatusCode::OK,
        Json(serde_json::json!({
            "items": items,
            "total": total,
            "page": page,
            "limit": limit,
        })),
    )
}

pub async fn action_sync_sitemap(State(state): State<AppState>) -> (StatusCode, Json<serde_json::Value>) {
    info!("🚀 [Action] 用户触发【同步 Sitemap】");
    let success = state.site_service.trigger_sync_sitemap().await.unwrap_or(false);
    (StatusCode::OK, Json(serde_json::json!({ "success": success })))
}

pub async fn action_inspect_gsc(State(state): State<AppState>) -> (StatusCode, Json<serde_json::Value>) {
    info!("🔍 [Action] 用户触发【GSC 收录检测】");
    let count = state.site_service.trigger_gsc_inspect().await.unwrap_or(0);
    (StatusCode::OK, Json(serde_json::json!({ "success": true, "tasks_created": count })))
}

pub async fn action_audit_seo(State(state): State<AppState>) -> (StatusCode, Json<serde_json::Value>) {
    info!("🛡️ [Action] 用户触发【全站 SEO 质检】");
    let count = state.site_service.trigger_seo_audit().await.unwrap_or(0);
    (StatusCode::OK, Json(serde_json::json!({ "success": true, "tasks_created": count })))
}

pub async fn action_submit_all(State(state): State<AppState>) -> (StatusCode, Json<serde_json::Value>) {
    info!("⚡ [Action] 用户触发【全引擎提交】");
    let (bing, google) = state.site_service.trigger_submit_all().await.unwrap_or((0, 0));
    info!(bing_queued = bing, google_queued = google, "✅ 已派发提交任务");
    (
        StatusCode::OK,
        Json(serde_json::json!({
            "success": true,
            "bing_queued": bing,
            "google_queued": google,
        })),
    )
}

pub async fn action_recheck_url(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> (StatusCode, Json<serde_json::Value>) {
    info!(url_id = id, "🔍 [Action] 单个 URL SEO 质检");
    let res = state.url_service.recheck(id).await.ok().flatten();
    (StatusCode::OK, Json(serde_json::json!({ "success": true, "result": res })))
}

pub async fn action_submit_url(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> (StatusCode, Json<serde_json::Value>) {
    info!(url_id = id, "📤 [Action] 单个 URL 提交");
    let bing_ok = state.url_service.submit_now(id, "bing").await.unwrap_or(false);
    let google_ok = state.url_service.submit_now(id, "google").await.unwrap_or(false);
    (
        StatusCode::OK,
        Json(serde_json::json!({
            "bing_success": bing_ok,
            "google_success": google_ok,
        })),
    )
}

pub async fn action_cancel_tasks(State(state): State<AppState>) -> (StatusCode, Json<serde_json::Value>) {
    tracing::info!("🛑 [Action] 用户触发【停止所有排队任务】");
    let canceled_count = state.site_service.cancel_tasks().await.unwrap_or(0);
    tracing::info!(canceled_count, "✅ 已终止所有排队任务");
    (StatusCode::OK, Json(serde_json::json!({ "success": true, "canceled_count": canceled_count })))
}