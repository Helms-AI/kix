//! Ollama-based embedding generation.
//!
//! This module provides embedding generation using Ollama's local AI server.
//! Uses the `nomic-embed-text` model by default, which provides high-quality
//! 768-dimensional embeddings.
//!
//! Ollama automatically handles GPU acceleration:
//! - Apple Silicon: Uses Metal (automatic)
//! - NVIDIA: Uses CUDA (automatic)
//! - CPU fallback when no GPU is available

use ollama_rs::Ollama;
use ollama_rs::generation::embeddings::request::GenerateEmbeddingsRequest;
use std::sync::Arc;
use tokio::sync::Semaphore;
use tracing::{debug, info};

use crate::error::EmbeddingError;

/// Default Ollama server URL
const DEFAULT_OLLAMA_URL: &str = "http://localhost:11434";

/// Default model for embeddings
const DEFAULT_MODEL: &str = "nomic-embed-text";

/// Default embedding dimensions for nomic-embed-text
const DEFAULT_DIMENSIONS: usize = 768;

/// Embedding configuration
#[derive(Debug, Clone)]
pub struct EmbeddingConfig {
    /// Ollama server URL
    pub ollama_url: String,

    /// Model name (default: nomic-embed-text)
    pub model: String,

    /// Expected embedding dimensions
    pub dimensions: usize,

    /// Maximum concurrent embedding requests
    pub max_concurrent: usize,

    /// Batch size for embedding requests
    pub batch_size: usize,
}

impl Default for EmbeddingConfig {
    fn default() -> Self {
        Self {
            ollama_url: std::env::var("OLLAMA_URL")
                .unwrap_or_else(|_| DEFAULT_OLLAMA_URL.to_string()),
            model: std::env::var("EMBEDDING_MODEL")
                .unwrap_or_else(|_| DEFAULT_MODEL.to_string()),
            dimensions: DEFAULT_DIMENSIONS,
            max_concurrent: std::env::var("EMBEDDING_CONCURRENCY")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(4),
            batch_size: 32,
        }
    }
}

impl EmbeddingConfig {
    /// Create config with custom URL
    pub fn with_url(mut self, url: &str) -> Self {
        self.ollama_url = url.to_string();
        self
    }

    /// Create config with custom model
    pub fn with_model(mut self, model: &str) -> Self {
        self.model = model.to_string();
        // Update dimensions based on model
        self.dimensions = model_dimensions(&self.model);
        self
    }

    /// Create config with custom batch size
    pub fn with_batch_size(mut self, size: usize) -> Self {
        self.batch_size = size;
        self
    }

    /// Create config with custom concurrency
    pub fn with_concurrency(mut self, concurrency: usize) -> Self {
        self.max_concurrent = concurrency;
        self
    }
}

/// Returns dimensions for known Ollama embedding models.
fn model_dimensions(model: &str) -> usize {
    match model {
        "nomic-embed-text" => 768,
        "mxbai-embed-large" => 1024,
        "all-minilm" => 384,
        "snowflake-arctic-embed" => 1024,
        _ => DEFAULT_DIMENSIONS,
    }
}

/// Ollama-based embedder.
///
/// Provides embedding generation using Ollama's local AI server.
/// Supports concurrent requests via semaphore and batch processing.
///
/// ## Example
///
/// ```ignore
/// use kix_embeddings::{OllamaEmbedder, EmbeddingConfig};
///
/// #[tokio::main]
/// async fn main() {
///     let embedder = OllamaEmbedder::new(EmbeddingConfig::default()).unwrap();
///     let embedding = embedder.embed_one("Hello, world!").await.unwrap();
///     assert_eq!(embedding.len(), 768);
/// }
/// ```
pub struct OllamaEmbedder {
    client: Ollama,
    config: EmbeddingConfig,
    semaphore: Arc<Semaphore>,
}

impl OllamaEmbedder {
    /// Create a new embedder with configuration
    pub fn new(config: EmbeddingConfig) -> Result<Self, EmbeddingError> {
        info!(
            url = %config.ollama_url,
            model = %config.model,
            dimensions = config.dimensions,
            "Initializing Ollama embedder"
        );

        // Parse URL to extract host and port
        let url = url::Url::parse(&config.ollama_url)
            .map_err(|e| EmbeddingError::ConnectionError(format!("Invalid URL: {}", e)))?;

        let host = url.host_str()
            .ok_or_else(|| EmbeddingError::ConnectionError("Missing host in URL".to_string()))?;
        let port = url.port().unwrap_or(11434);

        let client = Ollama::new(format!("http://{}", host), port);

        Ok(Self {
            client,
            semaphore: Arc::new(Semaphore::new(config.max_concurrent)),
            config,
        })
    }

    /// Create with default configuration
    pub fn default_config() -> Result<Self, EmbeddingError> {
        Self::new(EmbeddingConfig::default())
    }

    /// Get the embedding dimensions
    pub fn dimensions(&self) -> usize {
        self.config.dimensions
    }

    /// Get the model name
    pub fn model(&self) -> &str {
        &self.config.model
    }

    /// Get the batch size
    pub fn batch_size(&self) -> usize {
        self.config.batch_size
    }

    /// Generate embedding for a single text
    pub async fn embed_one(&self, text: &str) -> Result<Vec<f32>, EmbeddingError> {
        let _permit = self.semaphore.acquire().await
            .map_err(|_| EmbeddingError::SemaphoreError)?;

        debug!(text_len = text.len(), "Generating single embedding");

        let request = GenerateEmbeddingsRequest::new(
            self.config.model.clone(),
            text.into(),
        );

        let response = self.client.generate_embeddings(request)
            .await
            .map_err(|e| EmbeddingError::OllamaError(e.to_string()))?;

        let embedding = response.embeddings
            .into_iter()
            .next()
            .ok_or(EmbeddingError::EmptyResponse)?;

        // Validate dimensions
        if embedding.len() != self.config.dimensions {
            return Err(EmbeddingError::DimensionMismatch {
                expected: self.config.dimensions,
                actual: embedding.len(),
            });
        }

        Ok(embedding)
    }

    /// Generate embeddings for multiple texts
    pub async fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, EmbeddingError> {
        if texts.is_empty() {
            return Err(EmbeddingError::EmptyInput);
        }

        debug!(count = texts.len(), "Generating batch embeddings");

        let mut results = Vec::with_capacity(texts.len());

        // Process in batches
        for chunk in texts.chunks(self.config.batch_size) {
            let batch_results = self.embed_batch_internal(chunk).await?;
            results.extend(batch_results);
        }

        Ok(results)
    }

    /// Internal batch processing with concurrency control
    async fn embed_batch_internal(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, EmbeddingError> {
        let futures: Vec<_> = texts
            .iter()
            .map(|text| self.embed_one(text))
            .collect();

        let results = futures::future::join_all(futures).await;
        results.into_iter().collect()
    }

    /// Generate embeddings for text slices (convenience method)
    pub async fn embed_texts(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>, EmbeddingError> {
        let owned: Vec<String> = texts.iter().map(|s| s.to_string()).collect();
        self.embed_batch(&owned).await
    }

    /// Check if Ollama server is reachable and model is available
    pub async fn health_check(&self) -> Result<(), EmbeddingError> {
        debug!("Performing health check");

        // Try to generate a simple embedding
        self.embed_one("health check").await?;

        info!("Ollama health check passed");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = EmbeddingConfig::default();
        assert_eq!(config.model, "nomic-embed-text");
        assert_eq!(config.dimensions, 768);
        assert_eq!(config.max_concurrent, 4);
    }

    #[test]
    fn test_config_builder() {
        let config = EmbeddingConfig::default()
            .with_url("http://custom:11434")
            .with_model("mxbai-embed-large")
            .with_batch_size(64)
            .with_concurrency(8);

        assert_eq!(config.ollama_url, "http://custom:11434");
        assert_eq!(config.model, "mxbai-embed-large");
        assert_eq!(config.dimensions, 1024);
        assert_eq!(config.batch_size, 64);
        assert_eq!(config.max_concurrent, 8);
    }

    #[test]
    fn test_model_dimensions() {
        assert_eq!(model_dimensions("nomic-embed-text"), 768);
        assert_eq!(model_dimensions("mxbai-embed-large"), 1024);
        assert_eq!(model_dimensions("all-minilm"), 384);
        assert_eq!(model_dimensions("unknown-model"), 768);
    }

    #[tokio::test]
    #[ignore] // Requires running Ollama
    async fn test_single_embedding() {
        let embedder = OllamaEmbedder::default_config().unwrap();
        let embedding = embedder.embed_one("Hello, world!").await.unwrap();
        assert_eq!(embedding.len(), 768);
    }

    #[tokio::test]
    #[ignore] // Requires running Ollama
    async fn test_batch_embedding() {
        let embedder = OllamaEmbedder::default_config().unwrap();
        let texts = vec![
            "First document".to_string(),
            "Second document".to_string(),
            "Third document".to_string(),
        ];
        let embeddings = embedder.embed_batch(&texts).await.unwrap();
        assert_eq!(embeddings.len(), 3);
        for emb in embeddings {
            assert_eq!(emb.len(), 768);
        }
    }

    #[tokio::test]
    #[ignore] // Requires running Ollama
    async fn test_health_check() {
        let embedder = OllamaEmbedder::default_config().unwrap();
        embedder.health_check().await.unwrap();
    }

    #[tokio::test]
    async fn test_empty_batch_error() {
        let embedder = OllamaEmbedder::default_config().unwrap();
        let result = embedder.embed_batch(&[]).await;
        assert!(matches!(result, Err(EmbeddingError::EmptyInput)));
    }
}
