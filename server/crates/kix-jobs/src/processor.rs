//! Content processor for indexing documents into LanceDB

use std::path::Path;
use std::sync::Arc;

use tokio::sync::{mpsc, oneshot, Mutex, RwLock};
use tracing::{debug, info, warn};

use kix_embeddings::{ChunkingConfig, DocumentChunker, EmbeddingGenerator, AccelerationMode};
use kix_parser::{Entry, EntryChunk, EntryType, HtmlParser, PdfParser, SourceType};
use kix_store::KixStore;

use crate::linker::{LinkingConfig, PatternLink, PatternLinker};
use crate::JobError;

/// Request for the embedding worker
struct EmbeddingRequest {
    chunks: Vec<EntryChunk>,
    response_tx: oneshot::Sender<Result<Vec<Vec<f32>>, String>>,
}

/// Embedding worker pool for parallel embedding generation
///
/// Uses multiple workers in CPU mode for better multi-core utilization,
/// or a single worker in GPU mode since GPU is already parallel.
struct EmbeddingWorkerPool {
    tx: mpsc::Sender<EmbeddingRequest>,
    worker_count: usize,
}

impl EmbeddingWorkerPool {
    /// Create a new embedding worker pool
    ///
    /// - GPU mode: Single worker with larger queue (GPU handles parallelism)
    /// - CPU mode: Multiple workers (one per CPU core, up to max_workers)
    fn new(max_workers: usize, base_queue_size: usize) -> Result<Self, String> {
        // Detect if we should use multi-worker mode
        let sample_embedder = EmbeddingGenerator::new()
            .map_err(|e| e.to_string())?;

        let acceleration_mode = sample_embedder.acceleration_mode();
        let backend_info = sample_embedder.info();

        // Check if we're in CPU mode (use multiple workers)
        // GPU modes (Cuda, Metal) use single worker since GPU handles parallelism
        let is_cpu_mode = matches!(acceleration_mode, AccelerationMode::Cpu);

        // Determine worker count
        let worker_count = if is_cpu_mode {
            // Use multiple workers for CPU mode (one per core, capped at max)
            let cpu_cores = num_cpus::get();
            std::cmp::min(cpu_cores, max_workers)
        } else {
            1 // GPU handles parallelism internally
        };

        // Adjust queue size based on mode:
        // - GPU mode: Larger queue to maintain saturation
        // - CPU mode: Standard queue size
        let queue_size = if is_cpu_mode {
            base_queue_size
        } else {
            base_queue_size * 4 // Larger queue for GPU to maintain saturation
        };

        info!(
            "Creating embedding worker pool: {} workers, queue_size={}, backend={}, acceleration={}",
            worker_count, queue_size, backend_info.name, acceleration_mode
        );

        let (tx, rx) = mpsc::channel::<EmbeddingRequest>(queue_size);

        // Wrap receiver in Arc<Mutex> for sharing between workers
        let rx = std::sync::Arc::new(tokio::sync::Mutex::new(rx));

        // Spawn worker tasks
        for worker_id in 0..worker_count {
            let rx = rx.clone();

            tokio::spawn(async move {
                // Each worker creates its own embedder instance
                let mut embedder = match EmbeddingGenerator::new() {
                    Ok(e) => e,
                    Err(e) => {
                        warn!("Worker {} failed to init embedder: {}", worker_id, e);
                        return;
                    }
                };

                info!("Embedding worker {} started", worker_id);

                loop {
                    // Acquire lock, receive request, then release lock
                    let request = {
                        let mut rx_guard = rx.lock().await;
                        rx_guard.recv().await
                    };

                    match request {
                        Some(req) => {
                            let result = embedder
                                .embed_chunks(&req.chunks)
                                .map_err(|e| e.to_string());

                            let _ = req.response_tx.send(result);
                        }
                        None => {
                            info!("Embedding worker {} shutting down", worker_id);
                            break;
                        }
                    }
                }
            });
        }

        Ok(Self { tx, worker_count })
    }

    /// Get the number of workers in the pool
    fn worker_count(&self) -> usize {
        self.worker_count
    }

    /// Generate embeddings for chunks via the worker pool
    async fn embed_chunks(&self, chunks: Vec<EntryChunk>) -> Result<Vec<Vec<f32>>, String> {
        let (response_tx, response_rx) = oneshot::channel();

        let request = EmbeddingRequest {
            chunks,
            response_tx,
        };

        // Send request to worker pool
        self.tx
            .send(request)
            .await
            .map_err(|_| "Embedding worker pool channel closed".to_string())?;

        // Wait for response
        response_rx
            .await
            .map_err(|_| "Embedding response channel closed".to_string())?
    }
}

/// Content processor configuration
#[derive(Clone, Debug)]
pub struct ProcessorConfig {
    /// Chunking configuration
    pub chunk_size: usize,
    /// Chunk overlap
    pub chunk_overlap: usize,
    /// Batch size for embeddings
    pub embedding_batch_size: usize,
    /// Pattern linking configuration
    pub linking: LinkingConfig,
    /// Whether to enable automatic pattern linking
    pub enable_linking: bool,
}

impl Default for ProcessorConfig {
    fn default() -> Self {
        Self {
            chunk_size: 512,
            chunk_overlap: 50,
            embedding_batch_size: 32,
            linking: LinkingConfig::default(),
            enable_linking: true,
        }
    }
}

/// Content processor handles parsing, chunking, embedding, and storage
pub struct ContentProcessor {
    store: Arc<RwLock<KixStore>>,
    embedding_pool: Arc<EmbeddingWorkerPool>,
    linker: PatternLinker,
    chunker: DocumentChunker,
    html_parser: HtmlParser,
    pdf_parser: PdfParser,
    config: ProcessorConfig,
}

impl ContentProcessor {
    /// Create a new content processor
    pub async fn new(
        db_path: &str,
        config: ProcessorConfig,
    ) -> Result<Self, JobError> {
        info!("Initializing content processor with database at: {}", db_path);

        // Initialize store
        let mut store = KixStore::new(db_path)
            .await
            .map_err(|e| JobError::Processing(format!("Failed to create store: {}", e)))?;

        store.init_tables().await
            .map_err(|e| JobError::Processing(format!("Failed to init tables: {}", e)))?;

        // Create embedding worker pool (auto-scales based on GPU/CPU)
        // Max 8 workers for CPU mode, queue size of 64
        let embedding_pool = Arc::new(
            EmbeddingWorkerPool::new(8, 64)
                .map_err(|e| JobError::Processing(format!("Failed to create embedding pool: {}", e)))?
        );

        info!(
            "Embedding worker pool initialized with {} workers",
            embedding_pool.worker_count()
        );

        // Initialize a separate embedder for linker compatibility
        let embedder_compat = EmbeddingGenerator::new()
            .map_err(|e| JobError::Processing(format!("Failed to init linker embedder: {}", e)))?;

        // Initialize chunker
        let chunker = DocumentChunker::new(ChunkingConfig {
            max_chunk_size: config.chunk_size,
            overlap_size: config.chunk_overlap,
            ..Default::default()
        });

        // Use RwLock for better concurrent read access
        let store = Arc::new(RwLock::new(store));
        let embedder_compat = Arc::new(Mutex::new(embedder_compat));

        // Initialize pattern linker (still uses mutex-based embedder for compatibility)
        let linker = PatternLinker::new(
            store.clone(),
            embedder_compat.clone(),
            config.linking.clone(),
        );

        Ok(Self {
            store,
            embedding_pool,
            linker,
            chunker,
            html_parser: HtmlParser::new(),
            pdf_parser: PdfParser::new(),
            config,
        })
    }

    /// Process HTML content and store it
    pub async fn process_html(
        &self,
        content: &str,
        source_url: &str,
    ) -> Result<ProcessingResult, JobError> {
        debug!(url = source_url, "Processing HTML content");

        // Parse HTML
        let document = self.html_parser
            .parse(content, source_url)
            .map_err(|e| JobError::Processing(format!("Failed to parse HTML: {}", e)))?;

        self.process_document(document).await
    }

    /// Process a file and store it
    pub async fn process_file(
        &self,
        file_path: &Path,
        original_name: &str,
    ) -> Result<ProcessingResult, JobError> {
        debug!(path = ?file_path, name = original_name, "Processing file");

        let extension = file_path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();

        // Read file content
        let content = tokio::fs::read(file_path)
            .await
            .map_err(|e| JobError::Processing(format!("Failed to read file: {}", e)))?;

        // Process based on file type
        let document = match extension.as_str() {
            "html" | "htm" => {
                let text = String::from_utf8_lossy(&content);
                self.html_parser
                    .parse(&text, original_name)
                    .map_err(|e| JobError::Processing(format!("Failed to parse HTML: {}", e)))?
            }
            "pdf" => {
                self.pdf_parser
                    .parse(file_path.to_str().unwrap_or(""))
                    .map_err(|e| JobError::Processing(format!("Failed to parse PDF: {}", e)))?
            }
            "txt" | "md" | "markdown" => {
                let text = String::from_utf8_lossy(&content);
                self.create_text_document(&text, original_name, file_path)?
            }
            "json" => {
                let text = String::from_utf8_lossy(&content);
                self.create_text_document(&text, original_name, file_path)?
            }
            _ => {
                // Try to process as text
                if let Ok(text) = String::from_utf8(content.clone()) {
                    self.create_text_document(&text, original_name, file_path)?
                } else {
                    return Err(JobError::Processing(format!(
                        "Unsupported file type: {}",
                        extension
                    )));
                }
            }
        };

        self.process_document(document).await
    }

    /// Create a simple text document
    fn create_text_document(
        &self,
        content: &str,
        name: &str,
        path: &Path,
    ) -> Result<Entry, JobError> {
        use sha2::{Digest, Sha256};

        let mut hasher = Sha256::new();
        hasher.update(content.as_bytes());
        let hash = format!("{:x}", hasher.finalize());

        let title = Path::new(name)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or(name)
            .to_string();

        let source_path = path.to_string_lossy().to_string();
        let id = Entry::generate_id_from_path(&source_path);
        let description: String = content.chars().take(200).collect();

        let entry = Entry::with_id(id, title, source_path, hash)
            .with_description(description)
            .with_content(content.to_string())
            .with_tags(vec!["indexed-content".to_string()])
            .with_entry_type(EntryType::Document)
            .with_source_type(SourceType::Html);

        Ok(entry)
    }

    /// Process a document and store it
    async fn process_document(&self, entry: Entry) -> Result<ProcessingResult, JobError> {
        let doc_id = entry.id.clone();
        let doc_title = entry.title.clone();
        let doc_description = entry.description.clone();

        // Create chunks from entry
        let chunks = self.chunker.chunk(&entry);

        if chunks.is_empty() {
            warn!(doc_id = doc_id, "No chunks generated from document");
            return Ok(ProcessingResult {
                document_id: doc_id,
                chunks_created: 0,
                embeddings_generated: 0,
                related_patterns: vec![],
            });
        }

        info!(
            doc_id = doc_id,
            chunks = chunks.len(),
            "Generated chunks from document"
        );

        // Generate embeddings via worker pool (non-blocking, auto-scales)
        let embeddings = self
            .embedding_pool
            .embed_chunks(chunks.clone())
            .await
            .map_err(|e| JobError::Processing(format!("Failed to generate embeddings: {}", e)))?;

        // Store document and chunks using write lock
        {
            let store = self.store.write().await;

            // Check if document already exists
            let exists = store.document_exists(&doc_id).await
                .map_err(|e| JobError::Processing(format!("Failed to check document: {}", e)))?;

            if exists {
                // Delete existing document and chunks
                store.delete_chunks_by_document(&doc_id).await
                    .map_err(|e| JobError::Processing(format!("Failed to delete old chunks: {}", e)))?;
                store.delete_document(&doc_id).await
                    .map_err(|e| JobError::Processing(format!("Failed to delete old document: {}", e)))?;
            }

            // Insert new document
            store.insert_documents(&[entry]).await
                .map_err(|e| JobError::Processing(format!("Failed to insert document: {}", e)))?;

            // Insert chunks with embeddings
            store.insert_chunks(&chunks, &embeddings).await
                .map_err(|e| JobError::Processing(format!("Failed to insert chunks: {}", e)))?;
        }

        // Find related patterns via semantic similarity
        let related_patterns = if self.config.enable_linking {
            match self.linker.find_related_for_document(
                &doc_title,
                &doc_description,
                None, // problem field no longer exists
                None, // solution field no longer exists
                Some(&doc_id),
            ).await {
                Ok(links) => {
                    info!(
                        doc_id = doc_id,
                        related_count = links.len(),
                        "Found related patterns"
                    );
                    links
                }
                Err(e) => {
                    warn!(doc_id = doc_id, error = %e, "Failed to find related patterns");
                    vec![]
                }
            }
        } else {
            vec![]
        };

        info!(
            doc_id = doc_id,
            title = doc_title,
            chunks = chunks.len(),
            related = related_patterns.len(),
            "Successfully processed and stored document"
        );

        Ok(ProcessingResult {
            document_id: doc_id,
            chunks_created: chunks.len(),
            embeddings_generated: embeddings.len(),
            related_patterns,
        })
    }

    /// Get store reference for direct access (uses RwLock)
    pub fn store(&self) -> Arc<RwLock<KixStore>> {
        self.store.clone()
    }

    /// Get pattern linker reference
    pub fn linker(&self) -> &PatternLinker {
        &self.linker
    }

    /// Find related patterns for a document that's already stored
    pub async fn find_related_patterns(
        &self,
        document_id: &str,
        query_text: &str,
    ) -> Result<Vec<PatternLink>, JobError> {
        self.linker
            .find_related(query_text, Some(document_id))
            .await
    }

    /// Get document count (uses read lock for concurrent access)
    pub async fn document_count(&self) -> Result<usize, JobError> {
        let store = self.store.read().await;
        store.document_count().await
            .map_err(|e| JobError::Processing(format!("Failed to get document count: {}", e)))
    }

    /// Get chunk count (uses read lock for concurrent access)
    pub async fn chunk_count(&self) -> Result<usize, JobError> {
        let store = self.store.read().await;
        store.chunk_count().await
            .map_err(|e| JobError::Processing(format!("Failed to get chunk count: {}", e)))
    }
}

/// Result of processing a document
#[derive(Debug, Clone)]
pub struct ProcessingResult {
    /// Document ID
    pub document_id: String,
    /// Number of chunks created
    pub chunks_created: usize,
    /// Number of embeddings generated
    pub embeddings_generated: usize,
    /// Related patterns found via semantic similarity
    pub related_patterns: Vec<PatternLink>,
}
