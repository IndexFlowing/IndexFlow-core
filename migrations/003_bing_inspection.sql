-- 1. 为 sites 表增加 Bing Webmaster Tools API 密钥字段
ALTER TABLE sites ADD COLUMN bing_webmaster_api_key TEXT;

-- 2. 为 urls 表增加 Bing 官方收录检测相关状态字段
ALTER TABLE urls ADD COLUMN bing_index_status TEXT NOT NULL DEFAULT 'UNKNOWN'; -- INDEXED | NOT_INDEXED | UNKNOWN
ALTER TABLE urls ADD COLUMN bing_coverage_state TEXT;                          -- Bing 原始返回覆盖状态
ALTER TABLE urls ADD COLUMN bing_last_crawled_at TEXT;
ALTER TABLE urls ADD COLUMN bing_inspected_at TEXT;

-- 3. 创建索引以优化后台 Worker 调度与大盘统计
CREATE INDEX IF NOT EXISTS idx_urls_bing_index_status ON urls(site_id, bing_index_status);