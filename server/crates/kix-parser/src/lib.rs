//! Knowledge Indexer Parser - Content parsing for the Knowledge Indexer.
//!
//! This crate provides parsers for various content types including HTML, PDF, DOCX,
//! Excel, CSV, Markdown, and source code files.
//!
//! # Example
//!
//! ```rust,ignore
//! use kix_parser::{HtmlParser, PdfParser, MarkdownParser, Entry};
//!
//! // Parse an HTML page
//! let html_parser = HtmlParser::new();
//! let html_content = std::fs::read_to_string("page.html")?;
//! let entry = html_parser.parse(&html_content, "https://example.com/page.html")?;
//!
//! // Parse a Markdown document
//! let md_parser = MarkdownParser::new();
//! let md_content = std::fs::read_to_string("README.md")?;
//! let entry = md_parser.parse(&md_content, "README.md")?;
//!
//! // Parse a PDF document
//! let pdf_parser = PdfParser::new();
//! let entry = pdf_parser.parse("docs/manual.pdf")?;
//! ```

pub mod csv_parser;
pub mod docx;
pub mod document;
pub mod error;
pub mod excel;
pub mod html;
pub mod markdown;
pub mod pdf;
pub mod text;

// Re-export main types (new names)
pub use document::{
    // Core types
    Entry,
    EntryChunk,
    EntryType,
    ChunkType,
    ChunkMetadata,
    SourceType,

    // Collection types
    Collection,
    CollectionSource,

    // Relationship types
    EntryRelationship,
    RelationshipType,
    RelationshipSource,

    // Tag types
    ExtractedTag,
    TagSource,

    // Backward compatibility aliases
    Document,
    DocumentChunk,
    PatternType,
};

pub use error::ParseError;

// Re-export parsers
pub use csv_parser::CsvParser;
pub use docx::DocxParser;
pub use excel::ExcelParser;
pub use html::HtmlParser;
pub use markdown::MarkdownParser;
pub use pdf::PdfParser;
pub use text::TextParser;
