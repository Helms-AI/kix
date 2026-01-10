//! Backend trait definitions for embedding generation.

use crate::error::EmbeddingError;
use super::AccelerationMode;

/// Configuration for embedding backends.
#[derive(Debug, Clone)]
pub struct BackendConfig {
    /// Model name or path
    pub model_name: String,
    /// Custom batch size (overrides auto-detection if set)
    pub batch_size: Option<usize>,
    /// Whether to use quantized model (INT8)
    pub quantized: bool,
    /// Whether to show download progress
    pub show_download_progress: bool,
}

impl Default for BackendConfig {
    fn default() -> Self {
        // Allow model selection via environment variable
        // KIX_EMBEDDING_MODEL=bge-base (default), minilm, bge-small, bge-large
        let model_name = std::env::var("KIX_EMBEDDING_MODEL")
            .unwrap_or_else(|_| "bge-base-en-v1.5".to_string());

        // Allow quantization via environment variable
        let quantized = std::env::var("KIX_EMBEDDING_QUANTIZED")
            .map(|v| v == "1" || v.to_lowercase() == "true")
            .unwrap_or(false);

        Self {
            model_name,
            batch_size: None,
            quantized,
            show_download_progress: true,
        }
    }
}

impl BackendConfig {
    /// Creates a new config with the specified model.
    pub fn with_model(model_name: impl Into<String>) -> Self {
        Self {
            model_name: model_name.into(),
            ..Default::default()
        }
    }

    /// Sets the batch size.
    pub fn batch_size(mut self, size: usize) -> Self {
        self.batch_size = Some(size);
        self
    }

    /// Enables quantization (INT8).
    pub fn quantized(mut self, enabled: bool) -> Self {
        self.quantized = enabled;
        self
    }

    /// Sets whether to show download progress.
    pub fn show_progress(mut self, show: bool) -> Self {
        self.show_download_progress = show;
        self
    }
}

/// Information about an embedding backend.
#[derive(Debug, Clone)]
pub struct BackendInfo {
    /// Backend name (e.g., "FastEmbed", "ONNX Runtime")
    pub name: String,
    /// Model being used
    pub model: String,
    /// Acceleration mode
    pub acceleration: AccelerationMode,
    /// Embedding dimensions
    pub dimensions: usize,
    /// Batch size being used
    pub batch_size: usize,
    /// Whether quantization is enabled
    pub quantized: bool,
}

/// Trait for embedding backends.
///
/// All embedding backends must implement this trait to be used interchangeably.
pub trait EmbeddingBackend: Send + Sync {
    /// Returns information about this backend.
    fn info(&self) -> BackendInfo;

    /// Returns the embedding dimensions.
    fn dimensions(&self) -> usize;

    /// Returns the current batch size.
    fn batch_size(&self) -> usize;

    /// Sets the batch size for embedding generation.
    fn set_batch_size(&mut self, batch_size: usize);

    /// Returns the acceleration mode.
    fn acceleration_mode(&self) -> AccelerationMode;

    /// Generates embeddings for a list of text strings.
    fn embed_texts(&mut self, texts: &[&str]) -> Result<Vec<Vec<f32>>, EmbeddingError>;

    /// Generates an embedding for a single query string.
    fn embed_query(&mut self, query: &str) -> Result<Vec<f32>, EmbeddingError> {
        if query.is_empty() {
            return Err(EmbeddingError::EmptyInput);
        }

        let embeddings = self.embed_texts(&[query])?;
        embeddings
            .into_iter()
            .next()
            .ok_or_else(|| EmbeddingError::Generation("No embedding generated".to_string()))
    }
}