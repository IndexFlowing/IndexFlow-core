ALTER TABLE urls ADD COLUMN is_watched INTEGER NOT NULL DEFAULT 0;
ALTER TABLE urls ADD COLUMN watched_at TEXT;

CREATE INDEX IF NOT EXISTS idx_urls_is_watched ON urls(site_id, is_watched);

CREATE TABLE IF NOT EXISTS index_status_history (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    url_id INTEGER NOT NULL REFERENCES urls(id) ON DELETE CASCADE,
    provider TEXT NOT NULL,
    index_status TEXT NOT NULL,
    coverage_state TEXT,
    last_crawled_at TEXT,
    checked_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_index_status_history_url
    ON index_status_history(url_id, provider, checked_at DESC);
