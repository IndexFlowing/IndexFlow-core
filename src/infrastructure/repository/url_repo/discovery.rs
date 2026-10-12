use super::UrlRepo;
use crate::domain::{compute_url_priority, hash_url, SitemapUrlEntry};
use chrono::{DateTime, Utc};

impl UrlRepo {
    /// 批量流式 Upsert 从 Sitemap 发现的 URL
    pub async fn batch_upsert_discovered(
        &self,
        site_id: i64,
        entries: &[SitemapUrlEntry],
    ) -> anyhow::Result<(u64, Vec<i64>, Vec<i64>)> {
        if entries.is_empty() {
            return Ok((0, Vec::new(), Vec::new()));
        }

        let mut tx = self.pool.begin().await?;
        let now = Utc::now();
        let mut inserted_count = 0u64;
        let mut all_ids = Vec::with_capacity(entries.len());
        let mut new_ids = Vec::new();

        for entry in entries {
            let hash = hash_url(&entry.loc);
            let prev: Option<(Option<DateTime<Utc>>,)> = sqlx::query_as(
                r#"SELECT sitemap_lastmod FROM urls WHERE site_id = $1 AND url_hash = $2"#,
            )
            .bind(site_id)
            .bind(&hash)
            .fetch_optional(&mut *tx)
            .await?;

            let is_new = prev.is_none();
            let previous_lastmod = prev.and_then(|p| p.0);
            let computed =
                compute_url_priority(entry.priority, entry.lastmod, previous_lastmod, is_new, now);

            let row = sqlx::query_as::<_, (i64,)>(
                r#"
                INSERT INTO urls (
                    site_id, url, url_hash, seo_status, priority, sitemap_lastmod, sitemap_synced_at,
                    locale, path_prefix, discovered_via, first_seen_at, created_at, updated_at
                )
                VALUES ($1, $2, $3, 'PENDING', $4, $5, CURRENT_TIMESTAMP, $6, $7, 'sitemap', CURRENT_TIMESTAMP, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)
                ON CONFLICT(url_hash) DO UPDATE SET
                    sitemap_lastmod = EXCLUDED.sitemap_lastmod,
                    sitemap_synced_at = CURRENT_TIMESTAMP,
                    discovered_via = 'sitemap',
                    priority = EXCLUDED.priority,
                    updated_at = CURRENT_TIMESTAMP
                RETURNING id
                "#,
            )
            .bind(site_id)
            .bind(&entry.loc)
            .bind(&hash)
            .bind(computed)
            .bind(entry.lastmod)
            .bind(&entry.locale)
            .bind(&entry.path_prefix)
            .fetch_one(&mut *tx)
            .await?;

            all_ids.push(row.0);
            if is_new {
                inserted_count += 1;
                new_ids.push(row.0);
            }
        }

        tx.commit().await?;
        Ok((inserted_count, all_ids, new_ids))
    }

    /// 【严谨规范】：仅对已有 Sitemap 纳管的 URL 进行收录标记与容错比对，绝不反向混入历史垃圾 URL
    pub async fn batch_mark_gsc_indexed(
        &self,
        site_id: i64,
        urls: &[String],
    ) -> anyhow::Result<u64> {
        if urls.is_empty() {
            return Ok(0);
        }

        let mut tx = self.pool.begin().await?;
        let mut total_marked = 0u64;

        for raw_url in urls {
            let trimmed = raw_url.trim();
            if trimmed.is_empty() {
                continue;
            }

            let primary_hash = hash_url(trimmed);

            // 1. 严格 hash 比对更新已有资产
            let res = sqlx::query(
                r#"
                UPDATE urls
                SET
                    gsc_index_status = 'INDEXED',
                    gsc_coverage_state = COALESCE(gsc_coverage_state, 'Indexed (Search Analytics Confirmed)'),
                    gsc_inspected_at = CURRENT_TIMESTAMP,
                    updated_at = CURRENT_TIMESTAMP
                WHERE site_id = $1 AND url_hash = $2
                "#,
            )
            .bind(site_id)
            .bind(&primary_hash)
            .execute(&mut *tx)
            .await?;

            if res.rows_affected() > 0 {
                total_marked += res.rows_affected();
                continue;
            }

            // 2. 尾部斜杠容错比对更新已有资产
            let variant_url = if trimmed.ends_with('/') {
                trimmed.trim_end_matches('/').to_string()
            } else {
                format!("{trimmed}/")
            };
            let variant_hash = hash_url(&variant_url);

            let res_var = sqlx::query(
                r#"
                UPDATE urls
                SET
                    gsc_index_status = 'INDEXED',
                    gsc_coverage_state = COALESCE(gsc_coverage_state, 'Indexed (Search Analytics Confirmed)'),
                    gsc_inspected_at = CURRENT_TIMESTAMP,
                    updated_at = CURRENT_TIMESTAMP
                WHERE site_id = $1 AND url_hash = $2
                "#,
            )
            .bind(site_id)
            .bind(&variant_hash)
            .execute(&mut *tx)
            .await?;

            if res_var.rows_affected() > 0 {
                total_marked += res_var.rows_affected();
            }
        }

        tx.commit().await?;
        Ok(total_marked)
    }

    /// 将本次同步中未出现的 Sitemap URL 降级为孤岛状态 (gsc_orphan)
    /// 既不破坏历史诊断/收录历史，又让 Sitemap 视图精准匹配当前最新版本
    pub async fn demote_absent_sitemap_urls(
        &self,
        site_id: i64,
        sync_started_at: chrono::DateTime<chrono::Utc>,
    ) -> anyhow::Result<u64> {
        let res = sqlx::query(
            r#"
            UPDATE urls 
            SET discovered_via = 'gsc_orphan',
                updated_at = CURRENT_TIMESTAMP
            WHERE site_id = $1 
              AND discovered_via = 'sitemap'
              AND (sitemap_synced_at IS NULL OR datetime(sitemap_synced_at) < datetime($2))
            "#,
        )
        .bind(site_id)
        .bind(sync_started_at)
        .execute(&self.pool)
        .await?;

        Ok(res.rows_affected())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::SitemapUrlEntry;
    use sqlx::SqlitePool;

    #[tokio::test]
    async fn demote_absent_sitemap_urls_and_revival() -> anyhow::Result<()> {
        let pool = SqlitePool::connect("sqlite::memory:").await?;
        sqlx::migrate!("./migrations").run(&pool).await?;
        let repo = UrlRepo::new(pool);

        let entry1 = SitemapUrlEntry {
            loc: "https://example.com/keep".into(),
            lastmod: None,
            priority: None,
            locale: "en".into(),
            path_prefix: "/".into(),
        };
        let entry2 = SitemapUrlEntry {
            loc: "https://example.com/drop".into(),
            lastmod: None,
            priority: None,
            locale: "en".into(),
            path_prefix: "/".into(),
        };

        // First batch: both inserted as sitemap
        repo.batch_upsert_discovered(1, &[entry1.clone(), entry2.clone()]).await?;

        // Simulate time passing: manually set drop's sitemap_synced_at to an older time
        sqlx::query(
            "UPDATE urls SET sitemap_synced_at = '2026-01-01 00:00:00' WHERE url = 'https://example.com/drop'"
        )
        .execute(&repo.pool)
        .await?;

        let sync_start = Utc::now();

        // entry1 is refreshed
        repo.batch_upsert_discovered(1, &[entry1.clone()]).await?;

        // Demote absent URLs
        let demoted = repo.demote_absent_sitemap_urls(1, sync_start).await?;
        assert_eq!(demoted, 1);

        let drop_url = repo.find_by_id(2).await?.unwrap();
        assert_eq!(drop_url.discovered_via, "gsc_orphan");

        let keep_url = repo.find_by_id(1).await?.unwrap();
        assert_eq!(keep_url.discovered_via, "sitemap");

        // Revival test: drop reappears in sitemap
        repo.batch_upsert_discovered(1, &[entry2.clone()]).await?;
        let revived_url = repo.find_by_id(2).await?.unwrap();
        assert_eq!(revived_url.discovered_via, "sitemap");

        Ok(())
    }
}
