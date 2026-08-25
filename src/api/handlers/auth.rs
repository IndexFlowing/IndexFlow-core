use super::{AppState, Claims, HtmlTemplate, QueryParams};
use crate::i18n::{detect_language, get_translations, Translations};
use askama::Template;
use axum::{
    extract::{Query, State},
    http::{header::SET_COOKIE, HeaderMap},
    response::{IntoResponse, Redirect, Response},
    Form,
};
use jsonwebtoken::{encode, EncodingKey, Header};
use serde::Deserialize;
use tracing::info;

#[derive(Template)]
#[template(path = "login.html")]
pub struct LoginTemplate {
    pub lang: &'static str,
    pub t: &'static Translations,
    pub setup_required: bool,
    pub error: Option<String>,
}

#[derive(Deserialize)]
pub struct AuthForm {
    pub username: String,
    pub password: String,
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