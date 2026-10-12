// src/workers/sync_worker.rs
use crate::application::{PipelineManager, SitemapService};
use crate::config::AppConfig;
use crate::domain::PipelineStage;
use crate::infrastructure::{SiteRepo, UrlRepo};
use std::sync::Arc;
use std::time::Duration;
use tracing::{error, info, warn};

#[derive(Clone)]
pub struct SyncWorker {
    urls: UrlRepo,
    sites: SiteRepo,
    sitemap_service: SitemapService,
    pipeline: PipelineManager,
    config: AppConfig,
}

impl SyncWorker {
    pub fn new(
        urls: UrlRepo,
        sites: SiteRepo,
        sitemap_service: SitemapService,
        pipeline: PipelineManager,
        config: AppConfig,
    ) -> Self {
        Self {
            urls,
            sites,
            sitemap_service,
            pipeline,
            config,
        }
    }

    pub fn start(self: Arc<Self>) {
        let interval = Duration::from_secs(self.config.worker_poll_interval_secs);
        tokio::spawn(async move {
            info!("Sitemap Sync Worker 待机就绪");
            loop {
                if let Err(e) = self.tick().await {
                    error!(error = %e, "sync worker tick failed");
                }
                tokio::time::sleep(interval).await;
            }
        });
    }

    async fn tick(&self) -> anyhow::Result<()> {
        let running_sites = self.pipeline.running_sites_for_stage(PipelineStage::Sitemap);
        if running_sites.is_empty() {
            return Ok(());
        }

        for site_id in running_sites {
            if !self.pipeline.is_running(site_id, PipelineStage::Sitemap) {
                continue;
            }

            let site = match self.sites.find_by_id(site_id).await {
                Ok(Some(s)) => s,
                _ => {
                    self.pipeline.stop(site_id, PipelineStage::Sitemap);
                    continue;
                }
            };

            let Some(ref sm_url) = site.sitemap_url else {
                warn!(site_id, domain = %site.domain, "站点未配置 Sitemap URL，跳过同步");
                self.pipeline.stop(site_id, PipelineStage::Sitemap);
                continue;
            };

            let effective_ua = site.effective_crawler_ua();

            info!("==================================================");
            info!(
                site_id,
                domain = %site.domain,
                sitemap_url = %sm_url,
                ua = ?effective_ua,
                "🌐 [SyncWorker] 开始连接目标 Sitemap 并流式解析..."
            );

            let started = std::time::Instant::now();
            let sync_start_time = chrono::Utc::now();
            let (sitemap_type, entries) = match self
                .sitemap_service
                .expand_to_page_entries(sm_url, 3, effective_ua.as_deref())
                .await
            {
                Ok(res) => res,
                Err(e) => {
                    error!(
                        error = %e,
                        site_id,
                        sitemap_url = %sm_url,
                        "❌ [SyncWorker] Sitemap 下载或解析彻底失败"
                    );
                    self.pipeline.stop(site_id, PipelineStage::Sitemap);
                    continue;
                }
            };

            let elapsed = started.elapsed();
            info!(
                site_id,
                domain = %site.domain,
                elapsed_ms = elapsed.as_millis() as u64,
                sitemap_type = ?sitemap_type,
                total_urls_fetched = entries.len(),
                "📊 [SyncWorker] Sitemap 解析完毕！成功抓取到的 URL 总数: {}",
                entries.len()
            );

            // 采样打印前 5 条和后 3 条，一眼看出抓取的究竟是不是最新数据
            if !entries.is_empty() {
                info!("🔍 [SyncWorker] 抓取样本核对 (前 5 条):");
                for (idx, entry) in entries.iter().take(5).enumerate() {
                    info!(
                        "  [#{}] loc: {} | lastmod: {:?}",
                        idx + 1,
                        entry.loc,
                        entry.lastmod
                    );
                }
                if entries.len() > 5 {
                    info!(
                        "  ... 中间省略 {} 条 ...",
                        entries.len().saturating_sub(8)
                    );
                    for (idx, entry) in entries.iter().skip(entries.len().saturating_sub(3)).enumerate() {
                        info!(
                            "  [尾部 #{}] loc: {} | lastmod: {:?}",
                            idx + 1,
                            entry.loc,
                            entry.lastmod
                        );
                    }
                }
            } else {
                warn!(
                    site_id,
                    sitemap_url = %sm_url,
                    "⚠️ [SyncWorker] 警告：本次 Sitemap 解析出的 URL 列表为空！请检查 XML 结构或是否遭到了 WAF 403 阻断。"
                );
            }

            let chunk_size = 500usize;
            let mut total_inserted = 0u64;
            let mut chunk_index = 0;

            for chunk in entries.chunks(chunk_size) {
                chunk_index += 1;
                if !self.pipeline.is_running(site_id, PipelineStage::Sitemap) {
                    info!(site_id, "Sitemap 同步已被取消，Worker 回到待机");
                    return Ok(());
                }

                info!(
                    site_id,
                    chunk = chunk_index,
                    chunk_len = chunk.len(),
                    "💾 [SyncWorker] 正在批量入库写入数据库..."
                );

                let (inserted, _, _) = self.urls.batch_upsert_discovered(site.id, chunk).await?;
                total_inserted += inserted;
            }

            info!(
                domain = %site.domain,
                site_id,
                total_sitemap_urls = entries.len(),
                brand_new_inserted = total_inserted,
                already_known_updated = entries.len() as u64 - total_inserted,
                "🎉 [SyncWorker] 站点 Sitemap 全量同步与落库流程结束！"
            );

            if !entries.is_empty() {
                match self.urls.demote_absent_sitemap_urls(site.id, sync_start_time).await {
                    Ok(demoted_count) => {
                        if demoted_count > 0 {
                            info!(
                                site_id = site.id,
                                demoted_count,
                                "📦 [SyncWorker] 已将本次 Sitemap 未包含的陈旧 URL 降级为 gsc_orphan (孤岛资产)"
                            );
                        }
                    }
                    Err(e) => {
                        error!(error = %e, site_id = site.id, "降级陈旧 Sitemap URL 失败");
                    }
                }
            }

            info!("==================================================");

            self.pipeline.stop(site_id, PipelineStage::Sitemap);
        }

        Ok(())
    }
}