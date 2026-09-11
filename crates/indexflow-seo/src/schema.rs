// crates/indexflow-seo/src/schema.rs
use crate::html_utils::{attr_value, clamp_boundary, find_close_tag, find_open_tag, find_tag_end, safe_slice};
use crate::models::{collect_schema_types, JsonLdBlock, VideoObjectEntity};

/// 提取 HTML 中所有的 JSON-LD 结构化数据
pub fn extract_json_ld(html: &str) -> Vec<JsonLdBlock> {
    let mut blocks = Vec::new();
    let mut search_from = 0;

    while let Some(abs) = find_open_tag(html, search_from, "script", false) {
        let Some(tag_end) = find_tag_end(html, abs) else {
            break;
        };
        if let Some(open_tag) = safe_slice(html, abs, tag_end.saturating_add(1)) {
            let type_val = attr_value(open_tag, "type").unwrap_or_default();
            if is_ld_json_type(&type_val) {
                let body_start = tag_end.saturating_add(1);
                if let Some(close_at) = find_close_tag(html, body_start, "script") {
                    if let Some(raw_body) = safe_slice(html, body_start, close_at) {
                        emit_json_ld_blocks(raw_body, &mut blocks);
                    }
                    search_from = close_at.saturating_add(9);
                    search_from = clamp_boundary(html, search_from);
                    continue;
                }
            } else if let Some(close_at) = find_close_tag(html, tag_end + 1, "script") {
                search_from = close_at.saturating_add(9);
                search_from = clamp_boundary(html, search_from);
                continue;
            }
        }
        search_from = tag_end.saturating_add(1);
        search_from = clamp_boundary(html, search_from);
    }
    blocks
}

/// 深度解析所有 JSON-LD 中的 VideoObject 实体
pub fn extract_video_objects(blocks: &[JsonLdBlock]) -> Vec<VideoObjectEntity> {
    let mut results = Vec::new();
    for block in blocks {
        if let Ok(value) = serde_json::from_str::<serde_json::Value>(&block.raw_json) {
            find_video_objects_in_value(&value, &mut results);
        }
    }
    results
}

fn is_ld_json_type(t: &str) -> bool {
    let t = t.trim();
    t.eq_ignore_ascii_case("application/ld+json")
        || starts_with_ignore_ascii(t, "application/ld+json;")
}

fn emit_json_ld_blocks(raw_body: &str, blocks: &mut Vec<JsonLdBlock>) {
    let stripped = strip_json_ld_wrappers(raw_body);
    let trimmed = stripped.trim();
    if trimmed.is_empty() {
        return;
    }
    match serde_json::from_str::<serde_json::Value>(trimmed) {
        Ok(value) => expand_json_ld_value(&value, blocks),
        Err(_) => blocks.push(JsonLdBlock {
            schema_type: None,
            raw_json: trimmed.to_string(),
        }),
    }
}

fn expand_json_ld_value(value: &serde_json::Value, blocks: &mut Vec<JsonLdBlock>) {
    match value {
        serde_json::Value::Array(arr) => {
            for v in arr {
                expand_json_ld_value(v, blocks);
            }
        }
        serde_json::Value::Object(map) => {
            if let Some(graph) = map.get("@graph") {
                expand_json_ld_value(graph, blocks);
                if map.contains_key("@type") {
                    blocks.push(block_from_value(value));
                }
            } else {
                blocks.push(block_from_value(value));
            }
        }
        _ => {}
    }
}

fn block_from_value(value: &serde_json::Value) -> JsonLdBlock {
    let mut types = Vec::new();
    collect_schema_types(value, &mut types);
    JsonLdBlock {
        schema_type: types.first().cloned(),
        raw_json: value.to_string(),
    }
}

fn strip_json_ld_wrappers(s: &str) -> String {
    let mut t = s.trim().to_string();
    if let Some(rest) = t.strip_prefix("<!--") {
        if let Some(idx) = rest.rfind("-->") {
            t = rest[..idx].trim().to_string();
        }
    }
    let mut t = t.trim().to_string();
    if let Some(rest) = t.strip_prefix("//") {
        t = rest.trim_start().to_string();
    }
    if let Some(rest) = t.strip_prefix("<![CDATA[") {
        t = rest.to_string();
    }
    if let Some(rest) = t.strip_suffix("]]>") {
        t = rest.to_string();
    }
    if let Some(rest) = t.strip_suffix("//") {
        t = rest.to_string();
    }
    t.trim().to_string()
}

fn find_video_objects_in_value(value: &serde_json::Value, out: &mut Vec<VideoObjectEntity>) {
    match value {
        serde_json::Value::Array(arr) => {
            for v in arr {
                find_video_objects_in_value(v, out);
            }
        }
        serde_json::Value::Object(map) => {
            let is_video = map.get("@type").map(is_type_video_object).unwrap_or(false);
            if is_video {
                out.push(VideoObjectEntity {
                    name: map.get("name").and_then(|v| v.as_str()).map(str::to_string),
                    description: map
                        .get("description")
                        .and_then(|v| v.as_str())
                        .map(str::to_string),
                    thumbnail_url: map
                        .get("thumbnailUrl")
                        .or_else(|| map.get("thumbnail_url"))
                        .and_then(extract_string_or_first_array_str),
                    embed_url: map
                        .get("embedUrl")
                        .or_else(|| map.get("embed_url"))
                        .and_then(|v| v.as_str())
                        .map(str::to_string),
                    content_url: map
                        .get("contentUrl")
                        .or_else(|| map.get("content_url"))
                        .and_then(|v| v.as_str())
                        .map(str::to_string),
                    upload_date: map
                        .get("uploadDate")
                        .or_else(|| map.get("upload_date"))
                        .and_then(|v| v.as_str())
                        .map(str::to_string),
                });
            }

            if let Some(graph) = map.get("@graph") {
                find_video_objects_in_value(graph, out);
            }
        }
        _ => {}
    }
}

fn is_type_video_object(type_val: &serde_json::Value) -> bool {
    match type_val {
        serde_json::Value::String(s) => {
            let clean = s.rsplit('/').next().unwrap_or(s);
            clean.eq_ignore_ascii_case("VideoObject")
        }
        serde_json::Value::Array(arr) => arr.iter().any(is_type_video_object),
        _ => false,
    }
}

fn extract_string_or_first_array_str(val: &serde_json::Value) -> Option<String> {
    match val {
        serde_json::Value::String(s) => Some(s.to_string()),
        serde_json::Value::Array(arr) => arr.first().and_then(|v| v.as_str()).map(str::to_string),
        _ => None,
    }
}

fn starts_with_ignore_ascii(s: &str, prefix_lower: &str) -> bool {
    let sb = s.as_bytes();
    let pb = prefix_lower.as_bytes();
    if sb.len() < pb.len() {
        return false;
    }
    sb[..pb.len()]
        .iter()
        .zip(pb.iter())
        .all(|(a, b)| a.to_ascii_lowercase() == *b)
}