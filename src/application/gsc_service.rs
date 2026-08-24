use crate::domain::coverage_to_index_status;
use crate::infrastructure::{SiteRepo, UrlRepo};
use crate::providers::google::{GoogleProvider, GscInspectResult};
use chrono::Utc;

#[derive(Clone)]
pub struct GscService {
    google: GoogleProvider,
    sites: SiteRepo,
    urls: UrlRepo,
}

impl GscService {
    pub fn new(google: GoogleProvider, sites: SiteRepo, urls: UrlRepo) -> Self {
        Self {
            google,
            sites,
            urls,
        }
    }

    pub async fn inspect_one(
        &self,
        site: &crate::domain::Site,
        page_url: &str,
    ) -> anyhow::Result<GscInspectResult> {
        let sa = site
            .google_service_account_json
            .as_deref()
            .ok_or_else(|| anyhow::anyhow!("no Google credentials"))?;

        let property = if let Some(ref p) = site.gsc_property_url {
            p.clone()
        } else {
            let p = self.google.resolve_gsc_property(sa, &site.domain).await?;
            self.sites.set_gsc_property(&p).await?;
            p
        };

        self.google.inspect_url(sa, &property, page_url).await
    }

    pub async fn apply_inspect_result(
        &self,
        url_id: i64,
        result: &GscInspectResult,
    ) -> anyhow::Result<()> {
        if result.ok {
            let coverage = result.coverage_state.as_deref().unwrap_or("URL is unknown to Google");
            let index_status = coverage_to_index_status(coverage);
            let crawled = result
                .last_crawl_time
                .as_deref()
                .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
                .map(|d| d.with_timezone(&Utc));

            self.urls
                .apply_gsc_inspection(url_id, index_status, Some(coverage), crawled)
                .await?;
        }
        Ok(())
    }
}