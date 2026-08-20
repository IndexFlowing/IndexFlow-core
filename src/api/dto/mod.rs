use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct CreateSiteRequest {
    pub domain: String,
    pub sitemap_url: Option<String>,
    pub indexnow_key: Option<String>,
    /// Full Google service account JSON string (Community Edition, per-site).
    pub google_service_account_json: Option<String>,
}

/// Partial update for an existing site (provider credentials).
/// Omit a field to leave it unchanged; send empty string to clear.
#[derive(Debug, Deserialize)]
pub struct UpdateSiteRequest {
    #[serde(default)]
    pub indexnow_key: Option<String>,
    #[serde(default)]
    pub google_service_account_json: Option<String>,
    /// When true (or when indexnow_key is present), write indexnow_key.
    #[serde(default)]
    pub set_indexnow_key: Option<bool>,
    /// When true (or when google_service_account_json is present), write Google JSON.
    #[serde(default)]
    pub set_google_service_account_json: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct SyncSitemapRequest {
    pub sitemap_url: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ListQuery {
    pub status: Option<String>,
    pub locale: Option<String>,
    pub path_prefix: Option<String>,
    pub page: Option<i64>,
    pub limit: Option<i64>,
    /// `true` = last_checked_at IS NOT NULL; `false` = never scanned.
    pub seo_checked: Option<bool>,
    pub google_index_status: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct SubmitNowRequest {
    pub provider: String,
}

#[derive(Debug, Deserialize)]
pub struct FacetQuery {
    pub locale: Option<String>,
    pub path_prefix: Option<String>,
}

#[derive(Debug, serde::Serialize)]
pub struct PageResponse<T: serde::Serialize> {
    pub items: Vec<T>,
    pub total: i64,
    pub page: i64,
    pub limit: i64,
}


