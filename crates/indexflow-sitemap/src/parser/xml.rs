use crate::models::{
    ChangeFreq, HreflangEntry, ParsedSitemap, SitemapImage, SitemapNews, SitemapUrlEntry,
    SitemapVideo,
};
use chrono::{DateTime, NaiveDate, NaiveDateTime, Utc};
use quick_xml::events::attributes::Attributes;
use quick_xml::events::Event;
use quick_xml::reader::Reader;
use tracing::warn;

pub fn parse_xml(xml: &str) -> ParsedSitemap {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);

    let mut buf = Vec::new();
    let mut is_index = false;

    let mut in_url = false;
    let mut in_sitemap = false;
    let mut in_image = false;
    let mut in_video = false;
    let mut in_news = false;
    let mut in_news_publication = false;

    let mut current_tag = String::new();

    // 基础缓冲
    let mut loc_buf = String::new();
    let mut lastmod_buf = String::new();
    let mut changefreq_buf = String::new();
    let mut priority_buf = String::new();
    let mut alt_links: Vec<HreflangEntry> = Vec::new();

    // 扩展缓冲
    let mut images: Vec<SitemapImage> = Vec::new();
    let mut cur_image = SitemapImage::default();

    let mut videos: Vec<SitemapVideo> = Vec::new();
    let mut cur_video = SitemapVideo::default();

    let mut cur_news: Option<SitemapNews> = None;
    let mut news_pub_name = String::new();
    let mut news_pub_lang = String::new();
    let mut news_pub_date = String::new();
    let mut news_title = String::new();
    let mut news_keywords = Vec::new();

    // 结果集
    let mut entries: Vec<SitemapUrlEntry> = Vec::new();
    let mut index_locs: Vec<String> = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                let local = e.local_name();
                let name = local.as_ref();
                match name {
                    b"sitemapindex" => is_index = true,
                    b"url" => {
                        in_url = true;
                        loc_buf.clear();
                        lastmod_buf.clear();
                        changefreq_buf.clear();
                        priority_buf.clear();
                        alt_links.clear();
                        images.clear();
                        videos.clear();
                        cur_news = None;
                    }
                    b"sitemap" => {
                        in_sitemap = true;
                        loc_buf.clear();
                    }
                    b"image" => {
                        in_image = true;
                        cur_image = SitemapImage::default();
                    }
                    b"video" => {
                        in_video = true;
                        cur_video = SitemapVideo::default();
                    }
                    b"news" => {
                        in_news = true;
                        news_pub_name.clear();
                        news_pub_lang.clear();
                        news_pub_date.clear();
                        news_title.clear();
                        news_keywords.clear();
                    }
                    b"publication" if in_news => {
                        in_news_publication = true;
                    }
                    b"link" => {
                        if let Some(entry) = read_hreflang_link(e.attributes()) {
                            alt_links.push(entry);
                        }
                    }
                    _ => {
                        current_tag = String::from_utf8_lossy(name).into_owned();
                    }
                }
            }
            Ok(Event::Empty(ref e)) => {
                let local = e.local_name();
                match local.as_ref() {
                    b"sitemapindex" => is_index = true,
                    b"link" if in_url => {
                        if let Some(entry) = read_hreflang_link(e.attributes()) {
                            alt_links.push(entry);
                        }
                    }
                    _ => {}
                }
            }
            Ok(Event::Text(e)) => {
                if let Ok(text) = e.unescape() {
                    let s = text.trim();
                    if !s.is_empty() {
                        handle_tag_text(
                            &current_tag,
                            s,
                            in_image,
                            in_video,
                            in_news,
                            in_news_publication,
                            &mut loc_buf,
                            &mut lastmod_buf,
                            &mut changefreq_buf,
                            &mut priority_buf,
                            &mut cur_image,
                            &mut cur_video,
                            &mut news_pub_name,
                            &mut news_pub_lang,
                            &mut news_pub_date,
                            &mut news_title,
                            &mut news_keywords,
                        );
                    }
                }
            }
            Ok(Event::CData(e)) => {
                if let Ok(text) = std::str::from_utf8(&e) {
                    let s = text.trim();
                    if !s.is_empty() {
                        handle_tag_text(
                            &current_tag,
                            s,
                            in_image,
                            in_video,
                            in_news,
                            in_news_publication,
                            &mut loc_buf,
                            &mut lastmod_buf,
                            &mut changefreq_buf,
                            &mut priority_buf,
                            &mut cur_image,
                            &mut cur_video,
                            &mut news_pub_name,
                            &mut news_pub_lang,
                            &mut news_pub_date,
                            &mut news_title,
                            &mut news_keywords,
                        );
                    }
                }
            }
            Ok(Event::End(ref e)) => {
                let name = e.local_name();
                match name.as_ref() {
                    b"image" => {
                        if in_image && !cur_image.loc.is_empty() {
                            images.push(cur_image.clone());
                        }
                        in_image = false;
                    }
                    b"video" => {
                        if in_video && !cur_video.thumbnail_loc.is_empty() {
                            videos.push(cur_video.clone());
                        }
                        in_video = false;
                    }
                    b"publication" => {
                        in_news_publication = false;
                    }
                    b"news" => {
                        if in_news && !news_pub_name.is_empty() && !news_title.is_empty() {
                            cur_news = Some(SitemapNews {
                                publication_name: news_pub_name.clone(),
                                publication_language: news_pub_lang.clone(),
                                publication_date: parse_datetime(&news_pub_date),
                                title: news_title.clone(),
                                keywords: news_keywords.clone(),
                            });
                        }
                        in_news = false;
                    }
                    b"url" => {
                        if in_url && !loc_buf.is_empty() {
                            entries.push(SitemapUrlEntry {
                                loc: loc_buf.clone(),
                                lastmod: parse_datetime(&lastmod_buf),
                                changefreq: ChangeFreq::from_str_loose(&changefreq_buf),
                                priority: parse_priority(&priority_buf),
                                hreflangs: alt_links.clone(),
                                images: images.clone(),
                                videos: videos.clone(),
                                news: cur_news.clone(),
                            });
                        }
                        in_url = false;
                    }
                    b"sitemap" => {
                        if in_sitemap && !loc_buf.is_empty() {
                            index_locs.push(loc_buf.clone());
                        }
                        in_sitemap = false;
                    }
                    _ => {
                        current_tag.clear();
                    }
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => {
                warn!(error = %e, "Sitemap XML parser encountered partial error");
                break;
            }
            _ => {}
        }
        buf.clear();
    }

    if is_index {
        if index_locs.is_empty() && !entries.is_empty() {
            index_locs = entries.into_iter().map(|e| e.loc).collect();
        }
        ParsedSitemap::Index {
            child_urls: index_locs,
        }
    } else {
        ParsedSitemap::UrlSet { entries }
    }
}

#[allow(clippy::too_many_arguments)]
fn handle_tag_text(
    tag: &str,
    text: &str,
    in_image: bool,
    in_video: bool,
    in_news: bool,
    in_news_pub: bool,
    loc_buf: &mut String,
    lastmod_buf: &mut String,
    changefreq_buf: &mut String,
    priority_buf: &mut String,
    cur_image: &mut SitemapImage,
    cur_video: &mut SitemapVideo,
    news_pub_name: &mut String,
    news_pub_lang: &mut String,
    news_pub_date: &mut String,
    news_title: &mut String,
    news_keywords: &mut Vec<String>,
) {
    if in_image {
        match tag {
            "loc" => cur_image.loc = text.to_string(),
            "title" => cur_image.title = Some(text.to_string()),
            "caption" => cur_image.caption = Some(text.to_string()),
            "geo_location" => cur_image.geo_location = Some(text.to_string()),
            "license" => cur_image.license = Some(text.to_string()),
            _ => {}
        }
    } else if in_video {
        match tag {
            "thumbnail_loc" => cur_video.thumbnail_loc = text.to_string(),
            "title" => cur_video.title = text.to_string(),
            "description" => cur_video.description = text.to_string(),
            "content_loc" => cur_video.content_loc = Some(text.to_string()),
            "player_loc" => cur_video.player_loc = Some(text.to_string()),
            "duration" => cur_video.duration_seconds = text.parse().ok(),
            "view_count" => cur_video.view_count = text.parse().ok(),
            "rating" => cur_video.rating = text.parse().ok(),
            "publication_date" => cur_video.publication_date = parse_datetime(text),
            "expiration_date" => cur_video.expiration_date = parse_datetime(text),
            "family_friendly" => {
                cur_video.family_friendly = match text.to_ascii_lowercase().as_str() {
                    "yes" | "true" | "1" => Some(true),
                    "no" | "false" | "0" => Some(false),
                    _ => None,
                }
            }
            "tag" => cur_video.tags.push(text.to_string()),
            "category" => cur_video.category = Some(text.to_string()),
            _ => {}
        }
    } else if in_news {
        if in_news_pub {
            match tag {
                "name" => *news_pub_name = text.to_string(),
                "language" => *news_pub_lang = text.to_string(),
                _ => {}
            }
        } else {
            match tag {
                "publication_date" => *news_pub_date = text.to_string(),
                "title" => *news_title = text.to_string(),
                "keywords" => {
                    news_keywords.extend(text.split(',').map(|s| s.trim().to_string()));
                }
                _ => {}
            }
        }
    } else {
        match tag {
            "loc" => *loc_buf = text.to_string(),
            "lastmod" => *lastmod_buf = text.to_string(),
            "changefreq" => *changefreq_buf = text.to_string(),
            "priority" => *priority_buf = text.to_string(),
            _ => {}
        }
    }
}

fn read_hreflang_link(attrs: Attributes<'_>) -> Option<HreflangEntry> {
    let mut href = None;
    let mut hreflang = None;
    for attr in attrs.flatten() {
        match attr.key.local_name().as_ref() {
            b"href" => {
                if let Ok(v) = attr.unescape_value() {
                    href = Some(v.into_owned());
                }
            }
            b"hreflang" => {
                if let Ok(v) = attr.unescape_value() {
                    hreflang = Some(v.into_owned());
                }
            }
            _ => {}
        }
    }
    match (hreflang, href) {
        (Some(lang), Some(href)) if !lang.trim().is_empty() && !href.trim().is_empty() => {
            Some(HreflangEntry {
                lang: lang.trim().to_string(),
                href: href.trim().to_string(),
            })
        }
        _ => None,
    }
}

pub fn parse_priority(s: &str) -> Option<f64> {
    s.trim().parse::<f64>().ok().map(|v| v.clamp(0.0, 1.0))
}

pub fn parse_datetime(s: &str) -> Option<DateTime<Utc>> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }
    if let Ok(dt) = DateTime::parse_from_rfc3339(s) {
        return Some(dt.with_timezone(&Utc));
    }
    if let Ok(ndt) = NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S") {
        return Some(ndt.and_utc());
    }
    if let Ok(ndt) = NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S") {
        return Some(ndt.and_utc());
    }
    if let Ok(d) = NaiveDate::parse_from_str(s, "%Y-%m-%d") {
        return d.and_hms_opt(0, 0, 0).map(|ndt| ndt.and_utc());
    }
    None
}