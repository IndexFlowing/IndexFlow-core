-- Demote historical stale sitemap URLs that are no longer part of active sitemaps
UPDATE urls 
SET discovered_via = 'gsc_orphan',
    updated_at = CURRENT_TIMESTAMP
WHERE site_id = 2 
  AND discovered_via = 'sitemap' 
  AND sitemap_synced_at < datetime('now', '-30 minutes');
