-- Pause Google Indexing API submits until this timestamp (UTC) after 429 quota errors
ALTER TABLE sites
    ADD COLUMN IF NOT EXISTS google_quota_paused_until TIMESTAMPTZ;
