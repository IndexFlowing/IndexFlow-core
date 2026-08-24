-- 1. 单站点配置表 (固定 ID = 1)
CREATE TABLE IF NOT EXISTS site_config (
    id                          INTEGER PRIMARY KEY CHECK (id = 1),
    domain                      TEXT NOT NULL,
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

-- 2. URL 核心资产表 (直接回答 IndexFlow 5 大核心问题)
CREATE TABLE IF NOT EXISTS urls (
    id                  INTEGER PRIMARY KEY AUTOINCREMENT,
    url                 TEXT NOT NULL UNIQUE,
    url_hash            TEXT NOT NULL UNIQUE,
    
    -- 基础 SEO 诊断
    seo_status          TEXT NOT NULL DEFAULT 'PENDING',  -- PASS | WARN | FAIL | PENDING
    seo_issue           TEXT,                             -- 404 / noindex / canonical_mismatch / missing_title
    page_title          TEXT,
    meta_description    TEXT,
    h1_content          TEXT,
    canonical_url       TEXT,
    http_status         INTEGER,
    locale              TEXT NOT NULL DEFAULT 'default',
    path_prefix         TEXT NOT NULL DEFAULT '/',
    
    -- Google 收录状态 (GSC URL Inspection)
    gsc_index_status    TEXT NOT NULL DEFAULT 'UNKNOWN',  -- INDEXED | NOT_INDEXED | UNKNOWN
    gsc_coverage_state  TEXT,                             -- GSC 原始状态
    gsc_last_crawled_at TEXT,
    gsc_inspected_at    TEXT,
    
    -- 提交记录
    bing_status         TEXT NOT NULL DEFAULT 'NONE',     -- NONE | SUBMITTED | FAILED
    bing_submitted_at   TEXT,
    bing_error          TEXT,
    
    google_status       TEXT NOT NULL DEFAULT 'NONE',     -- NONE | SUBMITTED | FAILED
    google_submitted_at TEXT,
    google_error        TEXT,

    priority            INTEGER NOT NULL DEFAULT 100,
    sitemap_lastmod     TEXT,
    last_checked_at     TEXT,
    first_seen_at       TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    created_at          TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at          TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_urls_seo_status ON urls(seo_status);
CREATE INDEX IF NOT EXISTS idx_urls_gsc_status ON urls(gsc_index_status);
CREATE INDEX IF NOT EXISTS idx_urls_bing_status ON urls(bing_status);
CREATE INDEX IF NOT EXISTS idx_urls_google_status ON urls(google_status);
CREATE INDEX IF NOT EXISTS idx_urls_priority ON urls(priority ASC);

-- 3. 管理员用户表
CREATE TABLE IF NOT EXISTS admin_users (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    username      TEXT NOT NULL UNIQUE,
    password_hash TEXT NOT NULL,
    created_at    TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at    TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- 4. 提交日志表
CREATE TABLE IF NOT EXISTS submission_logs (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    url_id          INTEGER NOT NULL REFERENCES urls(id) ON DELETE CASCADE,
    provider        TEXT NOT NULL, -- google | bing
    success         INTEGER NOT NULL DEFAULT 0, -- 0 | 1
    response_code   INTEGER,
    response_body   TEXT,
    created_at      TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_submission_logs_url ON submission_logs(url_id, created_at DESC);

-- 5. 健康检查历史表
CREATE TABLE IF NOT EXISTS health_checks (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    url_id          INTEGER NOT NULL REFERENCES urls(id) ON DELETE CASCADE,
    http_status     INTEGER,
    response_time   INTEGER,
    has_noindex     INTEGER NOT NULL DEFAULT 0,
    has_canonical   INTEGER NOT NULL DEFAULT 0,
    meta_description TEXT,
    h1_content      TEXT,
    robots_directive TEXT,
    payload_bytes   INTEGER,
    hreflang        TEXT,
    checked_at      TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_health_checks_url ON health_checks(url_id, checked_at DESC);