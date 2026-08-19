-- Decouple overall 3-state from per-engine submit progress.
-- SUBMITTED (lifecycle) = every verified engine on the site has bing/google_status = SUBMITTED.
-- Partial (Bing done, Google still NONE) must stay PENDING so start_submit can resume.

CREATE INDEX IF NOT EXISTS idx_urls_site_bing_status ON urls (site_id, bing_status);
CREATE INDEX IF NOT EXISTS idx_urls_site_google_status ON urls (site_id, google_status);

-- Roll back false-saturated SUBMITTED rows: an enabled engine is still NONE/FAILED.
UPDATE urls u
SET
    status = 'PENDING',
    updated_at = NOW()
FROM sites s
WHERE u.site_id = s.id
  AND u.status = 'SUBMITTED'
  AND (
        (s.indexnow_status = 'VERIFIED' AND u.bing_status IN ('NONE', 'FAILED'))
     OR (s.google_status = 'VERIFIED' AND u.google_status IN ('NONE', 'FAILED'))
  );
