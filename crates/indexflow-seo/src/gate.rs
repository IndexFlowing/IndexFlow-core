// crates/indexflow-seo/src/gate.rs
use crate::canonical::canonical_matches_page;
use crate::models::{HttpLinkHeader, VideoObjectEntity};

/// 质量门禁阻断判定引擎
/// 返回 `Some(reason)` 表示触发拦截，`None` 表示门禁通过
pub fn evaluate_gate_block(
    page_url: &str,
    status_code: i32,
    has_noindex: bool,
    body_canonical: Option<&str>,
    page_title: Option<&str>,
    redirect_location: Option<&str>,
    http_links: &[HttpLinkHeader],
    video_objects: &[VideoObjectEntity],
) -> Option<String> {
    // 1. 重定向严格语义与死锁拦截
    if (300..=399).contains(&status_code) {
        let Some(loc) = redirect_location else {
            return Some(format!(
                "HTTP {status_code} 重定向缺少 Location 响应标头 (FATAL_MALFORMED_REDIRECT)"
            ));
        };
        if canonical_matches_page(page_url, loc) {
            return Some(format!(
                "HTTP {status_code} 重定向目标指向自身，存在死循环风险: {loc}"
            ));
        }
        return Some(format!("HTTP {status_code} 重定向至: {loc}"));
    }

    // 2. HTTP 状态非 200 拦截
    if status_code != 200 {
        return Some(format!("HTTP {status_code}"));
    }

    // 3. noindex 标头 / 标签拦截
    if has_noindex {
        return Some("noindex directive present".to_string());
    }

    // 4. HTTP Header vs HTML Body 跨层 Canonical 冲突死锁审计
    let header_canonical = http_links
        .iter()
        .find(|l| l.rel == "canonical")
        .map(|l| l.uri.as_str());

    if let (Some(h_canon), Some(b_canon)) = (header_canonical, body_canonical) {
        if !canonical_matches_page(h_canon, b_canon) {
            return Some(format!(
                "跨层 Canonical 冲突：HTTP Header 声明 ({h_canon}) 与 HTML Body 声明 ({b_canon}) 不一致 (CRITICAL_SEO_HEADER_LOOP)"
            ));
        }
    }

    // 5. Canonical 声明一致性校验
    let effective_canonical = header_canonical.or(body_canonical);
    if let Some(canon) = effective_canonical {
        if !canonical_matches_page(page_url, canon) {
            return Some(format!("Canonical URL mismatch: {canon}"));
        }
    }

    // 6. VideoObject 富媒体语义自环（自引用致盲）检测
    for (idx, video) in video_objects.iter().enumerate() {
        if let Some(ref embed) = video.embed_url {
            if canonical_matches_page(page_url, embed) {
                return Some(format!(
                    "VideoObject #{idx} 语义自环：embedUrl ({embed}) 直接指向当前网页 HTML，将导致 Google 视频抓取器解码崩溃"
                ));
            }
        }
        if let Some(ref content) = video.content_url {
            if canonical_matches_page(page_url, content) {
                return Some(format!(
                    "VideoObject #{idx} 语义自环：contentUrl ({content}) 直接指向当前网页 HTML，将被视为损坏的媒体流"
                ));
            }
        }
    }

    // 7. 缺失标题拦截
    if page_title.map(str::trim).map(str::is_empty).unwrap_or(true) {
        return Some("Missing <title> tag".to_string());
    }

    None
}