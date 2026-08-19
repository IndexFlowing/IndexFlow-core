//! URL schedule priority scoring.
//!
//! Convention: **lower integer = higher urgency** (matches task claim ORDER BY priority ASC).
//!
//! Inputs:
//! - Sitemap `<priority>` (0.0–1.0, default 0.5)
//! - Sitemap `<lastmod>` recency
//! - Whether the URL is newly discovered in this sync

use chrono::{DateTime, Duration, Utc};

/// Default when sitemap omits `<priority>`.
pub const DEFAULT_SITEMAP_PRIORITY: f64 = 0.5;

/// Compute schedule priority from sitemap signals + discovery freshness.
///
/// Components (approximate weights):
/// - Sitemap priority: up to 400 (1.0 → 0, 0.0 → 400)
/// - lastmod age: 0–300 (recent → 0)
/// - not-new penalty: +80 if already known
/// - lastmod became fresher than previous: −60 boost
pub fn compute_url_priority(
    sitemap_priority: Option<f64>,
    lastmod: Option<DateTime<Utc>>,
    previous_lastmod: Option<DateTime<Utc>>,
    is_new: bool,
    now: DateTime<Utc>,
) -> i32 {
    let sp = sitemap_priority
        .unwrap_or(DEFAULT_SITEMAP_PRIORITY)
        .clamp(0.0, 1.0);

    // Higher sitemap priority → lower score
    let from_sitemap = ((1.0 - sp) * 400.0).round() as i32;

    let from_lastmod = lastmod_age_score(lastmod, now);

    let from_new = if is_new { 0 } else { 80 };

    // Content/date update detected vs previous sitemap lastmod
    let from_update = match (lastmod, previous_lastmod) {
        (Some(cur), Some(prev)) if cur > prev => -60,
        (Some(_), None) if !is_new => -30, // first time we see lastmod on known URL
        _ => 0,
    };

    let raw = from_sitemap + from_lastmod + from_new + from_update;
    raw.clamp(1, 999)
}

fn lastmod_age_score(lastmod: Option<DateTime<Utc>>, now: DateTime<Utc>) -> i32 {
    let Some(lm) = lastmod else {
        return 150; // unknown age → medium
    };
    let age = now.signed_duration_since(lm);
    if age <= Duration::hours(24) {
        0
    } else if age <= Duration::days(7) {
        40
    } else if age <= Duration::days(30) {
        100
    } else if age <= Duration::days(90) {
        180
    } else {
        280
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn higher_sitemap_priority_is_more_urgent() {
        let now = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap();
        let high = compute_url_priority(Some(1.0), None, None, true, now);
        let low = compute_url_priority(Some(0.1), None, None, true, now);
        assert!(high < low, "high={high} low={low}");
    }

    #[test]
    fn new_pages_outrank_known_same_signals() {
        let now = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap();
        let n = compute_url_priority(Some(0.5), None, None, true, now);
        let k = compute_url_priority(Some(0.5), None, None, false, now);
        assert!(n < k);
    }

    #[test]
    fn recent_lastmod_is_more_urgent() {
        let now = Utc.with_ymd_and_hms(2026, 6, 1, 0, 0, 0).unwrap();
        let recent = now - Duration::hours(2);
        let old = now - Duration::days(120);
        let a = compute_url_priority(Some(0.5), Some(recent), None, false, now);
        let b = compute_url_priority(Some(0.5), Some(old), None, false, now);
        assert!(a < b);
    }

    #[test]
    fn lastmod_update_boosts() {
        let now = Utc.with_ymd_and_hms(2026, 6, 1, 0, 0, 0).unwrap();
        let prev = now - Duration::days(10);
        let cur = now - Duration::days(1);
        let boosted = compute_url_priority(Some(0.5), Some(cur), Some(prev), false, now);
        let flat = compute_url_priority(Some(0.5), Some(cur), Some(cur), false, now);
        assert!(boosted < flat);
    }
}
