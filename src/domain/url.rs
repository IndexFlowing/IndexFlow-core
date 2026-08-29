use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::FromRow;
use url::Url as ParsedUrl;

pub const GINDEX_UNKNOWN: &str = "UNKNOWN";
pub const GINDEX_INDEXED: &str = "INDEXED";
pub const GINDEX_NOT_INDEXED: &str = "NOT_INDEXED";
pub const GINDEX_CRAWLED_NOT_INDEXED: &str = "CRAWLED_NOT_INDEXED";
pub const GINDEX_DISCOVERED_NOT_INDEXED: &str = "DISCOVERED_NOT_INDEXED";

pub fn coverage_to_index_status(coverage: &str) -> &'static str {
    let l = coverage.to_ascii_lowercase();
    if l.contains("not indexed") {
        if l.contains("crawled") {
            GINDEX_CRAWLED_NOT_INDEXED
        } else if l.contains("discovered") {
            GINDEX_DISCOVERED_NOT_INDEXED
        } else {
            GINDEX_NOT_INDEXED
        }
    } else if l.contains("indexed") {
        GINDEX_INDEXED
    } else {
        GINDEX_UNKNOWN
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SitemapType {
    Index,
    UrlSet,
}

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
    pub last_checked_at: Option<DateTime<Utc>>,
    pub first_seen_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
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

    /// 是否为从搜索引擎反向发现的“孤岛资产”（不在当前 Sitemap 中）
    pub fn is_orphan(&self) -> bool {
        self.gsc_coverage_state
            .as_deref()
            .map(|s| s.contains("Auto-Discovered") || s.contains("Search Analytics"))
            .unwrap_or(false)
            && self.sitemap_lastmod.is_none()
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

#[derive(Debug, Clone)]
pub struct SitemapUrlEntry {
    pub loc: String,
    pub lastmod: Option<DateTime<Utc>>,
    pub priority: Option<f64>,
    pub locale: String,
    pub path_prefix: String,
}

const LANG_CODES: &[&str] = &[
    "aa", "ab", "ae", "af", "ak", "am", "an", "ar", "as", "av", "ay", "az", "ba", "be", "bg", "bh",
    "bi", "bm", "bn", "bo", "br", "bs", "ca", "ce", "ch", "co", "cr", "cs", "cu", "cv", "cy", "da",
    "de", "dv", "dz", "ee", "el", "en", "eo", "es", "et", "eu", "fa", "ff", "fi", "fj", "fo", "fr",
    "fy", "ga", "gd", "gl", "gn", "gu", "gv", "ha", "he", "hi", "ho", "hr", "ht", "hu", "hy", "hz",
    "ia", "id", "ie", "ig", "ii", "ik", "io", "is", "it", "iu", "ja", "jv", "ka", "kg", "ki", "kj",
    "kk", "kl", "km", "kn", "ko", "kr", "ks", "ku", "kv", "kw", "ky", "la", "lb", "lg", "li", "ln",
    "lo", "lt", "lu", "lv", "mg", "mh", "mi", "mk", "ml", "mn", "mr", "ms", "mt", "my", "na", "nb",
    "nd", "ne", "ng", "nl", "nn", "no", "nr", "nv", "ny", "oc", "oj", "om", "or", "os", "pa", "pi",
    "pl", "ps", "pt", "qu", "rm", "rn", "ro", "ru", "rw", "sa", "sc", "sd", "se", "sg", "si", "sk",
    "sl", "sm", "sn", "so", "sq", "sr", "ss", "st", "su", "sv", "sw", "ta", "te", "tg", "th", "ti",
    "tk", "tl", "tn", "to", "tr", "ts", "tt", "tw", "ty", "ug", "uk", "ur", "uz", "ve", "vi", "vo",
    "wa", "wo", "xh", "yi", "yo", "za", "zh", "zu",
];

pub fn is_locale_segment(seg: &str) -> bool {
    let lower = seg.to_ascii_lowercase();
    let mut parts = lower.split('-');
    let Some(primary) = parts.next() else {
        return false;
    };
    if primary.len() != 2 || !LANG_CODES.contains(&primary) {
        return false;
    }
    match parts.next() {
        None => true,
        Some(rest) => {
            let ok_len = (2..=8).contains(&rest.len()) && rest.chars().all(|c| c.is_ascii_alphanumeric());
            ok_len && parts.next().is_none()
        }
    }
}

pub fn extract_locale_and_path_prefix(page_url: &str, hreflang: Option<&str>) -> (String, String) {
    let locale_from_hreflang = hreflang
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| {
            if s.eq_ignore_ascii_case("x-default") {
                "default".to_string()
            } else {
                s.to_ascii_lowercase()
            }
        });

    let path = match ParsedUrl::parse(page_url) {
        Ok(u) => u.path().to_string(),
        Err(_) => path_from_raw(page_url),
    };

    let segments: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
    let (locale_from_path, rest): (Option<String>, &[&str]) = match segments.first() {
        Some(first) if is_locale_segment(first) => {
            (Some(first.to_ascii_lowercase()), &segments[1..])
        }
        _ => (None, segments.as_slice()),
    };

    let locale = locale_from_hreflang
        .or(locale_from_path)
        .unwrap_or_else(|| "default".to_string());

    let path_prefix = match rest.first() {
        Some(dir) => format!("/{dir}"),
        None => "/".to_string(),
    };

    (locale, path_prefix)
}

fn path_from_raw(page_url: &str) -> String {
    let rest = page_url.split_once("://").map(|(_, r)| r).unwrap_or(page_url);
    let path = rest.find('/').map(|i| &rest[i..]).unwrap_or("/");
    path.split(['?', '#']).next().unwrap_or("/").to_string()
}