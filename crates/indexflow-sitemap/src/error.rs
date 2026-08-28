use thiserror::Error;

/// Errors produced by sitemap download, decompression, or (strict) expansion.
///
/// Streaming XML parse itself is **fault-tolerant** and does not surface as an
/// error: malformed documents yield a partial [`crate::ParsedSitemap`] instead.
#[derive(Debug, Error)]
pub enum SitemapError {
    /// Underlying `quick-xml` tokenizer failure (reserved for strict parsers).
    #[error("XML parser error: {0}")]
    Xml(#[from] quick_xml::Error),

    /// Gzip inflate failed (truncated stream, CRC mismatch, …).
    #[error("Decompression error: {0}")]
    Decompression(String),

    /// Inflated payload exceeded the configured uncompressed-size cap.
    ///
    /// This is the primary defence against gzip/deflate bombs.
    #[error("decompression bomb: inflated size exceeds {limit} bytes")]
    DecompressionBomb { limit: usize },

    /// HTTP response body (or declared `Content-Length`) exceeded the download cap.
    #[error("payload too large: {size} bytes exceeds limit {limit}")]
    PayloadTooLarge { size: u64, limit: u64 },

    /// Byte stream could not be decoded as UTF-8 / UTF-16.
    #[error("encoding error: {0}")]
    Encoding(String),

    /// A URL failed to parse (kept for callers that validate locs strictly).
    #[error("Invalid URL: {0}")]
    InvalidUrl(#[from] url::ParseError),

    /// Local I/O failure while reading decompressed bytes.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// HTTP transport error from `reqwest`.
    #[cfg(feature = "fetch")]
    #[error("Network error: {0}")]
    Network(#[from] reqwest::Error),

    /// Non-success HTTP status from the sitemap origin.
    #[error("HTTP response error with status: {0}")]
    HttpStatus(u16),

    /// Recursive expansion exceeded the caller-supplied depth budget.
    ///
    /// `expand_all` isolates this per-child (it skips instead of failing the
    /// whole tree). Exposed so strict callers can treat it as fatal.
    #[error("Max recursive depth exceeded ({0})")]
    MaxDepthExceeded(u8),

    /// A sitemap index loc pointed at an already-visited ancestor (cycle).
    ///
    /// `expand_all` isolates this per-child. Exposed so strict callers can
    /// treat it as fatal.
    #[error("Circular reference detected for sitemap: {0}")]
    CircularReference(String),
}
