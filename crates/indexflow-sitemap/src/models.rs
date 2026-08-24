use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Sitemap 中声明的更新频率
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ChangeFreq {
    Always,
    Hourly,
    Daily,
    Weekly,
    Monthly,
    Yearly,
    Never,
}

impl ChangeFreq {
    pub fn from_str_loose(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "always" => Some(Self::Always),
            "hourly" => Some(Self::Hourly),
            "daily" => Some(Self::Daily),
            "weekly" => Some(Self::Weekly),
            "monthly" => Some(Self::Monthly),
            "yearly" => Some(Self::Yearly),
            "never" => Some(Self::Never),
            _ => None,
        }
    }
}

/// Google 图片扩展 `<image:image>`
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct SitemapImage {
    pub loc: String,
    pub title: Option<String>,
    pub caption: Option<String>,
    pub geo_location: Option<String>,
    pub license: Option<String>,
}

/// Google 视频扩展 `<video:video>`
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct SitemapVideo {
    pub thumbnail_loc: String,
    pub title: String,
    pub description: String,
    pub content_loc: Option<String>,
    pub player_loc: Option<String>,
    pub duration_seconds: Option<u32>,
    pub expiration_date: Option<DateTime<Utc>>,
    pub rating: Option<f64>,
    pub view_count: Option<u64>,
    pub publication_date: Option<DateTime<Utc>>,
    pub family_friendly: Option<bool>,
    pub tags: Vec<String>,
    pub category: Option<String>,
}

/// Google 新闻扩展 `<news:news>`
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct SitemapNews {
    pub publication_name: String,
    pub publication_language: String,
    pub publication_date: Option<DateTime<Utc>>,
    pub title: String,
    pub keywords: Vec<String>,
}

/// `<xhtml:link rel="alternate" hreflang="...">`
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct HreflangEntry {
    pub lang: String,
    pub href: String,
}

/// 单个 URL 完整实体条目
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct SitemapUrlEntry {
    pub loc: String,
    pub lastmod: Option<DateTime<Utc>>,
    pub changefreq: Option<ChangeFreq>,
    pub priority: Option<f64>,
    pub hreflangs: Vec<HreflangEntry>,
    pub images: Vec<SitemapImage>,
    pub videos: Vec<SitemapVideo>,
    pub news: Option<SitemapNews>,
}

/// 解析产物枚举
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ParsedSitemap {
    /// 索引文件（包含子 Sitemap 列表）
    Index { child_urls: Vec<String> },
    /// URL 集（包含页面条目）
    UrlSet { entries: Vec<SitemapUrlEntry> },
    /// 纯文本 URL 列表
    PlainText { urls: Vec<String> },
}

impl ParsedSitemap {
    pub fn is_empty(&self) -> bool {
        match self {
            Self::Index { child_urls } => child_urls.is_empty(),
            Self::UrlSet { entries } => entries.is_empty(),
            Self::PlainText { urls } => urls.is_empty(),
        }
    }
}