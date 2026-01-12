//! Knowledge Indexer Parser - Content parsing for the Knowledge Indexer.
//!
//! This crate provides parsers for various content types including PDF, DOCX,
//! Excel, CSV, Markdown, and source code files.
//!
//! # HTML Content Extraction
//!
//! For HTML content extraction, use `kix_crawler::ContentExtractor` which provides:
//! - Mozilla Readability algorithm for main content detection
//! - Boilerplate removal (navigation, footer, sidebar, ads)
//! - Code block preservation
//! - Markdown conversion
//!
//! # Example
//!
//! ```rust,ignore
//! use kix_parser::{PdfParser, MarkdownParser, Entry};
//! use kix_crawler::ContentExtractor;
//!
//! // Parse an HTML page using ContentExtractor
//! let extractor = ContentExtractor::default();
//! let html_content = std::fs::read_to_string("page.html")?;
//! let url = url::Url::parse("https://example.com/page.html")?;
//! let extracted = extractor.extract(&html_content, &url);
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

pub mod chunker;
pub mod csv_parser;
pub mod docx;
pub mod document;
pub mod error;
pub mod excel;
pub mod markdown;
pub mod pdf;
pub mod text;
pub mod validator;

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
// NOTE: For HTML content extraction, use kix_crawler::ContentExtractor
// which provides Mozilla Readability algorithm, boilerplate removal, and markdown conversion.
pub use markdown::MarkdownParser;
pub use pdf::PdfParser;
pub use text::TextParser;

// Re-export validator (multi-stage code validation)
pub use validator::{CodeValidator, ValidationResult, ValidatorConfig};

// Re-export chunker (smart chunking with boundary preservation)
pub use chunker::{
    ChunkerConfig, SmartChunk, SmartChunkType, SmartChunker,
    chunk_text, chunk_text_with_size, chunk_for_embeddings,
};
