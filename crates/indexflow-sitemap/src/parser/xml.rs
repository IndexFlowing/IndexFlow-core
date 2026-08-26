use crate::models::{
    ChangeFreq, HreflangEntry, ParsedSitemap, SitemapImage, SitemapNews, SitemapUrlEntry,
    SitemapVideo,
};
use chrono::{DateTime, NaiveDate, NaiveDateTime, Utc};
use quick_xml::events::attributes::Attributes;
use quick_xml::events::Event;
use quick_xml::reader::Reader;
use tracing::warn;

#[derive(Default)]
struct XmlParseFlags {
    pub in_image: bool,
    pub in_video: bool,
    pub in_news: bool,
    pub in_news_publication: bool,
}

#[derive(Default)]
struct XmlParseBuffers {
    pub loc_buf: String,
    pub lastmod_buf: String,
    pub changefreq_buf: String,
    pub priority_buf: String,
    pub cur_image: SitemapImage,
    pub cur_video: SitemapVideo,
    pub news_pub_name: String,
    pub news_pub_lang: String,
    pub news_pub_date: String,
    pub news_title: String,
    pub news_keywords: Vec<String>,
}

impl XmlParseBuffers {
    pub fn clear_url_buffers(&mut self) {
        self.loc_buf.clear();
        self.lastmod_buf.clear();
        self.changefreq_buf.clear();
        self.priority_buf.clear();
        self.cur_image = SitemapImage::default();
        self.cur_video = SitemapVideo::default();
        self.clear_news_buffers();
    }

    pub fn clear_news_buffers(&mut self) {
        self.news_pub_name.clear();
        self.news_pub_lang.clear();
        self.news_pub_date.clear();
        self.news_title.clear();
        self.news_keywords.clear();
    }
}

pub fn parse_xml(xml: &str) -> ParsedSitemap {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);

    let mut buf = Vec::new();
    let mut is_index = false;

    let mut in_url = false;
    let mut in_sitemap = false;
    let mut flags = XmlParseFlags::default();
    let mut parse_buf = XmlParseBuffers::default();

    let mut current_tag = String::new();
    let mut alt_links: Vec<HreflangEntry> = Vec::new();
    let mut images: Vec<SitemapImage> = Vec::new();
    let mut videos: Vec<SitemapVideo> = Vec::new();
    let mut cur_news: Option<SitemapNews> = None;

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
                        parse_buf.clear_url_buffers();
                        alt_links.clear();
                        images.clear();
                        videos.clear();
                        cur_news = None;
                    }
                    b"sitemap" => {
                        in_sitemap = true;
                        parse_buf.loc_buf.clear();
                    }
                    b"image" => {
                        flags.in_image = true;
                        parse_buf.cur_image = SitemapImage::default();
                    }
                    b"video" => {
                        flags.in_video = true;
                        parse_buf.cur_video = SitemapVideo::default();
                    }
                    b"news" => {
                        flags.in_news = true;
                        parse_buf.clear_news_buffers();
                    }
                    b"publication" if flags.in_news => {
                        flags.in_news_publication = true;
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
                        handle_tag_text(&current_tag, s, &flags, &mut parse_buf);
                    }
                }
            }
            Ok(Event::CData(e)) => {
                if let Ok(text) = std::str::from_utf8(&e) {
                    let s = text.trim();
                    if !s.is_empty() {
                        handle_tag_text(&current_tag, s, &flags, &mut parse_buf);
                    }
                }
            }
            Ok(Event::End(ref e)) => {
                let name = e.local_name();
                match name.as_ref() {
                    b"image" => {
                        if flags.in_image && !parse_buf.cur_image.loc.is_empty() {
                            images.push(parse_buf.cur_image.clone());
                        }
                        flags.in_image = false;
                    }
                    b"video" => {
                        if flags.in_video && !parse_buf.cur_video.thumbnail_loc.is_empty() {
                            videos.push(parse_buf.cur_video.clone());
                        }
                        flags.in_video = false;
                    }
                    b"publication" => {
                        flags.in_news_publication = false;
                    }
                    b"news" => {
                        if flags.in_news
                            && !parse_buf.news_pub_name.is_empty()
                            && !parse_buf.news_title.is_empty()
                        {
                            cur_news = Some(SitemapNews {
                                publication_name: parse_buf.news_pub_name.clone(),
                                publication_language: parse_buf.news_pub_lang.clone(),
                                publication_date: parse_datetime(&parse_buf.news_pub_date),
                                title: parse_buf.news_title.clone(),
                                keywords: parse_buf.news_keywords.clone(),
                            });
                        }
                        flags.in_news = false;
                    }
                    b"url" => {
                        if in_url && !parse_buf.loc_buf.is_empty() {
                            entries.push(SitemapUrlEntry {
                                loc: parse_buf.loc_buf.clone(),
                                lastmod: parse_datetime(&parse_buf.lastmod_buf),
                                changefreq: ChangeFreq::from_str_loose(&parse_buf.changefreq_buf),
                                priority: parse_priority(&parse_buf.priority_buf),
                                hreflangs: alt_links.clone(),
                                images: images.clone(),
                                videos: videos.clone(),
                                news: cur_news.clone(),
                            });
                        }
                        in_url = false;
                    }
                    b"sitemap" => {
                        if in_sitemap && !parse_buf.loc_buf.is_empty() {
                            index_locs.push(parse_buf.loc_buf.clone());
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

fn handle_tag_text(
    tag: &str,
    text: &str,
    flags: &XmlParseFlags,
    buf: &mut XmlParseBuffers,
) {
    if flags.in_image {
        match tag {
            "loc" => buf.cur_image.loc = text.to_string(),
            "title" => buf.cur_image.title = Some(text.to_string()),
            "caption" => buf.cur_image.caption = Some(text.to_string()),
            "geo_location" => buf.cur_image.geo_location = Some(text.to_string()),
            "license" => buf.cur_image.license = Some(text.to_string()),
            _ => {}
        }
    } else if flags.in_video {
        match tag {
            "thumbnail_loc" => buf.cur_video.thumbnail_loc = text.to_string(),
            "title" => buf.cur_video.title = text.to_string(),
            "description" => buf.cur_video.description = text.to_string(),
            "content_loc" => buf.cur_video.content_loc = Some(text.to_string()),
            "player_loc" => buf.cur_video.player_loc = Some(text.to_string()),
            "duration" => buf.cur_video.duration_seconds = text.parse().ok(),
            "view_count" => buf.cur_video.view_count = text.parse().ok(),
            "rating" => buf.cur_video.rating = text.parse().ok(),
            "publication_date" => buf.cur_video.publication_date = parse_datetime(text),
            "expiration_date" => buf.cur_video.expiration_date = parse_datetime(text),
            "family_friendly" => {
                buf.cur_video.family_friendly = match text.to_ascii_lowercase().as_str() {
                    "yes" | "true" | "1" => Some(true),
                    "no" | "false" | "0" => Some(false),
                    _ => None,
                }
            }
            "tag" => buf.cur_video.tags.push(text.to_string()),
            "category" => buf.cur_video.category = Some(text.to_string()),
            _ => {}
        }
    } else if flags.in_news {
        if flags.in_news_publication {
            match tag {
                "name" => buf.news_pub_name = text.to_string(),
                "language" => buf.news_pub_lang = text.to_string(),
                _ => {}
            }
        } else {
            match tag {
                "publication_date" => buf.news_pub_date = text.to_string(),
                "title" => buf.news_title = text.to_string(),
                "keywords" => {
                    buf.news_keywords.extend(text.split(',').map(|s| s.trim().to_string()));
                }
                _ => {}
            }
        }
    } else {
        match tag {
            "loc" => buf.loc_buf = text.to_string(),
            "lastmod" => buf.lastmod_buf = text.to_string(),
            "changefreq" => buf.changefreq_buf = text.to_string(),
            "priority" => buf.priority_buf = text.to_string(),
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