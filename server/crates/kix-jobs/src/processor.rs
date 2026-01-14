//! Content processor for indexing documents

use std::path::Path;
use std::sync::Arc;

use tokio::sync::RwLock;
use tracing::{debug, info, warn};

use kix_embeddings::{
    ChunkingConfig, CodeBlockInput, DocumentChunker, OllamaEmbedder, EmbeddingConfig,
    HeaderInput, SmartChunkingInput,
};
use kix_parser::{Entry, EntryChunk, EntryType, PdfParser, SourceType};
use kix_store::{KixStore, PageRecord};
use kix_crawler::ContentExtractor;
use kix_sse::event::{CodeValidationStats, LanguageCount};

use crate::linker::{LinkingConfig, PatternLink, PatternLinker};
use crate::JobError;

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
    embedder: Arc<OllamaEmbedder>,
    linker: PatternLinker,
    chunker: DocumentChunker,
    pdf_parser: PdfParser,
    content_extractor: ContentExtractor,
    config: ProcessorConfig,
}

impl ContentProcessor {
    /// Create a new content processor with its own KixStore instance.
    ///
    /// Use this for standalone CLI commands that don't share state with other services.
    /// For API/MCP server contexts, prefer `with_shared_store()` to share a single store.
    pub async fn new(
        db_path: &str,
        config: ProcessorConfig,
    ) -> Result<Self, JobError> {
        info!("Initializing content processor with database at: {}", db_path);

        // Initialize store
        let mut store = KixStore::new(Path::new(db_path))
            .await
            .map_err(|e| JobError::Processing(format!("Failed to create store: {}", e)))?;

        store.init_tables().await
            .map_err(|e| JobError::Processing(format!("Failed to init tables: {}", e)))?;

        // Use RwLock for better concurrent read access
        let store = Arc::new(RwLock::new(store));

        Self::with_shared_store(store, config).await
    }

    /// Create a new content processor with a shared KixStore instance.
    ///
    /// This is the preferred constructor for API/MCP server contexts where
    /// multiple components (API handlers, job executor) need to share the same
    /// store instance for consistent data visibility and reduced memory usage.
    pub async fn with_shared_store(
        store: Arc<RwLock<KixStore>>,
        config: ProcessorConfig,
    ) -> Result<Self, JobError> {
        info!("Initializing content processor with shared store");

        // Create Ollama embedder with config
        let embedding_config = EmbeddingConfig::default()
            .with_batch_size(config.embedding_batch_size);

        let embedder = OllamaEmbedder::new(embedding_config)
            .map_err(|e| JobError::Processing(format!("Failed to create embedder: {}", e)))?;

        let embedder = Arc::new(embedder);

        info!(
            "Ollama embedder initialized: model={}, dimensions={}",
            embedder.model(),
            embedder.dimensions()
        );

        // Initialize chunker
        let chunker = DocumentChunker::new(ChunkingConfig {
            max_chunk_size: config.chunk_size,
            overlap_size: config.chunk_overlap,
            ..Default::default()
        });

        // Initialize pattern linker
        let linker = PatternLinker::new(
            store.clone(),
            embedder.clone(),
            config.linking.clone(),
        );

        Ok(Self {
            store,
            embedder,
            linker,
            chunker,
            pdf_parser: PdfParser::new(),
            content_extractor: ContentExtractor::default(),
            config,
        })
    }

    /// Generate embeddings for chunks using the Ollama embedder
    async fn embed_chunks(&self, chunks: &[EntryChunk]) -> Result<Vec<Vec<f32>>, JobError> {
        if chunks.is_empty() {
            return Ok(vec![]);
        }

        let texts: Vec<String> = chunks.iter().map(|c| c.text.clone()).collect();

        self.embedder
            .embed_batch(&texts)
            .await
            .map_err(|e| JobError::Processing(format!("Failed to generate embeddings: {}", e)))
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

        // Use smart chunking with code blocks indexed separately
        self.process_document_with_markdown(
            entry,
            &extracted.markdown,
            &extracted.code_blocks,
            &extracted.headers,
        ).await
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
    async fn process_document_with_markdown(
        &self,
        entry: Entry,
        markdown: &str,
        code_blocks: &[kix_crawler::ExtractedCodeBlock],
        headers: &[kix_crawler::ExtractedHeader],
    ) -> Result<ProcessingResult, JobError> {
        let doc_id = entry.id.clone();
        let doc_title = entry.title.clone();
        let doc_description = entry.description.clone();

        // Build SmartChunkingInput with code blocks for separate indexing
        let smart_input = SmartChunkingInput {
            markdown: markdown.to_string(),
            code_blocks: code_blocks
                .iter()
                .filter(|cb| !cb.is_inline)
                .map(|cb| CodeBlockInput {
                    language: cb.language.clone(),
                    content: cb.content.clone(),
                })
                .collect(),
            headers: headers
                .iter()
                .map(|h| HeaderInput {
                    level: h.level,
                    text: h.text.clone(),
                })
                .collect(),
            plain_text: String::new(),
        };

        // Use smart chunking
        let chunks = self.chunker.chunk_smart(&entry, &smart_input);

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

        // Generate embeddings
        let embeddings = self.embed_chunks(&chunks).await?;

        // Store document and chunks
        {
            let store = self.store.write().await;

            let exists = store.document_exists(&doc_id).await
                .map_err(|e| JobError::Processing(format!("Failed to check document: {}", e)))?;

            if exists {
                store.delete_chunks_by_document(&doc_id).await
                    .map_err(|e| JobError::Processing(format!("Failed to delete old chunks: {}", e)))?;
                store.delete_document(&doc_id).await
                    .map_err(|e| JobError::Processing(format!("Failed to delete old document: {}", e)))?;
            }

            store.insert_documents_from_entries(&[entry]).await
                .map_err(|e| JobError::Processing(format!("Failed to insert document: {}", e)))?;

            store.insert_chunks(&chunks, &embeddings)
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
                    info!(doc_id = doc_id, related_count = links.len(), "Found related patterns");
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

        let content = tokio::fs::read(file_path)
            .await
            .map_err(|e| JobError::Processing(format!("Failed to read file: {}", e)))?;

        let document = match extension.as_str() {
            "html" | "htm" => {
                let text = String::from_utf8_lossy(&content);
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

    /// Process a file with two-layer storage pattern.
    pub async fn process_file_with_page(
        &self,
        file_path: &Path,
        original_name: &str,
    ) -> Result<TwoLayerResult, JobError> {
        debug!(path = ?file_path, name = original_name, "Processing file with two-layer storage");

        let extension = file_path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();

        let content = tokio::fs::read(file_path)
            .await
            .map_err(|e| JobError::Processing(format!("Failed to read file: {}", e)))?;

        let (document, markdown_content, code_blocks, headers) = match extension.as_str() {
            "html" | "htm" => {
                let text = String::from_utf8_lossy(&content);
                let url = url::Url::parse(&format!("file://{}", original_name))
                    .unwrap_or_else(|_| url::Url::parse("file:///unknown").unwrap());
                let extracted = self.content_extractor.extract(&text, &url);
                let entry = self.create_entry_from_extracted(&extracted, original_name)?;
                (entry, extracted.markdown, extracted.code_blocks, extracted.headers)
            }
            "pdf" => {
                let entry = self.pdf_parser
                    .parse(file_path.to_str().unwrap_or(""))
                    .map_err(|e| JobError::Processing(format!("Failed to parse PDF: {}", e)))?;
                let markdown = entry.content.clone();
                (entry, markdown, vec![], vec![])
            }
            "txt" | "md" | "markdown" => {
                let text = String::from_utf8_lossy(&content).to_string();
                let entry = self.create_text_document(&text, original_name, file_path)?;
                (entry, text, vec![], vec![])
            }
            "json" => {
                let text = String::from_utf8_lossy(&content).to_string();
                let entry = self.create_text_document(&text, original_name, file_path)?;
                (entry, text, vec![], vec![])
            }
            _ => {
                if let Ok(text) = String::from_utf8(content.clone()) {
                    let entry = self.create_text_document(&text, original_name, file_path)?;
                    (entry, text, vec![], vec![])
                } else {
                    return Err(JobError::Processing(format!(
                        "Unsupported file type: {}",
                        extension
                    )));
                }
            }
        };

        let entry_id = document.id.clone();
        let source_url = format!("file://{}", original_name);

        let page = PageRecord::new(&entry_id, &source_url, &markdown_content)
            .with_title(document.title.clone());

        self.process_document_with_page(
            document,
            &markdown_content,
            page,
            &code_blocks,
            &headers,
        ).await
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

        info!(doc_id = doc_id, chunks = chunks.len(), "Generated chunks from document");

        let embeddings = self.embed_chunks(&chunks).await?;

        {
            let store = self.store.write().await;

            let exists = store.document_exists(&doc_id).await
                .map_err(|e| JobError::Processing(format!("Failed to check document: {}", e)))?;

            if exists {
                store.delete_chunks_by_document(&doc_id).await
                    .map_err(|e| JobError::Processing(format!("Failed to delete old chunks: {}", e)))?;
                store.delete_document(&doc_id).await
                    .map_err(|e| JobError::Processing(format!("Failed to delete old document: {}", e)))?;
            }

            store.insert_documents_from_entries(&[entry]).await
                .map_err(|e| JobError::Processing(format!("Failed to insert document: {}", e)))?;

            store.insert_chunks(&chunks, &embeddings)
                .map_err(|e| JobError::Processing(format!("Failed to insert chunks: {}", e)))?;
        }

        let related_patterns = if self.config.enable_linking {
            match self.linker.find_related_for_document(
                &doc_title,
                &doc_description,
                None,
                None,
                Some(&doc_id),
            ).await {
                Ok(links) => {
                    info!(doc_id = doc_id, related_count = links.len(), "Found related patterns");
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

    /// Get store reference for direct access
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

    /// Get document count
    pub async fn document_count(&self) -> Result<usize, JobError> {
        let store = self.store.read().await;
        store.document_count().await
            .map_err(|e| JobError::Processing(format!("Failed to get document count: {}", e)))
    }

    /// Get chunk count
    pub async fn chunk_count(&self) -> Result<usize, JobError> {
        let store = self.store.read().await;
        store.chunk_count()
            .map_err(|e| JobError::Processing(format!("Failed to get chunk count: {}", e)))
    }

    /// Get page count
    pub async fn page_count(&self) -> Result<usize, JobError> {
        let store = self.store.read().await;
        store.page_count().await
            .map_err(|e| JobError::Processing(format!("Failed to get page count: {}", e)))
    }

    // ========================================================================
    // Two-Layer Storage Integration
    // ========================================================================

    /// Process HTML content using two-layer storage pattern for RAG.
    pub async fn process_html_with_page(
        &self,
        content: &str,
        source_url: &str,
        crawl_time_ms: Option<u64>,
        title: Option<String>,
    ) -> Result<TwoLayerResult, JobError> {
        debug!(url = source_url, "Processing HTML with two-layer storage using ContentExtractor");

        let url = url::Url::parse(source_url)
            .unwrap_or_else(|_| url::Url::parse("http://localhost").unwrap());

        let mut extracted = self.content_extractor.extract(content, &url);

        if let Some(t) = title {
            if !t.is_empty() {
                extracted.title = t;
            }
        }

        info!(
            url = source_url,
            title = %extracted.title,
            markdown_len = extracted.markdown.len(),
            header_count = extracted.headers.len(),
            code_blocks = extracted.code_blocks.len(),
            word_count = extracted.word_count,
            "ContentExtractor extraction complete for two-layer storage"
        );

        let entry = self.create_entry_from_extracted(&extracted, source_url)?;
        let entry_id = entry.id.clone();

        let page = PageRecord::new(&entry_id, source_url, &extracted.markdown)
            .with_title(extracted.title.clone());

        let page = if let Some(ms) = crawl_time_ms {
            page.with_crawl_time(ms)
        } else {
            page
        };

        self.process_document_with_page(
            entry,
            &extracted.markdown,
            page,
            &extracted.code_blocks,
            &extracted.headers,
        ).await
    }

    /// Process a document with two-layer storage pattern.
    async fn process_document_with_page(
        &self,
        entry: Entry,
        markdown: &str,
        page: PageRecord,
        code_blocks: &[kix_crawler::ExtractedCodeBlock],
        headers: &[kix_crawler::ExtractedHeader],
    ) -> Result<TwoLayerResult, JobError> {
        let doc_id = entry.id.clone();
        let doc_title = entry.title.clone();
        let doc_description = entry.description.clone();
        let page_id = page.page_id.clone();

        let smart_input = SmartChunkingInput {
            markdown: markdown.to_string(),
            code_blocks: code_blocks
                .iter()
                .filter(|cb| !cb.is_inline)
                .map(|cb| CodeBlockInput {
                    language: cb.language.clone(),
                    content: cb.content.clone(),
                })
                .collect(),
            headers: headers
                .iter()
                .map(|h| HeaderInput {
                    level: h.level,
                    text: h.text.clone(),
                })
                .collect(),
            plain_text: String::new(),
        };

        let mut chunks = self.chunker.chunk_smart(&entry, &smart_input);

        // Build code extraction stats from the code_blocks
        let code_extraction = Self::build_code_extraction_stats(code_blocks);

        if chunks.is_empty() {
            warn!(doc_id = doc_id, "No chunks generated from document");
            return Ok(TwoLayerResult {
                document_id: doc_id,
                page_id: Some(page_id),
                chunks_created: 0,
                embeddings_generated: 0,
                related_patterns: vec![],
                code_extraction,
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

        let embeddings = self.embed_chunks(&chunks).await?;

        {
            let store = self.store.write().await;

            let exists = store.document_exists(&doc_id).await
                .map_err(|e| JobError::Processing(format!("Failed to check document: {}", e)))?;

            if exists {
                store.delete_chunks_by_document(&doc_id).await
                    .map_err(|e| JobError::Processing(format!("Failed to delete old chunks: {}", e)))?;
                store.delete_document(&doc_id).await
                    .map_err(|e| JobError::Processing(format!("Failed to delete old document: {}", e)))?;
                store.delete_pages_by_source(&doc_id).await
                    .map_err(|e| JobError::Processing(format!("Failed to delete old pages: {}", e)))?;
            }

            store.insert_documents_from_entries(&[entry]).await
                .map_err(|e| JobError::Processing(format!("Failed to insert document: {}", e)))?;

            store.store_page_with_chunks(&page, &chunks, &embeddings).await
                .map_err(|e| JobError::Processing(format!("Failed to store page with chunks: {}", e)))?;
        }

        let related_patterns = if self.config.enable_linking {
            match self.linker.find_related_for_document(
                &doc_title,
                &doc_description,
                None,
                None,
                Some(&doc_id),
            ).await {
                Ok(links) => {
                    info!(doc_id = doc_id, related_count = links.len(), "Found related patterns");
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
            code_extraction,
        })
    }

    /// Build code extraction statistics from extracted code blocks
    fn build_code_extraction_stats(
        code_blocks: &[kix_crawler::ExtractedCodeBlock],
    ) -> Option<CodeExtractionResult> {
        // Only non-inline code blocks count
        let blocks: Vec<_> = code_blocks.iter().filter(|cb| !cb.is_inline).collect();

        if blocks.is_empty() {
            return None;
        }

        // Count languages
        let mut language_counts: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
        for block in &blocks {
            let lang = block.language.clone().unwrap_or_else(|| "unknown".to_string());
            *language_counts.entry(lang).or_insert(0) += 1;
        }

        // Convert to LanguageCount vec, sorted by count desc
        let mut languages: Vec<LanguageCount> = language_counts
            .into_iter()
            .map(|(language, count)| LanguageCount { language, count })
            .collect();
        languages.sort_by(|a, b| b.count.cmp(&a.count));

        // For now, we don't have pattern matching info from ContentExtractor
        // This would need to come from the CodeExtractor if we use it
        let patterns_matched = vec!["pre>code".to_string()]; // Default pattern

        // All blocks from ContentExtractor pass validation (it doesn't do validation)
        let validation_stats = CodeValidationStats {
            total_extracted: blocks.len(),
            passed_validation: blocks.len(),
            rejected_too_short: 0,
            rejected_placeholder: 0,
            rejected_no_structure: 0,
            rejected_high_prose: 0,
        };

        Some(CodeExtractionResult {
            blocks_found: blocks.len(),
            patterns_matched,
            languages,
            validation_stats,
        })
    }

    /// Get the full page context for a chunk (for RAG enrichment).
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
    /// Code extraction statistics for SSE visibility
    pub code_extraction: Option<CodeExtractionResult>,
}

/// Code extraction result for SSE visibility
#[derive(Debug, Clone)]
pub struct CodeExtractionResult {
    /// Total code blocks found
    pub blocks_found: usize,
    /// Patterns that matched during extraction
    pub patterns_matched: Vec<String>,
    /// Language breakdown
    pub languages: Vec<LanguageCount>,
    /// Validation statistics
    pub validation_stats: CodeValidationStats,
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
