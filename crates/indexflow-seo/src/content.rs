// crates/indexflow-seo/src/content.rs
use crate::html_utils::{
    clamp_boundary, decode_basic_entities, find_close_tag, find_open_tag, find_tag_end,
    normalize_visible_text, safe_slice, strip_tags, strip_tags_and_raw_elements,
};
use crate::models::HeadingItem;

/// 计算页面正文字数（剥离所有标签、JS、CSS 样式后的有效词汇统计）
pub fn compute_word_count(html: &str) -> usize {
    let stripped = strip_tags_and_raw_elements(html);
    stripped
        .split_whitespace()
        .map(|token| {
            let cjk_count = token.chars().filter(|c| is_cjk(*c)).count();
            if cjk_count > 0 {
                cjk_count
            } else {
                1
            }
        })
        .sum()
}

fn is_cjk(c: char) -> bool {
    matches!(c, '\u{4e00}'..='\u{9fff}' | '\u{3400}'..='\u{4dbf}')
}

/// 扫描提取完整 H1 ~ H6 标题大纲树（对标 Ahrefs 大纲）
pub fn extract_headings(html: &str) -> Vec<HeadingItem> {
    let mut items = Vec::new();
    let tags = ["h1", "h2", "h3", "h4", "h5", "h6"];
    let mut search_from = 0;

    while search_from < html.len() {
        let mut nearest_tag = None;
        let mut nearest_idx = usize::MAX;

        for (level_idx, &t) in tags.iter().enumerate() {
            if let Some(pos) = find_open_tag(html, search_from, t, true) {
                if pos < nearest_idx {
                    nearest_idx = pos;
                    nearest_tag = Some((level_idx as u8 + 1, t));
                }
            }
        }

        let Some((level, tag_name)) = nearest_tag else {
            break;
        };

        let Some(gt) = find_tag_end(html, nearest_idx) else {
            break;
        };
        let after_gt = gt + 1;
        let Some(close_at) = find_close_tag(html, after_gt, tag_name) else {
            search_from = after_gt;
            continue;
        };

        if let Some(raw) = safe_slice(html, after_gt, close_at) {
            let cleaned = normalize_visible_text(&decode_basic_entities(&strip_tags(raw)));
            if !cleaned.is_empty() {
                items.push(HeadingItem {
                    level,
                    text: cleaned,
                });
            }
        }

        search_from = clamp_boundary(html, close_at.saturating_add(tag_name.len() + 3));
    }
    items
}