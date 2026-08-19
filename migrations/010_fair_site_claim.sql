-- Fair per-site claim + quota circuit EXISTS joins.
CREATE INDEX IF NOT EXISTS idx_tasks_claim_by_site
    ON tasks (site_id, task_type, priority, scheduled_at)
    WHERE status = 'PENDING';

CREATE INDEX IF NOT EXISTS idx_tasks_pending_type_site
    ON tasks (task_type, site_id, scheduled_at)
    WHERE status = 'PENDING';
