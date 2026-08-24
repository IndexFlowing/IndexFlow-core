use thiserror::Error;

#[derive(Debug, Error)]
pub enum SitemapError {
    #[error("XML parser error: {0}")]
    Xml(#[from] quick_xml::Error),

    #[error("Decompression error: {0}")]
    Decompression(String),

    #[error("Invalid URL: {0}")]
    InvalidUrl(#[from] url::ParseError),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[cfg(feature = "fetch")]
    #[error("Network error: {0}")]
    Network(#[from] reqwest::Error),

    #[error("HTTP response error with status: {0}")]
    HttpStatus(u16),

    #[error("Max recursive depth exceeded ({0})")]
    MaxDepthExceeded(u8),

    #[error("Circular reference detected for sitemap: {0}")]
    CircularReference(String),
}