use super::UrlRepo;
use crate::domain::{compute_url_priority, extract_locale_and_path_prefix, hash_url, SitemapUrlEntry};
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
            let computed = compute_url_priority(
                entry.priority,
                entry.lastmod,
                previous_lastmod,
                is_new,
                now,
            );

            let row = sqlx::query_as::<_, (i64,)>(
                r#"
                INSERT INTO urls (
                    site_id, url, url_hash, seo_status, priority, sitemap_lastmod,
                    locale, path_prefix, first_seen_at, created_at, updated_at
                )
                VALUES ($1, $2, $3, 'PENDING', $4, $5, $6, $7, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)
                ON CONFLICT(url_hash) DO UPDATE SET
                    sitemap_lastmod = EXCLUDED.sitemap_lastmod,
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

    /// 【核心重构】：批量将 Google 曝光池 URL 标记为 INDEXED
    /// 1. 先尝试严格 hash 匹配；
    /// 2. 尝试尾部斜杠/非斜杠容错匹配；
    /// 3. 若数据库无此 URL（Sitemap 未收录但 Google 实际有曝光的孤岛页面），自动 INSERT 纳管！
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
            if trimmed.is_empty() { continue; }

            let primary_hash = hash_url(trimmed);

            // 1. 优先尝试直接匹配更新
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

            // 2. 容错匹配：尝试补全或去除末尾斜杠
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
                continue;
            }

            // 3. 孤岛资产反向纳管：如果表中完全没有该 URL，自动 INSERT 纳管入库！
            let (locale, path_prefix) = extract_locale_and_path_prefix(trimmed, None);
            let _ = sqlx::query(
                r#"
                INSERT INTO urls (
                    site_id, url, url_hash, gsc_index_status, gsc_coverage_state,
                    gsc_inspected_at, seo_status, priority, locale, path_prefix,
                    first_seen_at, created_at, updated_at
                )
                VALUES ($1, $2, $3, 'INDEXED', 'Indexed (Search Analytics Auto-Discovered)', CURRENT_TIMESTAMP, 'PENDING', 80, $4, $5, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)
                ON CONFLICT(url_hash) DO UPDATE SET
                    gsc_index_status = 'INDEXED',
                    gsc_coverage_state = 'Indexed (Search Analytics Auto-Discovered)',
                    gsc_inspected_at = CURRENT_TIMESTAMP,
                    updated_at = CURRENT_TIMESTAMP
                "#,
            )
            .bind(site_id)
            .bind(trimmed)
            .bind(&primary_hash)
            .bind(&locale)
            .bind(&path_prefix)
            .execute(&mut *tx)
            .await?;

            total_marked += 1;
        }

        tx.commit().await?;
        Ok(total_marked)
    }
}