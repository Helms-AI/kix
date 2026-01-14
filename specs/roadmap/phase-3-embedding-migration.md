# Phase 3: Embedding Migration

**Duration**: 1-2 days
**Dependencies**: Phase 0
**Status**: Not Started

---

## Objective

Replace FastEmbed with Ollama-only embeddings using nomic-embed-text model.

---

## Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                    Embedding Architecture                        │
├─────────────────────────────────────────────────────────────────┤
│                                                                  │
│  kix-embeddings/src/                                            │
│         │                                                        │
│         ▼                                                        │
│  ┌─────────────────────────────────────────────────────────┐    │
│  │  OllamaEmbedder                                          │    │
│  │  ├─ client: OllamaClient                                │    │
│  │  ├─ model: "nomic-embed-text"                           │    │
│  │  ├─ dimensions: 768                                      │    │
│  │  └─ embed(texts) → Vec<Vec<f32>>                        │    │
│  └─────────────────────────────────────────────────────────┘    │
│                    │                                             │
│         ┌─────────┴─────────┐                                   │
│         ▼                   ▼                                    │
│  ┌─────────────┐     ┌─────────────────┐                        │
│  │ Ollama API  │     │ Connection Pool │                        │
│  │ :11434      │     │ (tokio)         │                        │
│  └─────────────┘     └─────────────────┘                        │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘
```

---

## Tasks

### 3.1 Update Dependencies

**File**: `server/Cargo.toml` (workspace)

```toml
[workspace.dependencies]
ollama-rs = { version = "0.2", features = ["stream"] }

# REMOVE these dependencies:
# fastembed = "..."
# ort = "..."
```

**File**: `server/crates/kix-embeddings/Cargo.toml`

```toml
[dependencies]
ollama-rs = { workspace = true }
tokio = { workspace = true }
tracing = { workspace = true }
thiserror = { workspace = true }

# Remove fastembed and ort dependencies
```

**Verification**:
```bash
cargo check -p kix-embeddings
```

---

### 3.2 Create OllamaEmbedder

**File**: `server/crates/kix-embeddings/src/ollama.rs` (NEW or REPLACE existing)

```rust
use ollama_rs::Ollama;
use ollama_rs::generation::embeddings::request::GenerateEmbeddingsRequest;
use std::sync::Arc;
use tokio::sync::Semaphore;

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
            ollama_url: "http://localhost:11434".to_string(),
            model: "nomic-embed-text".to_string(),
            dimensions: 768,
            max_concurrent: 4,
            batch_size: 32,
        }
    }
}

/// Ollama-based embedder
pub struct OllamaEmbedder {
    client: Ollama,
    config: EmbeddingConfig,
    semaphore: Arc<Semaphore>,
}

impl OllamaEmbedder {
    /// Create a new embedder with configuration
    pub fn new(config: EmbeddingConfig) -> Result<Self, EmbeddingError> {
        let client = Ollama::new(&config.ollama_url);

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

    /// Generate embedding for a single text
    pub async fn embed_one(&self, text: &str) -> Result<Vec<f32>, EmbeddingError> {
        let _permit = self.semaphore.acquire().await
            .map_err(|_| EmbeddingError::SemaphoreError)?;

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
        let mut results = Vec::with_capacity(texts.len());

        // Process in batches
        for chunk in texts.chunks(self.config.batch_size) {
            let batch_results = self.embed_batch_internal(chunk).await?;
            results.extend(batch_results);
        }

        Ok(results)
    }

    /// Internal batch processing
    async fn embed_batch_internal(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, EmbeddingError> {
        let futures: Vec<_> = texts
            .iter()
            .map(|text| self.embed_one(text))
            .collect();

        let results = futures::future::join_all(futures).await;

        results.into_iter().collect()
    }

    /// Check if Ollama server is reachable and model is available
    pub async fn health_check(&self) -> Result<(), EmbeddingError> {
        // Try to generate a simple embedding
        self.embed_one("health check").await?;
        Ok(())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum EmbeddingError {
    #[error("Ollama error: {0}")]
    OllamaError(String),

    #[error("Empty response from Ollama")]
    EmptyResponse,

    #[error("Dimension mismatch: expected {expected}, got {actual}")]
    DimensionMismatch { expected: usize, actual: usize },

    #[error("Semaphore error")]
    SemaphoreError,

    #[error("Connection error: {0}")]
    ConnectionError(String),
}
```

---

### 3.3 Update Module Exports

**File**: `server/crates/kix-embeddings/src/lib.rs` (REPLACE)

```rust
mod ollama;

pub use ollama::{OllamaEmbedder, EmbeddingConfig, EmbeddingError};

/// Convenience type alias
pub type Embedder = OllamaEmbedder;
```

---

### 3.4 Update kix-store Integration

**File**: `server/crates/kix-store/src/store.rs` (MODIFY)

Update the store to use the new embedder:

```rust
use kix_embeddings::{OllamaEmbedder, EmbeddingConfig};

impl KixStore {
    pub async fn new(db_path: &Path) -> Result<Self, StoreError> {
        // ... existing initialization ...

        // Initialize embedder with Ollama
        let embedder = OllamaEmbedder::default_config()
            .map_err(|e| StoreError::EmbeddingError(e.to_string()))?;

        Ok(Self {
            // ... other fields ...
            embedder,
        })
    }

    /// Generate embedding for text
    pub async fn generate_embedding(&self, text: &str) -> Result<Vec<f32>, StoreError> {
        self.embedder.embed_one(text)
            .await
            .map_err(|e| StoreError::EmbeddingError(e.to_string()))
    }

    /// Generate embeddings for multiple texts
    pub async fn generate_embeddings(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, StoreError> {
        self.embedder.embed_batch(texts)
            .await
            .map_err(|e| StoreError::EmbeddingError(e.to_string()))
    }
}
```

---

### 3.5 Update kix-jobs Processor

**File**: `server/crates/kix-jobs/src/processor.rs` (MODIFY)

Ensure the processor uses the new embedding system:

```rust
use kix_embeddings::OllamaEmbedder;

impl ContentProcessor {
    /// Process chunks with Ollama embeddings
    async fn embed_chunks(
        &self,
        chunks: &[EntryChunk],
    ) -> Result<Vec<(EntryChunk, Vec<f32>)>, ProcessorError> {
        let texts: Vec<String> = chunks
            .iter()
            .map(|c| c.content.clone())
            .collect();

        let embeddings = self.embedder.embed_batch(&texts)
            .await
            .map_err(|e| ProcessorError::EmbeddingError(e.to_string()))?;

        Ok(chunks.iter()
            .cloned()
            .zip(embeddings.into_iter())
            .collect())
    }
}
```

---

### 3.6 Write Tests

**File**: `server/crates/kix-embeddings/src/ollama.rs` (add at end)

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_embedder_creation() {
        let config = EmbeddingConfig::default();
        assert_eq!(config.model, "nomic-embed-text");
        assert_eq!(config.dimensions, 768);
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
    async fn test_custom_config() {
        let config = EmbeddingConfig {
            ollama_url: "http://custom:11434".to_string(),
            model: "custom-model".to_string(),
            dimensions: 512,
            max_concurrent: 8,
            batch_size: 64,
        };

        assert_eq!(config.dimensions, 512);
        assert_eq!(config.max_concurrent, 8);
    }
}
```

---

### 3.7 Remove FastEmbed/ONNX Code

**Files to delete or clean**:
- Remove any `fastembed` initialization code
- Remove ONNX runtime configuration
- Remove GPU detection for ONNX (Ollama handles GPU automatically)
- Update any `EmbeddingModel` references to use `OllamaEmbedder`

---

## Deliverables

| Deliverable | File | Description |
|-------------|------|-------------|
| OllamaEmbedder | `kix-embeddings/src/ollama.rs` | Main embedder implementation |
| Updated exports | `kix-embeddings/src/lib.rs` | Clean public API |
| Store integration | `kix-store/src/store.rs` | Updated embedding calls |
| Processor integration | `kix-jobs/src/processor.rs` | Updated embedding in pipeline |
| Tests | `kix-embeddings/src/ollama.rs` | Unit tests |

---

## Exit Criteria

- [ ] `cargo check -p kix-embeddings` passes
- [ ] Ollama health check succeeds
- [ ] Single text embedding returns 768 dimensions
- [ ] Batch embedding works with multiple texts
- [ ] Store can generate embeddings
- [ ] Processor embeds chunks successfully
- [ ] No FastEmbed/ONNX dependencies remain
- [ ] All existing tests still pass

---

## Testing Commands

```bash
# Ensure Ollama is running with model
ollama pull nomic-embed-text
curl http://localhost:11434/api/tags | grep nomic

# Run embedding tests (requires Ollama)
cargo test -p kix-embeddings --release -- --ignored

# Verify no ONNX dependencies
cargo tree -p kix-embeddings | grep -i onnx  # Should be empty
cargo tree -p kix-embeddings | grep -i fastembed  # Should be empty
```

---

## Configuration

### Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `OLLAMA_URL` | `http://localhost:11434` | Ollama server URL |
| `EMBEDDING_MODEL` | `nomic-embed-text` | Model for embeddings |
| `EMBEDDING_CONCURRENCY` | `4` | Max concurrent requests |

### nomic-embed-text Model Info

| Property | Value |
|----------|-------|
| Dimensions | 768 |
| Max tokens | 8192 |
| Type | Text embedding |
| License | Apache 2.0 |

---

## Next Phase

Upon completion, this phase can be integrated with Phase 5 (API & SSE Updates).

Phase 4 (Tree-sitter) can start in parallel after Phase 2.
