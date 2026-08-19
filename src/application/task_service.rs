use crate::domain::Task;
use crate::infrastructure::TaskRepo;

#[derive(Clone)]
pub struct TaskService {
    tasks: TaskRepo,
}

impl TaskService {
    pub fn new(tasks: TaskRepo) -> Self {
        Self { tasks }
    }

    pub async fn list(
        &self,
        status: Option<&str>,
        page: i64,
        limit: i64,
    ) -> anyhow::Result<(Vec<Task>, i64)> {
        self.tasks.list(status, page, limit).await
    }

    pub async fn retry(&self, id: i64) -> anyhow::Result<Option<Task>> {
        self.tasks.retry_now(id).await
    }
}
