use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SitemapType {
    Index,
    UrlSet,
}

#[derive(Debug, Clone)]
pub struct SitemapUrlEntry {
    pub loc: String,
    pub lastmod: Option<DateTime<Utc>>,
    pub priority: Option<f64>,
    pub locale: String,
    pub path_prefix: String,
}