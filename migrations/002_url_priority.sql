-- Add SEO priority for URL inspection / scheduling
ALTER TABLE urls
    ADD COLUMN IF NOT EXISTS priority INTEGER NOT NULL DEFAULT 100;

CREATE INDEX IF NOT EXISTS idx_urls_site_priority ON urls(site_id, priority ASC);
