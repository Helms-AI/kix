//! Content processor for indexing documents into LanceDB

use std::path::Path;
use std::sync::Arc;

use tokio::sync::{mpsc, oneshot, Mutex, RwLock};
use tracing::{debug, info, warn};

use kix_embeddings::{ChunkingConfig, DocumentChunker, EmbeddingGenerator, AccelerationMode};
use kix_parser::{Entry, EntryChunk, EntryType, PdfParser, SourceType};
use kix_store::{KixStore, PageRecord};
use kix_crawler::ContentExtractor;

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
            // Use 2000 chars to stay within embedding model token limits
            // (nomic-embed-text has 2048 token limit; 2000 chars ≈ 500-1000 tokens)
            chunk_size: 2000,
            chunk_overlap: 100,
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
    pdf_parser: PdfParser,
    content_extractor: ContentExtractor,
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
            pdf_parser: PdfParser::new(),
            content_extractor: ContentExtractor::default(),
            config,
        })
    }

    /// Process HTML content and store it
    ///
    /// Uses ContentExtractor for comprehensive content extraction
    /// with boilerplate removal, code block extraction, and metadata capture.
    pub async fn process_html(
        &self,
        content: &str,
        source_url: &str,
    ) -> Result<ProcessingResult, JobError> {
        debug!(url = source_url, "Processing HTML content with ContentExtractor");

        // Parse URL for extractor
        let url = url::Url::parse(source_url)
            .unwrap_or_else(|_| url::Url::parse("http://localhost").unwrap());

        // Use content extraction (never fails)
        let extracted = self.content_extractor.extract(content, &url);

        debug!(
            url = source_url,
            title = %extracted.title,
            markdown_len = extracted.markdown.len(),
            header_count = extracted.headers.len(),
            code_blocks = extracted.code_blocks.len(),
            word_count = extracted.word_count,
            "ContentExtractor extraction complete"
        );

        // Create entry with extracted content
        let entry = self.create_entry_from_extracted(&extracted, source_url)?;

        // Use markdown-aware chunking
        self.process_document_with_markdown(entry, &extracted.markdown).await
    }

    /// Create an entry from ContentExtractor output
    fn create_entry_from_extracted(
        &self,
        extracted: &kix_crawler::ExtractedContent,
        source_url: &str,
    ) -> Result<Entry, JobError> {
        use sha2::{Digest, Sha256};

        // Compute hash for change detection
        let mut hasher = Sha256::new();
        hasher.update(extracted.markdown.as_bytes());
        let source_hash = format!("{:x}", hasher.finalize());

        // Generate slug/ID from URL
        let slug = source_url.to_string();
        let id = slug.clone();

        // Use extracted description or derive from markdown
        let description = extracted.description.clone()
            .unwrap_or_else(|| extracted.markdown.chars().take(300).collect());

        // Determine entry type
        let entry_type = if source_url.contains("/blog/") || source_url.contains("/article/") {
            EntryType::Article
        } else if source_url.contains("/docs/") || source_url.contains("/documentation/") {
            EntryType::Document
        } else {
            EntryType::Document
        };

        let entry = Entry::with_id(id, extracted.title.clone(), source_url.to_string(), source_hash)
            .with_description(description)
            .with_content(extracted.markdown.clone())
            .with_tags(vec![])
            .with_entry_type(entry_type)
            .with_source_type(SourceType::Html)
            .with_slug(slug);

        Ok(entry)
    }

    /// Process a document with smart chunking
    ///
    /// Uses the smart chunking algorithm that prioritizes:
    /// 1. Code block boundaries (never splits code blocks)
    /// 2. Paragraph boundaries (\n\n)
    /// 3. Sentence boundaries (. )
    /// 4. Consolidates small chunks (<200 chars) together
    async fn process_document_with_markdown(
        &self,
        entry: Entry,
        markdown: &str,
    ) -> Result<ProcessingResult, JobError> {
        let doc_id = entry.id.clone();
        let doc_title = entry.title.clone();
        let doc_description = entry.description.clone();

        // Use smart chunking that preserves content boundaries
        let chunks = self.chunker.chunk_smart_text(&entry, markdown);

        if chunks.is_empty() {
            warn!(doc_id = doc_id, "No chunks generated from markdown document");
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
            "Generated smart chunks from document"
        );

        // Generate embeddings via worker pool
        let embeddings = self
            .embedding_pool
            .embed_chunks(chunks.clone())
            .await
            .map_err(|e| JobError::Processing(format!("Failed to generate embeddings: {}", e)))?;

        // Store document and chunks
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

        // Find related patterns
        let related_patterns = if self.config.enable_linking {
            match self.linker.find_related_for_document(
                &doc_title,
                &doc_description,
                None,
                None,
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
            "Successfully processed document with smart chunking"
        );

        Ok(ProcessingResult {
            document_id: doc_id,
            chunks_created: chunks.len(),
            embeddings_generated: embeddings.len(),
            related_patterns,
        })
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
                // Use ContentExtractor for consistent HTML processing
                let url = url::Url::parse(&format!("file://{}", original_name))
                    .unwrap_or_else(|_| url::Url::parse("file:///unknown").unwrap());
                let extracted = self.content_extractor.extract(&text, &url);
                self.create_entry_from_extracted(&extracted, original_name)?
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

    /// Get page count (uses read lock for concurrent access)
    pub async fn page_count(&self) -> Result<usize, JobError> {
        let store = self.store.read().await;
        store.page_count().await
            .map_err(|e| JobError::Processing(format!("Failed to get page count: {}", e)))
    }

    // ========================================================================
    // Two-Layer Storage Integration
    // ========================================================================

    /// Process HTML content using two-layer storage pattern for RAG.
    ///
    /// This method:
    /// 1. Stores the full page content in the pages table (for RAG context)
    /// 2. Stores chunks with page_id FK (for vector search)
    /// 3. Generates embeddings with optional page context
    ///
    /// Use this for crawled pages where full context is valuable for RAG.
    /// Uses ContentExtractor (source of truth) for all HTML processing.
    pub async fn process_html_with_page(
        &self,
        content: &str,
        source_url: &str,
        crawl_time_ms: Option<u64>,
    ) -> Result<TwoLayerResult, JobError> {
        debug!(url = source_url, "Processing HTML with two-layer storage using ContentExtractor");

        // Parse URL for extractor
        let url = url::Url::parse(source_url)
            .unwrap_or_else(|_| url::Url::parse("http://localhost").unwrap());

        // Use content extraction (source of truth, never fails)
        let extracted = self.content_extractor.extract(content, &url);

        debug!(
            url = source_url,
            title = %extracted.title,
            markdown_len = extracted.markdown.len(),
            header_count = extracted.headers.len(),
            code_blocks = extracted.code_blocks.len(),
            word_count = extracted.word_count,
            "ContentExtractor extraction complete for two-layer storage"
        );

        // Create entry with extracted content
        let entry = self.create_entry_from_extracted(&extracted, source_url)?;
        let entry_id = entry.id.clone();

        // Create page record for full content storage
        let page = PageRecord::new(&entry_id, source_url, &extracted.markdown)
            .with_title(extracted.title.clone());

        let page = if let Some(ms) = crawl_time_ms {
            page.with_crawl_time(ms)
        } else {
            page
        };

        // Process with two-layer storage
        self.process_document_with_page(entry, &extracted.markdown, page).await
    }

    /// Process a document with two-layer storage pattern.
    ///
    /// Stores both the full page content and smaller chunks for search.
    async fn process_document_with_page(
        &self,
        entry: Entry,
        markdown: &str,
        page: PageRecord,
    ) -> Result<TwoLayerResult, JobError> {
        let doc_id = entry.id.clone();
        let doc_title = entry.title.clone();
        let doc_description = entry.description.clone();
        let page_id = page.page_id.clone();

        // Use smart chunking
        let mut chunks = self.chunker.chunk_smart_text(&entry, markdown);

        if chunks.is_empty() {
            warn!(doc_id = doc_id, "No chunks generated from document");
            return Ok(TwoLayerResult {
                document_id: doc_id,
                page_id: Some(page_id),
                chunks_created: 0,
                embeddings_generated: 0,
                related_patterns: vec![],
            });
        }

        // Add page_id FK to all chunks
        for chunk in &mut chunks {
            chunk.page_id = Some(page_id.clone());
        }

        info!(
            doc_id = doc_id,
            page_id = page_id,
            chunks = chunks.len(),
            "Generated chunks with page FK reference"
        );

        // Generate embeddings via worker pool
        let embeddings = self
            .embedding_pool
            .embed_chunks(chunks.clone())
            .await
            .map_err(|e| JobError::Processing(format!("Failed to generate embeddings: {}", e)))?;

        // Store using two-layer storage
        {
            let store = self.store.write().await;

            // Check if document already exists
            let exists = store.document_exists(&doc_id).await
                .map_err(|e| JobError::Processing(format!("Failed to check document: {}", e)))?;

            if exists {
                // Delete existing document, chunks, and pages
                store.delete_chunks_by_document(&doc_id).await
                    .map_err(|e| JobError::Processing(format!("Failed to delete old chunks: {}", e)))?;
                store.delete_document(&doc_id).await
                    .map_err(|e| JobError::Processing(format!("Failed to delete old document: {}", e)))?;

                // Delete pages by source_id (entry_id)
                let page_store = store.page_store()
                    .map_err(|e| JobError::Processing(format!("Failed to get page store: {}", e)))?;
                page_store.delete_pages_by_source(&doc_id).await
                    .map_err(|e| JobError::Processing(format!("Failed to delete old pages: {}", e)))?;
            }

            // Insert new document
            store.insert_documents(&[entry]).await
                .map_err(|e| JobError::Processing(format!("Failed to insert document: {}", e)))?;

            // Insert page and chunks together using two-layer storage
            store.store_page_with_chunks(&page, &chunks, &embeddings).await
                .map_err(|e| JobError::Processing(format!("Failed to store page with chunks: {}", e)))?;
        }

        // Find related patterns
        let related_patterns = if self.config.enable_linking {
            match self.linker.find_related_for_document(
                &doc_title,
                &doc_description,
                None,
                None,
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
            page_id = page_id,
            title = doc_title,
            chunks = chunks.len(),
            related = related_patterns.len(),
            "Successfully processed with two-layer storage"
        );

        Ok(TwoLayerResult {
            document_id: doc_id,
            page_id: Some(page_id),
            chunks_created: chunks.len(),
            embeddings_generated: embeddings.len(),
            related_patterns,
        })
    }

    /// Get the full page context for a chunk (for RAG enrichment).
    ///
    /// When a search returns chunks, use this to retrieve the full page
    /// content for better context in RAG applications.
    pub async fn get_page_context(&self, page_id: &str) -> Result<Option<PageRecord>, JobError> {
        let store = self.store.read().await;
        store.get_page_for_chunk(page_id).await
            .map_err(|e| JobError::Processing(format!("Failed to get page context: {}", e)))
    }
}

/// Result of processing with two-layer storage
#[derive(Debug, Clone)]
pub struct TwoLayerResult {
    /// Document ID
    pub document_id: String,
    /// Page ID (for context retrieval)
    pub page_id: Option<String>,
    /// Number of chunks created
    pub chunks_created: usize,
    /// Number of embeddings generated
    pub embeddings_generated: usize,
    /// Related patterns found via semantic similarity
    pub related_patterns: Vec<PatternLink>,
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
