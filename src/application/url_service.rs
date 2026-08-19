use crate::domain::{HealthCheck, SubmissionLog, Url};
use crate::infrastructure::{
    HealthCheckRepo, LocaleCount, PathPrefixCount, SiteUrlStats, SubmissionLogRepo, UrlDiagnostic,
    UrlRepo,
};

#[derive(Clone)]
pub struct UrlService {
    urls: UrlRepo,
    health: HealthCheckRepo,
    submissions: SubmissionLogRepo,
}

#[derive(Debug, serde::Serialize)]
pub struct UrlDetail {
    pub url: Url,
    pub recent_checks: Vec<HealthCheck>,
    pub recent_submissions: Vec<SubmissionLog>,
}

impl UrlService {
    pub fn new(
        urls: UrlRepo,
        health: HealthCheckRepo,
        submissions: SubmissionLogRepo,
    ) -> Self {
        Self {
            urls,
            health,
            submissions,
        }
    }

    pub async fn list(
        &self,
        site_id: i64,
        status: Option<&str>,
        locale: Option<&str>,
        path_prefix: Option<&str>,
        page: i64,
        limit: i64,
    ) -> anyhow::Result<(Vec<Url>, i64)> {
        self.urls
            .list_by_site(site_id, status, locale, path_prefix, page, limit)
            .await
    }

    pub async fn list_diagnostics(
        &self,
        site_id: i64,
        status: Option<&str>,
        locale: Option<&str>,
        path_prefix: Option<&str>,
        page: i64,
        limit: i64,
    ) -> anyhow::Result<(Vec<UrlDiagnostic>, i64)> {
        self.urls
            .list_diagnostics(site_id, status, locale, path_prefix, page, limit)
            .await
    }

    pub async fn stats(
        &self,
        site_id: i64,
        locale: Option<&str>,
        path_prefix: Option<&str>,
    ) -> anyhow::Result<SiteUrlStats> {
        self.urls.site_three_state(site_id, locale, path_prefix).await
    }

    pub async fn locales(
        &self,
        site_id: i64,
        path_prefix: Option<&str>,
    ) -> anyhow::Result<Vec<LocaleCount>> {
        self.urls.list_locales(site_id, path_prefix).await
    }

    pub async fn path_prefixes(
        &self,
        site_id: i64,
        locale: Option<&str>,
    ) -> anyhow::Result<Vec<PathPrefixCount>> {
        self.urls.list_path_prefixes(site_id, locale).await
    }

    pub async fn get_detail(&self, id: i64) -> anyhow::Result<Option<UrlDetail>> {
        let Some(url) = self.urls.find_by_id(id).await? else {
            return Ok(None);
        };
        let recent_checks = self.health.list_by_url(id, 20).await?;
        let recent_submissions = self.submissions.list_by_url(id, 20).await?;
        Ok(Some(UrlDetail {
            url,
            recent_checks,
            recent_submissions,
        }))
    }
}
