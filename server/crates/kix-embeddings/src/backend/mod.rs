//! Multi-backend embedding system with GPU acceleration support.
//!
//! This module provides a unified interface for different embedding backends:
//! - **FastEmbed**: Default CPU-based backend (fast, no dependencies)
//! - **ONNX Runtime**: High-performance backend with GPU support (CUDA/Metal)

mod traits;

#[cfg(feature = "fastembed-backend")]
pub mod fastembed;

#[cfg(feature = "onnx-backend")]
pub mod onnx;

pub use traits::{EmbeddingBackend, BackendInfo, BackendConfig};

use crate::error::EmbeddingError;
use tracing::info;

#[allow(unused_imports)]
use tracing::warn;

/// Detected acceleration mode for embedding generation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccelerationMode {
    /// CPU-only mode (default)
    Cpu,
    /// NVIDIA CUDA GPU acceleration
    Cuda,
    /// Apple CoreML/Metal acceleration
    Metal,
}

impl std::fmt::Display for AccelerationMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AccelerationMode::Cpu => write!(f, "CPU"),
            AccelerationMode::Cuda => write!(f, "CUDA GPU"),
            AccelerationMode::Metal => write!(f, "Apple Metal"),
        }
    }
}

/// Available embedding backends
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackendType {
    /// FastEmbed backend (CPU-only, default)
    FastEmbed,
    /// ONNX Runtime backend (supports GPU)
    OnnxRuntime,
}

impl std::fmt::Display for BackendType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BackendType::FastEmbed => write!(f, "FastEmbed"),
            BackendType::OnnxRuntime => write!(f, "ONNX Runtime"),
        }
    }
}

/// Creates the best available embedding backend based on features and hardware.
///
/// Selection priority:
/// 1. ONNX Runtime with CUDA (if cuda feature enabled and GPU available)
/// 2. ONNX Runtime with Metal (if coreml feature enabled on Apple Silicon)
/// 3. ONNX Runtime CPU (if onnx-backend feature enabled)
/// 4. FastEmbed CPU (default fallback)
pub fn create_best_backend(config: Option<BackendConfig>) -> Result<Box<dyn EmbeddingBackend>, EmbeddingError> {
    let config = config.unwrap_or_default();

    // Try ONNX with CUDA first
    #[cfg(feature = "onnx-cuda")]
    {
        if cuda_available() {
            info!("CUDA GPU detected - attempting ONNX Runtime with CUDA");
            match onnx::OnnxBackend::new_cuda(config.clone()) {
                Ok(backend) => {
                    info!("ONNX Runtime CUDA backend initialized successfully");
                    return Ok(Box::new(backend));
                }
                Err(e) => {
                    warn!("Failed to initialize ONNX CUDA backend: {}, falling back", e);
                }
            }
        }
    }

    // Try ONNX with Metal (Apple Silicon)
    #[cfg(feature = "onnx-coreml")]
    {
        if metal_available() {
            info!("Apple Silicon detected - attempting ONNX Runtime with CoreML");
            match onnx::OnnxBackend::new_coreml(config.clone()) {
                Ok(backend) => {
                    info!("ONNX Runtime CoreML backend initialized successfully");
                    return Ok(Box::new(backend));
                }
                Err(e) => {
                    warn!("Failed to initialize ONNX CoreML backend: {}, falling back", e);
                }
            }
        }
    }

    // Try ONNX CPU
    #[cfg(feature = "onnx-backend")]
    {
        info!("Attempting ONNX Runtime CPU backend");
        match onnx::OnnxBackend::new_cpu(config.clone()) {
            Ok(backend) => {
                info!("ONNX Runtime CPU backend initialized successfully");
                return Ok(Box::new(backend));
            }
            Err(e) => {
                warn!("Failed to initialize ONNX CPU backend: {}, falling back to FastEmbed", e);
            }
        }
    }

    // Fall back to FastEmbed
    #[cfg(feature = "fastembed-backend")]
    {
        info!("Using FastEmbed backend (CPU)");
        let backend = fastembed::FastEmbedBackend::new(config)?;
        return Ok(Box::new(backend));
    }

    #[cfg(not(feature = "fastembed-backend"))]
    {
        Err(EmbeddingError::NoBackendAvailable(
            "No embedding backend available. Enable 'fastembed-backend' or 'onnx-backend' feature.".to_string()
        ))
    }
}

/// Creates a specific backend by type.
pub fn create_backend(backend_type: BackendType, config: Option<BackendConfig>) -> Result<Box<dyn EmbeddingBackend>, EmbeddingError> {
    let config = config.unwrap_or_default();

    match backend_type {
        #[cfg(feature = "fastembed-backend")]
        BackendType::FastEmbed => {
            let backend = fastembed::FastEmbedBackend::new(config)?;
            Ok(Box::new(backend))
        }
        #[cfg(not(feature = "fastembed-backend"))]
        BackendType::FastEmbed => {
            Err(EmbeddingError::NoBackendAvailable(
                "FastEmbed backend not available. Enable 'fastembed-backend' feature.".to_string()
            ))
        }

        #[cfg(feature = "onnx-backend")]
        BackendType::OnnxRuntime => {
            let backend = onnx::OnnxBackend::new_cpu(config)?;
            Ok(Box::new(backend))
        }
        #[cfg(not(feature = "onnx-backend"))]
        BackendType::OnnxRuntime => {
            Err(EmbeddingError::NoBackendAvailable(
                "ONNX Runtime backend not available. Enable 'onnx-backend' feature.".to_string()
            ))
        }
    }
}

/// Check if CUDA GPU is available.
#[cfg(feature = "onnx-cuda")]
fn cuda_available() -> bool {
    std::env::var("CUDA_VISIBLE_DEVICES").is_ok()
        || std::path::Path::new("/usr/local/cuda").exists()
        || std::path::Path::new("/opt/cuda").exists()
        || std::env::var("NVIDIA_VISIBLE_DEVICES").is_ok()
}

/// Check if Metal/CoreML is available (Apple Silicon).
#[cfg(feature = "onnx-coreml")]
fn metal_available() -> bool {
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    return true;
    #[cfg(not(all(target_os = "macos", target_arch = "aarch64")))]
    return false;
}

/// Returns the optimal batch size for the given acceleration mode.
pub fn optimal_batch_size(mode: AccelerationMode, quantized: bool) -> usize {
    match (mode, quantized) {
        (AccelerationMode::Cuda, true) => 4096,   // INT8 on GPU
        (AccelerationMode::Cuda, false) => 2048,  // FP32 on GPU
        (AccelerationMode::Metal, true) => 2048,  // INT8 on Metal
        (AccelerationMode::Metal, false) => 1024, // FP32 on Metal
        (AccelerationMode::Cpu, true) => 512,     // INT8 on CPU
        (AccelerationMode::Cpu, false) => {
            let cpu_cores = num_cpus::get();
            if cpu_cores >= 16 { 512 }
            else if cpu_cores >= 8 { 256 }
            else { 128 }
        }
    }
}

/// Setup information returned by ensure_setup.
#[derive(Debug, Clone)]
pub struct SetupInfo {
    /// Path to the model cache directory
    pub cache_dir: std::path::PathBuf,
    /// Whether models were downloaded (false if already present)
    pub downloaded: bool,
    /// Model name that was set up
    pub model_name: String,
}

/// Ensures the embedding model is downloaded and ready.
/// Call this on server startup to pre-download models.
/// Returns setup info including whether download was needed.
#[cfg(feature = "onnx-backend")]
pub fn ensure_setup() -> Result<SetupInfo, EmbeddingError> {
    let model_name = std::env::var("KIX_EMBEDDING_MODEL")
        .unwrap_or_else(|_| "bge-base-en-v1.5".to_string());

    info!("Ensuring embedding model is set up: {}", model_name);

    let (cache_dir, downloaded) = onnx::OnnxBackend::ensure_model_downloaded(&model_name)?;

    Ok(SetupInfo {
        cache_dir,
        downloaded,
        model_name,
    })
}

/// Checks if the embedding model is already downloaded and ready.
#[cfg(feature = "onnx-backend")]
pub fn is_setup() -> bool {
    let model_name = std::env::var("KIX_EMBEDDING_MODEL")
        .unwrap_or_else(|_| "bge-base-en-v1.5".to_string());
    onnx::OnnxBackend::is_model_downloaded(&model_name)
}

/// Returns the cache directory for models.
#[cfg(feature = "onnx-backend")]
pub fn model_cache_dir() -> std::path::PathBuf {
    onnx::OnnxBackend::cache_dir()
}

/// Stub for when ONNX backend is not enabled.
#[cfg(not(feature = "onnx-backend"))]
pub fn ensure_setup() -> Result<SetupInfo, EmbeddingError> {
    // FastEmbed downloads models automatically on first use
    Ok(SetupInfo {
        cache_dir: std::path::PathBuf::from("."),
        downloaded: false,
        model_name: "all-MiniLM-L6-v2".to_string(),
    })
}

/// Stub for when ONNX backend is not enabled.
#[cfg(not(feature = "onnx-backend"))]
pub fn is_setup() -> bool {
    true // FastEmbed handles this internally
}

/// Stub for when ONNX backend is not enabled.
#[cfg(not(feature = "onnx-backend"))]
pub fn model_cache_dir() -> std::path::PathBuf {
    // FastEmbed uses its own cache directory logic
    std::path::PathBuf::from(".cache/fastembed")
}