use serde::{Deserialize, Serialize};

/// 多语言交替映射项
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct HreflangItem {
    pub lang: String,
    pub href: String,
}

/// OpenGraph 社交与 AI 摘要元数据
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct OpenGraphMeta {
    pub title: Option<String>,
    pub description: Option<String>,
    pub image: Option<String>,
    pub og_type: Option<String>,
    pub url: Option<String>,
    pub site_name: Option<String>,
}

/// Twitter Card 标记
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct TwitterCardMeta {
    pub card: Option<String>,
    pub title: Option<String>,
    pub description: Option<String>,
    pub image: Option<String>,
}

/// 针对主流 AI 搜索引擎爬虫的屏蔽指令嗅探
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct AiBotDirectives {
    pub gptbot_blocked: bool,
    pub perplexity_blocked: bool,
    pub claudebot_blocked: bool,
    pub google_extended_blocked: bool,
}

/// 页面级结构化数据块 (Schema.org)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct JsonLdBlock {
    pub schema_type: Option<String>,
    pub raw_json: String,
}

/// SEO 质量门禁与技术检查综合结果
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SeoAuditResult {
    // 基础网络层指标
    pub http_status: Option<i32>,
    pub response_time_ms: Option<i32>,
    pub payload_bytes: Option<i32>,

    // 核心 HTML 标签
    pub page_title: Option<String>,
    pub meta_description: Option<String>,
    pub h1_content: Option<String>,
    pub h1_count: usize,

    // 规范链接与指令
    pub canonical_url: Option<String>,
    pub has_canonical: bool,
    pub has_noindex: bool,
    pub has_nofollow: bool,
    pub robots_directive: Option<String>,
    pub hreflang: Vec<HreflangItem>,

    // GEO 与 AI 搜索指标
    pub opengraph: OpenGraphMeta,
    pub twitter_card: TwitterCardMeta,
    pub json_ld: Vec<JsonLdBlock>,
    pub ai_directives: AiBotDirectives,

    // 门禁判定决策
    pub passed: bool,
    pub block_reason: Option<String>,
}

impl SeoAuditResult {
    /// 序列化 hreflang 数组为 JSON 字符串，供数据库存储
    pub fn hreflang_json(&self) -> Option<String> {
        if self.hreflang.is_empty() {
            None
        } else {
            serde_json::to_string(&self.hreflang).ok()
        }
    }

    /// 提取页面中包含的所有 Schema.org `@type` 类型清单（如 `["Article", "FAQPage"]`）
    pub fn schema_types(&self) -> Vec<String> {
        self.json_ld
            .iter()
            .filter_map(|b| b.schema_type.clone())
            .collect()
    }
}