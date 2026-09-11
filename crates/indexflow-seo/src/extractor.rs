// crates/indexflow-seo/src/extractor.rs
use crate::content::{compute_word_count, extract_headings};
use crate::headers::RobotsTokens;
use crate::html_utils::clip_html;
use crate::meta::{
    count_images_missing_alt, extract_ai_directives, extract_canonical, extract_hreflang,
    extract_html_lang, extract_meta_content, extract_opengraph, extract_title,
    extract_twitter_card, extract_viewport,
};
use crate::models::{
    AiBotDirectives, HeadingItem, HreflangItem, JsonLdBlock, OpenGraphMeta, TwitterCardMeta,
    VideoObjectEntity,
};
use crate::schema::{extract_json_ld, extract_video_objects};

#[derive(Debug, Default)]
pub struct RawHtmlInspection {
    pub page_title: Option<String>,
    pub meta_description: Option<String>,
    pub canonical_url: Option<String>,
    pub h1_content: Option<String>,
    pub h1_count: usize,
    pub headings: Vec<HeadingItem>,
    pub word_count: usize,
    pub has_noindex: bool,
    pub has_nofollow: bool,
    pub robots_meta: Option<String>,
    pub hreflangs: Vec<HreflangItem>,
    pub opengraph: OpenGraphMeta,
    pub twitter_card: TwitterCardMeta,
    pub json_ld: Vec<JsonLdBlock>,
    pub video_objects: Vec<VideoObjectEntity>,
    pub ai_directives: AiBotDirectives,
    pub has_viewport: bool,
    pub html_lang: Option<String>,
    pub images_missing_alt: usize,
}

/// 纯粹的提取协调器：调用各功能模块并装配 RawHtmlInspection
pub fn inspect_html(html: &str) -> RawHtmlInspection {
    let html = clip_html(html);

    let page_title = extract_title(html);
    let headings = extract_headings(html);
    let h1_count = headings.iter().filter(|h| h.level == 1).count();
    let h1_content = headings.iter().find(|h| h.level == 1).map(|h| h.text.clone());
    let word_count = compute_word_count(html);

    let canonical_url = extract_canonical(html);
    let meta_description = extract_meta_content(html, "name", "description");

    let robots_meta = extract_meta_content(html, "name", "robots")
        .or_else(|| extract_meta_content(html, "name", "googlebot"))
        .or_else(|| extract_meta_content(html, "http-equiv", "robots"));

    let robots_tokens = RobotsTokens::parse(robots_meta.as_deref().unwrap_or(""));
    let ai_directives = extract_ai_directives(html);

    let opengraph = extract_opengraph(html);
    let twitter_card = extract_twitter_card(html);

    let json_ld = extract_json_ld(html);
    let video_objects = extract_video_objects(&json_ld);
    let hreflangs = extract_hreflang(html);
    let has_viewport = extract_viewport(html);
    let html_lang = extract_html_lang(html);
    let images_missing_alt = count_images_missing_alt(html);

    RawHtmlInspection {
        page_title,
        meta_description,
        canonical_url,
        h1_content,
        h1_count,
        headings,
        word_count,
        has_noindex: robots_tokens.noindex,
        has_nofollow: robots_tokens.nofollow,
        robots_meta,
        hreflangs,
        opengraph,
        twitter_card,
        json_ld,
        video_objects,
        ai_directives,
        has_viewport,
        html_lang,
        images_missing_alt,
    }
}