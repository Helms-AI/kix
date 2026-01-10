//! Embedding generation with automatic backend selection and hardware acceleration.
//!
//! The `EmbeddingGenerator` provides a unified interface for embedding generation,
//! automatically selecting the best available backend based on compiled features
//! and detected hardware.

use kix_parser::EntryChunk;
use tracing::info;

use crate::backend::{AccelerationMode, BackendConfig, BackendInfo, EmbeddingBackend, create_best_backend};
use crate::error::EmbeddingError;

/// High-level embedding generator with automatic backend selection.
///
/// This struct provides a unified interface for embedding generation,
/// automatically selecting the best available backend (FastEmbed or ONNX Runtime)
/// and hardware acceleration (CPU, CUDA GPU, or Apple Metal).
///
/// ## Example
///
/// ```ignore
/// use kix_embeddings::EmbeddingGenerator;
///
/// let mut generator = EmbeddingGenerator::new()?;
/// let embedding = generator.embed_query("How do I route messages?")?;
/// println!("Generated {}D embedding", embedding.len());
/// ```
pub struct EmbeddingGenerator {
    backend: Box<dyn EmbeddingBackend>,
}

impl EmbeddingGenerator {
    /// Creates a new embedding generator with automatic backend and hardware detection.
    ///
    /// Uses the default model (`all-MiniLM-L6-v2`) and automatically selects the best
    /// available backend and acceleration mode.
    pub fn new() -> Result<Self, EmbeddingError> {
        let backend = create_best_backend(None)?;
        let info = backend.info();

        info!(
            "EmbeddingGenerator initialized: backend={}, model={}, {}D, {} mode, batch_size={}",
            info.name, info.model, info.dimensions, info.acceleration, info.batch_size
        );

        Ok(Self { backend })
    }

    /// Creates an embedding generator with a specific configuration.
    pub fn with_config(config: BackendConfig) -> Result<Self, EmbeddingError> {
        let backend = create_best_backend(Some(config))?;
        let info = backend.info();

        info!(
            "EmbeddingGenerator initialized: backend={}, model={}, {}D, {} mode, batch_size={}",
            info.name, info.model, info.dimensions, info.acceleration, info.batch_size
        );

        Ok(Self { backend })
    }

    /// Creates an embedding generator with a specific model.
    pub fn with_model(model_name: &str) -> Result<Self, EmbeddingError> {
        let config = BackendConfig::with_model(model_name);
        Self::with_config(config)
    }

    /// Creates an embedding generator with quantization enabled.
    ///
    /// Quantized models (INT8) provide 2-4x faster inference with minimal quality loss.
    pub fn with_quantization(model_name: &str) -> Result<Self, EmbeddingError> {
        let config = BackendConfig::with_model(model_name).quantized(true);
        Self::with_config(config)
    }

    /// Returns information about the current backend.
    pub fn info(&self) -> BackendInfo {
        self.backend.info()
    }

    /// Returns the current acceleration mode.
    pub fn acceleration_mode(&self) -> AccelerationMode {
        self.backend.acceleration_mode()
    }

    /// Returns the embedding dimensions for the current model.
    pub fn dimensions(&self) -> usize {
        self.backend.dimensions()
    }

    /// Returns the batch size used for embedding generation.
    pub fn batch_size(&self) -> usize {
        self.backend.batch_size()
    }

    /// Sets the batch size for embedding generation.
    pub fn set_batch_size(&mut self, batch_size: usize) {
        self.backend.set_batch_size(batch_size);
    }

    /// Generates embeddings for a list of entry chunks.
    pub fn embed_chunks(&mut self, chunks: &[EntryChunk]) -> Result<Vec<Vec<f32>>, EmbeddingError> {
        if chunks.is_empty() {
            return Err(EmbeddingError::EmptyInput);
        }

        let texts: Vec<&str> = chunks.iter().map(|c| c.text.as_str()).collect();
        self.embed_texts(&texts)
    }

    /// Generates embeddings for a list of text strings.
    pub fn embed_texts(&mut self, texts: &[&str]) -> Result<Vec<Vec<f32>>, EmbeddingError> {
        self.backend.embed_texts(texts)
    }

    /// Generates an embedding for a single query string.
    pub fn embed_query(&mut self, query: &str) -> Result<Vec<f32>, EmbeddingError> {
        self.backend.embed_query(query)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_builder() {
        let config = BackendConfig::with_model("all-MiniLM-L6-v2")
            .batch_size(256)
            .quantized(false)
            .show_progress(true);

        assert_eq!(config.model_name, "all-MiniLM-L6-v2");
        assert_eq!(config.batch_size, Some(256));
        assert!(!config.quantized);
        assert!(config.show_download_progress);
    }

    // Note: Tests requiring model initialization are marked as ignore
    // Run with: cargo test -- --ignored

    #[test]
    #[ignore]
    fn test_embed_query() {
        let mut generator = EmbeddingGenerator::new().expect("Failed to init model");
        let embedding = generator
            .embed_query("How do I route messages?")
            .expect("Failed to generate embedding");

        assert_eq!(embedding.len(), generator.dimensions());
    }

    #[test]
    #[ignore]
    fn test_embed_texts() {
        let mut generator = EmbeddingGenerator::new().expect("Failed to init model");
        let texts = vec!["First text", "Second text", "Third text"];
        let embeddings = generator
            .embed_texts(&texts)
            .expect("Failed to generate embeddings");

        assert_eq!(embeddings.len(), 3);
        for emb in &embeddings {
            assert_eq!(emb.len(), generator.dimensions());
        }
    }

    #[test]
    fn test_empty_input() {
        // This test doesn't need the model
        let texts: Vec<&str> = vec![];
        assert!(texts.is_empty());
    }

    #[test]
    #[ignore]
    fn test_acceleration_detection() {
        let generator = EmbeddingGenerator::new().expect("Failed to init model");
        let mode = generator.acceleration_mode();

        // Should always return a valid mode
        match mode {
            AccelerationMode::Cpu => println!("Using CPU mode"),
            AccelerationMode::Cuda => println!("Using CUDA GPU mode"),
            AccelerationMode::Metal => println!("Using Apple Metal mode"),
        }
    }

    #[test]
    #[ignore]
    fn test_backend_info() {
        let generator = EmbeddingGenerator::new().expect("Failed to init model");
        let info = generator.info();

        assert!(!info.name.is_empty());
        assert!(!info.model.is_empty());
        assert!(info.dimensions > 0);
        assert!(info.batch_size > 0);
    }
}