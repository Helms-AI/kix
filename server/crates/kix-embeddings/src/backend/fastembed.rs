//! FastEmbed backend implementation.
//!
//! This is the default CPU-only backend using the fastembed-rs library.

use fastembed::{EmbeddingModel, InitOptions, TextEmbedding};
use tracing::info;

use super::{AccelerationMode, BackendConfig, BackendInfo, EmbeddingBackend};
use crate::error::EmbeddingError;

/// FastEmbed-based embedding backend.
///
/// Uses the fastembed-rs library for CPU-based embedding generation.
/// This is the default backend when no GPU acceleration is available.
pub struct FastEmbedBackend {
    model: TextEmbedding,
    model_name: String,
    batch_size: usize,
    dimensions: usize,
}

impl FastEmbedBackend {
    /// Creates a new FastEmbed backend with the given configuration.
    pub fn new(config: BackendConfig) -> Result<Self, EmbeddingError> {
        let embedding_model = Self::model_from_name(&config.model_name, config.quantized)?;
        let dimensions = Self::get_model_dimensions(&embedding_model);

        // Auto-detect batch size if not specified
        let batch_size = config.batch_size.unwrap_or_else(|| {
            let cpu_cores = num_cpus::get();
            if cpu_cores >= 16 { 512 }
            else if cpu_cores >= 8 { 256 }
            else { 128 }
        });

        info!(
            "Initializing FastEmbed backend: model={}, dimensions={}, batch_size={}",
            config.model_name, dimensions, batch_size
        );

        let text_embedding = TextEmbedding::try_new(
            InitOptions::new(embedding_model.clone())
                .with_show_download_progress(config.show_download_progress),
        )
        .map_err(|e| EmbeddingError::ModelInit(e.to_string()))?;

        info!("FastEmbed backend initialized successfully");

        Ok(Self {
            model: text_embedding,
            model_name: config.model_name,
            batch_size,
            dimensions,
        })
    }

    /// Maps model name to EmbeddingModel enum.
    fn model_from_name(name: &str, quantized: bool) -> Result<EmbeddingModel, EmbeddingError> {
        let model = match name.to_lowercase().as_str() {
            "all-minilm-l6-v2" | "allminilml6v2" => {
                if quantized { EmbeddingModel::AllMiniLML6V2Q }
                else { EmbeddingModel::AllMiniLML6V2 }
            }
            "all-minilm-l12-v2" | "allminilml12v2" => {
                if quantized { EmbeddingModel::AllMiniLML12V2Q }
                else { EmbeddingModel::AllMiniLML12V2 }
            }
            "bge-base-en-v1.5" | "bgebaseenv15" => {
                if quantized { EmbeddingModel::BGEBaseENV15Q }
                else { EmbeddingModel::BGEBaseENV15 }
            }
            "bge-small-en-v1.5" | "bgesmallenv15" => {
                if quantized { EmbeddingModel::BGESmallENV15Q }
                else { EmbeddingModel::BGESmallENV15 }
            }
            "bge-large-en-v1.5" | "bgelargeenv15" => {
                if quantized { EmbeddingModel::BGELargeENV15Q }
                else { EmbeddingModel::BGELargeENV15 }
            }
            "paraphrase-multilingual-minilm-l12-v2" => {
                if quantized { EmbeddingModel::ParaphraseMLMiniLML12V2Q }
                else { EmbeddingModel::ParaphraseMLMiniLML12V2 }
            }
            "nomic-embed-text-v1" => EmbeddingModel::NomicEmbedTextV1,
            "nomic-embed-text-v1.5" => {
                if quantized { EmbeddingModel::NomicEmbedTextV15Q }
                else { EmbeddingModel::NomicEmbedTextV15 }
            }
            _ => {
                // Default to AllMiniLML6V2
                if quantized { EmbeddingModel::AllMiniLML6V2Q }
                else { EmbeddingModel::AllMiniLML6V2 }
            }
        };

        Ok(model)
    }

    /// Returns the dimensions for a given model.
    fn get_model_dimensions(model: &EmbeddingModel) -> usize {
        match model {
            EmbeddingModel::AllMiniLML6V2 | EmbeddingModel::AllMiniLML6V2Q => 384,
            EmbeddingModel::AllMiniLML12V2 | EmbeddingModel::AllMiniLML12V2Q => 384,
            EmbeddingModel::BGEBaseENV15 | EmbeddingModel::BGEBaseENV15Q => 768,
            EmbeddingModel::BGESmallENV15 | EmbeddingModel::BGESmallENV15Q => 384,
            EmbeddingModel::BGELargeENV15 | EmbeddingModel::BGELargeENV15Q => 1024,
            EmbeddingModel::ParaphraseMLMiniLML12V2 | EmbeddingModel::ParaphraseMLMiniLML12V2Q => 384,
            EmbeddingModel::ParaphraseMLMpnetBaseV2 => 768,
            EmbeddingModel::NomicEmbedTextV1 => 768,
            EmbeddingModel::NomicEmbedTextV15 | EmbeddingModel::NomicEmbedTextV15Q => 768,
            _ => 384, // Default fallback
        }
    }
}

impl EmbeddingBackend for FastEmbedBackend {
    fn info(&self) -> BackendInfo {
        BackendInfo {
            name: "FastEmbed".to_string(),
            model: self.model_name.clone(),
            acceleration: AccelerationMode::Cpu,
            dimensions: self.dimensions,
            batch_size: self.batch_size,
            quantized: self.model_name.ends_with("Q"),
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
        AccelerationMode::Cpu
    }

    fn embed_texts(&mut self, texts: &[&str]) -> Result<Vec<Vec<f32>>, EmbeddingError> {
        if texts.is_empty() {
            return Err(EmbeddingError::EmptyInput);
        }

        self.model
            .embed(texts.to_vec(), Some(self.batch_size))
            .map_err(|e| EmbeddingError::Generation(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_model_mapping() {
        let model = FastEmbedBackend::model_from_name("all-minilm-l6-v2", false).unwrap();
        assert_eq!(FastEmbedBackend::get_model_dimensions(&model), 384);

        let model = FastEmbedBackend::model_from_name("bge-base-en-v1.5", false).unwrap();
        assert_eq!(FastEmbedBackend::get_model_dimensions(&model), 768);
    }
}