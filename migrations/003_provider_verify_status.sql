-- Provider credential fill vs verify status
-- UNSET  = not filled
-- SAVED  = filled but not verified (or re-saved after edit)
-- VERIFIED = last channel test succeeded
-- FAILED   = last channel test failed

ALTER TABLE sites
    ADD COLUMN IF NOT EXISTS indexnow_status VARCHAR(32) NOT NULL DEFAULT 'UNSET',
    ADD COLUMN IF NOT EXISTS indexnow_last_error TEXT,
    ADD COLUMN IF NOT EXISTS indexnow_verified_at TIMESTAMPTZ,
    ADD COLUMN IF NOT EXISTS google_status VARCHAR(32) NOT NULL DEFAULT 'UNSET',
    ADD COLUMN IF NOT EXISTS google_last_error TEXT,
    ADD COLUMN IF NOT EXISTS google_verified_at TIMESTAMPTZ;

-- Backfill from existing credential columns
UPDATE sites
SET
    indexnow_status = CASE
        WHEN indexnow_key IS NOT NULL AND btrim(indexnow_key) <> '' THEN 'SAVED'
        ELSE 'UNSET'
    END,
    google_status = CASE
        WHEN google_service_account_json IS NOT NULL
             AND btrim(google_service_account_json) <> '' THEN 'SAVED'
        ELSE 'UNSET'
    END;
