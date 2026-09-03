pub mod admin_repo;
pub mod health_check_repo;
pub mod index_history_repo;
pub mod site_repo;
pub mod submission_log_repo;
pub mod url_repo; // 自动指向 url_repo/ 模块目录

pub use admin_repo::AdminRepo;
pub use health_check_repo::HealthCheckRepo;
pub use index_history_repo::IndexHistoryRepo;
pub use site_repo::{Site, SiteRepo};
pub use submission_log_repo::SubmissionLogRepo;
pub use url_repo::UrlRepo;

pub use crate::domain::DashboardStats;
