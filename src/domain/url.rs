use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::FromRow;
use url::Url as ParsedUrl;

use super::error::{DomainError, DomainResult};

/// Per-engine submit outcome stored on `urls.bing_status` / `urls.google_status`.
pub const ENGINE_NONE: &str = "NONE";
pub const ENGINE_SUBMITTED: &str = "SUBMITTED";
pub const ENGINE_FAILED: &str = "FAILED";

/// True when this engine has already been accepted (do not call the API again).
pub fn engine_is_submitted(status: &str) -> bool {
    status.eq_ignore_ascii_case(ENGINE_SUBMITTED)
}

/// True when this engine still needs a submit attempt (never tried, or retryable failure).
pub fn engine_needs_submit(status: &str) -> bool {
    let s = status.to_ascii_uppercase();
    s == ENGINE_NONE || s == ENGINE_FAILED
}

/// GSC / index coverage stored on `urls.google_index_status`.
pub const GINDEX_UNKNOWN: &str = "UNKNOWN";
pub const GINDEX_INDEXED: &str = "INDEXED";
pub const GINDEX_CRAWLED_NOT_INDEXED: &str = "CRAWLED_NOT_INDEXED";
pub const GINDEX_DISCOVERED_NOT_INDEXED: &str = "DISCOVERED_NOT_INDEXED";

/// True when GSC (Search Analytics or URL Inspection) confirmed the URL is indexed.
pub fn google_is_indexed(index_status: &str) -> bool {
    index_status.eq_ignore_ascii_case(GINDEX_INDEXED)
}

/// Map a GSC `coverageState` string onto our funnel bucket.
pub fn coverage_to_index_status(coverage: &str) -> &'static str {
    let l = coverage.to_ascii_lowercase();
    if l.contains("not indexed") {
        if l.contains("crawled") {
            GINDEX_CRAWLED_NOT_INDEXED
        } else if l.contains("discovered") {
            GINDEX_DISCOVERED_NOT_INDEXED
        } else {
            GINDEX_UNKNOWN
        }
    } else if l.contains("indexed") {
        GINDEX_INDEXED
    } else {
        GINDEX_UNKNOWN
    }
}

/// True when every **enabled** engine failed (used to mark BLOCKED after severe submit failure).
#[allow(dead_code)]
pub fn all_enabled_engines_failed(
    bing_enabled: bool,
    google_enabled: bool,
    bing_status: &str,
    google_status: &str,
) -> bool {
    if !bing_enabled && !google_enabled {
        return false;
    }
    let bing_failed = !bing_enabled || bing_status.eq_ignore_ascii_case(ENGINE_FAILED);
    let google_failed = !google_enabled || google_status.eq_ignore_ascii_case(ENGINE_FAILED);
    bing_failed && google_failed
}

/// Resolve the 3-state lifecycle from per-engine outcomes.
///
/// - `SUBMITTED`: every **enabled** engine is `SUBMITTED`.
/// - `PENDING`: at least one enabled engine is still `NONE` or `FAILED`
///   (includes partial: Bing done, Google not).
/// - `BLOCKED` is decided by the caller (SEO gate or all-engine severe failure).
pub fn resolve_lifecycle_after_submit(
    bing_enabled: bool,
    google_enabled: bool,
    bing_status: &str,
    google_status: &str,
) -> UrlStatus {
    let bing_done = !bing_enabled || engine_is_submitted(bing_status);
    let google_done = !google_enabled || engine_is_submitted(google_status);
    if bing_done && google_done {
        UrlStatus::Submitted
    } else {
        UrlStatus::Pending
    }
}

/// URL lifecycle — 3 mutually exclusive final states.
/// Conservation: site URL total = PENDING + SUBMITTED + BLOCKED.
///
/// `SUBMITTED` means **all enabled search engines** accepted the URL.
/// A Bing-only success with Google still `NONE` stays `PENDING` (partial).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum UrlStatus {
    /// Newly discovered, `<lastmod>` newer, or still missing an enabled engine.
    Pending,
    /// Passed the inline SEO gate and every enabled engine accepted the URL.
    Submitted,
    /// Failed the SEO gate, or every enabled engine failed severely.
    Blocked,
}

#[allow(dead_code)]
impl UrlStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "PENDING",
            Self::Submitted => "SUBMITTED",
            Self::Blocked => "BLOCKED",
        }
    }

    pub fn parse(s: &str) -> DomainResult<Self> {
        match s {
            "PENDING" => Ok(Self::Pending),
            "SUBMITTED" => Ok(Self::Submitted),
            "BLOCKED" => Ok(Self::Blocked),
            other => Err(DomainError::InvalidStatus(other.to_string())),
        }
    }

    pub fn can_transition_to(self, next: Self) -> bool {
        if self == next {
            return true;
        }
        matches!(
            (self, next),
            (Self::Pending, Self::Submitted)
                | (Self::Pending, Self::Blocked)
                | (Self::Submitted, Self::Pending)
                | (Self::Submitted, Self::Blocked)
                | (Self::Blocked, Self::Pending)
                | (Self::Blocked, Self::Submitted)
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Url {
    pub id: i64,
    pub site_id: i64,
    pub url: String,
    pub url_hash: String,
    pub status: String,
    /// Computed schedule priority (lower = higher urgency).
    pub priority: i32,
    /// Raw sitemap `<priority>` 0.0–1.0.
    pub sitemap_priority: Option<f64>,
    /// Raw sitemap `<lastmod>`.
    pub sitemap_lastmod: Option<DateTime<Utc>>,
    pub locale: String,
    pub path_prefix: String,
    pub page_title: Option<String>,
    pub canonical_url: Option<String>,
    pub meta_description: Option<String>,
    pub h1_content: Option<String>,
    pub block_reason: Option<String>,
    pub bing_status: String,
    pub google_status: String,
    pub google_index_status: String,
    pub google_coverage_state: Option<String>,
    pub google_last_crawled_at: Option<DateTime<Utc>>,
    pub google_inspected_at: Option<DateTime<Utc>>,
    pub bing_index_status: String,
    pub bing_last_crawled_at: Option<DateTime<Utc>>,
    pub bing_inspected_at: Option<DateTime<Utc>>,
    pub bing_submitted_at: Option<DateTime<Utc>>,
    pub google_submitted_at: Option<DateTime<Utc>>,
    pub bing_error: Option<String>,
    pub google_error: Option<String>,
    pub first_seen_at: DateTime<Utc>,
    pub last_seen_at: DateTime<Utc>,
    pub last_http_status: Option<i32>,
    pub last_checked_at: Option<DateTime<Utc>>,
    pub next_check_at: Option<DateTime<Utc>>,
    pub last_submitted_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[allow(dead_code)]
impl Url {
    pub fn status_enum(&self) -> DomainResult<UrlStatus> {
        UrlStatus::parse(&self.status)
    }
}

/// SHA-256 hex digest of a URL string (for uniqueness per site).
pub fn hash_url(url: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(url.as_bytes());
    hex::encode(hasher.finalize())
}

/// One page entry parsed from a sitemap `<urlset>`.
#[derive(Debug, Clone)]
pub struct SitemapUrlEntry {
    pub loc: String,
    pub lastmod: Option<DateTime<Utc>>,
    /// Sitemap `<priority>` 0.0–1.0
    pub priority: Option<f64>,
    pub locale: String,
    pub path_prefix: String,
}

/// ISO 639-1 primary language codes used as URL path prefixes.
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

/// True when a path segment looks like a language / locale code (`zh`, `en-US`, `zh-Hans`).
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

/// Extract `(locale, path_prefix)` from a page URL.
///
/// `hreflang` (from matching `<xhtml:link>`) wins; `x-default` becomes `default`.
/// Otherwise the first path segment is used when it is a language code.
/// `path_prefix` is the first directory after the locale is stripped (`/` for a root page).
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

/// Compare a page URL with its declared canonical (absolute or relative).
pub fn canonical_matches_page(page_url: &str, canonical: &str) -> bool {
    let Ok(page) = ParsedUrl::parse(page_url) else {
        return normalize_loose(page_url) == normalize_loose(canonical);
    };
    let resolved = match ParsedUrl::parse(canonical) {
        Ok(abs) => abs,
        Err(_) => match page.join(canonical) {
            Ok(joined) => joined,
            Err(_) => return false,
        },
    };
    normalize_url(&page) == normalize_url(&resolved)
}

fn normalize_url(u: &ParsedUrl) -> String {
    let scheme = u.scheme().to_ascii_lowercase();
    let host = u.host_str().unwrap_or("").to_ascii_lowercase();
    let mut path = u.path().to_string();
    if path.len() > 1 && path.ends_with('/') {
        path.pop();
    }
    let mut out = format!("{scheme}://{host}");
    if let Some(port) = u.port() {
        let default = matches!((scheme.as_str(), port), ("http", 80) | ("https", 443));
        if !default {
            out.push(':');
            out.push_str(&port.to_string());
        }
    }
    out.push_str(&path);
    if let Some(q) = u.query() {
        out.push('?');
        out.push_str(q);
    }
    out
}

fn normalize_loose(s: &str) -> String {
    s.trim().trim_end_matches('/').to_ascii_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn three_states_only() {
        assert_eq!(UrlStatus::parse("PENDING").unwrap(), UrlStatus::Pending);
        assert_eq!(UrlStatus::parse("SUBMITTED").unwrap(), UrlStatus::Submitted);
        assert_eq!(UrlStatus::parse("BLOCKED").unwrap(), UrlStatus::Blocked);
        assert!(UrlStatus::parse("HEALTHY").is_err());
        assert!(UrlStatus::parse("READY_SUBMIT").is_err());
        assert!(UrlStatus::parse("DISCOVERED").is_err());
    }

    #[test]
    fn engine_submit_predicates() {
        assert!(engine_is_submitted("SUBMITTED"));
        assert!(engine_is_submitted("submitted"));
        assert!(!engine_is_submitted("NONE"));
        assert!(engine_needs_submit("NONE"));
        assert!(engine_needs_submit("FAILED"));
        assert!(!engine_needs_submit("SUBMITTED"));
    }

    #[test]
    fn lifecycle_stays_pending_until_every_enabled_engine_submits() {
        assert_eq!(
            resolve_lifecycle_after_submit(true, true, "SUBMITTED", "NONE"),
            UrlStatus::Pending
        );
        assert_eq!(
            resolve_lifecycle_after_submit(true, true, "SUBMITTED", "FAILED"),
            UrlStatus::Pending
        );
        assert_eq!(
            resolve_lifecycle_after_submit(true, true, "SUBMITTED", "SUBMITTED"),
            UrlStatus::Submitted
        );
        assert_eq!(
            resolve_lifecycle_after_submit(true, false, "SUBMITTED", "NONE"),
            UrlStatus::Submitted
        );
        assert_eq!(
            resolve_lifecycle_after_submit(false, true, "NONE", "SUBMITTED"),
            UrlStatus::Submitted
        );
    }

    #[test]
    fn all_enabled_failed_ignores_disabled_engine() {
        assert!(all_enabled_engines_failed(true, true, "FAILED", "FAILED"));
        assert!(all_enabled_engines_failed(true, false, "FAILED", "NONE"));
        assert!(!all_enabled_engines_failed(true, true, "SUBMITTED", "FAILED"));
        assert!(!all_enabled_engines_failed(true, true, "FAILED", "NONE"));
    }

    #[test]
    fn coverage_state_maps_to_funnel() {
        assert_eq!(
            coverage_to_index_status("Submitted and indexed"),
            GINDEX_INDEXED
        );
        assert_eq!(
            coverage_to_index_status("Indexed, not submitted in sitemap"),
            GINDEX_INDEXED
        );
        assert_eq!(
            coverage_to_index_status("Crawled - currently not indexed"),
            GINDEX_CRAWLED_NOT_INDEXED
        );
        assert_eq!(
            coverage_to_index_status("Discovered - currently not indexed"),
            GINDEX_DISCOVERED_NOT_INDEXED
        );
        assert_eq!(
            coverage_to_index_status("URL is unknown to Google"),
            GINDEX_UNKNOWN
        );
        assert!(google_is_indexed("INDEXED"));
        assert!(!google_is_indexed("UNKNOWN"));
    }

    #[test]
    fn locale_from_path() {
        let (loc, prefix) = extract_locale_and_path_prefix("https://ex.com/zh/clips/123", None);
        assert_eq!(loc, "zh");
        assert_eq!(prefix, "/clips");

        let (loc, prefix) = extract_locale_and_path_prefix("https://ex.com/en/tools", None);
        assert_eq!(loc, "en");
        assert_eq!(prefix, "/tools");

        let (loc, prefix) = extract_locale_and_path_prefix("https://ex.com/ja", None);
        assert_eq!(loc, "ja");
        assert_eq!(prefix, "/");
    }

    #[test]
    fn locale_default_and_root() {
        let (loc, prefix) = extract_locale_and_path_prefix("https://ex.com/clips/123", None);
        assert_eq!(loc, "default");
        assert_eq!(prefix, "/clips");

        let (loc, prefix) = extract_locale_and_path_prefix("https://ex.com/", None);
        assert_eq!(loc, "default");
        assert_eq!(prefix, "/");

        let (loc, prefix) = extract_locale_and_path_prefix("https://ex.com", None);
        assert_eq!(loc, "default");
        assert_eq!(prefix, "/");
    }

    #[test]
    fn hreflang_wins_over_path() {
        let (loc, prefix) =
            extract_locale_and_path_prefix("https://ex.com/zh/clips/1", Some("ja"));
        assert_eq!(loc, "ja");
        assert_eq!(prefix, "/clips");

        let (loc, _) = extract_locale_and_path_prefix("https://ex.com/page", Some("x-default"));
        assert_eq!(loc, "default");
    }

    #[test]
    fn regional_locale_segment() {
        let (loc, prefix) = extract_locale_and_path_prefix("https://ex.com/zh-CN/blog/a", None);
        assert_eq!(loc, "zh-cn");
        assert_eq!(prefix, "/blog");
    }

    #[test]
    fn canonical_equivalence() {
        assert!(canonical_matches_page(
            "https://ex.com/zh/clips/1",
            "https://ex.com/zh/clips/1"
        ));
        assert!(canonical_matches_page(
            "https://ex.com/zh/clips/1",
            "https://ex.com/zh/clips/1/"
        ));
        assert!(canonical_matches_page(
            "https://ex.com/zh/clips/1",
            "/zh/clips/1"
        ));
        assert!(!canonical_matches_page(
            "https://ex.com/zh/clips/1",
            "https://ex.com/clips/1"
        ));
    }
}
