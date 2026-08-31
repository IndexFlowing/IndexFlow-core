use super::{AppState, HtmlTemplate, QueryParams};
use crate::domain::PipelineStage;
use askama::Template;
use axum::{
    extract::{Path, Query, State},
    http::{HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use serde::Deserialize;
use tracing::info;

const HX_TRIGGER_REFRESH: HeaderValue = HeaderValue::from_static("refreshTable");

#[derive(Template)]
#[template(path = "partials/pipeline_action.html")]
pub struct PipelineActionTemplate {
    pub stage: &'static str,
    pub is_running: bool,
    pub current_site_id: i64,
    pub idle_text: &'static str,
    pub running_text: &'static str,
    pub color_theme: &'static str,
    pub icon: &'static str,
    pub idle_class: &'static str,
    pub is_disabled: bool,
    pub disabled_text: &'static str,
}

#[derive(Deserialize)]
pub struct InspectQuery {
    pub engine: String,
}

#[derive(Deserialize)]
pub struct SubmitQuery {
    pub provider: String,
}

#[derive(Deserialize)]
pub struct BatchInspectForm {
    #[serde(default)]
    pub selected_ids: Vec<i64>,
    pub engine: String,
}

#[derive(Deserialize)]
pub struct BatchSubmitForm {
    #[serde(default)]
    pub selected_ids: Vec<i64>,
    pub provider: String,
}

fn with_refresh(mut response: Response) -> Response {
    response.headers_mut().insert("HX-Trigger", HX_TRIGGER_REFRESH);
    response
}

async fn build_action(
    state: &AppState,
    stage: PipelineStage,
    site_id: i64,
) -> PipelineActionTemplate {
    let is_running = state.site_service.pipeline.is_running(stage);
    let (is_disabled, disabled_text) = if stage == PipelineStage::GscInspect && !is_running {
        let stats = state
            .site_service
            .dashboard_stats(site_id)
            .await
            .unwrap_or_default();
        if stats.gsc_remaining_quota() == 0 {
            (true, "GSC 配额耗尽")
        } else {
            (false, "")
        }
    } else {
        (false, "")
    };

    PipelineActionTemplate {
        stage: stage.slug(),
        is_running,
        current_site_id: site_id,
        idle_text: stage.idle_text(),
        running_text: stage.running_text(),
        color_theme: stage.color_theme(),
        icon: stage.icon(),
        idle_class: stage.idle_button_class(),
        is_disabled,
        disabled_text,
    }
}

async fn render_action(state: AppState, stage: PipelineStage, site_id: i64) -> Response {
    let tpl = build_action(&state, stage, site_id).await;
    HtmlTemplate(tpl, None).into_response()
}

pub async fn render_pipeline_action(
    State(state): State<AppState>,
    Path(stage): Path<PipelineStage>,
    Query(q): Query<QueryParams>,
) -> Response {
    render_action(state, stage, q.site_id.unwrap_or(1)).await
}

pub async fn action_pipeline_start(
    State(state): State<AppState>,
    Path(stage): Path<PipelineStage>,
    Query(q): Query<QueryParams>,
) -> Response {
    let site_id = q.site_id.unwrap_or(1);
    let started = state.site_service.pipeline.start(stage);
    info!(
        stage = %stage,
        site_id,
        started,
        running = ?state.site_service.pipeline.running_stages(),
        "🚀 [Pipeline] 启动阶段"
    );
    with_refresh(render_action(state, stage, site_id).await)
}

pub async fn action_pipeline_stop(
    State(state): State<AppState>,
    Path(stage): Path<PipelineStage>,
    Query(q): Query<QueryParams>,
) -> Response {
    let site_id = q.site_id.unwrap_or(1);
    let stopped = state.site_service.pipeline.stop(stage);
    info!(
        stage = %stage,
        site_id,
        stopped,
        running = ?state.site_service.pipeline.running_stages(),
        "⏹️ [Pipeline] 停止阶段"
    );
    with_refresh(render_action(state, stage, site_id).await)
}

pub async fn action_pipeline_sync(
    State(state): State<AppState>,
    Path(stage): Path<PipelineStage>,
    Query(q): Query<QueryParams>,
) -> Response {
    if stage != PipelineStage::GscInspect {
        return StatusCode::BAD_REQUEST.into_response();
    }
    let site_id = q.site_id.unwrap_or(1);
    info!(site_id, "⚡ [Pipeline] 同步 Google 曝光收录池");
    let _ = state.url_service.sync_gsc_analytics(site_id).await;
    with_refresh(StatusCode::NO_CONTENT.into_response())
}

pub async fn action_inspect_url(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Query(q): Query<InspectQuery>,
) -> Response {
    info!(url_id = id, engine = %q.engine, "🔍 [URL] 统一质检 / 收录探测");
    let _ = state.url_service.inspect_now(id, &q.engine).await;
    with_refresh(super::url_detail::render_url_detail_modal(State(state), Path(id)).await)
}

pub async fn action_submit_url(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Query(q): Query<SubmitQuery>,
) -> Response {
    info!(url_id = id, provider = %q.provider, "🚀 [URL] 统一搜索引擎推送");
    let _ = state.url_service.submit_now(id, &q.provider).await;
    with_refresh(super::url_detail::render_url_detail_modal(State(state), Path(id)).await)
}

pub async fn action_batch_inspect(
    State(state): State<AppState>,
    Json(form): Json<BatchInspectForm>,
) -> (StatusCode, Json<serde_json::Value>) {
    info!(
        count = form.selected_ids.len(),
        engine = %form.engine,
        "🔍 [URL] 批量质检"
    );
    let processed = state
        .url_service
        .batch_inspect(&form.selected_ids, &form.engine)
        .await
        .unwrap_or(0);
    (
        StatusCode::OK,
        Json(serde_json::json!({
            "success": true,
            "processed_count": processed,
            "rechecked_count": processed
        })),
    )
}

pub async fn action_batch_submit(
    State(state): State<AppState>,
    Json(form): Json<BatchSubmitForm>,
) -> (StatusCode, Json<serde_json::Value>) {
    info!(
        count = form.selected_ids.len(),
        provider = %form.provider,
        "🚀 [URL] 批量推送"
    );
    let success_count = match form.provider.trim().to_ascii_lowercase().as_str() {
        "bing" => state.url_service.submit_bing_batch(&form.selected_ids).await.unwrap_or(0),
        "google" => state.url_service.submit_google_batch(&form.selected_ids).await.unwrap_or(0),
        _ => 0,
    };
    (
        StatusCode::OK,
        Json(serde_json::json!({
            "success": true,
            "submitted_count": success_count
        })),
    )
}
