use super::UrlRepo;
use crate::domain::QualityGateResult;
use chrono::{DateTime, Utc};

impl UrlRepo {
    /// 持久化技术 SEO 门禁扫描结果
    pub async fn persist_seo_scan(&self, id: i64, gate: &QualityGateResult) -> anyhow::Result<()> {
        let status = if gate.passed { "PASS" } else { "FAIL" };
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
                updated_at = CURRENT_TIMESTAMP
            WHERE id = $8
            "#,
        )
        .bind(gate.http_status)
        .bind(gate.page_title.as_deref())
        .bind(gate.canonical_url.as_deref())
        .bind(gate.meta_description.as_deref())
        .bind(gate.h1_content.as_deref())
        .bind(status)
        .bind(gate.block_reason.as_deref())
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
    ) -> anyhow::Result<()> {
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
}