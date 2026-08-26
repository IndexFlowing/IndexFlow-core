pub mod admin_repo;
pub mod health_check_repo;
pub mod site_repo;
pub mod submission_log_repo;
pub mod url_repo;

pub use admin_repo::AdminRepo;
pub use health_check_repo::HealthCheckRepo;
pub use site_repo::{Site, SiteRepo};
pub use submission_log_repo::SubmissionLogRepo;
pub use url_repo::UrlRepo;

// 向后兼容重导出 DashboardStats
pub use crate::domain::DashboardStats;