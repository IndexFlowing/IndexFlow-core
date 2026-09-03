pub const GINDEX_UNKNOWN: &str = "UNKNOWN";
pub const GINDEX_INDEXED: &str = "INDEXED";
pub const GINDEX_NOT_INDEXED: &str = "NOT_INDEXED";
pub const GINDEX_CRAWLED_NOT_INDEXED: &str = "CRAWLED_NOT_INDEXED";
pub const GINDEX_DISCOVERED_NOT_INDEXED: &str = "DISCOVERED_NOT_INDEXED";

/// 将 GSC 原始 coverageState 字符串规范化为系统标准索引枚举
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_coverage_mapping() {
        assert_eq!(coverage_to_index_status("Crawled - currently not indexed"), GINDEX_CRAWLED_NOT_INDEXED);
        assert_eq!(coverage_to_index_status("Discovered - currently not indexed"), GINDEX_DISCOVERED_NOT_INDEXED);
        assert_eq!(coverage_to_index_status("Submitted and indexed"), GINDEX_INDEXED);
        assert_eq!(coverage_to_index_status("URL is unknown to Google"), GINDEX_UNKNOWN);
    }
}