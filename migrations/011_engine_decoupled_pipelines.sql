-- Engine-decoupled pipelines: SUBMIT_BING and SUBMIT_GOOGLE task types.
--
-- New submissions use SUBMIT_BING / SUBMIT_GOOGLE instead of the legacy
-- SUBMIT_URL task. Both workers follow priority ASC so Sitemap priority is
-- preserved. SUBMIT_URL tasks remain for backward compatibility (legacy worker
-- drains them; scheduler retries them as usual).

-- Partial index for BingSubmitWorker claim queries.
CREATE INDEX IF NOT EXISTS idx_tasks_submit_bing
    ON tasks (site_id, priority, scheduled_at)
    WHERE status = 'PENDING' AND task_type = 'SUBMIT_BING';

-- Partial index for GoogleSubmitWorker claim queries.
CREATE INDEX IF NOT EXISTS idx_tasks_submit_google
    ON tasks (site_id, priority, scheduled_at)
    WHERE status = 'PENDING' AND task_type = 'SUBMIT_GOOGLE';

-- Allow the scheduler to retry SUBMIT_BING / SUBMIT_GOOGLE failures.
-- (The existing find_failed_for_retry query already covers SUBMIT_URL;
--  extend it to the new types via the task_type IN (...) check in code.)
