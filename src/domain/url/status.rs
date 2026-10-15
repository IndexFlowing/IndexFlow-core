// src/domain/url/status.rs

pub const GINDEX_UNKNOWN: &str = "UNKNOWN";
pub const GINDEX_INDEXED: &str = "INDEXED";
pub const GINDEX_NOT_INDEXED: &str = "NOT_INDEXED";
pub const GINDEX_CRAWLED_NOT_INDEXED: &str = "CRAWLED_NOT_INDEXED";
pub const GINDEX_DISCOVERED_NOT_INDEXED: &str = "DISCOVERED_NOT_INDEXED";

/// 将 GSC 原始 coverageState 字符串规范化为系统标准索引枚举
pub fn coverage_to_index_status(coverage: &str) -> &'static str {
    let l = coverage.to_ascii_lowercase();

    // 1. 已编入索引
    if l.contains("indexed") && !l.contains("not indexed") {
        return GINDEX_INDEXED;
    }

    // 2. 细分原因：已抓取未编入
    if l.contains("crawled") {
        return GINDEX_CRAWLED_NOT_INDEXED;
    }

    // 3. 细分原因：发现待抓取
    if l.contains("discovered") {
        return GINDEX_DISCOVERED_NOT_INDEXED;
    }

    // 4. Google 完全未收录 / 未知 (核心修复：彻底覆盖各种未收录形态)
    if l.contains("not indexed")
        || l.contains("unknown to google")
        || l.contains("not on google")
        || l.contains("excluded")
        || l.contains("redirect")
        || l.contains("404")
        || l.contains("blocked")
        || l.contains("duplicate")
    {
        return GINDEX_NOT_INDEXED;
    }

    // 兜底
    GINDEX_UNKNOWN
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_coverage_mapping() {
        assert_eq!(coverage_to_index_status("Crawled - currently not indexed"), GINDEX_CRAWLED_NOT_INDEXED);
        assert_eq!(coverage_to_index_status("Discovered - currently not indexed"), GINDEX_DISCOVERED_NOT_INDEXED);
        assert_eq!(coverage_to_index_status("Submitted and indexed"), GINDEX_INDEXED);
        assert_eq!(coverage_to_index_status("Indexed, not submitted in sitemap"), GINDEX_INDEXED);
        
        // 核心测试：验证修复 URL is unknown to Google
        assert_eq!(coverage_to_index_status("URL is unknown to Google"), GINDEX_NOT_INDEXED);
        assert_eq!(coverage_to_index_status("URL is not on Google"), GINDEX_NOT_INDEXED);
        assert_eq!(coverage_to_index_status("Page with redirect"), GINDEX_NOT_INDEXED);
        assert_eq!(coverage_to_index_status("Not found (404)"), GINDEX_NOT_INDEXED);
    }
}