-- 4-module diagnostics + GSC index monitoring
-- Adds per-URL SEO signals, Google/Bing index coverage, and GSC inspect task indexes.

ALTER TABLE urls
    ADD COLUMN IF NOT EXISTS meta_description TEXT,
    ADD COLUMN IF NOT EXISTS h1_content TEXT,
    ADD COLUMN IF NOT EXISTS google_index_status VARCHAR(32) NOT NULL DEFAULT 'UNKNOWN',
    ADD COLUMN IF NOT EXISTS google_coverage_state TEXT,
    ADD COLUMN IF NOT EXISTS google_last_crawled_at TIMESTAMPTZ,
    ADD COLUMN IF NOT EXISTS google_inspected_at TIMESTAMPTZ,
    ADD COLUMN IF NOT EXISTS bing_index_status VARCHAR(32) NOT NULL DEFAULT 'UNKNOWN',
    ADD COLUMN IF NOT EXISTS bing_last_crawled_at TIMESTAMPTZ,
    ADD COLUMN IF NOT EXISTS bing_inspected_at TIMESTAMPTZ;

CREATE INDEX IF NOT EXISTS idx_urls_site_g_index ON urls(site_id, google_index_status);
CREATE INDEX IF NOT EXISTS idx_urls_site_b_index ON urls(site_id, bing_index_status);
CREATE INDEX IF NOT EXISTS idx_urls_site_unchecked
    ON urls(site_id)
    WHERE last_checked_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_urls_site_g_inspect
    ON urls(site_id, google_inspected_at)
    WHERE google_index_status <> 'INDEXED';

ALTER TABLE health_checks
    ADD COLUMN IF NOT EXISTS meta_description TEXT,
    ADD COLUMN IF NOT EXISTS h1_content TEXT,
    ADD COLUMN IF NOT EXISTS robots_directive TEXT,
    ADD COLUMN IF NOT EXISTS payload_bytes INTEGER,
    ADD COLUMN IF NOT EXISTS hreflang TEXT;

ALTER TABLE sites
    ADD COLUMN IF NOT EXISTS gsc_analytics_synced_at TIMESTAMPTZ,
    ADD COLUMN IF NOT EXISTS gsc_property_url TEXT;

COMMENT ON COLUMN urls.meta_description IS 'Last observed <meta name="description"> from SEO quality gate';
COMMENT ON COLUMN urls.h1_content IS 'Last observed <h1> inner text from SEO quality gate';
COMMENT ON COLUMN urls.google_index_status IS 'UNKNOWN | INDEXED | CRAWLED_NOT_INDEXED | DISCOVERED_NOT_INDEXED';
COMMENT ON COLUMN urls.google_coverage_state IS 'Raw GSC URL Inspection coverageState (or Search Analytics note)';
COMMENT ON COLUMN urls.bing_index_status IS 'UNKNOWN | INDEXED (reserved for Bing webmaster signals)';

-- Standalone SEO scanner (CHECK_URL) and GSC inspection (GSC_INSPECT) queues.
CREATE INDEX IF NOT EXISTS idx_tasks_check_url
    ON tasks (site_id, priority, scheduled_at)
    WHERE status = 'PENDING' AND task_type = 'CHECK_URL';

CREATE INDEX IF NOT EXISTS idx_tasks_gsc_inspect
    ON tasks (site_id, priority, scheduled_at)
    WHERE status = 'PENDING' AND task_type = 'GSC_INSPECT';
