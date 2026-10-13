use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;
use std::str::FromStr;

/// 全站流水线生命周期阶段。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PipelineStage {
    /// Sitemap 资产发现
    Sitemap,
    /// 本地 SEO 质量门禁
    SeoGate,
    /// Google 官方收录检测
    GscInspect,
    /// Bing 官方收录检测
    BingInspect,
    /// Google Indexing API 主动推送
    GoogleSubmit,
    /// Bing IndexNow 极速推送
    BingSubmit,
}

impl PipelineStage {
    pub const ALL: [PipelineStage; 6] = [
        PipelineStage::Sitemap,
        PipelineStage::SeoGate,
        PipelineStage::GscInspect,
        PipelineStage::BingInspect,
        PipelineStage::GoogleSubmit,
        PipelineStage::BingSubmit,
    ];

    /// URL 路径与 HTMX 组件使用的 kebab-case 标识。
    pub fn slug(self) -> &'static str {
        match self {
            Self::Sitemap => "sitemap",
            Self::SeoGate => "seo-gate",
            Self::GscInspect => "gsc-inspect",
            Self::BingInspect => "bing-inspect",
            Self::GoogleSubmit => "google-submit",
            Self::BingSubmit => "bing-submit",
        }
    }

    pub fn idle_text(self) -> &'static str {
        match self {
            Self::Sitemap => "开始同步 Sitemap",
            Self::SeoGate => "启动 SEO 增量质检",
            Self::GscInspect => "GSC 增量检测",
            Self::BingInspect => "Bing 增量检测",
            Self::GoogleSubmit => "启动 Google 增量推送",
            Self::BingSubmit => "启动 Bing IndexNow 批量推送",
        }
    }

    pub fn running_text(self) -> &'static str {
        match self {
            Self::Sitemap => "停止同步 (流式解析中...)",
            Self::SeoGate => "停止 SEO (30并发质检中...)",
            Self::GscInspect => "停止 GSC (检测中...)",
            Self::BingInspect => "停止 Bing (检测中...)",
            Self::GoogleSubmit => "停止 Google 推送 (运行中...)",
            Self::BingSubmit => "停止 Bing 推送 (运行中...)",
        }
    }

    pub fn color_theme(self) -> &'static str {
        match self {
            Self::Sitemap => "blue",
            Self::SeoGate => "amber",
            Self::GscInspect => "cyan",
            Self::BingInspect => "blue",
            Self::GoogleSubmit => "emerald",
            Self::BingSubmit => "sky",
        }
    }

    pub fn icon(self) -> &'static str {
        match self {
            Self::Sitemap => "🔄",
            Self::SeoGate => "🛡️",
            Self::GscInspect => "🔍",
            Self::BingInspect => "⚡",
            Self::GoogleSubmit => "🚀",
            Self::BingSubmit => "⚡",
        }
    }

    pub fn idle_button_class(self) -> &'static str {
        match self {
            Self::Sitemap => {
                "bg-gradient-to-r from-blue-600 to-cyan-500 hover:from-blue-500 hover:to-cyan-400 text-white shadow-lg shadow-cyan-500/20"
            }
            Self::SeoGate => {
                "bg-gradient-to-r from-amber-600 to-amber-500 hover:from-amber-500 hover:to-amber-400 text-white shadow-lg shadow-amber-900/20"
            }
            Self::GscInspect => {
                "bg-gradient-to-r from-blue-600 to-cyan-500 hover:from-blue-500 hover:to-cyan-400 text-white shadow-lg shadow-cyan-500/20"
            }
            Self::BingInspect => {
                "bg-sky-600 hover:bg-sky-500 text-white shadow-lg shadow-sky-900/30"
            }
            Self::GoogleSubmit => {
                "bg-gradient-to-r from-emerald-600 to-emerald-500 hover:from-emerald-500 hover:to-emerald-400 text-white shadow-lg shadow-emerald-900/30"
            }
            Self::BingSubmit => {
                "bg-gradient-to-r from-sky-600 to-cyan-500 hover:from-sky-500 hover:to-cyan-400 text-white shadow-lg shadow-sky-900/30"
            }
        }
    }
}

impl fmt::Display for PipelineStage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.slug())
    }
}

impl FromStr for PipelineStage {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let normalized = s.trim().to_ascii_lowercase().replace('_', "-");
        match normalized.as_str() {
            "sitemap" => Ok(Self::Sitemap),
            "seo-gate" | "seogate" | "seo" => Ok(Self::SeoGate),
            "gsc-inspect" | "gscinspect" | "gsc" => Ok(Self::GscInspect),
            "bing-inspect" | "binginspect" | "bing" => Ok(Self::BingInspect),
            "google-submit" | "googlesubmit" => Ok(Self::GoogleSubmit),
            "bing-submit" | "bingsubmit" => Ok(Self::BingSubmit),
            "push-submit" | "pushsubmit" | "submit" => Ok(Self::GoogleSubmit),
            other => Err(format!("unknown pipeline stage: {other}")),
        }
    }
}

impl Serialize for PipelineStage {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.slug())
    }
}

impl<'de> Deserialize<'de> for PipelineStage {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(deserializer)?;
        raw.parse().map_err(serde::de::Error::custom)
    }
}
