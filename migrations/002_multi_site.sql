-- 1. 创建全新的多站点管理表 sites（去除 id=1 约束，支持自增）
CREATE TABLE IF NOT EXISTS sites (
    id                          INTEGER PRIMARY KEY AUTOINCREMENT,
    domain                      TEXT NOT NULL UNIQUE,
    sitemap_url                 TEXT,
    bing_indexnow_key           TEXT,
    google_service_account_json TEXT,
    gsc_property_url            TEXT,
    gsc_daily_quota             INTEGER NOT NULL DEFAULT 2000,
    google_daily_quota          INTEGER NOT NULL DEFAULT 200,
    google_quota_paused_until   TEXT,
    created_at                  TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at                  TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- 2. 将原 site_config 的数据无损迁移至 sites
INSERT OR IGNORE INTO sites (
    id, domain, sitemap_url, bing_indexnow_key, google_service_account_json,
    gsc_property_url, gsc_daily_quota, google_daily_quota, google_quota_paused_until,
    created_at, updated_at
)
SELECT 
    id, domain, sitemap_url, bing_indexnow_key, google_service_account_json,
    gsc_property_url, gsc_daily_quota, google_daily_quota, google_quota_paused_until,
    created_at, updated_at
FROM site_config WHERE id = 1;

-- 3. 给 urls 表新增 site_id 外键，默认归属到站点 1 (保护已有 1 万多条数据)
ALTER TABLE urls ADD COLUMN site_id INTEGER NOT NULL DEFAULT 1;

-- 4. 创建多站点高效联合索引
CREATE INDEX IF NOT EXISTS idx_urls_site_id ON urls(site_id);
CREATE INDEX IF NOT EXISTS idx_urls_site_status ON urls(site_id, seo_status, gsc_index_status);
CREATE INDEX IF NOT EXISTS idx_urls_site_priority ON urls(site_id, priority ASC);