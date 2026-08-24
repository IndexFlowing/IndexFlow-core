use crate::error::SitemapError;
use std::io::Read;

/// 自动嗅探 Magic Header（`0x1F, 0x8B`）并解压 Gzip 数据
pub fn decode_if_gzipped(bytes: &[u8]) -> Result<String, SitemapError> {
    if bytes.len() >= 2 && bytes[0] == 0x1F && bytes[1] == 0x8B {
        #[cfg(feature = "gzip")]
        {
            let mut decoder = flate2::read::GzDecoder::new(bytes);
            let mut decompressed = String::new();
            decoder
                .read_to_string(&mut decompressed)
                .map_err(|e| SitemapError::Decompression(e.to_string()))?;
            Ok(decompressed)
        }
        #[cfg(not(feature = "gzip"))]
        {
            Err(SitemapError::Decompression(
                "Gzip compression detected but 'gzip' feature is disabled".into(),
            ))
        }
    } else {
        // 尝试按 UTF-8 解码，若带 BOM 则剥离
        let s = std::str::from_utf8(bytes)
            .map_err(|e| SitemapError::Decompression(format!("UTF-8 decode failed: {e}")))?;
        Ok(s.trim_start_matches('\u{feff}').to_string())
    }
}