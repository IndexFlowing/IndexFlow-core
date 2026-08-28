use serde::{Deserialize, Serialize};

/// 多语言交替映射项.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct HreflangItem {
    pub lang: String,
    pub href: String,
}

/// OpenGraph 社交与 AI 摘要元数据.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct OpenGraphMeta {
    pub title: Option<String>,
    pub description: Option<String>,
    pub image: Option<String>,
    pub og_type: Option<String>,
    pub url: Option<String>,
    pub site_name: Option<String>,
}

/// Twitter Card 标记.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct TwitterCardMeta {
    pub card: Option<String>,
    pub title: Option<String>,
    pub description: Option<String>,
    pub image: Option<String>,
}

/// 针对主流 AI 搜索引擎爬虫的屏蔽指令嗅探.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct AiBotDirectives {
    pub gptbot_blocked: bool,
    pub perplexity_blocked: bool,
    pub claudebot_blocked: bool,
    pub google_extended_blocked: bool,
}

impl AiBotDirectives {
    /// `true` if any tracked AI crawler is blocked from indexing / training.
    pub fn any_blocked(&self) -> bool {
        self.gptbot_blocked
            || self.perplexity_blocked
            || self.claudebot_blocked
            || self.google_extended_blocked
    }

    pub(crate) fn merge(&mut self, other: &AiBotDirectives) {
        self.gptbot_blocked |= other.gptbot_blocked;
        self.perplexity_blocked |= other.perplexity_blocked;
        self.claudebot_blocked |= other.claudebot_blocked;
        self.google_extended_blocked |= other.google_extended_blocked;
    }
}

/// 页面级结构化数据块 (Schema.org).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct JsonLdBlock {
    pub schema_type: Option<String>,
    pub raw_json: String,
}

/// SEO 质量门禁与技术检查综合结果.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SeoAuditResult {
    pub http_status: Option<i32>,
    pub response_time_ms: Option<i32>,
    pub payload_bytes: Option<i32>,

    pub page_title: Option<String>,
    pub meta_description: Option<String>,
    pub h1_content: Option<String>,
    pub h1_count: usize,

    pub canonical_url: Option<String>,
    pub has_canonical: bool,
    pub has_noindex: bool,
    pub has_nofollow: bool,
    pub robots_directive: Option<String>,
    pub hreflang: Vec<HreflangItem>,

    pub opengraph: OpenGraphMeta,
    pub twitter_card: TwitterCardMeta,
    pub json_ld: Vec<JsonLdBlock>,
    pub ai_directives: AiBotDirectives,

    pub passed: bool,
    pub block_reason: Option<String>,
}

impl SeoAuditResult {
    /// 序列化 hreflang 数组为 JSON 字符串，供数据库存储.
    pub fn hreflang_json(&self) -> Option<String> {
        if self.hreflang.is_empty() {
            None
        } else {
            serde_json::to_string(&self.hreflang).ok()
        }
    }

    /// All Schema.org `@type` values, including those nested under `@graph`
    /// and array-typed `@type`. Order is document order; duplicates dropped.
    pub fn schema_types(&self) -> Vec<String> {
        let mut out = Vec::new();
        for block in &self.json_ld {
            let before = out.len();
            collect_schema_types_from_raw(&block.raw_json, &mut out);
            if out.len() == before {
                if let Some(ref t) = block.schema_type {
                    push_unique(&mut out, t.clone());
                }
            }
        }
        out
    }
}

pub(crate) fn collect_schema_types_from_raw(raw: &str, out: &mut Vec<String>) {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return;
    }
    if let Ok(value) = serde_json::from_str::<serde_json::Value>(trimmed) {
        collect_schema_types(&value, out);
    }
}

pub(crate) fn collect_schema_types(value: &serde_json::Value, out: &mut Vec<String>) {
    match value {
        serde_json::Value::Array(arr) => {
            for v in arr {
                collect_schema_types(v, out);
            }
        }
        serde_json::Value::Object(map) => {
            if let Some(t) = map.get("@type") {
                push_type_value(t, out);
            }
            if let Some(graph) = map.get("@graph") {
                collect_schema_types(graph, out);
            }
        }
        _ => {}
    }
}

fn push_type_value(t: &serde_json::Value, out: &mut Vec<String>) {
    match t {
        serde_json::Value::String(s) => {
            let short = match s.rsplit('/').next() {
                Some(part) if !part.is_empty() => part,
                _ => s.as_str(),
            };
            push_unique(out, short.to_string());
        }
        serde_json::Value::Array(arr) => {
            for v in arr {
                push_type_value(v, out);
            }
        }
        _ => {}
    }
}

fn push_unique(out: &mut Vec<String>, t: String) {
    if !t.is_empty() && !out.iter().any(|s| s == &t) {
        out.push(t);
    }
}
