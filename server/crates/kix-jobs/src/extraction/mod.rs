//! Code extraction module for HTML documents.
//!
//! This module provides framework-aware code extraction from HTML,
//! supporting 30+ documentation frameworks and syntax highlighters.
//!
//! # Features
//!
//! - **30+ patterns**: Docusaurus, MkDocs, Sphinx, Hugo, GitHub, etc.
//! - **Language detection**: Automatic detection from class/attributes
//! - **Validation**: Filter non-code content (prose, placeholders)
//! - **Deduplication**: Prevent duplicate code blocks
//!
//! # Example
//!
//! ```ignore
//! use kix_jobs::extraction::{CodeExtractor, CodeExtractionConfig};
//!
//! let extractor = CodeExtractor::with_defaults();
//! let result = extractor.extract(&html_content);
//!
//! for block in result.code_blocks {
//!     println!("Found {} code ({} lines):", block.language, block.line_count);
//!     println!("{}", block.content);
//! }
//! ```

mod code_extractor;
mod language;
mod patterns;
mod validation;

pub use code_extractor::{
    CodeExtractor,
    CodeExtractionConfig,
    CodeBlock,
    ExtractionResult,
};
pub use language::Language;
pub use patterns::CodePattern;
pub use validation::{ValidationConfig, ValidationStats, ValidationResult, validate_code};
