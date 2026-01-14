//! Tree-sitter based AST-aware code chunking.
//!
//! This module provides tree-sitter integration for parsing source code files
//! and extracting semantic chunks based on AST structure (functions, classes, etc.).
//!
//! # Example
//!
//! ```rust,ignore
//! use kix_parser::treesitter::{TreeSitterChunker, SourceLanguage};
//!
//! let chunker = TreeSitterChunker::default();
//! let source = r#"
//! fn main() {
//!     println!("Hello, world!");
//! }
//! "#;
//!
//! let chunks = chunker.chunk_source(source, SourceLanguage::Rust, "main.rs".to_string())?;
//! for chunk in chunks {
//!     println!("Chunk: {}", chunk.title());
//! }
//! ```

mod registry;
mod symbols;
mod chunker;

pub use registry::{LanguageRegistry, SourceLanguage};
pub use symbols::{Symbol, SymbolKind, CodeChunk};
pub use chunker::{TreeSitterChunker, ChunkerConfig, ChunkerError};
