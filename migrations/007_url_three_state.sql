-- V1 architecture: 3-state URL machine + site-scoped physical features
-- PENDING | SUBMITTED | BLOCKED  (strict conservation: total = pending + submitted + blocked)

ALTER TABLE urls
    ADD COLUMN IF NOT EXISTS locale VARCHAR(32) NOT NULL DEFAULT 'default',
    ADD COLUMN IF NOT EXISTS path_prefix VARCHAR(512) NOT NULL DEFAULT '/',
    ADD COLUMN IF NOT EXISTS page_title TEXT,
    ADD COLUMN IF NOT EXISTS canonical_url TEXT,
    ADD COLUMN IF NOT EXISTS block_reason TEXT;

COMMENT ON COLUMN urls.locale IS 'Language code: hreflang, else first path segment, else default';
COMMENT ON COLUMN urls.path_prefix IS 'First path directory after locale strip; root page is /';
COMMENT ON COLUMN urls.page_title IS 'Last observed <title> from inline quality gate';
COMMENT ON COLUMN urls.canonical_url IS 'Last observed canonical href from inline quality gate';
COMMENT ON COLUMN urls.block_reason IS 'Why this URL is BLOCKED (gate or submit failure)';

-- Converge legacy intermediate / error states into the 3 final states
UPDATE urls
SET status = 'PENDING',
    updated_at = NOW()
WHERE status IN (
    'DISCOVERED',
    'PENDING_CHECK',
    'CHECKING',
    'HEALTHY',
    'READY_SUBMIT',
    'SUBMITTING'
);

UPDATE urls
SET status = 'BLOCKED',
    block_reason = COALESCE(block_reason, 'legacy error/failed status'),
    updated_at = NOW()
WHERE status IN ('ERROR', 'FAILED');

-- Any unexpected leftover status (not the 3 finals) → PENDING so conservation holds
UPDATE urls
SET status = 'PENDING',
    updated_at = NOW()
WHERE status NOT IN ('PENDING', 'SUBMITTED', 'BLOCKED');

-- Retire independent check-queue tasks; quality gate now runs inside SubmitWorker
UPDATE tasks
SET
    status = 'SUCCESS',
    finished_at = COALESCE(finished_at, NOW()),
    last_error = 'retired: independent CHECK_URL worker removed (inline quality gate)',
    updated_at = NOW()
WHERE task_type = 'CHECK_URL'
  AND status IN ('PENDING', 'PROCESSING');

CREATE INDEX IF NOT EXISTS idx_urls_site_status ON urls(site_id, status);
CREATE INDEX IF NOT EXISTS idx_urls_site_path_prefix ON urls(site_id, path_prefix);
CREATE INDEX IF NOT EXISTS idx_urls_site_locale ON urls(site_id, locale);

DROP INDEX IF EXISTS idx_urls_status_ready;
