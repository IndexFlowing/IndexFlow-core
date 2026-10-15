use super::UrlRepo;
use crate::domain::QualityGateResult;
use chrono::{DateTime, Utc};

#[derive(Debug, Clone, Default)]
pub struct GscInspectionDetails<'a> {
    pub verdict: Option<&'a str>,
    pub crawled_as: Option<&'a str>,
    pub page_fetch_state: Option<&'a str>,
    pub robots_txt_state: Option<&'a str>,
    pub indexing_state: Option<&'a str>,
    pub user_canonical: Option<&'a str>,
    pub google_canonical: Option<&'a str>,
    pub referring_urls: Option<&'a str>,
}

impl UrlRepo {
    pub async fn set_watched(&self, id: i64, watched: bool) -> anyhow::Result<()> {
        sqlx::query("UPDATE urls SET is_watched = $2, watched_at = CASE WHEN $2 = 1 THEN CURRENT_TIMESTAMP ELSE watched_at END WHERE id = $1")
            .bind(id).bind(watched).execute(&self.pool).await?;
        Ok(())
    }
    /// 持久化技术 SEO 门禁扫描结果
    pub async fn persist_seo_scan(&self, id: i64, gate: &QualityGateResult) -> anyhow::Result<()> {
        let status = if !gate.passed {
            "FAIL"
        } else if gate.warnings.is_empty() {
            "PASS"
        } else {
            "WARN"
        };
        sqlx::query(
            r#"
            UPDATE urls
            SET
                http_status = $1,
                last_checked_at = CURRENT_TIMESTAMP,
                page_title = $2,
                canonical_url = $3,
                meta_description = $4,
                h1_content = $5,
                seo_status = $6,
                seo_issue = $7,
                h1_count = $8,
                has_nofollow = $9,
                ai_blocked_bots = $10,
                has_opengraph = $11,
                has_twitter_card = $12,
                schema_types = $13,
                response_time_ms = $14,
                payload_bytes = $15,
                has_viewport = $16,
                html_lang = $17,
                images_missing_alt = $18,
                seo_warnings = $19,
                updated_at = CURRENT_TIMESTAMP
            WHERE id = $20
            "#,
        )
        .bind(gate.http_status)
        .bind(gate.page_title.as_deref())
        .bind(gate.canonical_url.as_deref())
        .bind(gate.meta_description.as_deref())
        .bind(gate.h1_content.as_deref())
        .bind(status)
        .bind(gate.block_reason.as_deref())
        .bind(i64::try_from(gate.h1_count).unwrap_or(i64::MAX))
        .bind(gate.has_nofollow)
        .bind({
            let names = gate.ai_directives.blocked_names();
            (!names.is_empty()).then(|| names.join(","))
        })
        .bind(
            gate.opengraph.title.is_some()
                || gate.opengraph.description.is_some()
                || gate.opengraph.image.is_some()
                || gate.opengraph.og_type.is_some()
                || gate.opengraph.url.is_some()
                || gate.opengraph.site_name.is_some(),
        )
        .bind(gate.twitter_card.card.is_some())
        .bind({
            let types = gate.schema_types();
            (!types.is_empty()).then(|| types.join(","))
        })
        .bind(gate.response_time_ms)
        .bind(gate.payload_bytes)
        .bind(gate.has_viewport)
        .bind(gate.html_lang.as_deref())
        .bind(i64::try_from(gate.images_missing_alt).unwrap_or(i64::MAX))
        .bind((!gate.warnings.is_empty()).then(|| gate.warnings.join("\n")))
        .bind(id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// 回写 GSC URL Inspection 状态
    pub async fn apply_gsc_inspection(
        &self,
        id: i64,
        index_status: &str,
        coverage_state: Option<&str>,
        last_crawled_at: Option<DateTime<Utc>>,
        details: Option<&GscInspectionDetails<'_>>,
    ) -> anyhow::Result<()> {
        if let Some(d) = details {
            sqlx::query(
                r#"
                UPDATE urls
                SET
                    gsc_index_status = $2,
                    gsc_coverage_state = $3,
                    gsc_last_crawled_at = $4,
                    gsc_verdict = $5,
                    gsc_crawled_as = $6,
                    gsc_page_fetch_state = $7,
                    gsc_robots_txt_state = $8,
                    gsc_indexing_state = $9,
                    gsc_user_canonical = $10,
                    gsc_google_canonical = $11,
                    gsc_referring_urls = $12,
                    gsc_inspected_at = CURRENT_TIMESTAMP,
                    updated_at = CURRENT_TIMESTAMP
                WHERE id = $1
                "#,
            )
            .bind(id)
            .bind(index_status)
            .bind(coverage_state)
            .bind(last_crawled_at)
            .bind(d.verdict)
            .bind(d.crawled_as)
            .bind(d.page_fetch_state)
            .bind(d.robots_txt_state)
            .bind(d.indexing_state)
            .bind(d.user_canonical)
            .bind(d.google_canonical)
            .bind(d.referring_urls)
            .execute(&self.pool)
            .await?;
        } else {
            sqlx::query(
                r#"
                UPDATE urls
                SET
                    gsc_index_status = $2,
                    gsc_coverage_state = $3,
                    gsc_last_crawled_at = $4,
                    gsc_inspected_at = CURRENT_TIMESTAMP,
                    updated_at = CURRENT_TIMESTAMP
                WHERE id = $1
                "#,
            )
            .bind(id)
            .bind(index_status)
            .bind(coverage_state)
            .bind(last_crawled_at)
            .execute(&self.pool)
            .await?;
        }
        Ok(())
    }

    /// 回写 Bing URL Inspection 状态
    pub async fn apply_bing_inspection(
        &self,
        id: i64,
        index_status: &str,
        coverage_state: Option<&str>,
        last_crawled_at: Option<DateTime<Utc>>,
    ) -> anyhow::Result<()> {
        sqlx::query(
            r#"
            UPDATE urls
            SET
                bing_index_status = $2,
                bing_coverage_state = $3,
                bing_last_crawled_at = $4,
                bing_inspected_at = CURRENT_TIMESTAMP,
                updated_at = CURRENT_TIMESTAMP
            WHERE id = $1
            "#,
        )
        .bind(id)
        .bind(index_status)
        .bind(coverage_state)
        .bind(last_crawled_at)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// 回写搜索引擎推送结果状态
    pub async fn apply_submit_outcome(
        &self,
        id: i64,
        bing_status: Option<&str>,
        bing_error: Option<&str>,
        google_status: Option<&str>,
        google_error: Option<&str>,
    ) -> anyhow::Result<()> {
        sqlx::query(
            r#"
            UPDATE urls
            SET
                bing_status = COALESCE($2, bing_status),
                bing_submitted_at = CASE WHEN $2 = 'SUBMITTED' THEN CURRENT_TIMESTAMP ELSE bing_submitted_at END,
                bing_error = $3,
                google_status = COALESCE($4, google_status),
                google_submitted_at = CASE WHEN $4 = 'SUBMITTED' THEN CURRENT_TIMESTAMP ELSE google_submitted_at END,
                google_error = $5,
                updated_at = CURRENT_TIMESTAMP
            WHERE id = $1
            "#,
        )
        .bind(id)
        .bind(bing_status)
        .bind(bing_error)
        .bind(google_status)
        .bind(google_error)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// 将匹配筛选条件的已纳管 URL 状态重置为 PENDING，并重置最后检查时间，使其重新进入质检队列
    pub async fn reset_seo_status_for_recheck(
        &self,
        site_id: i64,
        status: Option<&str>,
        query_str: Option<&str>,
    ) -> anyhow::Result<u64> {
        let q_pattern = query_str.map(|s| format!("%{}%", s.trim()));
        let res = sqlx::query(
            r#"
            UPDATE urls
            SET seo_status = 'PENDING',
                updated_at = CURRENT_TIMESTAMP
            WHERE site_id = $1
              AND discovered_via = 'sitemap'
              AND ($2 IS NULL OR seo_status = $2)
              AND ($3 IS NULL OR url LIKE $3 OR page_title LIKE $3)
            "#,
        )
        .bind(site_id)
        .bind(status)
        .bind(&q_pattern)
        .execute(&self.pool)
        .await?;

        Ok(res.rows_affected())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::SqlitePool;

    #[tokio::test]
    async fn test_apply_gsc_inspection_details() -> anyhow::Result<()> {
        let pool = SqlitePool::connect("sqlite::memory:").await?;
        sqlx::migrate!("./migrations").run(&pool).await?;
        let repo = UrlRepo::new(pool);

        // Insert a dummy URL
        sqlx::query(
            r#"
            INSERT INTO urls (site_id, url, url_hash, discovered_via)
            VALUES (1, 'https://example.com/test', 'hash123', 'sitemap')
            "#,
        )
        .execute(&repo.pool)
        .await?;

        let details = GscInspectionDetails {
            verdict: Some("NEUTRAL"),
            crawled_as: Some("GOOGLEBOT_DESKTOP"),
            page_fetch_state: Some("SUCCESSFUL"),
            robots_txt_state: Some("ALLOWED"),
            indexing_state: Some("INDEXING_ALLOWED"),
            user_canonical: Some("https://example.com/canonical"),
            google_canonical: Some("https://example.com/canonical"),
            referring_urls: Some("https://ref1.com\nhttps://ref2.com"),
        };

        let crawled_at = Utc::now();
        repo.apply_gsc_inspection(
            1,
            "CRAWLED_NOT_INDEXED",
            Some("Crawled - currently not indexed"),
            Some(crawled_at),
            Some(&details),
        )
        .await?;

        let url = repo.find_by_id(1).await?.expect("url should exist");
        assert_eq!(url.gsc_index_status, "CRAWLED_NOT_INDEXED");
        assert_eq!(
            url.gsc_coverage_state.as_deref(),
            Some("Crawled - currently not indexed")
        );
        assert_eq!(url.gsc_verdict.as_deref(), Some("NEUTRAL"));
        assert_eq!(url.gsc_crawled_as.as_deref(), Some("GOOGLEBOT_DESKTOP"));
        assert_eq!(url.gsc_page_fetch_state.as_deref(), Some("SUCCESSFUL"));
        assert_eq!(url.gsc_robots_txt_state.as_deref(), Some("ALLOWED"));
        assert_eq!(url.gsc_indexing_state.as_deref(), Some("INDEXING_ALLOWED"));
        assert_eq!(
            url.gsc_user_canonical.as_deref(),
            Some("https://example.com/canonical")
        );
        assert_eq!(
            url.gsc_google_canonical.as_deref(),
            Some("https://example.com/canonical")
        );
        assert_eq!(
            url.referring_urls_list(),
            vec!["https://ref1.com", "https://ref2.com"]
        );

        // Test passing None keeps existing deep inspection fields
        repo.apply_gsc_inspection(1, "FAILED", Some("Inspection quota limit"), None, None)
            .await?;
        let url_failed = repo.find_by_id(1).await?.expect("url should exist");
        assert_eq!(url_failed.gsc_index_status, "FAILED");
        assert_eq!(
            url_failed.gsc_coverage_state.as_deref(),
            Some("Inspection quota limit")
        );
        assert_eq!(url_failed.gsc_crawled_as.as_deref(), Some("GOOGLEBOT_DESKTOP"));
        assert_eq!(
            url_failed.referring_urls_list(),
            vec!["https://ref1.com", "https://ref2.com"]
        );

        Ok(())
    }

    #[tokio::test]
    async fn test_reset_seo_status_for_recheck() -> anyhow::Result<()> {
        let pool = SqlitePool::connect("sqlite::memory:").await?;
        sqlx::migrate!("./migrations").run(&pool).await?;
        let repo = UrlRepo::new(pool);

        // Insert dummy URLs with different statuses
        sqlx::query(
            r#"
            INSERT INTO urls (id, site_id, url, url_hash, discovered_via, seo_status, page_title)
            VALUES 
                (1, 1, 'https://example.com/warn1', 'h1', 'sitemap', 'WARN', 'Test Page 1'),
                (2, 1, 'https://example.com/warn2', 'h2', 'sitemap', 'WARN', 'Special Search Term'),
                (3, 1, 'https://example.com/fail1', 'h3', 'sitemap', 'FAIL', 'Page 3'),
                (4, 1, 'https://example.com/pass1', 'h4', 'sitemap', 'PASS', 'Page 4'),
                (5, 1, 'https://example.com/orphan', 'h5', 'gsc_orphan', 'WARN', 'Orphan Page'),
                (6, 2, 'https://example.com/site2', 'h6', 'sitemap', 'WARN', 'Site 2 Page')
            "#,
        )
        .execute(&repo.pool)
        .await?;

        // 1. Reset WARN with query_str matching "Search"
        let affected = repo
            .reset_seo_status_for_recheck(1, Some("WARN"), Some("Search"))
            .await?;
        assert_eq!(affected, 1);
        let u2 = repo.find_by_id(2).await?.unwrap();
        assert_eq!(u2.seo_status, "PENDING");

        // 2. Reset all remaining WARN under site 1 (without query filter)
        let affected2 = repo
            .reset_seo_status_for_recheck(1, Some("WARN"), None)
            .await?;
        assert_eq!(affected2, 1); // only url 1 (url 5 is gsc_orphan, url 6 is site 2, url 2 is already PENDING)
        let u1 = repo.find_by_id(1).await?.unwrap();
        assert_eq!(u1.seo_status, "PENDING");

        // 3. Reset all under site 1 (status is None)
        let affected3 = repo
            .reset_seo_status_for_recheck(1, None, None)
            .await?;
        // url 1 and 2 are PENDING, url 3 is FAIL, url 4 is PASS -> all 4 sitemap urls get reset to PENDING
        assert_eq!(affected3, 4);

        Ok(())
    }
}
