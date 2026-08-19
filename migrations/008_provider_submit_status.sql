-- Per-engine submit outcome (does not replace the 3-state lifecycle).
-- NONE | SUBMITTED | FAILED

ALTER TABLE urls
    ADD COLUMN IF NOT EXISTS bing_status VARCHAR(32) NOT NULL DEFAULT 'NONE',
    ADD COLUMN IF NOT EXISTS google_status VARCHAR(32) NOT NULL DEFAULT 'NONE',
    ADD COLUMN IF NOT EXISTS bing_submitted_at TIMESTAMPTZ,
    ADD COLUMN IF NOT EXISTS google_submitted_at TIMESTAMPTZ,
    ADD COLUMN IF NOT EXISTS bing_error TEXT,
    ADD COLUMN IF NOT EXISTS google_error TEXT;

COMMENT ON COLUMN urls.bing_status IS 'IndexNow outcome: NONE | SUBMITTED | FAILED';
COMMENT ON COLUMN urls.google_status IS 'Google Indexing API outcome: NONE | SUBMITTED | FAILED';

-- Latest successful push per provider
UPDATE urls u
SET
    bing_status = 'SUBMITTED',
    bing_submitted_at = s.created_at,
    bing_error = NULL
FROM (
    SELECT DISTINCT ON (url_id) url_id, created_at
    FROM submission_logs
    WHERE provider = 'bing' AND success = true
    ORDER BY url_id, created_at DESC
) s
WHERE u.id = s.url_id;

UPDATE urls u
SET
    google_status = 'SUBMITTED',
    google_submitted_at = s.created_at,
    google_error = NULL
FROM (
    SELECT DISTINCT ON (url_id) url_id, created_at
    FROM submission_logs
    WHERE provider = 'google' AND success = true
    ORDER BY url_id, created_at DESC
) s
WHERE u.id = s.url_id;

-- Latest failure only when that provider never succeeded
UPDATE urls u
SET
    bing_status = 'FAILED',
    bing_error = LEFT(s.response_body, 500)
FROM (
    SELECT DISTINCT ON (url_id) url_id, response_body
    FROM submission_logs
    WHERE provider = 'bing' AND success = false
    ORDER BY url_id, created_at DESC
) s
WHERE u.id = s.url_id
  AND u.bing_status = 'NONE';

UPDATE urls u
SET
    google_status = 'FAILED',
    google_error = LEFT(s.response_body, 500)
FROM (
    SELECT DISTINCT ON (url_id) url_id, response_body
    FROM submission_logs
    WHERE provider = 'google' AND success = false
    ORDER BY url_id, created_at DESC
) s
WHERE u.id = s.url_id
  AND u.google_status = 'NONE';
