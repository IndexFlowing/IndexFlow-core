//! Transparent encoding sniff + gzip inflate with a hard uncompressed cap.
//!
//! The sitemap protocol allows `.xml.gz`. A hostile 1 KiB gzip can expand to
//! gigabytes; every inflate path therefore reads through a `Take` limiter and
//! fails with [`SitemapError::DecompressionBomb`] rather than growing the heap
//! without bound.

use crate::error::SitemapError;
use crate::models::MAX_UNCOMPRESSED_BYTES;
use std::io::{Read, Take};

/// Sniff gzip (`1F 8B`) or a Unicode BOM and return a UTF-8 string.
///
/// The uncompressed (or raw) payload is capped at [`MAX_UNCOMPRESSED_BYTES`].
pub fn decode_if_gzipped(bytes: &[u8]) -> Result<String, SitemapError> {
    decode_if_gzipped_with_limit(bytes, MAX_UNCOMPRESSED_BYTES)
}

/// Variant of [`decode_if_gzipped`] with an explicit uncompressed-size cap.
///
/// Useful for tests (tiny caps) and for callers that want a tighter budget
/// than the Google 50 MiB default.
pub fn decode_if_gzipped_with_limit(bytes: &[u8], max_bytes: usize) -> Result<String, SitemapError> {
    if bytes.len() >= 2 && bytes[0] == 0x1F && bytes[1] == 0x8B {
        #[cfg(feature = "gzip")]
        {
            let decoder = flate2::read::GzDecoder::new(bytes);
            let inflated = read_limited(decoder, max_bytes)?;
            return decode_text(&inflated);
        }
        #[cfg(not(feature = "gzip"))]
        {
            let _ = max_bytes;
            return Err(SitemapError::Decompression(
                "Gzip compression detected but 'gzip' feature is disabled".into(),
            ));
        }
    }

    if bytes.len() > max_bytes {
        return Err(SitemapError::DecompressionBomb { limit: max_bytes });
    }
    decode_text(bytes)
}

/// Read `reader` until EOF or `max + 1` bytes; error if the cap is crossed.
fn read_limited<R: Read>(reader: R, max: usize) -> Result<Vec<u8>, SitemapError> {
    let mut limited: Take<R> = reader.take(max as u64 + 1);
    let mut buf = Vec::new();
    limited
        .read_to_end(&mut buf)
        .map_err(|e| SitemapError::Decompression(e.to_string()))?;
    if buf.len() > max {
        return Err(SitemapError::DecompressionBomb { limit: max });
    }
    Ok(buf)
}

/// Decode a raw sitemap document: UTF-8 (BOM optional), UTF-16 LE/BE, then lossy UTF-8.
fn decode_text(bytes: &[u8]) -> Result<String, SitemapError> {
    if bytes.is_empty() {
        return Ok(String::new());
    }

    // UTF-8 BOM.
    if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
        return decode_utf8(&bytes[3..]);
    }
    // UTF-16 BE BOM.
    if bytes.starts_with(&[0xFE, 0xFF]) {
        return Ok(decode_utf16(&bytes[2..], false));
    }
    // UTF-16 LE BOM.
    if bytes.starts_with(&[0xFF, 0xFE]) {
        return Ok(decode_utf16(&bytes[2..], true));
    }

    decode_utf8(bytes)
}

fn decode_utf8(bytes: &[u8]) -> Result<String, SitemapError> {
    match std::str::from_utf8(bytes) {
        Ok(s) => Ok(s.trim_start_matches('\u{feff}').to_string()),
        Err(_) => {
            // Real-world sitemaps occasionally mix a few illegal bytes into an
            // otherwise UTF-8 document. Lossy decode keeps the crawl alive.
            let s = String::from_utf8_lossy(bytes);
            Ok(s.trim_start_matches('\u{feff}').to_string())
        }
    }
}

fn decode_utf16(bytes: &[u8], le: bool) -> String {
    let units: Vec<u16> = bytes
        .chunks_exact(2)
        .map(|c| {
            if le {
                u16::from_le_bytes([c[0], c[1]])
            } else {
                u16::from_be_bytes([c[0], c[1]])
            }
        })
        .collect();
    let s = String::from_utf16_lossy(&units);
    s.trim_start_matches('\u{feff}').to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn utf8_passthrough_strips_bom() {
        let mut raw = vec![0xEF, 0xBB, 0xBF];
        raw.extend_from_slice(b"<urlset></urlset>");
        let s = decode_if_gzipped(&raw).expect("utf8");
        assert_eq!(s, "<urlset></urlset>");
    }

    #[test]
    fn utf16_le_roundtrip() {
        let text = "<?xml version=\"1.0\"?><urlset></urlset>";
        let mut bytes = vec![0xFF, 0xFE];
        for u in text.encode_utf16() {
            bytes.extend_from_slice(&u.to_le_bytes());
        }
        let s = decode_if_gzipped(&bytes).expect("utf16le");
        assert!(s.contains("<urlset>"));
    }

    #[test]
    fn utf16_be_roundtrip() {
        let text = "<urlset></urlset>";
        let mut bytes = vec![0xFE, 0xFF];
        for u in text.encode_utf16() {
            bytes.extend_from_slice(&u.to_be_bytes());
        }
        let s = decode_if_gzipped(&bytes).expect("utf16be");
        assert_eq!(s, "<urlset></urlset>");
    }

    #[test]
    fn invalid_utf8_is_lossy_not_fatal() {
        let s = decode_if_gzipped(b"hello\xFFworld").expect("lossy");
        assert!(s.starts_with("hello"));
        assert!(s.ends_with("world"));
    }

    #[test]
    fn raw_payload_cap() {
        let err = decode_if_gzipped_with_limit(&[b'x'; 16], 8).unwrap_err();
        assert!(matches!(err, SitemapError::DecompressionBomb { limit: 8 }));
    }

    #[cfg(feature = "gzip")]
    #[test]
    fn gzip_roundtrip() {
        use flate2::write::GzEncoder;
        use flate2::Compression;
        use std::io::Write;

        let payload = b"<?xml version=\"1.0\"?><urlset></urlset>";
        let mut enc = GzEncoder::new(Vec::new(), Compression::default());
        enc.write_all(payload).unwrap();
        let gz = enc.finish().unwrap();
        assert_eq!(&gz[..2], &[0x1F, 0x8B]);
        let s = decode_if_gzipped(&gz).expect("inflate");
        assert!(s.contains("<urlset>"));
    }

    #[cfg(feature = "gzip")]
    #[test]
    fn gzip_bomb_rejected() {
        use flate2::write::GzEncoder;
        use flate2::Compression;
        use std::io::Write;

        // Highly compressible zeros: tiny on the wire, huge inflated.
        let mut enc = GzEncoder::new(Vec::new(), Compression::best());
        enc.write_all(&[0u8; 64 * 1024]).unwrap();
        let gz = enc.finish().unwrap();
        assert!(gz.len() < 1024, "fixture should be a small compressed blob");
        let err = decode_if_gzipped_with_limit(&gz, 1024).unwrap_err();
        assert!(matches!(err, SitemapError::DecompressionBomb { limit: 1024 }));
    }
}
