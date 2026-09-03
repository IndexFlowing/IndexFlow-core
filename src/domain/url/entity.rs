use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::FromRow;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Url {
    pub id: i64,
    pub site_id: i64,
    pub url: String,
    pub url_hash: String,

    // SEO 诊断
    pub seo_status: String,
    pub seo_issue: Option<String>,
    pub page_title: Option<String>,
    pub meta_description: Option<String>,
    pub h1_content: Option<String>,
    pub h1_count: Option<i32>,
    pub has_nofollow: bool,
    pub ai_blocked_bots: Option<String>,
    pub has_opengraph: bool,
    pub has_twitter_card: bool,
    pub schema_types: Option<String>,
    pub response_time_ms: Option<i32>,
    pub payload_bytes: Option<i32>,
    pub has_viewport: bool,
    pub html_lang: Option<String>,
    pub images_missing_alt: Option<i32>,
    pub seo_warnings: Option<String>,
    pub canonical_url: Option<String>,
    pub http_status: Option<i32>,
    pub locale: String,
    pub path_prefix: String,

    // Google 收录状态
    pub gsc_index_status: String,
    pub gsc_coverage_state: Option<String>,
    pub gsc_last_crawled_at: Option<DateTime<Utc>>,
    pub gsc_inspected_at: Option<DateTime<Utc>>,

    // Bing 官方收录检测状态
    pub bing_index_status: String,
    pub bing_coverage_state: Option<String>,
    pub bing_last_crawled_at: Option<DateTime<Utc>>,
    pub bing_inspected_at: Option<DateTime<Utc>>,

    // 提交记录
    pub bing_status: String,
    pub bing_submitted_at: Option<DateTime<Utc>>,
    pub bing_error: Option<String>,

    pub google_status: String,
    pub google_submitted_at: Option<DateTime<Utc>>,
    pub google_error: Option<String>,

    pub priority: i32,
    pub sitemap_lastmod: Option<DateTime<Utc>>,
    pub sitemap_synced_at: Option<DateTime<Utc>>,
    pub discovered_via: String,
    pub last_checked_at: Option<DateTime<Utc>>,
    pub first_seen_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub is_watched: bool,
    pub watched_at: Option<DateTime<Utc>>,
}

#[allow(dead_code)]
impl Url {
    pub fn display_title(&self) -> &str {
        if let Some(ref t) = self.page_title {
            if !t.trim().is_empty() {
                return t.as_str();
            }
        }
        self.url.as_str()
    }

    /// Title 字符统计（用于 Ahrefs 38/60 标尺）
    pub fn title_char_count(&self) -> usize {
        self.page_title.as_deref().map(|s| s.chars().count()).unwrap_or(0)
    }

    /// Meta Description 字符统计（建议 50~160 字符）
    pub fn desc_char_count(&self) -> usize {
        self.meta_description.as_deref().map(|s| s.chars().count()).unwrap_or(0)
    }

    /// 当前 URL 存在的软性优化建议总条数
    pub fn warning_count(&self) -> usize {
        self.seo_warnings_list().len()
    }

    /// 获取第一条关键警告（用于列表摘要展示）
    pub fn primary_warning(&self) -> Option<&str> {
        self.seo_warnings_list().first().copied()
    }

    pub fn is_orphan(&self) -> bool {
        self.discovered_via == "gsc_orphan"
    }

    pub fn is_gsc_indexed(&self) -> bool {
        self.gsc_index_status == "INDEXED"
    }

    pub fn is_bing_indexed(&self) -> bool {
        self.bing_index_status == "INDEXED"
    }

    pub fn is_seo_pass(&self) -> bool {
        self.seo_status == "PASS"
    }

    pub fn is_seo_fail(&self) -> bool {
        self.seo_status == "FAIL"
    }

    pub fn is_seo_warn(&self) -> bool {
        self.seo_status == "WARN"
    }

    pub fn seo_warnings_list(&self) -> Vec<&str> {
        self.seo_warnings
            .as_deref()
            .unwrap_or("")
            .lines()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .collect()
    }

    pub fn is_bing_submitted(&self) -> bool {
        self.bing_status == "SUBMITTED"
    }

    pub fn is_google_submitted(&self) -> bool {
        self.google_status == "SUBMITTED"
    }
}

pub fn hash_url(url: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(url.as_bytes());
    hex::encode(hasher.finalize())
}