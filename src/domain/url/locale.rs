use url::Url as ParsedUrl;

const LANG_CODES: &[&str] = &[
    "aa", "ab", "ae", "af", "ak", "am", "an", "ar", "as", "av", "ay", "az", "ba", "be", "bg", "bh",
    "bi", "bm", "bn", "bo", "br", "bs", "ca", "ce", "ch", "co", "cr", "cs", "cu", "cv", "cy", "da",
    "de", "dv", "dz", "ee", "el", "en", "eo", "es", "et", "eu", "fa", "ff", "fi", "fj", "fo", "fr",
    "fy", "ga", "gd", "gl", "gn", "gu", "gv", "ha", "he", "hi", "ho", "hr", "ht", "hu", "hy", "hz",
    "ia", "id", "ie", "ig", "ii", "ik", "io", "is", "it", "iu", "ja", "jv", "ka", "kg", "ki", "kj",
    "kk", "kl", "km", "kn", "ko", "kr", "ks", "ku", "kv", "kw", "ky", "la", "lb", "lg", "li", "ln",
    "lo", "lt", "lu", "lv", "mg", "mh", "mi", "mk", "ml", "mn", "mr", "ms", "mt", "my", "na", "nb",
    "nd", "ne", "ng", "nl", "nn", "no", "nr", "nv", "ny", "oc", "oj", "om", "or", "os", "pa", "pi",
    "pl", "ps", "pt", "qu", "rm", "rn", "ro", "ru", "rw", "sa", "sc", "sd", "se", "sg", "si", "sk",
    "sl", "sm", "sn", "so", "sq", "sr", "ss", "st", "su", "sv", "sw", "ta", "te", "tg", "th", "ti",
    "tk", "tl", "tn", "to", "tr", "ts", "tt", "tw", "ty", "ug", "uk", "ur", "uz", "ve", "vi", "vo",
    "wa", "wo", "xh", "yi", "yo", "za", "zh", "zu",
];

fn is_locale_segment(seg: &str) -> bool {
    let lower = seg.to_ascii_lowercase();
    let mut parts = lower.split('-');
    let Some(primary) = parts.next() else {
        return false;
    };
    if primary.len() != 2 || !LANG_CODES.contains(&primary) {
        return false;
    }
    match parts.next() {
        None => true,
        Some(rest) => {
            let ok_len =
                (2..=8).contains(&rest.len()) && rest.chars().all(|c| c.is_ascii_alphanumeric());
            ok_len && parts.next().is_none()
        }
    }
}

pub fn extract_locale_and_path_prefix(page_url: &str, hreflang: Option<&str>) -> (String, String) {
    let locale_from_hreflang = hreflang.map(str::trim).filter(|s| !s.is_empty()).map(|s| {
        if s.eq_ignore_ascii_case("x-default") {
            "default".to_string()
        } else {
            s.to_ascii_lowercase()
        }
    });

    let path = match ParsedUrl::parse(page_url) {
        Ok(u) => u.path().to_string(),
        Err(_) => path_from_raw(page_url),
    };

    let segments: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
    let (locale_from_path, rest): (Option<String>, &[&str]) = match segments.first() {
        Some(first) if is_locale_segment(first) => {
            (Some(first.to_ascii_lowercase()), &segments[1..])
        }
        _ => (None, segments.as_slice()),
    };

    let locale = locale_from_hreflang
        .or(locale_from_path)
        .unwrap_or_else(|| "default".to_string());

    let path_prefix = match rest.first() {
        Some(dir) => format!("/{dir}"),
        None => "/".to_string(),
    };

    (locale, path_prefix)
}

fn path_from_raw(page_url: &str) -> String {
    let rest = page_url
        .split_once("://")
        .map(|(_, r)| r)
        .unwrap_or(page_url);
    let path = rest.find('/').map(|i| &rest[i..]).unwrap_or("/");
    path.split(['?', '#']).next().unwrap_or("/").to_string()
}