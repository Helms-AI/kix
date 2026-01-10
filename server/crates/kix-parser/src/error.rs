//! Error types for the EIP parser.

use thiserror::Error;

/// Errors that can occur during parsing.
#[derive(Error, Debug)]
pub enum ParseError {
    /// The document is missing a title.
    #[error("Document is missing a title")]
    MissingTitle,

    /// The document is missing required content.
    #[error("Document is missing required content: {0}")]
    MissingContent(String),

    /// Failed to read the file.
    #[error("Failed to read file: {0}")]
    IoError(#[from] std::io::Error),

    /// Failed to extract text from PDF.
    #[error("Failed to extract PDF text: {0}")]
    PdfExtraction(String),

    /// Invalid file path.
    #[error("Invalid file path: {0}")]
    InvalidPath(String),

    /// Malformed HTML content.
    #[error("Malformed HTML: {0}")]
    MalformedHtml(String),

    /// Generic I/O error (string-based for external sources)
    #[error("I/O error: {0}")]
    Io(String),

    /// Generic parse error
    #[error("Parse error: {0}")]
    Parse(String),
}
