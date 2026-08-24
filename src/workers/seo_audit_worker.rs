use crate::application::HealthService;
use crate::config::AppConfig;
use crate::infrastructure::{HealthCheckRepo, UrlRepo};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Semaphore;
use tokio::task::JoinSet;
use tracing::{error, info};

#[derive(Clone)]
pub struct SeoAuditWorker {
    urls: UrlRepo,
    health_repo: HealthCheckRepo,
    health: HealthService,
    is_running: Arc<AtomicBool>,
    config: AppConfig,
}

impl SeoAuditWorker {
    pub fn new(
        urls: UrlRepo,
        health_repo: HealthCheckRepo,
        health: HealthService,
        is_running: Arc<AtomicBool>,
        config: AppConfig,
    ) -> Self {
        Self {
            urls,
            health_repo,
            health,
            is_running,
            config,
        }
    }

    pub fn start(self: Arc<Self>) {
        let interval = Duration::from_secs(self.config.worker_poll_interval_secs);
        tokio::spawn(async move {
            info!("SEO Audit Worker 待机就绪 (等待用户触发)");
            loop {
                if let Err(e) = self.tick().await {
                    error!(error = %e, "SEO audit worker tick failed");
                }
                tokio::time::sleep(interval).await;
            }
        });
    }

    async fn tick(&self) -> anyhow::Result<()> {
        // 未手动启动时直接跳过，零消耗
        if !self.is_running.load(Ordering::Relaxed) {
            return Ok(());
        }

        let pending = self.urls.fetch_pending_seo(300).await?;
        if pending.is_empty() {
            // 全部处理完毕，自动关闭开关回到待机
            self.is_running.store(false, Ordering::Relaxed);
            info!("🎉 全站 SEO 质检已全部完成，Worker 回到待机状态");
            return Ok(());
        }

        info!(count = pending.len(), "🛡️ [SEO Worker] 正在并发执行页面质检...");

        let semaphore = Arc::new(Semaphore::new(30));
        let mut set = JoinSet::new();

        for url in pending {
            if !self.is_running.load(Ordering::Relaxed) {
                break;
            }

            let sem = semaphore.clone();
            let health_svc = self.health.clone();
            let health_repo = self.health_repo.clone();
            let url_repo = self.urls.clone();

            set.spawn(async move {
                let _permit = sem.acquire().await.unwrap();
                let gate = health_svc.check_url(&url.url).await;
                let _ = health_repo.insert_from_gate(url.id, &gate).await;
                let _ = url_repo.persist_seo_scan(url.id, &gate).await;
                (url.url, gate.passed)
            });
        }

        let mut passed_count = 0;
        let mut blocked_count = 0;

        while let Some(res) = set.join_next().await {
            if let Ok((_url, passed)) = res {
                if passed {
                    passed_count += 1;
                } else {
                    blocked_count += 1;
                }
            }
        }

        info!(
            passed = passed_count,
            blocked = blocked_count,
            "✅ [SEO Worker] 本批次并发质检已完成并落库"
        );
        Ok(())
    }
}