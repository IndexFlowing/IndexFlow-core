use crate::domain::{Task, TaskStatus, TaskType};
use chrono::{DateTime, Utc};
use sqlx::PgPool;

#[derive(Clone)]
pub struct TaskRepo {
    pool: PgPool,
}

impl TaskRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn create(
        &self,
        site_id: i64,
        url_id: Option<i64>,
        sitemap_id: Option<i64>,
        task_type: TaskType,
        priority: i32,
        scheduled_at: DateTime<Utc>,
    ) -> anyhow::Result<Option<Task>> {
        // Unique partial indexes skip duplicates for pending work.
        let task = sqlx::query_as::<_, Task>(
            r#"
            INSERT INTO tasks (site_id, url_id, sitemap_id, task_type, status, priority, scheduled_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            ON CONFLICT DO NOTHING
            RETURNING *
            "#,
        )
        .bind(site_id)
        .bind(url_id)
        .bind(sitemap_id)
        .bind(task_type.as_str())
        .bind(TaskStatus::Pending.as_str())
        .bind(priority)
        .bind(scheduled_at)
        .fetch_optional(&self.pool)
        .await?;
        Ok(task)
    }

    /// Create SUBMIT_URL tasks; priority from urls.priority when available.
    /// Kept for backward compatibility — new submissions use create_bing/google_tasks_batch.
    #[allow(dead_code)]
    pub async fn create_submit_tasks_batch(
        &self,
        site_id: i64,
        url_ids: &[i64],
        fallback_priority: i32,
    ) -> anyhow::Result<u64> {
        self.create_engine_tasks_batch(site_id, url_ids, fallback_priority, TaskType::SubmitUrl)
            .await
    }

    /// Create SUBMIT_BING tasks; priority from urls.priority when available.
    pub async fn create_bing_tasks_batch(
        &self,
        site_id: i64,
        url_ids: &[i64],
        fallback_priority: i32,
    ) -> anyhow::Result<u64> {
        self.create_engine_tasks_batch(site_id, url_ids, fallback_priority, TaskType::SubmitBing)
            .await
    }

    /// Create SUBMIT_GOOGLE tasks; priority from urls.priority when available.
    pub async fn create_google_tasks_batch(
        &self,
        site_id: i64,
        url_ids: &[i64],
        fallback_priority: i32,
    ) -> anyhow::Result<u64> {
        self.create_engine_tasks_batch(site_id, url_ids, fallback_priority, TaskType::SubmitGoogle)
            .await
    }

    /// Create CHECK_URL tasks for the standalone SEO quality scanner.
    pub async fn create_check_tasks_batch(
        &self,
        site_id: i64,
        url_ids: &[i64],
        fallback_priority: i32,
    ) -> anyhow::Result<u64> {
        self.create_engine_tasks_batch(site_id, url_ids, fallback_priority, TaskType::CheckUrl)
            .await
    }

    /// Create GSC_INSPECT tasks (URL Inspection API).
    pub async fn create_gsc_inspect_tasks_batch(
        &self,
        site_id: i64,
        url_ids: &[i64],
        fallback_priority: i32,
    ) -> anyhow::Result<u64> {
        self.create_engine_tasks_batch(site_id, url_ids, fallback_priority, TaskType::GscInspect)
            .await
    }

    pub async fn count_pending_type(&self, site_id: i64, task_type: TaskType) -> anyhow::Result<i64> {
        let row: (i64,) = sqlx::query_as(
            r#"
            SELECT COUNT(*)::bigint FROM tasks
            WHERE site_id = $1 AND task_type = $2 AND status IN ('PENDING', 'PROCESSING')
            "#,
        )
        .bind(site_id)
        .bind(task_type.as_str())
        .fetch_one(&self.pool)
        .await?;
        Ok(row.0)
    }

    /// Internal: create engine-typed tasks for a batch of URL ids.
    async fn create_engine_tasks_batch(
        &self,
        site_id: i64,
        url_ids: &[i64],
        fallback_priority: i32,
        task_type: TaskType,
    ) -> anyhow::Result<u64> {
        if url_ids.is_empty() {
            return Ok(0);
        }
        let mut inserted = 0u64;
        let now = Utc::now();
        let rows: Vec<(i64, i32)> = sqlx::query_as(
            r#"SELECT id, priority FROM urls WHERE id = ANY($1)"#,
        )
        .bind(url_ids)
        .fetch_all(&self.pool)
        .await?;
        let map: std::collections::HashMap<i64, i32> = rows.into_iter().collect();

        for url_id in url_ids {
            let p = map.get(url_id).copied().unwrap_or(fallback_priority);
            let res = self
                .create(
                    site_id,
                    Some(*url_id),
                    None,
                    task_type,
                    p,
                    now,
                )
                .await?;
            if res.is_some() {
                inserted += 1;
            }
        }
        Ok(inserted)
    }

    /// Atomically claim pending tasks of a given type (FOR UPDATE SKIP LOCKED).
    ///
    /// Global FIFO — used by SYNC_SITEMAP. Submit work uses
    /// [`Self::pending_site_ids`] + [`Self::claim_for_site`] so sites share the worker.
    pub async fn claim(
        &self,
        task_type: TaskType,
        limit: i64,
    ) -> anyhow::Result<Vec<Task>> {
        let tasks = sqlx::query_as::<_, Task>(
            r#"
            UPDATE tasks
            SET
                status = $1,
                started_at = NOW(),
                locked_at = NOW(),
                updated_at = NOW()
            WHERE id IN (
                SELECT id FROM tasks
                WHERE status = $2
                  AND task_type = $3
                  AND scheduled_at <= NOW()
                ORDER BY priority ASC, scheduled_at ASC
                LIMIT $4
                FOR UPDATE SKIP LOCKED
            )
            RETURNING *
            "#,
        )
        .bind(TaskStatus::Processing.as_str())
        .bind(TaskStatus::Pending.as_str())
        .bind(task_type.as_str())
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;
        Ok(tasks)
    }

    /// Distinct sites that currently have claimable tasks of this type.
    pub async fn pending_site_ids(&self, task_type: TaskType) -> anyhow::Result<Vec<i64>> {
        let rows: Vec<(i64,)> = sqlx::query_as(
            r#"
            SELECT DISTINCT site_id
            FROM tasks
            WHERE status = $1
              AND task_type = $2
              AND scheduled_at <= NOW()
            ORDER BY site_id
            "#,
        )
        .bind(TaskStatus::Pending.as_str())
        .bind(task_type.as_str())
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.into_iter().map(|r| r.0).collect())
    }

    /// Atomically claim pending tasks for one site (FOR UPDATE SKIP LOCKED).
    pub async fn claim_for_site(
        &self,
        site_id: i64,
        task_type: TaskType,
        limit: i64,
    ) -> anyhow::Result<Vec<Task>> {
        let tasks = sqlx::query_as::<_, Task>(
            r#"
            UPDATE tasks
            SET
                status = $1,
                started_at = NOW(),
                locked_at = NOW(),
                updated_at = NOW()
            WHERE id IN (
                SELECT id FROM tasks
                WHERE status = $2
                  AND task_type = $3
                  AND site_id = $5
                  AND scheduled_at <= NOW()
                ORDER BY priority ASC, scheduled_at ASC
                LIMIT $4
                FOR UPDATE SKIP LOCKED
            )
            RETURNING *
            "#,
        )
        .bind(TaskStatus::Processing.as_str())
        .bind(TaskStatus::Pending.as_str())
        .bind(task_type.as_str())
        .bind(limit)
        .bind(site_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(tasks)
    }

    /// Sleep every claimable task of this type until `scheduled_at` (quota circuit).
    /// Does not increment `retry_count` — quota exhaustion is not a task failure.
    pub async fn sleep_pending_for_site(
        &self,
        site_id: i64,
        task_type: TaskType,
        scheduled_at: DateTime<Utc>,
        error: &str,
    ) -> anyhow::Result<u64> {
        let result = sqlx::query(
            r#"
            UPDATE tasks
            SET
                scheduled_at = $2,
                last_error = $3,
                updated_at = NOW()
            WHERE site_id = $1
              AND status = $4
              AND task_type = $5
              AND scheduled_at <= NOW()
            "#,
        )
        .bind(site_id)
        .bind(scheduled_at)
        .bind(error)
        .bind(TaskStatus::Pending.as_str())
        .bind(task_type.as_str())
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected())
    }

    /// Fail every claimable task of this type for a site (no usable provider).
    pub async fn fail_pending_for_site(
        &self,
        site_id: i64,
        task_type: TaskType,
        error: &str,
    ) -> anyhow::Result<u64> {
        let result = sqlx::query(
            r#"
            UPDATE tasks
            SET
                status = $1,
                finished_at = NOW(),
                last_error = $2,
                retry_count = retry_count + 1,
                updated_at = NOW()
            WHERE site_id = $3
              AND status = $4
              AND task_type = $5
              AND scheduled_at <= NOW()
            "#,
        )
        .bind(TaskStatus::Failed.as_str())
        .bind(error)
        .bind(site_id)
        .bind(TaskStatus::Pending.as_str())
        .bind(task_type.as_str())
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected())
    }

    pub async fn mark_success(&self, id: i64) -> anyhow::Result<()> {
        sqlx::query(
            r#"
            UPDATE tasks
            SET status = $1, finished_at = NOW(), updated_at = NOW(), last_error = NULL
            WHERE id = $2
            "#,
        )
        .bind(TaskStatus::Success.as_str())
        .bind(id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn mark_failed(&self, id: i64, error: &str) -> anyhow::Result<()> {
        sqlx::query(
            r#"
            UPDATE tasks
            SET
                status = $1,
                finished_at = NOW(),
                last_error = $2,
                retry_count = retry_count + 1,
                updated_at = NOW()
            WHERE id = $3
            "#,
        )
        .bind(TaskStatus::Failed.as_str())
        .bind(error)
        .bind(id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Put a claimed task back to PENDING at `scheduled_at` without burning a retry.
    /// Used for quota / circuit-breaker sleeps.
    pub async fn reschedule(
        &self,
        id: i64,
        scheduled_at: DateTime<Utc>,
        error: Option<&str>,
    ) -> anyhow::Result<()> {
        sqlx::query(
            r#"
            UPDATE tasks
            SET
                status = $1,
                scheduled_at = $2,
                started_at = NULL,
                finished_at = NULL,
                locked_at = NULL,
                last_error = $3,
                updated_at = NOW()
            WHERE id = $4
            "#,
        )
        .bind(TaskStatus::Pending.as_str())
        .bind(scheduled_at)
        .bind(error)
        .bind(id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn requeue(
        &self,
        id: i64,
        scheduled_at: DateTime<Utc>,
        error: Option<&str>,
    ) -> anyhow::Result<()> {
        sqlx::query(
            r#"
            UPDATE tasks
            SET
                status = $1,
                scheduled_at = $2,
                started_at = NULL,
                finished_at = NULL,
                locked_at = NULL,
                last_error = $3,
                retry_count = retry_count + 1,
                updated_at = NOW()
            WHERE id = $4
            "#,
        )
        .bind(TaskStatus::Pending.as_str())
        .bind(scheduled_at)
        .bind(error)
        .bind(id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn list(
        &self,
        status: Option<&str>,
        page: i64,
        limit: i64,
    ) -> anyhow::Result<(Vec<Task>, i64)> {
        let offset = (page.max(1) - 1) * limit;

        let total: (i64,) = if let Some(st) = status {
            sqlx::query_as(r#"SELECT COUNT(*) FROM tasks WHERE status = $1"#)
                .bind(st)
                .fetch_one(&self.pool)
                .await?
        } else {
            sqlx::query_as(r#"SELECT COUNT(*) FROM tasks"#)
                .fetch_one(&self.pool)
                .await?
        };

        let rows = if let Some(st) = status {
            sqlx::query_as::<_, Task>(
                r#"
                SELECT * FROM tasks
                WHERE status = $1
                ORDER BY created_at DESC
                LIMIT $2 OFFSET $3
                "#,
            )
            .bind(st)
            .bind(limit)
            .bind(offset)
            .fetch_all(&self.pool)
            .await?
        } else {
            sqlx::query_as::<_, Task>(
                r#"
                SELECT * FROM tasks
                ORDER BY created_at DESC
                LIMIT $1 OFFSET $2
                "#,
            )
            .bind(limit)
            .bind(offset)
            .fetch_all(&self.pool)
            .await?
        };

        Ok((rows, total.0))
    }

    /// Failed tasks eligible for automatic retry (retry_count < max).
    pub async fn find_failed_for_retry(
        &self,
        max_retry: i32,
        limit: i64,
    ) -> anyhow::Result<Vec<Task>> {
        let rows = sqlx::query_as::<_, Task>(
            r#"
            SELECT * FROM tasks
            WHERE status = 'FAILED'
              AND retry_count < $1
              AND task_type IN ('SUBMIT_URL', 'RETRY_SUBMISSION', 'SYNC_SITEMAP', 'SUBMIT_BING', 'SUBMIT_GOOGLE', 'CHECK_URL', 'GSC_INSPECT')
            ORDER BY finished_at ASC NULLS FIRST
            LIMIT $2
            "#,
        )
        .bind(max_retry)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    pub async fn retry_now(&self, id: i64) -> anyhow::Result<Option<Task>> {
        let task = sqlx::query_as::<_, Task>(
            r#"
            UPDATE tasks
            SET
                status = 'PENDING',
                scheduled_at = NOW(),
                started_at = NULL,
                finished_at = NULL,
                locked_at = NULL,
                last_error = NULL,
                updated_at = NOW()
            WHERE id = $1
              AND status IN ('FAILED', 'SUCCESS')
            RETURNING *
            "#,
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(task)
    }

    pub async fn site_queue_counts(&self, site_id: i64) -> anyhow::Result<Vec<TaskQueueCount>> {
        let rows = sqlx::query_as::<_, TaskQueueCount>(
            r#"
            SELECT task_type, status, COUNT(*)::bigint AS count
            FROM tasks
            WHERE site_id = $1
              AND status IN ('PENDING', 'PROCESSING')
            GROUP BY task_type, status
            "#,
        )
        .bind(site_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    pub async fn all_sites_queue_counts(&self) -> anyhow::Result<Vec<SiteTaskQueueCount>> {
        let rows = sqlx::query_as::<_, SiteTaskQueueCount>(
            r#"
            SELECT site_id, task_type, status, COUNT(*)::bigint AS count
            FROM tasks
            WHERE status IN ('PENDING', 'PROCESSING')
            GROUP BY site_id, task_type, status
            "#,
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    /// Recover stale PROCESSING tasks that timed out (mark FAILED).
    pub async fn recover_stale_processing(&self, timeout_minutes: i64) -> anyhow::Result<u64> {
        let result = sqlx::query(
            r#"
            UPDATE tasks
            SET
                status = 'FAILED',
                finished_at = NOW(),
                last_error = 'Task timed out in PROCESSING (stale lock recovered)',
                retry_count = retry_count + 1,
                updated_at = NOW()
            WHERE status = 'PROCESSING'
              AND (locked_at < NOW() - ($1 * INTERVAL '1 minute') 
                   OR (locked_at IS NULL AND updated_at < NOW() - ($1 * INTERVAL '1 minute')))
            "#,
        )
        .bind(timeout_minutes)
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected())
    }
}

#[derive(Debug, Clone, serde::Serialize, sqlx::FromRow)]
pub struct TaskQueueCount {
    pub task_type: String,
    pub status: String,
    pub count: i64,
}

#[derive(Debug, Clone, serde::Serialize, sqlx::FromRow)]
pub struct SiteTaskQueueCount {
    pub site_id: i64,
    pub task_type: String,
    pub status: String,
    pub count: i64,
}
