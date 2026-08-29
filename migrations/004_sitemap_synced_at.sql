ALTER TABLE urls ADD COLUMN sitemap_synced_at TEXT;

UPDATE urls
SET sitemap_synced_at = updated_at
WHERE sitemap_synced_at IS NULL;
