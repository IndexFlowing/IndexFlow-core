use super::{AppState, HtmlTemplate};
use crate::domain::Url;
use askama::Template;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};

#[derive(Template)]
#[template(path = "partials/url_detail_modal.html")]
pub struct UrlDetailModalTemplate {
    pub url: Url,
}

pub async fn render_url_detail_modal(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Response {
    if let Ok(Some(url)) = state.url_service.find_by_id(id).await {
        HtmlTemplate(UrlDetailModalTemplate { url }, None).into_response()
    } else {
        StatusCode::NOT_FOUND.into_response()
    }
}