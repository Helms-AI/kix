//! Ollama backend implementation for embedding generation.
//!
//! This backend connects to a local or remote Ollama instance
//! for embedding generation using models like nomic-embed-text.
//!
//! Ollama automatically uses GPU acceleration when available:
//! - Apple Silicon: Uses Metal (automatic)
//! - NVIDIA: Uses CUDA (automatic)
//! - CPU fallback when no GPU is available

use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tracing::{debug, info, warn};

use super::{AccelerationMode, BackendConfig, BackendInfo, EmbeddingBackend};
use crate::error::EmbeddingError;

/// Default Ollama host URL
const DEFAULT_OLLAMA_HOST: &str = "http://localhost:11434";

/// Default model for Ollama embeddings
const DEFAULT_OLLAMA_MODEL: &str = "nomic-embed-text";

/// Connection timeout in seconds
const CONNECT_TIMEOUT_SECS: u64 = 30;

/// Request timeout in seconds (embedding generation can take a while for large batches)
const REQUEST_TIMEOUT_SECS: u64 = 300;

/// Model pull timeout in seconds (model download can take a while)
const PULL_TIMEOUT_SECS: u64 = 1800; // 30 minutes

/// Ollama embedding request
#[derive(Debug, Serialize)]
struct EmbedRequest {
    model: String,
    input: Vec<String>,
}

/// Ollama embedding response
#[derive(Debug, Deserialize)]
struct EmbedResponse {
    embeddings: Vec<Vec<f32>>,
}

/// Ollama version/health response
#[derive(Debug, Deserialize)]
struct VersionResponse {
    version: String,
}

/// Ollama model info
#[derive(Debug, Deserialize)]
struct ModelInfo {
    name: String,
}

/// Ollama models list response
#[derive(Debug, Deserialize)]
struct ModelsResponse {
    models: Vec<ModelInfo>,
}

/// Ollama pull response (streaming lines)
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct PullResponse {
    status: String,
    #[serde(default)]
    completed: Option<u64>,
    #[serde(default)]
    total: Option<u64>,
}

/// Ollama-based embedding backend.
///
/// Connects to a local or remote Ollama server for embedding generation.
/// Supports batch embedding via the `/api/embed` endpoint.
///
/// Ollama automatically detects and uses GPU acceleration:
/// - On macOS with Apple Silicon: Metal acceleration (automatic)
/// - On Linux/Windows with NVIDIA: CUDA acceleration (automatic)
/// - Falls back to CPU if no GPU is available
pub struct OllamaBackend {
    client: Client,
    host: String,
    model: String,
    batch_size: usize,
    dimensions: usize,
}

impl OllamaBackend {
    /// Creates a new Ollama backend with automatic configuration.
    ///
    /// Configuration is read from environment variables:
    /// - `OLLAMA_HOST`: Ollama server URL (default: http://localhost:11434)
    /// - `OLLAMA_MODEL`: Model name (default: nomic-embed-text)
    pub fn new(config: BackendConfig) -> Result<Self, EmbeddingError> {
        let host = std::env::var("OLLAMA_HOST")
            .unwrap_or_else(|_| DEFAULT_OLLAMA_HOST.to_string());

        // Determine model - use OLLAMA_MODEL env var, or map from config model name
        let model = std::env::var("OLLAMA_MODEL")
            .unwrap_or_else(|_| Self::map_model_name(&config.model_name));

        let dimensions = Self::get_model_dimensions(&model);
        let batch_size = config.batch_size.unwrap_or(64); // Conservative default for API

        info!(
            host = %host,
            model = %model,
            dimensions = dimensions,
            "Initializing Ollama backend"
        );

        let client = Client::builder()
            .connect_timeout(Duration::from_secs(CONNECT_TIMEOUT_SECS))
            .timeout(Duration::from_secs(REQUEST_TIMEOUT_SECS))
            .build()
            .map_err(|e| EmbeddingError::BackendInit(format!("Failed to create HTTP client: {}", e)))?;

        let backend = Self {
            client,
            host,
            model,
            batch_size,
            dimensions,
        };

        // Verify connection and model availability
        backend.verify_connection()?;

        info!("Ollama backend initialized successfully");
        Ok(backend)
    }

    /// Maps KIX model names to Ollama model names.
    ///
    /// Since Ollama uses its own model registry, we map common model names
    /// to their Ollama equivalents.
    fn map_model_name(name: &str) -> String {
        match name.to_lowercase().as_str() {
            // Nomic models (native Ollama support)
            "nomic-embed-text" | "nomic" | "nomic-embed-text-v1" | "nomic-embed-text-v1.5" => {
                "nomic-embed-text".to_string()
            }
            // MxBai models
            "mxbai-embed-large" | "mxbai" => "mxbai-embed-large".to_string(),
            // All-MiniLM (smaller, faster)
            "all-minilm" | "minilm" | "all-minilm-l6-v2" => "all-minilm".to_string(),
            // For any BGE or other models, default to nomic-embed-text
            // as it provides good quality embeddings for general use
            _ => DEFAULT_OLLAMA_MODEL.to_string(),
        }
    }

    /// Returns dimensions for known Ollama embedding models.
    fn get_model_dimensions(model: &str) -> usize {
        match model {
            "nomic-embed-text" => 768,
            "mxbai-embed-large" => 1024,
            "all-minilm" => 384,
            "snowflake-arctic-embed" => 1024,
            _ => 768, // Default fallback (nomic dimensions)
        }
    }

    /// Verifies connection to Ollama and ensures model is available.
    fn verify_connection(&self) -> Result<(), EmbeddingError> {
        // Check if Ollama is running
        let version_url = format!("{}/api/version", self.host);

        match self.client.get(&version_url).send() {
            Ok(resp) if resp.status().is_success() => {
                if let Ok(version) = resp.json::<VersionResponse>() {
                    info!(version = %version.version, "Connected to Ollama");
                }
            }
            Ok(resp) => {
                return Err(EmbeddingError::BackendInit(
                    format!("Ollama returned error status: {}", resp.status())
                ));
            }
            Err(e) => {
                return Err(EmbeddingError::BackendInit(
                    format!("Cannot connect to Ollama at {}: {}. Is Ollama running?", self.host, e)
                ));
            }
        }

        // Check if model is available, pull if needed
        if !self.is_model_available()? {
            info!(model = %self.model, "Model not found, attempting to pull");
            self.pull_model()?;
        }

        Ok(())
    }

    /// Checks if the model is available locally.
    fn is_model_available(&self) -> Result<bool, EmbeddingError> {
        let tags_url = format!("{}/api/tags", self.host);

        let response = self.client.get(&tags_url)
            .send()
            .map_err(|e| EmbeddingError::BackendInit(format!("Failed to list models: {}", e)))?;

        if !response.status().is_success() {
            return Ok(false);
        }

        let models: ModelsResponse = response.json()
            .map_err(|e| EmbeddingError::BackendInit(format!("Failed to parse models response: {}", e)))?;

        // Check if model name starts with our model (handles versioned names like "nomic-embed-text:latest")
        Ok(models.models.iter().any(|m| m.name.starts_with(&self.model)))
    }

    /// Pulls the model from Ollama registry.
    fn pull_model(&self) -> Result<(), EmbeddingError> {
        let pull_url = format!("{}/api/pull", self.host);

        info!(model = %self.model, "Pulling model (this may take several minutes on first run)");

        // Create a client with longer timeout for model download
        let pull_client = Client::builder()
            .connect_timeout(Duration::from_secs(CONNECT_TIMEOUT_SECS))
            .timeout(Duration::from_secs(PULL_TIMEOUT_SECS))
            .build()
            .map_err(|e| EmbeddingError::BackendInit(format!("Failed to create pull client: {}", e)))?;

        let response = pull_client
            .post(&pull_url)
            .json(&serde_json::json!({ "name": self.model }))
            .send()
            .map_err(|e| EmbeddingError::BackendInit(format!("Failed to pull model: {}", e)))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().unwrap_or_default();
            return Err(EmbeddingError::BackendInit(
                format!("Failed to pull model {}: {} - {}", self.model, status, body)
            ));
        }

        // Read streaming response to track progress
        let text = response.text()
            .map_err(|e| EmbeddingError::BackendInit(format!("Failed to read pull response: {}", e)))?;

        // Parse last line for final status
        for line in text.lines() {
            if let Ok(status) = serde_json::from_str::<PullResponse>(line) {
                if status.status.contains("success") || status.status.contains("pulling") {
                    debug!(status = %status.status, "Pull progress");
                }
            }
        }

        info!(model = %self.model, "Model pull complete");
        Ok(())
    }

    /// Checks if Ollama is available at the configured host.
    ///
    /// This is a quick connectivity check used by the backend selection logic
    /// to determine if Ollama should be attempted.
    pub fn is_available() -> bool {
        let host = std::env::var("OLLAMA_HOST")
            .unwrap_or_else(|_| DEFAULT_OLLAMA_HOST.to_string());

        let client = match Client::builder()
            .connect_timeout(Duration::from_secs(2))
            .timeout(Duration::from_secs(5))
            .build() {
                Ok(c) => c,
                Err(_) => return false,
            };

        let version_url = format!("{}/api/version", host);
        match client.get(&version_url).send() {
            Ok(resp) => resp.status().is_success(),
            Err(_) => false,
        }
    }
}

impl EmbeddingBackend for OllamaBackend {
    fn info(&self) -> BackendInfo {
        BackendInfo {
            name: "Ollama".to_string(),
            model: self.model.clone(),
            // Ollama handles acceleration internally (Metal/CUDA/CPU)
            // We report CPU since we don't know what Ollama is using internally
            acceleration: AccelerationMode::Cpu,
            dimensions: self.dimensions,
            batch_size: self.batch_size,
            quantized: false, // Ollama handles quantization internally
        }
    }

    fn dimensions(&self) -> usize {
        self.dimensions
    }

    fn batch_size(&self) -> usize {
        self.batch_size
    }

    fn set_batch_size(&mut self, batch_size: usize) {
        self.batch_size = batch_size;
    }

    fn acceleration_mode(&self) -> AccelerationMode {
        // Ollama abstracts acceleration - it automatically uses GPU when available
        AccelerationMode::Cpu
    }

    fn embed_texts(&mut self, texts: &[&str]) -> Result<Vec<Vec<f32>>, EmbeddingError> {
        if texts.is_empty() {
            return Err(EmbeddingError::EmptyInput);
        }

        let embed_url = format!("{}/api/embed", self.host);
        let mut all_embeddings = Vec::with_capacity(texts.len());

        // Process in batches
        for chunk in texts.chunks(self.batch_size) {
            let request = EmbedRequest {
                model: self.model.clone(),
                input: chunk.iter().map(|s| s.to_string()).collect(),
            };

            debug!(batch_size = chunk.len(), "Sending batch to Ollama");

            let response = self.client
                .post(&embed_url)
                .json(&request)
                .send()
                .map_err(|e| {
                    if e.is_timeout() {
                        EmbeddingError::Generation(format!(
                            "Ollama request timed out. Consider reducing batch size or increasing timeout. Error: {}", e
                        ))
                    } else if e.is_connect() {
                        EmbeddingError::Generation(format!(
                            "Cannot connect to Ollama at {}. Is Ollama running? Error: {}", self.host, e
                        ))
                    } else {
                        EmbeddingError::Generation(format!("Ollama request failed: {}", e))
                    }
                })?;

            if !response.status().is_success() {
                let status = response.status();
                let body = response.text().unwrap_or_default();
                return Err(EmbeddingError::Generation(
                    format!("Ollama returned error {}: {}", status, body)
                ));
            }

            let embed_response: EmbedResponse = response.json()
                .map_err(|e| EmbeddingError::Generation(format!("Failed to parse Ollama response: {}", e)))?;

            // Verify dimensions match expected
            for (i, embedding) in embed_response.embeddings.iter().enumerate() {
                if embedding.len() != self.dimensions {
                    warn!(
                        expected = self.dimensions,
                        actual = embedding.len(),
                        index = i,
                        "Embedding dimension mismatch"
                    );
                }
            }

            all_embeddings.extend(embed_response.embeddings);
        }

        debug!(
            total_embeddings = all_embeddings.len(),
            "Ollama embedding generation complete"
        );

        Ok(all_embeddings)
    }
}

// Implement Send and Sync for OllamaBackend
// reqwest::blocking::Client is Send + Sync, and all our fields are too
unsafe impl Send for OllamaBackend {}
unsafe impl Sync for OllamaBackend {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_model_dimensions() {
        assert_eq!(OllamaBackend::get_model_dimensions("nomic-embed-text"), 768);
        assert_eq!(OllamaBackend::get_model_dimensions("mxbai-embed-large"), 1024);
        assert_eq!(OllamaBackend::get_model_dimensions("all-minilm"), 384);
        assert_eq!(OllamaBackend::get_model_dimensions("unknown-model"), 768);
    }

    #[test]
    fn test_model_name_mapping() {
        assert_eq!(OllamaBackend::map_model_name("nomic-embed-text"), "nomic-embed-text");
        assert_eq!(OllamaBackend::map_model_name("nomic"), "nomic-embed-text");
        assert_eq!(OllamaBackend::map_model_name("NOMIC"), "nomic-embed-text");
        assert_eq!(OllamaBackend::map_model_name("mxbai-embed-large"), "mxbai-embed-large");
        assert_eq!(OllamaBackend::map_model_name("all-minilm"), "all-minilm");
        assert_eq!(OllamaBackend::map_model_name("bge-base-en-v1.5"), "nomic-embed-text");
    }

    #[test]
    fn test_is_available_when_not_running() {
        // When Ollama isn't running on a random port, should return false quickly
        std::env::set_var("OLLAMA_HOST", "http://localhost:59999");
        assert!(!OllamaBackend::is_available());
        std::env::remove_var("OLLAMA_HOST");
    }

    #[test]
    #[ignore] // Requires running Ollama
    fn test_embed_single() {
        let config = BackendConfig::default();
        let mut backend = OllamaBackend::new(config).unwrap();
        let embedding = backend.embed_query("test query").unwrap();
        assert_eq!(embedding.len(), 768);
    }

    #[test]
    #[ignore] // Requires running Ollama
    fn test_embed_batch() {
        let config = BackendConfig::default();
        let mut backend = OllamaBackend::new(config).unwrap();
        let texts = vec!["text one", "text two", "text three"];
        let embeddings = backend.embed_texts(&texts).unwrap();
        assert_eq!(embeddings.len(), 3);
        for emb in embeddings {
            assert_eq!(emb.len(), 768);
        }
    }
}
