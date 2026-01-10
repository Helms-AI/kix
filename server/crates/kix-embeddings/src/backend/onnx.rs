//! ONNX Runtime backend implementation with GPU support and proper tokenization.
//!
//! This backend provides high-performance embedding generation using ONNX Runtime,
//! with optional GPU acceleration via CUDA (NVIDIA) or CoreML (Apple Silicon).
//! Uses the HuggingFace tokenizers library for proper text tokenization.

use ort::{execution_providers::ExecutionProviderDispatch, session::Session, value::Value};
use std::path::PathBuf;
use tokenizers::Tokenizer;
use tracing::{debug, info, warn};

use super::{AccelerationMode, BackendConfig, BackendInfo, EmbeddingBackend};
use crate::error::EmbeddingError;

/// Supported embedding models
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EmbeddingModel {
    /// all-MiniLM-L6-v2: Fast, 384 dimensions
    AllMiniLmL6V2,
    /// BGE-small-en-v1.5: Fast, 384 dimensions, better quality
    BgeSmallEnV15,
    /// BGE-base-en-v1.5: Balanced, 768 dimensions, good quality
    BgeBaseEnV15,
    /// BGE-large-en-v1.5: High quality, 1024 dimensions
    BgeLargeEnV15,
}

impl EmbeddingModel {
    /// Returns embedding dimensions for this model
    pub fn dimensions(&self) -> usize {
        match self {
            Self::AllMiniLmL6V2 => 384,
            Self::BgeSmallEnV15 => 384,
            Self::BgeBaseEnV15 => 768,
            Self::BgeLargeEnV15 => 1024,
        }
    }

    /// Returns max sequence length for this model
    pub fn max_seq_length(&self) -> usize {
        match self {
            Self::AllMiniLmL6V2 => 256,
            Self::BgeSmallEnV15 | Self::BgeBaseEnV15 | Self::BgeLargeEnV15 => 512,
        }
    }

    /// Returns HuggingFace repo ID
    pub fn repo_id(&self) -> &'static str {
        match self {
            Self::AllMiniLmL6V2 => "sentence-transformers/all-MiniLM-L6-v2",
            Self::BgeSmallEnV15 => "BAAI/bge-small-en-v1.5",
            Self::BgeBaseEnV15 => "BAAI/bge-base-en-v1.5",
            Self::BgeLargeEnV15 => "BAAI/bge-large-en-v1.5",
        }
    }

    /// Returns model name
    pub fn name(&self) -> &'static str {
        match self {
            Self::AllMiniLmL6V2 => "all-MiniLM-L6-v2",
            Self::BgeSmallEnV15 => "bge-small-en-v1.5",
            Self::BgeBaseEnV15 => "bge-base-en-v1.5",
            Self::BgeLargeEnV15 => "bge-large-en-v1.5",
        }
    }

    /// Parse model from string
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "all-minilm-l6-v2" | "allminilml6v2" | "minilm" => Self::AllMiniLmL6V2,
            "bge-small-en-v1.5" | "bgesmallenv15" | "bge-small" => Self::BgeSmallEnV15,
            "bge-base-en-v1.5" | "bgebaseenv15" | "bge-base" | "bge" => Self::BgeBaseEnV15,
            "bge-large-en-v1.5" | "bgelargeenv15" | "bge-large" => Self::BgeLargeEnV15,
            _ => Self::BgeBaseEnV15, // Default to BGE-base for best quality/performance
        }
    }
}

/// ONNX Runtime embedding backend with GPU support and proper tokenization.
pub struct OnnxBackend {
    session: Session,
    tokenizer: Tokenizer,
    model: EmbeddingModel,
    batch_size: usize,
    acceleration: AccelerationMode,
    quantized: bool,
}

impl OnnxBackend {
    /// Pre-downloads the model and tokenizer to the cache directory.
    /// Returns the cache directory path and whether download was needed.
    pub fn ensure_model_downloaded(model_name: &str) -> Result<(PathBuf, bool), EmbeddingError> {
        let model = EmbeddingModel::from_str(model_name);

        // Get cache directory
        let cache_dir = dirs::cache_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("kix")
            .join("models");
        std::fs::create_dir_all(&cache_dir)
            .map_err(|e| EmbeddingError::ModelInit(format!("Failed to create cache dir: {}", e)))?;

        let model_path = cache_dir.join(format!("{}.onnx", model.name().replace("-", "_")));
        let tokenizer_path = cache_dir.join(format!("{}_tokenizer.json", model.name().replace("-", "_")));

        let needs_download = !model_path.exists() || !tokenizer_path.exists();

        if needs_download {
            info!("Downloading model {} to {:?}", model.name(), cache_dir);
            Self::download_model_and_tokenizer(&model, &model_path, &tokenizer_path)?;
            Ok((cache_dir, true))
        } else {
            info!("Model {} already exists at {:?}", model.name(), cache_dir);
            Ok((cache_dir, false))
        }
    }

    /// Returns the cache directory path for models.
    pub fn cache_dir() -> PathBuf {
        dirs::cache_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("kix")
            .join("models")
    }

    /// Checks if a model is already downloaded.
    pub fn is_model_downloaded(model_name: &str) -> bool {
        let model = EmbeddingModel::from_str(model_name);
        let cache_dir = Self::cache_dir();
        let model_path = cache_dir.join(format!("{}.onnx", model.name().replace("-", "_")));
        let tokenizer_path = cache_dir.join(format!("{}_tokenizer.json", model.name().replace("-", "_")));
        model_path.exists() && tokenizer_path.exists()
    }

    /// Creates a new ONNX backend with CPU execution.
    pub fn new_cpu(config: BackendConfig) -> Result<Self, EmbeddingError> {
        Self::new_with_provider(config, AccelerationMode::Cpu, vec![])
    }

    /// Creates a new ONNX backend with CUDA GPU acceleration.
    #[cfg(feature = "onnx-cuda")]
    pub fn new_cuda(config: BackendConfig) -> Result<Self, EmbeddingError> {
        use ort::execution_providers::CUDAExecutionProvider;

        let cuda_provider = CUDAExecutionProvider::default()
            .with_device_id(0)
            .build();

        Self::new_with_provider(
            config,
            AccelerationMode::Cuda,
            vec![cuda_provider.into()],
        )
    }

    /// Creates a new ONNX backend with CoreML acceleration (Apple Silicon).
    #[cfg(feature = "onnx-coreml")]
    pub fn new_coreml(config: BackendConfig) -> Result<Self, EmbeddingError> {
        use ort::execution_providers::CoreMLExecutionProvider;

        let coreml_provider = CoreMLExecutionProvider::default()
            .with_subgraphs()
            .with_ane_only()
            .build();

        Self::new_with_provider(
            config,
            AccelerationMode::Metal,
            vec![coreml_provider.into()],
        )
    }

    /// Creates an ONNX backend with specified execution providers.
    fn new_with_provider(
        config: BackendConfig,
        acceleration: AccelerationMode,
        providers: Vec<ExecutionProviderDispatch>,
    ) -> Result<Self, EmbeddingError> {
        let model = EmbeddingModel::from_str(&config.model_name);

        // Determine batch size
        let batch_size = config.batch_size.unwrap_or_else(|| {
            super::optimal_batch_size(acceleration, config.quantized)
        });

        info!(
            "Initializing ONNX Runtime backend: model={}, dims={}, acceleration={}, batch_size={}, quantized={}",
            model.name(), model.dimensions(), acceleration, batch_size, config.quantized
        );

        // Get cache directory
        let cache_dir = dirs::cache_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("kix")
            .join("models");
        std::fs::create_dir_all(&cache_dir)
            .map_err(|e| EmbeddingError::ModelInit(format!("Failed to create cache dir: {}", e)))?;

        // Download model and tokenizer if needed
        let model_path = cache_dir.join(format!("{}.onnx", model.name().replace("-", "_")));
        let tokenizer_path = cache_dir.join(format!("{}_tokenizer.json", model.name().replace("-", "_")));

        if !model_path.exists() || !tokenizer_path.exists() {
            info!("Downloading model {} to {:?}", model.name(), cache_dir);
            Self::download_model_and_tokenizer(&model, &model_path, &tokenizer_path)?;
        }

        // Load tokenizer
        info!("Loading tokenizer from {:?}", tokenizer_path);
        let tokenizer = Tokenizer::from_file(&tokenizer_path)
            .map_err(|e| EmbeddingError::ModelInit(format!("Failed to load tokenizer: {}", e)))?;

        // Initialize ONNX Runtime
        ort::init()
            .with_name("kix-embeddings")
            .commit()
            .map_err(|e| EmbeddingError::ModelInit(format!("Failed to initialize ONNX Runtime: {}", e)))?;

        // Build session with execution providers
        let mut session_builder = Session::builder()
            .map_err(|e| EmbeddingError::ModelInit(format!("Failed to create session builder: {}", e)))?;

        // Add execution providers
        for provider in providers {
            session_builder = session_builder.with_execution_providers([provider])
                .map_err(|e| {
                    warn!("Failed to add execution provider: {}", e);
                    EmbeddingError::ModelInit(format!("Failed to add execution provider: {}", e))
                })?;
        }

        // Load the model
        let session = session_builder
            .commit_from_file(&model_path)
            .map_err(|e| EmbeddingError::ModelInit(format!("Failed to load model: {}", e)))?;

        info!(
            "ONNX Runtime backend initialized: {}D embeddings, {} mode",
            model.dimensions(), acceleration
        );

        Ok(Self {
            session,
            tokenizer,
            model,
            batch_size,
            acceleration,
            quantized: config.quantized,
        })
    }

    /// Downloads the ONNX model and tokenizer from Hugging Face.
    fn download_model_and_tokenizer(
        model: &EmbeddingModel,
        model_path: &PathBuf,
        tokenizer_path: &PathBuf,
    ) -> Result<(), EmbeddingError> {
        let repo_id = model.repo_id();

        // Determine ONNX model path in the repo
        // BGE models have ONNX in onnx/ subdirectory, sentence-transformers in root
        let onnx_path = match model {
            EmbeddingModel::AllMiniLmL6V2 => "onnx/model.onnx",
            _ => "onnx/model.onnx",
        };

        // Download model
        let model_url = format!(
            "https://huggingface.co/{}/resolve/main/{}",
            repo_id, onnx_path
        );
        info!("Downloading ONNX model from: {}", model_url);
        Self::download_file(&model_url, model_path)?;

        // Download tokenizer.json
        let tokenizer_url = format!(
            "https://huggingface.co/{}/resolve/main/tokenizer.json",
            repo_id
        );
        info!("Downloading tokenizer from: {}", tokenizer_url);
        Self::download_file(&tokenizer_url, tokenizer_path)?;

        Ok(())
    }

    /// Downloads a file from URL to the target path.
    fn download_file(url: &str, target: &PathBuf) -> Result<(), EmbeddingError> {
        let response = ureq::get(url)
            .call()
            .map_err(|e| EmbeddingError::ModelInit(format!("Download failed: {}", e)))?;

        let mut file = std::fs::File::create(target)
            .map_err(|e| EmbeddingError::ModelInit(format!("Failed to create file: {}", e)))?;

        std::io::copy(&mut response.into_reader(), &mut file)
            .map_err(|e| EmbeddingError::ModelInit(format!("Failed to write file: {}", e)))?;

        info!("Downloaded to {:?}", target);
        Ok(())
    }

    /// Tokenizes texts using the HuggingFace tokenizer.
    fn tokenize_batch(&self, texts: &[&str]) -> Result<(Vec<i64>, Vec<i64>, Vec<i64>), EmbeddingError> {
        let max_len = self.model.max_seq_length();
        let batch_size = texts.len();

        // Configure tokenizer for batch processing
        let encodings = self.tokenizer
            .encode_batch(texts.to_vec(), true)
            .map_err(|e| EmbeddingError::Generation(format!("Tokenization failed: {}", e)))?;

        let mut all_input_ids = Vec::with_capacity(batch_size * max_len);
        let mut all_attention_mask = Vec::with_capacity(batch_size * max_len);
        let mut all_token_type_ids = Vec::with_capacity(batch_size * max_len);

        for encoding in encodings {
            let ids = encoding.get_ids();
            let mask = encoding.get_attention_mask();
            let types = encoding.get_type_ids();

            // Truncate or pad to max_len
            let actual_len = ids.len().min(max_len);

            // Add actual tokens
            for i in 0..actual_len {
                all_input_ids.push(ids[i] as i64);
                all_attention_mask.push(mask[i] as i64);
                all_token_type_ids.push(types[i] as i64);
            }

            // Pad to max_len
            for _ in actual_len..max_len {
                all_input_ids.push(0);
                all_attention_mask.push(0);
                all_token_type_ids.push(0);
            }
        }

        Ok((all_input_ids, all_attention_mask, all_token_type_ids))
    }

    /// Runs inference on a batch of texts.
    fn run_inference(&mut self, texts: &[&str]) -> Result<Vec<Vec<f32>>, EmbeddingError> {
        let batch_size = texts.len();
        let max_len = self.model.max_seq_length();

        // Tokenize
        let (input_ids, attention_mask, token_type_ids) = self.tokenize_batch(texts)?;

        debug!("Running inference on batch of {} texts", batch_size);

        // Create input tensors - ort v2 needs owned Vecs
        let input_ids_tensor = Value::from_array(
            ([batch_size, max_len], input_ids.clone())
        ).map_err(|e| EmbeddingError::Generation(format!("Failed to create input tensor: {}", e)))?;

        let attention_mask_tensor = Value::from_array(
            ([batch_size, max_len], attention_mask.clone())
        ).map_err(|e| EmbeddingError::Generation(format!("Failed to create attention mask tensor: {}", e)))?;

        let token_type_tensor = Value::from_array(
            ([batch_size, max_len], token_type_ids.clone())
        ).map_err(|e| EmbeddingError::Generation(format!("Failed to create token type tensor: {}", e)))?;

        // Run inference
        let inputs = ort::inputs![
            "input_ids" => input_ids_tensor,
            "attention_mask" => attention_mask_tensor,
            "token_type_ids" => token_type_tensor,
        ];

        let outputs = self.session.run(inputs)
            .map_err(|e| EmbeddingError::Generation(format!("Inference failed: {}", e)))?;

        // Extract embeddings from output
        // Try different output names used by different models
        let output = outputs.get("last_hidden_state")
            .or_else(|| outputs.get("sentence_embedding"))
            .or_else(|| outputs.get("token_embeddings"))
            .ok_or_else(|| EmbeddingError::Generation("No valid output tensor found".to_string()))?;

        let output_view = output.try_extract_tensor::<f32>()
            .map_err(|e| EmbeddingError::Generation(format!("Failed to extract output: {}", e)))?;

        // Get shape and data - Shape derefs to [i64]
        let (shape, data) = (output_view.0, output_view.1);
        let output_shape: Vec<usize> = shape.iter().map(|&d| d as usize).collect();
        let mut embeddings = Vec::with_capacity(batch_size);

        if output_shape.len() == 3 {
            // Shape: [batch, seq_len, hidden]
            let seq_len = output_shape[1];
            let hidden_size = output_shape[2];

            for b in 0..batch_size {
                let mut embedding = vec![0.0f32; hidden_size];
                let mut total_weight = 0.0f32;

                for s in 0..seq_len {
                    let mask_val = attention_mask[b * max_len + s] as f32;
                    if mask_val > 0.0 {
                        for h in 0..hidden_size {
                            let idx = b * seq_len * hidden_size + s * hidden_size + h;
                            embedding[h] += data[idx] * mask_val;
                        }
                        total_weight += mask_val;
                    }
                }

                // Mean pooling
                if total_weight > 0.0 {
                    for h in 0..hidden_size {
                        embedding[h] /= total_weight;
                    }
                }

                // L2 normalize
                let norm: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
                if norm > 0.0 {
                    for h in 0..hidden_size {
                        embedding[h] /= norm;
                    }
                }

                embeddings.push(embedding);
            }
        } else if output_shape.len() == 2 {
            // Shape: [batch, hidden] - already pooled
            let hidden_size = output_shape[1];

            for b in 0..batch_size {
                let mut embedding = Vec::with_capacity(hidden_size);
                for h in 0..hidden_size {
                    let idx = b * hidden_size + h;
                    embedding.push(data[idx]);
                }

                // L2 normalize
                let norm: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
                if norm > 0.0 {
                    for h in 0..hidden_size {
                        embedding[h] /= norm;
                    }
                }

                embeddings.push(embedding);
            }
        } else {
            return Err(EmbeddingError::Generation(
                format!("Unexpected output shape: {:?}", output_shape)
            ));
        }

        Ok(embeddings)
    }
}

impl EmbeddingBackend for OnnxBackend {
    fn info(&self) -> BackendInfo {
        BackendInfo {
            name: format!("ONNX Runtime ({})", self.acceleration),
            model: self.model.name().to_string(),
            acceleration: self.acceleration,
            dimensions: self.model.dimensions(),
            batch_size: self.batch_size,
            quantized: self.quantized,
        }
    }

    fn dimensions(&self) -> usize {
        self.model.dimensions()
    }

    fn batch_size(&self) -> usize {
        self.batch_size
    }

    fn set_batch_size(&mut self, batch_size: usize) {
        self.batch_size = batch_size;
    }

    fn acceleration_mode(&self) -> AccelerationMode {
        self.acceleration
    }

    fn embed_texts(&mut self, texts: &[&str]) -> Result<Vec<Vec<f32>>, EmbeddingError> {
        if texts.is_empty() {
            return Err(EmbeddingError::EmptyInput);
        }

        // Process in batches
        let mut all_embeddings = Vec::with_capacity(texts.len());

        for chunk in texts.chunks(self.batch_size) {
            let embeddings = self.run_inference(chunk)?;
            all_embeddings.extend(embeddings);
        }

        Ok(all_embeddings)
    }
}
