// crates/indexflow-seo/src/warnings.rs
use crate::canonical::canonical_matches_page;
use crate::extractor::RawHtmlInspection;
use crate::models::{AiBotDirectives, HttpLinkHeader};

/// 页面优化建议与工程警示计算引擎
pub fn compute_warnings(
    status_code: i32,
    inspected: &RawHtmlInspection,
    ai: &AiBotDirectives,
    redirect_location: Option<&str>,
    http_links: &[HttpLinkHeader],
) -> Vec<String> {
    let mut warnings = Vec::new();

    // 1. 临时重定向权重流失警示 (302/307)
    if status_code == 302 || status_code == 307 {
        let loc = redirect_location.unwrap_or("<未知>");
        warnings.push(format!(
            "检测到 HTTP {status_code} 临时重定向至 {loc}，可能导致搜索引擎权重不继承，规范化跳转建议使用 301 或 308 (WARN_TEMPORARY_REDIRECT_SEO_LEAK)"
        ));
    }

    // 2. x-default 跨层冲突检测
    let header_x_default = http_links.iter().find(|l| {
        l.rel == "alternate"
            && l.hreflang
                .as_deref()
                .map(|s| s.eq_ignore_ascii_case("x-default"))
                .unwrap_or(false)
    });
    if let Some(link) = header_x_default {
        let body_x_default = inspected
            .hreflangs
            .iter()
            .find(|h| h.lang.eq_ignore_ascii_case("x-default"));
        if let Some(b) = body_x_default {
            if !canonical_matches_page(&link.uri, &b.href) {
                warnings.push(format!(
                    "x-default 跨层冲突：HTTP Header ({}) 与 HTML Body ({}) 目标不一致",
                    link.uri, b.href
                ));
            }
        }
    }

    // 3. 社交元数据协议类型一致性 (OG Type 错配)
    let og = &inspected.opengraph;
    if let Some(ref og_type) = og.og_type {
        if og_type.starts_with("video.") && og.video.is_none() && inspected.video_objects.is_empty() {
            warnings.push(format!(
                "og:type 声明为 '{og_type}' 但未提供有效 og:video 或 VideoObject，建议降级为 'article' 或 'website' 避免搜索引擎视频爬虫误判"
            ));
        }
    }

    // 4. 视频源文件格式校验
    for (i, v) in inspected.video_objects.iter().enumerate() {
        if let Some(ref content) = v.content_url {
            let clean = content.split('?').next().unwrap_or(content).to_ascii_lowercase();
            let is_known_media = clean.ends_with(".mp4")
                || clean.ends_with(".m3u8")
                || clean.ends_with(".webm")
                || clean.ends_with(".mov")
                || clean.ends_with(".avi")
                || clean.ends_with(".ts");
            if !is_known_media && !clean.contains("video") {
                warnings.push(format!(
                    "VideoObject #{i} 的 contentUrl ({content}) 未包含常见视频后缀或协议，请确保其返回正确的 Content-Type: video/*"
                ));
            }
        }
    }

    // 5. 核心技术 SEO 标签规范
    if inspected.h1_count == 0 {
        warnings.push("缺少 H1 标题标签".to_string());
    }
    if inspected.h1_count > 1 {
        warnings.push(format!(
            "存在 {} 个 H1 标签，建议全页仅保留一个核心 H1",
            inspected.h1_count
        ));
    }
    if inspected.has_nofollow {
        warnings.push("页面带有 nofollow 指令".to_string());
    }

    let blocked = ai.blocked_names();
    if !blocked.is_empty() {
        warnings.push(format!("屏蔽了 AI 爬虫抓取: {}", blocked.join(", ")));
    }

    if [
        og.title.as_ref(),
        og.description.as_ref(),
        og.image.as_ref(),
        og.og_type.as_ref(),
        og.url.as_ref(),
        og.site_name.as_ref(),
    ]
    .iter()
    .all(Option::is_none)
    {
        warnings.push("缺少 OpenGraph 社交卡片标签".to_string());
    }

    if inspected.twitter_card.card.is_none() {
        warnings.push("缺少 X (Twitter) Card 标记".to_string());
    }
    if inspected.json_ld.is_empty() {
        warnings.push("未发现 JSON-LD 结构化数据".to_string());
    }
    if !inspected.has_viewport {
        warnings.push("缺少 viewport 移动适配标签".to_string());
    }
    if inspected.html_lang.is_none() {
        warnings.push("未声明 <html lang> 语言".to_string());
    }
    if inspected.images_missing_alt > 0 {
        warnings.push(format!(
            "{} 张图片缺失 alt 替代文本",
            inspected.images_missing_alt
        ));
    }

    match inspected.meta_description.as_deref() {
        None => warnings.push("缺少 meta description 描述".to_string()),
        Some(s) if !(50..=160).contains(&s.chars().count()) => warnings.push(format!(
            "meta description 长度为 {} 字，建议在 50~160 字之间",
            s.chars().count()
        )),
        _ => {}
    }

    if inspected
        .page_title
        .as_ref()
        .is_some_and(|s| s.chars().count() > 60)
    {
        warnings.push("page title 超过 60 字符，在搜索结果中可能被截断".to_string());
    }

    warnings
}