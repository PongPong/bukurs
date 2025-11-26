/// Custom error type for bukurs library
///
/// This enum provides better type safety and error handling compared to `crate::error::BukursError`.
/// Using `thiserror` crate for automatic `Error` trait implementation and `From` conversions.
#[derive(Debug, thiserror::Error)]
pub enum BukursError {
    /// Database-related errors (SQLite)
    #[error("Database error: {0}")]
    Database(#[from] rusqlite::Error),

    /// I/O errors (file operations, network)
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// HTTP request errors
    #[error("HTTP request error: {0}")]
    Http(#[from] reqwest::Error),

    /// URL parsing errors
    #[error("Invalid URL: {0}")]
    UrlParse(String),

    /// Bookmark not found
    #[error("Bookmark with ID {0} not found")]
    BookmarkNotFound(usize),

    /// Invalid input or arguments
    #[error("Invalid input: {0}")]
    InvalidInput(String),

    /// Crypto/encryption errors
    #[error("Encryption error: {0}")]
    Crypto(String),

    /// Configuration errors
    #[error("Configuration error: {0}")]
    Config(String),

    /// Import/Export errors
    #[error("Import/Export error: {0}")]
    ImportExport(String),

    /// Browser integration errors
    #[error("Browser error: {0}")]
    Browser(String),

    /// Fuzzy search/picker errors
    #[error("Fuzzy search error: {0}")]
    FuzzySearch(String),

    /// YAML parsing/serialization errors
    #[error("YAML error: {0}")]
    Yaml(String),

    /// HTML parsing errors
    #[error("HTML parse error: {0}")]
    HtmlParse(String),

    /// JSON errors
    #[error("JSON error: {0}")]
    Json(String),

    /// Generic error for cases that don't fit other categories
    #[error("{0}")]
    Other(String),
}

/// Result type alias using BukursError
pub type Result<T> = std::result::Result<T, BukursError>;

impl From<String> for BukursError {
    fn from(s: String) -> Self {
        BukursError::Other(s)
    }
}

impl From<&str> for BukursError {
    fn from(s: &str) -> Self {
        BukursError::Other(s.to_string())
    }
}

impl From<serde_yaml::Error> for BukursError {
    fn from(err: serde_yaml::Error) -> Self {
        BukursError::Yaml(err.to_string())
    }
}

impl From<serde_json::Error> for BukursError {
    fn from(err: serde_json::Error) -> Self {
        BukursError::Json(err.to_string())
    }
}

impl From<simd_json::Error> for BukursError {
    fn from(err: simd_json::Error) -> Self {
        BukursError::Json(err.to_string())
    }
}

impl From<tl::ParseError> for BukursError {
    fn from(err: tl::ParseError) -> Self {
        BukursError::HtmlParse(err.to_string())
    }
}

// Note: nucleo_picker::PickError is private, so we can't implement From for it
// Errors from picker.pick() are handled manually in fuzzy.rs
