//! Kix Embeddings - Embedding generation for the Knowledge Indexer.
//!
//! This crate provides embedding generation with multiple backend support:
//! - **FastEmbed** (default): CPU-based, no external dependencies
//! - **ONNX Runtime**: High-performance with GPU support (CUDA/Metal)
//!
//! Also includes document chunking strategies and auto-tagging via keyword extraction.
//!
//! ## Feature Flags
//!
//! - `fastembed-backend` (default): Enable FastEmbed backend
//! - `onnx-backend`: Enable ONNX Runtime backend (CPU)
//! - `onnx-cuda`: Enable ONNX Runtime with CUDA GPU support
//! - `onnx-coreml`: Enable ONNX Runtime with Apple CoreML support

pub mod backend;
pub mod chunker;
pub mod embedder;
pub mod error;
pub mod tagger;

pub use backend::{
    AccelerationMode, BackendConfig, BackendInfo, BackendType, EmbeddingBackend,
    create_backend, create_best_backend, optimal_batch_size,
    ensure_setup, is_setup, model_cache_dir, SetupInfo,
};
pub use chunker::{
    ChunkingConfig, CodeBlockInput, DocumentChunker, EntryChunker,
    HeaderInput, SmartChunkingInput,
};
pub use embedder::EmbeddingGenerator;
pub use error::EmbeddingError;
pub use tagger::{ExtractedTag, TagExtractionConfig, TagExtractor, TagSource};
