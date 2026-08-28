//! Streaming `quick-xml` sitemap parser.
//!
//! Design notes
//! ------------
//! * All comparisons use **local-name** bytes, so `<image:image>`, `<img:image>`
//!   and an unprefixed `<image>` are treated identically.
//! * Text and CDATA events for the same open leaf tag are **concatenated**, so
//!   `<loc>https://x.com/<![CDATA[a?b=1&c=2]]></loc>` round-trips.
//! * `unescape()` covers the five XML entities; a pre-pass rewrites *bare*
//!   `&` outside CDATA (common in hand-written locs) into `&amp;` so the
//!   tokenizer does not abort mid-document.
//! * Structural buffers are `mem::take`n into the output — no per-URL clone.

use crate::models::{
    ChangeFreq, HreflangEntry, ParsedSitemap, SitemapImage, SitemapNews, SitemapUrlEntry,
    SitemapVideo, MAX_URLS_PER_SITEMAP, MAX_URL_LEN,
};
use chrono::{DateTime, FixedOffset, NaiveDate, NaiveDateTime, TimeZone, Utc};
use quick_xml::events::attributes::Attributes;
use quick_xml::events::Event;
use quick_xml::reader::Reader;
use std::borrow::Cow;
use tracing::warn;
use url::Url;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tag {
    Urlset,
    SitemapIndex,
    Url,
    Sitemap,
    Loc,
    Lastmod,
    Changefreq,
    Priority,
    Image,
    Video,
    News,
    Publication,
    Link,
    Title,
    Caption,
    GeoLocation,
    License,
    ThumbnailLoc,
    Description,
    ContentLoc,
    PlayerLoc,
    Duration,
    ViewCount,
    Rating,
    PublicationDate,
    ExpirationDate,
    FamilyFriendly,
    VideoTag,
    Category,
    Name,
    Language,
    Keywords,
    Other,
}

impl Tag {
    fn from_local(name: &[u8]) -> Self {
        match name {
            b"urlset" | b"URLSET" => Self::Urlset,
            b"sitemapindex" | b"sitemapIndex" | b"SITEMAPINDEX" => Self::SitemapIndex,
            b"url" | b"URL" => Self::Url,
            b"sitemap" | b"SITEMAP" => Self::Sitemap,
            b"loc" | b"LOC" => Self::Loc,
            b"lastmod" | b"LASTMOD" => Self::Lastmod,
            b"changefreq" | b"CHANGEFREQ" => Self::Changefreq,
            b"priority" | b"PRIORITY" => Self::Priority,
            b"image" => Self::Image,
            b"video" => Self::Video,
            b"news" => Self::News,
            b"publication" => Self::Publication,
            b"link" => Self::Link,
            b"title" => Self::Title,
            b"caption" => Self::Caption,
            b"geo_location" => Self::GeoLocation,
            b"license" => Self::License,
            b"thumbnail_loc" => Self::ThumbnailLoc,
            b"description" => Self::Description,
            b"content_loc" => Self::ContentLoc,
            b"player_loc" => Self::PlayerLoc,
            b"duration" => Self::Duration,
            b"view_count" => Self::ViewCount,
            b"rating" => Self::Rating,
            b"publication_date" => Self::PublicationDate,
            b"expiration_date" => Self::ExpirationDate,
            b"family_friendly" => Self::FamilyFriendly,
            b"tag" => Self::VideoTag,
            b"category" => Self::Category,
            b"name" => Self::Name,
            b"language" => Self::Language,
            b"keywords" => Self::Keywords,
            _ => Self::Other,
        }
    }
}

#[derive(Default)]
struct XmlParseFlags {
    in_image: bool,
    in_video: bool,
    in_news: bool,
    in_news_publication: bool,
}

#[derive(Default)]
struct XmlParseBuffers {
    loc_buf: String,
    lastmod_buf: String,
    changefreq_buf: String,
    priority_buf: String,
    cur_image: SitemapImage,
    cur_video: SitemapVideo,
    news_pub_name: String,
    news_pub_lang: String,
    news_pub_date: String,
    news_title: String,
    news_keywords: Vec<String>,
}

impl XmlParseBuffers {
    fn clear_url_buffers(&mut self) {
        self.loc_buf.clear();
        self.lastmod_buf.clear();
        self.changefreq_buf.clear();
        self.priority_buf.clear();
        self.cur_image = SitemapImage::default();
        self.cur_video = SitemapVideo::default();
        self.clear_news_buffers();
    }

    fn clear_news_buffers(&mut self) {
        self.news_pub_name.clear();
        self.news_pub_lang.clear();
        self.news_pub_date.clear();
        self.news_title.clear();
        self.news_keywords.clear();
    }

    fn clear_leaf(&mut self, tag: Tag, flags: &XmlParseFlags) {
        match tag {
            Tag::Loc if flags.in_image => self.cur_image.loc.clear(),
            Tag::Loc if flags.in_video => self.cur_video.content_loc = None,
            Tag::Loc => self.loc_buf.clear(),
            Tag::Lastmod => self.lastmod_buf.clear(),
            Tag::Changefreq => self.changefreq_buf.clear(),
            Tag::Priority => self.priority_buf.clear(),
            Tag::Title if flags.in_image => self.cur_image.title = None,
            Tag::Title if flags.in_video => self.cur_video.title.clear(),
            Tag::Title if flags.in_news => self.news_title.clear(),
            Tag::Caption => self.cur_image.caption = None,
            Tag::GeoLocation => self.cur_image.geo_location = None,
            Tag::License => self.cur_image.license = None,
            Tag::ThumbnailLoc => self.cur_video.thumbnail_loc.clear(),
            Tag::Description => self.cur_video.description.clear(),
            Tag::ContentLoc => self.cur_video.content_loc = None,
            Tag::PlayerLoc => self.cur_video.player_loc = None,
            Tag::Name if flags.in_news_publication => self.news_pub_name.clear(),
            Tag::Language => self.news_pub_lang.clear(),
            Tag::PublicationDate if flags.in_news => self.news_pub_date.clear(),
            Tag::Keywords => {}
            _ => {}
        }
    }
}

/// Parse a sitemap XML document. Never panics; malformed input yields a partial result.
pub fn parse_xml(xml: &str) -> ParsedSitemap {
    parse_xml_limited(xml, MAX_URLS_PER_SITEMAP)
}

/// Streaming parse with an explicit URL / child-sitemap cap (Google: 50_000).
pub fn parse_xml_limited(xml: &str, max_urls: usize) -> ParsedSitemap {
    let sanitized = sanitize_bare_ampersands(xml);
    let mut reader = Reader::from_str(sanitized.as_ref());
    {
        let cfg = reader.config_mut();
        cfg.trim_text(true);
        cfg.check_end_names = false;
        cfg.check_comments = false;
    }

    let mut is_index = false;
    let mut in_url = false;
    let mut in_sitemap = false;
    let mut flags = XmlParseFlags::default();
    let mut parse_buf = XmlParseBuffers::default();
    let mut current_tag = Tag::Other;

    let mut alt_links: Vec<HreflangEntry> = Vec::new();
    let mut images: Vec<SitemapImage> = Vec::new();
    let mut videos: Vec<SitemapVideo> = Vec::new();
    let mut cur_news: Option<SitemapNews> = None;

    let mut entries: Vec<SitemapUrlEntry> = Vec::new();
    let mut index_locs: Vec<String> = Vec::new();

    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                let tag = Tag::from_local(e.local_name().as_ref());
                match tag {
                    Tag::SitemapIndex => is_index = true,
                    Tag::Url => {
                        in_url = true;
                        parse_buf.clear_url_buffers();
                        alt_links.clear();
                        images.clear();
                        videos.clear();
                        cur_news = None;
                    }
                    Tag::Sitemap => {
                        in_sitemap = true;
                        parse_buf.loc_buf.clear();
                    }
                    Tag::Image => {
                        flags.in_image = true;
                        parse_buf.cur_image = SitemapImage::default();
                    }
                    Tag::Video => {
                        flags.in_video = true;
                        parse_buf.cur_video = SitemapVideo::default();
                    }
                    Tag::News => {
                        flags.in_news = true;
                        parse_buf.clear_news_buffers();
                    }
                    Tag::Publication if flags.in_news => {
                        flags.in_news_publication = true;
                    }
                    Tag::Link => {
                        if let Some(entry) = read_hreflang_link(e.attributes()) {
                            alt_links.push(entry);
                        }
                    }
                    other => {
                        current_tag = other;
                        parse_buf.clear_leaf(other, &flags);
                    }
                }
            }
            Ok(Event::Empty(e)) => {
                let tag = Tag::from_local(e.local_name().as_ref());
                match tag {
                    Tag::SitemapIndex => is_index = true,
                    Tag::Link if in_url => {
                        if let Some(entry) = read_hreflang_link(e.attributes()) {
                            alt_links.push(entry);
                        }
                    }
                    _ => {}
                }
            }
            Ok(Event::Text(e)) => {
                let text = match e.unescape() {
                    Ok(t) => t,
                    Err(_) => Cow::Owned(String::from_utf8_lossy(e.as_ref()).into_owned()),
                };
                if !text.is_empty() {
                    handle_tag_text(current_tag, text.as_ref(), &flags, &mut parse_buf);
                }
            }
            Ok(Event::CData(e)) => {
                let text = String::from_utf8_lossy(e.as_ref());
                if !text.is_empty() {
                    handle_tag_text(current_tag, text.as_ref(), &flags, &mut parse_buf);
                }
            }
            Ok(Event::End(e)) => {
                let tag = Tag::from_local(e.local_name().as_ref());
                match tag {
                    Tag::Image => {
                        if flags.in_image && is_http_url(&parse_buf.cur_image.loc) {
                            images.push(std::mem::take(&mut parse_buf.cur_image));
                        }
                        flags.in_image = false;
                    }
                    Tag::Video => {
                        if flags.in_video && !parse_buf.cur_video.thumbnail_loc.is_empty() {
                            videos.push(std::mem::take(&mut parse_buf.cur_video));
                        }
                        flags.in_video = false;
                    }
                    Tag::Publication => {
                        flags.in_news_publication = false;
                    }
                    Tag::News => {
                        if flags.in_news
                            && !parse_buf.news_pub_name.is_empty()
                            && !parse_buf.news_title.is_empty()
                        {
                            cur_news = Some(SitemapNews {
                                publication_name: std::mem::take(&mut parse_buf.news_pub_name),
                                publication_language: std::mem::take(&mut parse_buf.news_pub_lang),
                                publication_date: parse_datetime(&parse_buf.news_pub_date),
                                title: std::mem::take(&mut parse_buf.news_title),
                                keywords: std::mem::take(&mut parse_buf.news_keywords),
                            });
                        }
                        flags.in_news = false;
                    }
                    Tag::Url => {
                        if in_url {
                            if let Some(entry) = finish_url_entry(
                                &mut parse_buf,
                                &mut alt_links,
                                &mut images,
                                &mut videos,
                                &mut cur_news,
                            ) {
                                if entries.len() < max_urls {
                                    entries.push(entry);
                                } else {
                                    warn!(
                                        max_urls,
                                        "urlset exceeded per-document URL cap; remaining <url> skipped"
                                    );
                                }
                            }
                        }
                        in_url = false;
                    }
                    Tag::Sitemap => {
                        if in_sitemap {
                            if let Some(loc) = finish_loc(&mut parse_buf.loc_buf) {
                                if index_locs.len() < max_urls {
                                    index_locs.push(loc);
                                } else {
                                    warn!(
                                        max_urls,
                                        "sitemapindex exceeded per-document child cap; remaining <sitemap> skipped"
                                    );
                                }
                            }
                        }
                        in_sitemap = false;
                    }
                    _ => {
                        current_tag = Tag::Other;
                    }
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => {
                // Tokenizer gave up (truncated file, illegal control char, …).
                // Return whatever we already collected — never panic.
                warn!(error = %e, "Sitemap XML parser encountered a non-fatal error; returning partial result");
                break;
            }
            _ => {}
        }
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

fn finish_url_entry(
    parse_buf: &mut XmlParseBuffers,
    alt_links: &mut Vec<HreflangEntry>,
    images: &mut Vec<SitemapImage>,
    videos: &mut Vec<SitemapVideo>,
    cur_news: &mut Option<SitemapNews>,
) -> Option<SitemapUrlEntry> {
    let loc = finish_loc(&mut parse_buf.loc_buf)?;
    Some(SitemapUrlEntry {
        loc,
        lastmod: parse_datetime(&parse_buf.lastmod_buf),
        changefreq: ChangeFreq::from_str_loose(&parse_buf.changefreq_buf),
        priority: parse_priority(&parse_buf.priority_buf),
        hreflangs: std::mem::take(alt_links),
        images: std::mem::take(images),
        videos: std::mem::take(videos),
        news: cur_news.take(),
    })
}

fn finish_loc(buf: &mut String) -> Option<String> {
    let loc = take_trimmed(buf);
    if loc.is_empty() || loc.len() > MAX_URL_LEN {
        return None;
    }
    if is_http_url(&loc) {
        Some(loc)
    } else {
        None
    }
}

fn take_trimmed(s: &mut String) -> String {
    let trimmed = s.trim();
    if trimmed.is_empty() {
        s.clear();
        return String::new();
    }
    if trimmed.len() == s.len() {
        std::mem::take(s)
    } else {
        let out = trimmed.to_string();
        s.clear();
        out
    }
}

fn is_http_url(s: &str) -> bool {
    match Url::parse(s) {
        Ok(u) => matches!(u.scheme(), "http" | "https") && u.host_str().is_some(),
        Err(_) => false,
    }
}

fn handle_tag_text(tag: Tag, text: &str, flags: &XmlParseFlags, buf: &mut XmlParseBuffers) {
    if flags.in_image {
        match tag {
            Tag::Loc => buf.cur_image.loc.push_str(text),
            Tag::Title => append_opt(&mut buf.cur_image.title, text),
            Tag::Caption => append_opt(&mut buf.cur_image.caption, text),
            Tag::GeoLocation => append_opt(&mut buf.cur_image.geo_location, text),
            Tag::License => append_opt(&mut buf.cur_image.license, text),
            _ => {}
        }
        return;
    }
    if flags.in_video {
        match tag {
            Tag::ThumbnailLoc => buf.cur_video.thumbnail_loc.push_str(text),
            Tag::Title => buf.cur_video.title.push_str(text),
            Tag::Description => buf.cur_video.description.push_str(text),
            Tag::ContentLoc | Tag::Loc => append_opt(&mut buf.cur_video.content_loc, text),
            Tag::PlayerLoc => append_opt(&mut buf.cur_video.player_loc, text),
            Tag::Duration => buf.cur_video.duration_seconds = parse_u32_loose(text),
            Tag::ViewCount => buf.cur_video.view_count = parse_u64_loose(text),
            Tag::Rating => buf.cur_video.rating = text.trim().parse().ok(),
            Tag::PublicationDate => buf.cur_video.publication_date = parse_datetime(text),
            Tag::ExpirationDate => buf.cur_video.expiration_date = parse_datetime(text),
            Tag::FamilyFriendly => buf.cur_video.family_friendly = parse_bool_loose(text),
            Tag::VideoTag => buf.cur_video.tags.push(text.trim().to_string()),
            Tag::Category => append_opt(&mut buf.cur_video.category, text),
            _ => {}
        }
        return;
    }
    if flags.in_news {
        if flags.in_news_publication {
            match tag {
                Tag::Name => buf.news_pub_name.push_str(text),
                Tag::Language => buf.news_pub_lang.push_str(text),
                _ => {}
            }
        } else {
            match tag {
                Tag::PublicationDate => buf.news_pub_date.push_str(text),
                Tag::Title => buf.news_title.push_str(text),
                Tag::Keywords => {
                    buf.news_keywords.extend(
                        text.split(',')
                            .map(str::trim)
                            .filter(|s| !s.is_empty())
                            .map(str::to_string),
                    );
                }
                _ => {}
            }
        }
        return;
    }
    match tag {
        Tag::Loc => buf.loc_buf.push_str(text),
        Tag::Lastmod => buf.lastmod_buf.push_str(text),
        Tag::Changefreq => buf.changefreq_buf.push_str(text),
        Tag::Priority => buf.priority_buf.push_str(text),
        _ => {}
    }
}

fn append_opt(slot: &mut Option<String>, text: &str) {
    match slot {
        Some(existing) => existing.push_str(text),
        None => *slot = Some(text.to_string()),
    }
}

fn parse_bool_loose(s: &str) -> Option<bool> {
    match s.trim().to_ascii_lowercase().as_str() {
        "yes" | "true" | "1" => Some(true),
        "no" | "false" | "0" => Some(false),
        _ => None,
    }
}

fn parse_u32_loose(s: &str) -> Option<u32> {
    let s = s.trim();
    if let Ok(v) = s.parse::<u32>() {
        return Some(v);
    }
    s.parse::<f64>().ok().and_then(|f| {
        if f.is_finite() && (0.0..=u32::MAX as f64).contains(&f) {
            Some(f as u32)
        } else {
            None
        }
    })
}

fn parse_u64_loose(s: &str) -> Option<u64> {
    let s = s.trim();
    if let Ok(v) = s.parse::<u64>() {
        return Some(v);
    }
    s.parse::<f64>().ok().and_then(|f| {
        if f.is_finite() && (0.0..=u64::MAX as f64).contains(&f) {
            Some(f as u64)
        } else {
            None
        }
    })
}

fn read_hreflang_link(attrs: Attributes<'_>) -> Option<HreflangEntry> {
    let mut href = None;
    let mut hreflang = None;
    let mut rel = None;
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
            b"rel" => {
                if let Ok(v) = attr.unescape_value() {
                    rel = Some(v.into_owned());
                }
            }
            _ => {}
        }
    }
    if let Some(rel) = rel.as_deref() {
        let ok = rel
            .split_ascii_whitespace()
            .any(|t| t.eq_ignore_ascii_case("alternate"));
        if !ok {
            return None;
        }
    }
    match (hreflang, href) {
        (Some(lang), Some(href)) => {
            let lang = lang.trim();
            let href = href.trim();
            if lang.is_empty() || href.is_empty() || href.len() > MAX_URL_LEN {
                return None;
            }
            Some(HreflangEntry {
                lang: lang.to_string(),
                href: href.to_string(),
            })
        }
        _ => None,
    }
}

/// Clamp sitemap `<priority>` to the spec range `[0.0, 1.0]`.
pub fn parse_priority(s: &str) -> Option<f64> {
    s.trim()
        .parse::<f64>()
        .ok()
        .filter(|v| v.is_finite())
        .map(|v| v.clamp(0.0, 1.0))
}

/// Parse W3C Datetime, RFC 3339, ISO 8601, date-only, and common sloppy variants.
///
/// Timezone-less values are interpreted as UTC. Never panics.
pub fn parse_datetime(s: &str) -> Option<DateTime<Utc>> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }

    if let Ok(dt) = DateTime::parse_from_rfc3339(s) {
        return Some(dt.with_timezone(&Utc));
    }

    // `2026-03-30 10:00:00` → insert T.
    let owned;
    let s = if let Some(sp) = s.find(' ') {
        if s.as_bytes().get(4) == Some(&b'-') {
            owned = format!("{}T{}", &s[..sp], s[sp + 1..].trim());
            owned.as_str()
        } else {
            s
        }
    } else {
        s
    };

    let (naive_part, offset) = split_timezone(s)?;
    let naive_part = naive_part.trim();
    if naive_part.is_empty() {
        return None;
    }

    if let Some(ndt) = parse_naive_datetime(naive_part) {
        return offset
            .from_local_datetime(&ndt)
            .single()
            .map(|dt| dt.with_timezone(&Utc));
    }

    None
}

/// Split a trailing `Z` / `+hh:mm` / `+hhmm` / `-hh:mm` from `s`.
///
/// Returns `(naive_body, offset)` where a missing timezone is UTC.
fn split_timezone(s: &str) -> Option<(&str, FixedOffset)> {
    let utc = FixedOffset::east_opt(0)?;
    let bytes = s.as_bytes();
    if bytes.is_empty() {
        return None;
    }
    let last = bytes[bytes.len() - 1];
    if last == b'Z' || last == b'z' {
        let body = match s.get(..s.len() - 1) {
            Some(b) => b,
            None => return None,
        };
        return Some((body, utc));
    }

    // Timezone offset only appears after the date (`YYYY-MM-DD` is 10 chars).
    let search_from = 10.min(s.len());
    let mut tz_at = None;
    for (i, &b) in bytes.iter().enumerate().skip(search_from) {
        if b == b'+' || (b == b'-' && i > search_from && bytes.get(i - 1) != Some(&b'T')) {
            // A `-` after `T` begins the time (`T10:00:00`); a `-` later that
            // is not a date separator is the timezone. Date uses `-` at 4 and 7.
            if i != 4 && i != 7 {
                tz_at = Some(i);
            }
        }
    }
    // Prefer the last +/- after the time designator.
    if let Some(idx) = tz_at {
        if let Some(off) = parse_offset(&s[idx..]) {
            return Some((&s[..idx], off));
        }
    }
    Some((s, utc))
}

fn parse_offset(s: &str) -> Option<FixedOffset> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }
    let sign: i32 = match s.as_bytes().first().copied() {
        Some(b'+') => 1,
        Some(b'-') => -1,
        _ => return None,
    };
    let rest = s.get(1..)?;
    let digits: String = rest.chars().filter(|c| c.is_ascii_digit()).collect();
    if digits.len() < 2 {
        return None;
    }
    let hh: i32 = digits.get(..2)?.parse().ok()?;
    let mm: i32 = if digits.len() >= 4 {
        digits.get(2..4)?.parse().ok()?
    } else {
        0
    };
    if !(0..=14).contains(&hh) || !(0..60).contains(&mm) {
        return None;
    }
    FixedOffset::east_opt(sign * (hh * 3600 + mm * 60))
}

fn parse_naive_datetime(s: &str) -> Option<NaiveDateTime> {
    const DT_FMTS: &[&str] = &[
        "%Y-%m-%dT%H:%M:%S%.f",
        "%Y-%m-%dT%H:%M:%S",
        "%Y-%m-%dT%H:%M",
        "%Y-%m-%dT%H",
        "%Y%m%dT%H%M%S",
        "%Y%m%dT%H%M",
    ];
    for fmt in DT_FMTS {
        if let Ok(ndt) = NaiveDateTime::parse_from_str(s, fmt) {
            return Some(ndt);
        }
    }
    const D_FMTS: &[&str] = &["%Y-%m-%d", "%Y-%m", "%Y", "%Y%m%d"];
    for fmt in D_FMTS {
        if let Ok(d) = NaiveDate::parse_from_str(s, fmt) {
            return d.and_hms_opt(0, 0, 0);
        }
    }
    None
}

/// Rewrite `&` that is **not** the start of an XML entity, skipping CDATA
/// sections so literal ampersands inside `<![CDATA[...]]>` stay untouched.
fn sanitize_bare_ampersands(input: &str) -> Cow<'_, str> {
    if !input.as_bytes().contains(&b'&') {
        return Cow::Borrowed(input);
    }
    let bytes = input.as_bytes();
    let mut out = String::with_capacity(input.len() + 16);
    let mut i = 0;
    let mut copied_upto = 0;
    let mut in_cdata = false;
    while i < bytes.len() {
        if !in_cdata && bytes[i] == b'<' && starts_with_at(bytes, i, b"<![CDATA[") {
            in_cdata = true;
            i += 9;
            continue;
        }
        if in_cdata && bytes[i] == b']' && starts_with_at(bytes, i, b"]]>") {
            in_cdata = false;
            i += 3;
            continue;
        }
        if !in_cdata && bytes[i] == b'&' && !is_well_formed_entity(bytes, i) {
            if let Some(chunk) = input.get(copied_upto..i) {
                out.push_str(chunk);
            }
            out.push_str("&amp;");
            copied_upto = i + 1;
        }
        i += 1;
    }
    if copied_upto == 0 {
        Cow::Borrowed(input)
    } else {
        if let Some(tail) = input.get(copied_upto..) {
            out.push_str(tail);
        }
        Cow::Owned(out)
    }
}

fn starts_with_at(bytes: &[u8], i: usize, pat: &[u8]) -> bool {
    bytes.get(i..).map(|s| s.starts_with(pat)).unwrap_or(false)
}

/// `true` when `bytes[i..]` looks like `&name;`, `&#123;` or `&#x1F;`.
fn is_well_formed_entity(bytes: &[u8], amp_idx: usize) -> bool {
    let rest = match bytes.get(amp_idx + 1..) {
        Some(r) => r,
        None => return false,
    };
    if rest.is_empty() {
        return false;
    }
    let mut j;
    if rest[0] == b'#' {
        j = 1;
        if rest.get(1) == Some(&b'x') || rest.get(1) == Some(&b'X') {
            j = 2;
            if j >= rest.len() || !rest[j].is_ascii_hexdigit() {
                return false;
            }
            j += 1;
            while j < rest.len() && rest[j].is_ascii_hexdigit() {
                j += 1;
                if j > 8 {
                    break;
                }
            }
        } else {
            if j >= rest.len() || !rest[j].is_ascii_digit() {
                return false;
            }
            j += 1;
            while j < rest.len() && rest[j].is_ascii_digit() {
                j += 1;
                if j > 10 {
                    break;
                }
            }
        }
    } else {
        if !rest[0].is_ascii_alphabetic() {
            return false;
        }
        j = 1;
        while j < rest.len() && rest[j].is_ascii_alphanumeric() {
            j += 1;
            if j > 32 {
                break;
            }
        }
    }
    rest.get(j) == Some(&b';')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn namespace_prefix_and_unprefixed_are_equivalent() {
        let prefixed = r#"<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9"
                xmlns:image="http://www.google.com/schemas/sitemap-image/1.1">
          <url>
            <loc>https://example.com/a</loc>
            <image:image><image:loc>https://example.com/a.jpg</image:loc></image:image>
          </url>
        </urlset>"#;
        let unprefixed = r#"<urlset>
          <url>
            <loc>https://example.com/a</loc>
            <image><loc>https://example.com/a.jpg</loc></image>
          </url>
        </urlset>"#;
        let a = match parse_xml(prefixed) {
            ParsedSitemap::UrlSet { entries } => entries,
            _ => panic!("prefixed"),
        };
        let b = match parse_xml(unprefixed) {
            ParsedSitemap::UrlSet { entries } => entries,
            _ => panic!("unprefixed"),
        };
        assert_eq!(a[0].images[0].loc, b[0].images[0].loc);
        assert_eq!(a[0].images[0].loc, "https://example.com/a.jpg");
    }

    #[test]
    fn custom_prefix_image_and_xhtml_link() {
        let xml = r#"<urlset xmlns:img="http://www.google.com/schemas/sitemap-image/1.1"
                             xmlns:xh="http://www.w3.org/1999/xhtml">
          <url>
            <loc>https://example.com/p</loc>
            <xh:link rel="alternate" hreflang="zh-Hans" href="https://example.com/zh/p"/>
            <img:image><img:loc>https://example.com/p.jpg</img:loc><img:title>封面</img:title></img:image>
          </url>
        </urlset>"#;
        match parse_xml(xml) {
            ParsedSitemap::UrlSet { entries } => {
                assert_eq!(entries[0].hreflangs[0].lang, "zh-Hans");
                assert_eq!(entries[0].images[0].title.as_deref(), Some("封面"));
            }
            _ => panic!("expected urlset"),
        }
    }

    #[test]
    fn cdata_concat_and_entity_unescape() {
        let xml = r#"<urlset>
          <url>
            <loc>https://example.com/<![CDATA[q?a=1&b=2]]></loc>
            <lastmod><![CDATA[2026-03-30T12:00:00Z]]></lastmod>
          </url>
          <url>
            <loc>https://example.com/x?a=1&amp;b=2</loc>
          </url>
        </urlset>"#;
        match parse_xml(xml) {
            ParsedSitemap::UrlSet { entries } => {
                assert_eq!(entries[0].loc, "https://example.com/q?a=1&b=2");
                assert!(entries[0].lastmod.is_some());
                assert_eq!(entries[1].loc, "https://example.com/x?a=1&b=2");
            }
            _ => panic!("expected urlset"),
        }
    }

    #[test]
    fn bare_ampersand_in_loc_is_tolerated() {
        let xml = r#"<urlset>
          <url><loc>https://example.com/search?q=a&b=2</loc></url>
        </urlset>"#;
        match parse_xml(xml) {
            ParsedSitemap::UrlSet { entries } => {
                assert_eq!(entries.len(), 1);
                assert_eq!(entries[0].loc, "https://example.com/search?q=a&b=2");
            }
            _ => panic!("expected urlset"),
        }
    }

    #[test]
    fn cdata_ampersand_is_not_double_escaped() {
        let xml = r#"<urlset>
          <url><loc><![CDATA[https://example.com/a?x=1&y=2]]></loc></url>
        </urlset>"#;
        match parse_xml(xml) {
            ParsedSitemap::UrlSet { entries } => {
                assert_eq!(entries[0].loc, "https://example.com/a?x=1&y=2");
            }
            _ => panic!("expected urlset"),
        }
    }

    #[test]
    fn datetime_format_matrix() {
        let cases = [
            "2026-03-30T10:00:00Z",
            "2026-03-30T10:00:00.123Z",
            "2026-03-30T10:00:00+08:00",
            "2026-03-30T10:00:00.5+00:00",
            "2026-03-30T10:00:00",
            "2026-03-30 10:00:00",
            "2026-03-30T10:00Z",
            "2026-03-30",
            "2026-03-30T18:00:00+0800",
        ];
        for c in cases {
            assert!(
                parse_datetime(c).is_some(),
                "should parse datetime {c:?}"
            );
        }
        assert!(parse_datetime("").is_none());
        assert!(parse_datetime("not-a-date").is_none());

        let a = parse_datetime("2026-03-30T10:00:00+08:00").unwrap();
        let b = parse_datetime("2026-03-30T02:00:00Z").unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn priority_clamped_and_changefreq_loose() {
        assert_eq!(parse_priority("0.8"), Some(0.8));
        assert_eq!(parse_priority("1.5"), Some(1.0));
        assert_eq!(parse_priority("-1"), Some(0.0));
        assert_eq!(parse_priority("nope"), None);
        assert_eq!(ChangeFreq::from_str_loose("Weekly"), Some(ChangeFreq::Weekly));
        assert_eq!(ChangeFreq::from_str_loose("nope"), None);
    }

    #[test]
    fn truncated_xml_returns_partial() {
        let xml = r#"<urlset>
          <url><loc>https://example.com/ok</loc></url>
          <url><loc>https://example.com/trunc"#;
        match parse_xml(xml) {
            ParsedSitemap::UrlSet { entries } => {
                assert_eq!(entries.len(), 1);
                assert_eq!(entries[0].loc, "https://example.com/ok");
            }
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn skips_javascript_and_oversized_locs() {
        let xml = format!(
            r#"<urlset>
              <url><loc>javascript:alert(1)</loc></url>
              <url><loc>https://example.com/{}</loc></url>
              <url><loc>https://example.com/ok</loc></url>
            </urlset>"#,
            "a".repeat(3000)
        );
        match parse_xml(&xml) {
            ParsedSitemap::UrlSet { entries } => {
                assert_eq!(entries.len(), 1);
                assert_eq!(entries[0].loc, "https://example.com/ok");
            }
            _ => panic!("expected urlset"),
        }
    }

    #[test]
    fn url_cap_is_honoured() {
        let mut xml = String::from("<urlset>");
        for i in 0..8 {
            xml.push_str(&format!("<url><loc>https://example.com/{i}</loc></url>"));
        }
        xml.push_str("</urlset>");
        match parse_xml_limited(&xml, 3) {
            ParsedSitemap::UrlSet { entries } => assert_eq!(entries.len(), 3),
            _ => panic!("expected urlset"),
        }
    }

    #[test]
    fn comment_before_root_still_parses_via_direct_xml() {
        let xml = r#"<!-- generated -->
        <urlset><url><loc>https://example.com/</loc></url></urlset>"#;
        match parse_xml(xml) {
            ParsedSitemap::UrlSet { entries } => assert_eq!(entries.len(), 1),
            _ => panic!("expected urlset"),
        }
    }
}
