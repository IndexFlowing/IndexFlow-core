use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DashboardStats {
    pub url_total: i64,
    pub google_indexed: i64,
    pub google_crawled_not_indexed: i64,
    pub google_discovered_not_indexed: i64,
    pub google_not_indexed: i64,
    pub google_uninspected: i64,
    pub bing_indexed: i64,
    pub bing_not_indexed: i64,
    pub bing_failed: i64,
    pub bing_uninspected: i64,
    pub seo_passed: i64,
    pub seo_issues: i64,
    pub seo_warnings: i64,
    pub pending_submit: i64,
    pub gsc_used_24h: i64,
    pub last_seo_scan_at: Option<DateTime<Utc>>,
}

#[allow(dead_code)]
impl DashboardStats {
    pub fn seo_health_score(&self) -> i64 {
        let audited = self.seo_passed + self.seo_issues;
        if audited == 0 {
            100
        } else {
            ((self.seo_passed as f64 / audited as f64) * 100.0)
                .clamp(0.0, 100.0)
                .round() as i64
        }
    }

    pub fn seo_health_style_attr(&self) -> String {
        format!("style=\"width: {}%;\"", self.seo_health_score())
    }

    pub fn google_index_rate(&self) -> i64 {
        let inspected = self.google_indexed + self.google_not_indexed;
        if inspected == 0 {
            0
        } else {
            ((self.google_indexed as f64 / inspected as f64) * 100.0)
                .clamp(0.0, 100.0)
                .round() as i64
        }
    }

    pub fn google_index_style_attr(&self) -> String {
        format!("style=\"width: {}%;\"", self.google_index_rate())
    }

    pub fn bing_index_rate(&self) -> i64 {
        let inspected = self.bing_indexed + self.bing_not_indexed;
        if inspected == 0 {
            0
        } else {
            ((self.bing_indexed as f64 / inspected as f64) * 100.0)
                .clamp(0.0, 100.0)
                .round() as i64
        }
    }

    pub fn bing_index_style_attr(&self) -> String {
        format!("style=\"width: {}%;\"", self.bing_index_rate())
    }

    pub fn gsc_quota_percent(&self) -> i64 {
        ((self.gsc_used_24h as f64 / 2000.0) * 100.0)
            .clamp(0.0, 100.0)
            .round() as i64
    }

    pub fn gsc_quota_style_attr(&self) -> String {
        format!("style=\"width: {}%;\"", self.gsc_quota_percent())
    }

    pub fn gsc_remaining_quota(&self) -> i64 {
        (2000 - self.gsc_used_24h).max(0)
    }

    pub fn audited_total(&self) -> i64 {
        self.seo_passed + self.seo_issues
    }

    pub fn pending_seo_audit(&self) -> i64 {
        (self.url_total - self.audited_total()).max(0)
    }
}
