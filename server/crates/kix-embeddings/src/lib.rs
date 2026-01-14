//! Kix Embeddings - Ollama-based embedding generation for the Knowledge Indexer.
//!
//! This crate provides embedding generation using Ollama's local AI server
//! with the `nomic-embed-text` model (768 dimensions).
//!
//! Ollama automatically handles GPU acceleration:
//! - Apple Silicon: Uses Metal
//! - NVIDIA: Uses CUDA
//! - CPU fallback when no GPU is available
//!
//! ## Example
//!
//! ```ignore
//! use kix_embeddings::{OllamaEmbedder, EmbeddingConfig};
//!
//! #[tokio::main]
//! async fn main() {
//!     let embedder = OllamaEmbedder::default_config().unwrap();
//!     let embedding = embedder.embed_one("Hello, world!").await.unwrap();
//!     println!("Generated {}D embedding", embedding.len());
//! }
//! ```

pub mod chunker;
pub mod error;
pub mod ollama;
pub mod tagger;

// Primary exports: Ollama embedder
pub use ollama::{OllamaEmbedder, EmbeddingConfig};
pub use error::EmbeddingError;

// Chunking exports
pub use chunker::{
    ChunkingConfig, CodeBlockInput, DocumentChunker, EntryChunker,
    HeaderInput, SmartChunkingInput,
};

// Tagging exports
pub use tagger::{ExtractedTag, TagExtractionConfig, TagExtractor, TagSource};

/// Convenience type alias
pub type Embedder = OllamaEmbedder;
