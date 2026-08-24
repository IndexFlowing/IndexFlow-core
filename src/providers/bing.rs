use super::{SearchProvider, SubmissionResult};
use async_trait::async_trait;
use tracing::info;

#[derive(Clone)]
pub struct BingProvider {
    #[allow(dead_code)]
    client: reqwest::Client,
}

impl BingProvider {
    pub fn new(client: reqwest::Client) -> Self {
        Self { client }
    }
}

#[async_trait]
impl SearchProvider for BingProvider {
    fn name(&self) -> &'static str {
        "bing"
    }

    async fn submit_batch(
        &self,
        domain: &str,
        key: &str,
        urls: &[String],
    ) -> anyhow::Result<Vec<SubmissionResult>> {
        if urls.is_empty() {
            return Ok(vec![]);
        }

        // ==========================================
        // 🧪 测试安全演练模式：仅打印日志，不发真实请求
        // ==========================================
        info!(
            mode = "DRY_RUN (演练模式)",
            domain = %domain,
            key = %key,
            count = urls.len(),
            urls = ?urls,
            "【Bing IndexNow 模拟提交】拦截真实网络请求，已记录推送日志"
        );

        // 模拟 200 成功响应返回给业务层
        Ok(urls
            .iter()
            .map(|url| SubmissionResult {
                url: url.clone(),
                is_success: true,
                status_code: Some(200),
                response_msg: Some("DRY_RUN_MOCK: 已演练并记录日志，未向 IndexNow 发起网络请求".into()),
                is_quota_exceeded: false,
            })
            .collect())
    }
}