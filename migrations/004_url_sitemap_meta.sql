-- Store raw sitemap signals for priority recomputation
ALTER TABLE urls
    ADD COLUMN IF NOT EXISTS sitemap_priority DOUBLE PRECISION,
    ADD COLUMN IF NOT EXISTS sitemap_lastmod TIMESTAMPTZ;

COMMENT ON COLUMN urls.priority IS 'Computed schedule priority (lower = higher urgency)';
COMMENT ON COLUMN urls.sitemap_priority IS 'Raw <priority> from sitemap (0.0-1.0)';
COMMENT ON COLUMN urls.sitemap_lastmod IS 'Raw <lastmod> from sitemap';
