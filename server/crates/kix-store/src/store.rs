//! Main Knowledge Indexer store implementation using LanceDB.

use arrow_array::{
    ArrayRef, FixedSizeListArray, Float32Array, RecordBatch, RecordBatchIterator, StringArray,
    UInt32Array,
};
use arrow_schema::{ArrowError, DataType, Field, Schema};
use lancedb::index::scalar::FtsIndexBuilder;
use lancedb::index::vector::IvfHnswSqIndexBuilder;
use lancedb::index::Index;
use lancedb::{connect, Connection, Table};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tracing::{info, warn};

use kix_parser::{Entry, EntryChunk};

use crate::error::StoreError;
use crate::pages::{PageRecord, PageStore};
use crate::schema::page_schema;

/// Default embedding dimensions (768 for bge-base-en-v1.5).
/// Can be overridden via KIX_EMBEDDING_DIM environment variable.
pub const DEFAULT_EMBEDDING_DIM: i32 = 768;

/// Returns the configured embedding dimensions.
///
/// Reads from KIX_EMBEDDING_DIM environment variable, or uses default (768).
/// Supported models:
/// - bge-base-en-v1.5: 768 dimensions (default, recommended)
/// - bge-small-en-v1.5: 384 dimensions
/// - bge-large-en-v1.5: 1024 dimensions
/// - all-MiniLM-L6-v2: 384 dimensions (legacy)
pub fn get_embedding_dim() -> i32 {
    std::env::var("KIX_EMBEDDING_DIM")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or_else(|| {
            // Auto-detect from model name if set
            if let Ok(model) = std::env::var("KIX_EMBEDDING_MODEL") {
                match model.to_lowercase().as_str() {
                    s if s.contains("large") => 1024,
                    s if s.contains("base") => 768,
                    s if s.contains("small") || s.contains("minilm") => 384,
                    _ => DEFAULT_EMBEDDING_DIM,
                }
            } else {
                DEFAULT_EMBEDDING_DIM
            }
        })
}

/// The main Knowledge Indexer store for entry and chunk storage.
pub struct KixStore {
    db: Connection,
    entries_table: Option<Table>,
    chunks_table: Option<Table>,
    /// Pages table for two-layer storage (RAG context retrieval)
    pages_table: Option<Table>,
    /// Flag to track if indexes have been created
    indexes_created: AtomicBool,
    /// Embedding dimensions for this store
    embedding_dim: i32,
}

impl KixStore {
    /// Creates a new Knowledge Indexer store at the given path.
    pub async fn new(db_path: &str) -> Result<Self, StoreError> {
        Self::new_with_dim(db_path, get_embedding_dim()).await
    }

    /// Creates a new Knowledge Indexer store with specified embedding dimensions.
    pub async fn new_with_dim(db_path: &str, embedding_dim: i32) -> Result<Self, StoreError> {
        info!("Connecting to LanceDB at: {} (embedding_dim={})", db_path, embedding_dim);

        let db = connect(db_path)
            .execute()
            .await
            .map_err(|e| StoreError::Database(e.to_string()))?;

        Ok(Self {
            db,
            entries_table: None,
            chunks_table: None,
            pages_table: None,
            indexes_created: AtomicBool::new(false),
            embedding_dim,
        })
    }

    /// Returns the embedding dimensions for this store.
    pub fn embedding_dim(&self) -> i32 {
        self.embedding_dim
    }

    /// Initializes tables, creating them if they don't exist.
    pub async fn init_tables(&mut self) -> Result<(), StoreError> {
        // Try to open existing tables or create new ones
        self.entries_table = Some(self.get_or_create_entries_table().await?);
        self.chunks_table = Some(self.get_or_create_chunks_table().await?);
        self.pages_table = Some(self.get_or_create_pages_table().await?);

        info!("Tables initialized successfully (entries, chunks, pages)");
        Ok(())
    }

    /// Gets or creates the entries table.
    async fn get_or_create_entries_table(&self) -> Result<Table, StoreError> {
        let table_names = self
            .db
            .table_names()
            .execute()
            .await
            .map_err(|e| StoreError::Database(e.to_string()))?;

        if table_names.contains(&"entries".to_string()) {
            info!("Opening existing entries table");
            self.db
                .open_table("entries")
                .execute()
                .await
                .map_err(|e| StoreError::Database(e.to_string()))
        } else {
            info!("Creating new entries table");
            let schema = Self::entries_schema();
            self.db
                .create_empty_table("entries", schema)
                .execute()
                .await
                .map_err(|e| StoreError::Database(e.to_string()))
        }
    }

    /// Gets or creates the chunks table.
    ///
    /// Includes schema migration: if the existing table is missing the `page_id` field
    /// (added for two-layer storage), the table is dropped and recreated.
    async fn get_or_create_chunks_table(&self) -> Result<Table, StoreError> {
        let table_names = self
            .db
            .table_names()
            .execute()
            .await
            .map_err(|e| StoreError::Database(e.to_string()))?;

        if table_names.contains(&"chunks".to_string()) {
            info!("Opening existing chunks table");
            let table = self.db
                .open_table("chunks")
                .execute()
                .await
                .map_err(|e| StoreError::Database(e.to_string()))?;

            // Check if schema has page_id field (required for two-layer storage)
            let schema = table.schema().await
                .map_err(|e| StoreError::Database(format!("Failed to get chunks schema: {}", e)))?;

            let has_page_id = schema.fields.iter().any(|f| f.name() == "page_id");

            if !has_page_id {
                warn!("Chunks table missing page_id field, migrating to new schema...");
                // Drop old table and recreate with new schema
                self.db
                    .drop_table("chunks", &[])
                    .await
                    .map_err(|e| StoreError::Database(format!("Failed to drop old chunks table: {}", e)))?;

                info!("Creating new chunks table with page_id field (embedding_dim={})", self.embedding_dim);
                let schema = Self::chunks_schema_with_dim(self.embedding_dim);
                return self.db
                    .create_empty_table("chunks", schema)
                    .execute()
                    .await
                    .map_err(|e| StoreError::Database(e.to_string()));
            }

            Ok(table)
        } else {
            info!("Creating new chunks table (embedding_dim={})", self.embedding_dim);
            let schema = Self::chunks_schema_with_dim(self.embedding_dim);
            self.db
                .create_empty_table("chunks", schema)
                .execute()
                .await
                .map_err(|e| StoreError::Database(e.to_string()))
        }
    }

    /// Gets or creates the pages table (two-layer storage for RAG context).
    async fn get_or_create_pages_table(&self) -> Result<Table, StoreError> {
        let table_names = self
            .db
            .table_names()
            .execute()
            .await
            .map_err(|e| StoreError::Database(e.to_string()))?;

        if table_names.contains(&"pages".to_string()) {
            info!("Opening existing pages table");
            self.db
                .open_table("pages")
                .execute()
                .await
                .map_err(|e| StoreError::Database(e.to_string()))
        } else {
            info!("Creating new pages table");
            self.db
                .create_empty_table("pages", page_schema())
                .execute()
                .await
                .map_err(|e| StoreError::Database(e.to_string()))
        }
    }

    /// Returns the schema for the entries table.
    fn entries_schema() -> Arc<Schema> {
        Arc::new(Schema::new(vec![
            Field::new("id", DataType::Utf8, false),
            Field::new("title", DataType::Utf8, false),
            Field::new("description", DataType::Utf8, true),
            Field::new("content", DataType::Utf8, true),
            Field::new("tags", DataType::Utf8, true),            // JSON array as string
            Field::new("collection_ids", DataType::Utf8, true),  // JSON array as string
            Field::new("entry_type", DataType::Utf8, false),
            Field::new("source_type", DataType::Utf8, false),
            Field::new("source_path", DataType::Utf8, false),
            Field::new("slug", DataType::Utf8, false),
            Field::new("source_hash", DataType::Utf8, false),
            Field::new("created_at", DataType::Utf8, false),
            Field::new("updated_at", DataType::Utf8, false),
        ]))
    }

    /// Returns the schema for the chunks table with specified embedding dimensions.
    fn chunks_schema_with_dim(embedding_dim: i32) -> Arc<Schema> {
        Arc::new(Schema::new(vec![
            Field::new("chunk_id", DataType::Utf8, false),
            Field::new("entry_id", DataType::Utf8, false),
            Field::new("page_id", DataType::Utf8, true),  // FK to pages table (two-layer storage)
            Field::new("chunk_index", DataType::UInt32, false),
            Field::new("chunk_type", DataType::Utf8, false),
            Field::new("text", DataType::Utf8, false),
            Field::new("entry_title", DataType::Utf8, false),
            Field::new("entry_type", DataType::Utf8, false),
            Field::new("tags", DataType::Utf8, true),  // JSON array as string
            Field::new(
                "vector",
                DataType::FixedSizeList(
                    Arc::new(Field::new("item", DataType::Float32, true)),
                    embedding_dim,
                ),
                false,
            ),
        ]))
    }

    /// Inserts entries into the store.
    pub async fn insert_entries(&self, entries: &[Entry]) -> Result<(), StoreError> {
        let table = self
            .entries_table
            .as_ref()
            .ok_or_else(|| StoreError::Database("Entries table not initialized".to_string()))?;

        if entries.is_empty() {
            return Ok(());
        }

        let batch = Self::entries_to_batch(entries)?;
        let schema = batch.schema();
        let batches: Vec<Result<RecordBatch, ArrowError>> = vec![Ok(batch)];
        let reader = RecordBatchIterator::new(batches, schema);

        table
            .add(Box::new(reader))
            .execute()
            .await
            .map_err(|e| StoreError::Database(e.to_string()))?;

        info!("Inserted {} entries", entries.len());
        Ok(())
    }

    /// Converts entries to a RecordBatch.
    fn entries_to_batch(entries: &[Entry]) -> Result<RecordBatch, StoreError> {
        let ids: Vec<&str> = entries.iter().map(|e| e.id.as_str()).collect();
        let titles: Vec<&str> = entries.iter().map(|e| e.title.as_str()).collect();
        let descriptions: Vec<&str> = entries.iter().map(|e| e.description.as_str()).collect();
        let contents: Vec<&str> = entries.iter().map(|e| e.content.as_str()).collect();
        let tags: Vec<String> = entries
            .iter()
            .map(|e| serde_json::to_string(&e.tags).unwrap_or_default())
            .collect();
        let collection_ids: Vec<String> = entries
            .iter()
            .map(|e| serde_json::to_string(&e.collection_ids).unwrap_or_default())
            .collect();
        let entry_types: Vec<&str> = entries
            .iter()
            .map(|e| e.entry_type.as_str())
            .collect();
        let source_types: Vec<String> = entries
            .iter()
            .map(|e| e.source_type.to_string())
            .collect();
        let source_paths: Vec<&str> = entries.iter().map(|e| e.source_path.as_str()).collect();
        let slugs: Vec<&str> = entries.iter().map(|e| e.slug.as_str()).collect();
        let hashes: Vec<&str> = entries.iter().map(|e| e.source_hash.as_str()).collect();
        let created_ats: Vec<String> = entries
            .iter()
            .map(|e| e.created_at.to_rfc3339())
            .collect();
        let updated_ats: Vec<String> = entries
            .iter()
            .map(|e| e.updated_at.to_rfc3339())
            .collect();

        let columns: Vec<ArrayRef> = vec![
            Arc::new(StringArray::from(ids)),
            Arc::new(StringArray::from(titles)),
            Arc::new(StringArray::from(descriptions)),
            Arc::new(StringArray::from(contents)),
            Arc::new(StringArray::from(tags.iter().map(|s| s.as_str()).collect::<Vec<_>>())),
            Arc::new(StringArray::from(collection_ids.iter().map(|s| s.as_str()).collect::<Vec<_>>())),
            Arc::new(StringArray::from(entry_types)),
            Arc::new(StringArray::from(source_types.iter().map(|s| s.as_str()).collect::<Vec<_>>())),
            Arc::new(StringArray::from(source_paths)),
            Arc::new(StringArray::from(slugs)),
            Arc::new(StringArray::from(hashes)),
            Arc::new(StringArray::from(created_ats.iter().map(|s| s.as_str()).collect::<Vec<_>>())),
            Arc::new(StringArray::from(updated_ats.iter().map(|s| s.as_str()).collect::<Vec<_>>())),
        ];

        RecordBatch::try_new(Self::entries_schema(), columns)
            .map_err(|e| StoreError::Serialization(e.to_string()))
    }

    /// Inserts chunks with embeddings into the store.
    pub async fn insert_chunks(
        &self,
        chunks: &[EntryChunk],
        embeddings: &[Vec<f32>],
    ) -> Result<(), StoreError> {
        let table = self
            .chunks_table
            .as_ref()
            .ok_or_else(|| StoreError::Database("Chunks table not initialized".to_string()))?;

        if chunks.is_empty() || embeddings.is_empty() {
            return Ok(());
        }

        if chunks.len() != embeddings.len() {
            return Err(StoreError::Serialization(format!(
                "Chunks ({}) and embeddings ({}) count mismatch",
                chunks.len(),
                embeddings.len()
            )));
        }

        let batch = Self::chunks_to_batch(chunks, embeddings, self.embedding_dim)?;
        let schema = batch.schema();
        let batches: Vec<Result<RecordBatch, ArrowError>> = vec![Ok(batch)];
        let reader = RecordBatchIterator::new(batches, schema);

        table
            .add(Box::new(reader))
            .execute()
            .await
            .map_err(|e| StoreError::Database(e.to_string()))?;

        info!("Inserted {} chunks with embeddings", chunks.len());

        // Create indexes on first insertion
        self.ensure_indexes().await;

        Ok(())
    }

    /// Ensures indexes are created (called after first data insertion).
    async fn ensure_indexes(&self) {
        // Use compare_exchange to ensure only one thread creates indexes
        if self
            .indexes_created
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok()
        {
            info!("Creating indexes on first data insertion...");
            if let Err(e) = self.create_indexes().await {
                warn!("Index creation had issues: {}", e);
                // Reset flag so we try again next time
                self.indexes_created.store(false, Ordering::SeqCst);
            }
        }
    }

    /// Converts chunks and embeddings to a RecordBatch.
    fn chunks_to_batch(
        chunks: &[EntryChunk],
        embeddings: &[Vec<f32>],
        embedding_dim: i32,
    ) -> Result<RecordBatch, StoreError> {
        let chunk_ids: Vec<&str> = chunks.iter().map(|c| c.chunk_id.as_str()).collect();
        let entry_ids: Vec<&str> = chunks.iter().map(|c| c.entry_id.as_str()).collect();
        let page_ids: Vec<Option<&str>> = chunks
            .iter()
            .map(|c| c.page_id.as_deref())
            .collect();
        let indices: Vec<u32> = chunks.iter().map(|c| c.chunk_index).collect();
        let chunk_types: Vec<&str> = chunks.iter().map(|c| c.chunk_type.as_str()).collect();
        let texts: Vec<&str> = chunks.iter().map(|c| c.text.as_str()).collect();
        let entry_titles: Vec<&str> = chunks
            .iter()
            .map(|c| c.metadata.entry_title.as_str())
            .collect();
        let entry_types: Vec<&str> = chunks
            .iter()
            .map(|c| c.metadata.entry_type.as_str())
            .collect();
        let tags: Vec<String> = chunks
            .iter()
            .map(|c| serde_json::to_string(&c.metadata.tags).unwrap_or_default())
            .collect();

        // Create fixed size list array for vectors
        let flat_vectors: Vec<f32> = embeddings.iter().flatten().copied().collect();
        let values = Arc::new(Float32Array::from(flat_vectors)) as ArrayRef;
        let field = Arc::new(Field::new("item", DataType::Float32, true));
        let vector_array = FixedSizeListArray::try_new(field, embedding_dim, values, None)
            .map_err(|e| StoreError::Serialization(e.to_string()))?;

        let columns: Vec<ArrayRef> = vec![
            Arc::new(StringArray::from(chunk_ids)),
            Arc::new(StringArray::from(entry_ids)),
            Arc::new(StringArray::from(page_ids)),
            Arc::new(UInt32Array::from(indices)),
            Arc::new(StringArray::from(chunk_types)),
            Arc::new(StringArray::from(texts)),
            Arc::new(StringArray::from(entry_titles)),
            Arc::new(StringArray::from(entry_types)),
            Arc::new(StringArray::from(tags.iter().map(|s| s.as_str()).collect::<Vec<_>>())),
            Arc::new(vector_array),
        ];

        RecordBatch::try_new(Self::chunks_schema_with_dim(embedding_dim), columns)
            .map_err(|e| StoreError::Serialization(e.to_string()))
    }

    /// Creates indexes for efficient search.
    /// Uses IVF-HNSW-SQ index for better performance than default IVF-PQ.
    pub async fn create_indexes(&self) -> Result<(), StoreError> {
        let chunks_table = self
            .chunks_table
            .as_ref()
            .ok_or_else(|| StoreError::Database("Chunks table not initialized".to_string()))?;

        // Get row count to determine optimal index parameters
        let row_count = chunks_table
            .count_rows(None)
            .await
            .unwrap_or(1000);

        // Create optimized vector index using IVF-HNSW-SQ
        // - IVF partitions data for faster search on large datasets
        // - HNSW provides high recall with low latency
        // - SQ (Scalar Quantization) reduces memory while maintaining accuracy
        info!("Creating optimized IVF-HNSW-SQ vector index on chunks table...");

        // Calculate optimal number of IVF partitions based on dataset size
        // Rule of thumb: sqrt(n) partitions, but at least 8 and at most 1024
        let num_partitions = ((row_count as f64).sqrt() as u32).clamp(8, 1024);

        let index_builder = IvfHnswSqIndexBuilder::default()
            // Number of IVF partitions - more = faster search, fewer = better recall
            .num_partitions(num_partitions)
            // HNSW parameters for the graph within each partition
            // num_edges = max connections per node (higher = better recall, more memory)
            .num_edges(32)
            // ef_construction = search width during build (higher = better quality, slower build)
            .ef_construction(200);

        match chunks_table
            .create_index(&["vector"], Index::IvfHnswSq(index_builder))
            .execute()
            .await
        {
            Ok(_) => info!(
                "IVF-HNSW-SQ vector index created successfully (partitions: {}, m: 32, ef_construction: 200)",
                num_partitions
            ),
            Err(e) => warn!("Vector index creation skipped or failed: {}", e),
        }

        // Create FTS index on text
        info!("Creating FTS index on text column...");
        match chunks_table
            .create_index(&["text"], Index::FTS(FtsIndexBuilder::default()))
            .execute()
            .await
        {
            Ok(_) => info!("FTS index created successfully"),
            Err(e) => warn!("FTS index creation skipped or failed: {}", e),
        }

        // Create FTS index on entry_title
        info!("Creating FTS index on entry_title column...");
        match chunks_table
            .create_index(&["entry_title"], Index::FTS(FtsIndexBuilder::default()))
            .execute()
            .await
        {
            Ok(_) => info!("Entry title FTS index created successfully"),
            Err(e) => warn!("Entry title FTS index creation skipped or failed: {}", e),
        }

        Ok(())
    }

    /// Clears all data from the tables.
    pub async fn clear_tables(&mut self) -> Result<(), StoreError> {
        info!("Clearing all tables...");

        // Drop and recreate tables
        let table_names = self
            .db
            .table_names()
            .execute()
            .await
            .map_err(|e| StoreError::Database(e.to_string()))?;

        for name in ["entries", "chunks", "pages"] {
            if table_names.contains(&name.to_string()) {
                self.db
                    .drop_table(name, &[])
                    .await
                    .map_err(|e| StoreError::Database(e.to_string()))?;
            }
        }

        // Reinitialize tables
        self.init_tables().await?;

        info!("Tables cleared successfully");
        Ok(())
    }

    /// Returns the entries table reference.
    pub fn entries_table(&self) -> Option<&Table> {
        self.entries_table.as_ref()
    }

    /// Returns the chunks table reference.
    pub fn chunks_table(&self) -> Option<&Table> {
        self.chunks_table.as_ref()
    }

    /// Returns the pages table reference (two-layer storage for RAG context).
    pub fn pages_table(&self) -> Option<&Table> {
        self.pages_table.as_ref()
    }

    /// Gets entry count.
    pub async fn entry_count(&self) -> Result<usize, StoreError> {
        let table = self
            .entries_table
            .as_ref()
            .ok_or_else(|| StoreError::Database("Entries table not initialized".to_string()))?;

        let count = table
            .count_rows(None)
            .await
            .map_err(|e| StoreError::Database(e.to_string()))?;

        Ok(count)
    }

    /// Gets chunk count.
    pub async fn chunk_count(&self) -> Result<usize, StoreError> {
        let table = self
            .chunks_table
            .as_ref()
            .ok_or_else(|| StoreError::Database("Chunks table not initialized".to_string()))?;

        let count = table
            .count_rows(None)
            .await
            .map_err(|e| StoreError::Database(e.to_string()))?;

        Ok(count)
    }

    /// Deletes an entry by ID.
    pub async fn delete_entry(&self, id: &str) -> Result<(), StoreError> {
        let table = self
            .entries_table
            .as_ref()
            .ok_or_else(|| StoreError::Database("Entries table not initialized".to_string()))?;

        let filter = format!("id = '{}'", id.replace('\'', "''"));
        table
            .delete(&filter)
            .await
            .map_err(|e| StoreError::Database(e.to_string()))?;

        info!("Deleted entry: {}", id);
        Ok(())
    }

    /// Deletes all chunks belonging to an entry.
    pub async fn delete_chunks_by_entry(&self, entry_id: &str) -> Result<(), StoreError> {
        let table = self
            .chunks_table
            .as_ref()
            .ok_or_else(|| StoreError::Database("Chunks table not initialized".to_string()))?;

        let filter = format!("entry_id = '{}'", entry_id.replace('\'', "''"));
        table
            .delete(&filter)
            .await
            .map_err(|e| StoreError::Database(e.to_string()))?;

        info!("Deleted chunks for entry: {}", entry_id);
        Ok(())
    }

    /// Checks if an entry exists by ID.
    pub async fn entry_exists(&self, id: &str) -> Result<bool, StoreError> {
        let table = self
            .entries_table
            .as_ref()
            .ok_or_else(|| StoreError::Database("Entries table not initialized".to_string()))?;

        let filter = format!("id = '{}'", id.replace('\'', "''"));
        let count = table
            .count_rows(Some(filter))
            .await
            .map_err(|e| StoreError::Database(e.to_string()))?;

        Ok(count > 0)
    }

    // ========================================================================
    // Two-Layer Storage Methods
    // ========================================================================

    /// Stores a page and its associated chunks atomically.
    ///
    /// This is the primary method for two-layer storage pattern:
    /// - Pages store full content for RAG context retrieval
    /// - Chunks store smaller pieces with embeddings for vector search
    pub async fn store_page_with_chunks(
        &self,
        page: &PageRecord,
        chunks: &[EntryChunk],
        embeddings: &[Vec<f32>],
    ) -> Result<(), StoreError> {
        // Store page first
        let pages_table = self
            .pages_table
            .as_ref()
            .ok_or_else(|| StoreError::Database("Pages table not initialized".to_string()))?;

        let page_store = PageStore::new(pages_table.clone());
        page_store.insert_pages(&[page.clone()]).await?;

        // Store chunks with page_id reference
        self.insert_chunks(chunks, embeddings).await?;

        info!(
            "Stored page {} with {} chunks",
            page.page_id,
            chunks.len()
        );
        Ok(())
    }

    /// Retrieves the full page context for a given chunk.
    ///
    /// Used for RAG context retrieval - when a vector search finds a relevant
    /// chunk, use this to get the full page content for better context.
    pub async fn get_page_for_chunk(&self, page_id: &str) -> Result<Option<PageRecord>, StoreError> {
        let pages_table = self
            .pages_table
            .as_ref()
            .ok_or_else(|| StoreError::Database("Pages table not initialized".to_string()))?;

        let page_store = PageStore::new(pages_table.clone());
        page_store.get_page(page_id).await
    }

    /// Gets the PageStore for direct page operations.
    pub fn page_store(&self) -> Result<PageStore, StoreError> {
        let pages_table = self
            .pages_table
            .as_ref()
            .ok_or_else(|| StoreError::Database("Pages table not initialized".to_string()))?;

        Ok(PageStore::new(pages_table.clone()))
    }

    /// Gets page count.
    pub async fn page_count(&self) -> Result<usize, StoreError> {
        let table = self
            .pages_table
            .as_ref()
            .ok_or_else(|| StoreError::Database("Pages table not initialized".to_string()))?;

        table
            .count_rows(None)
            .await
            .map_err(|e| StoreError::Database(e.to_string()))
    }

    // Backward compatibility aliases
    /// Alias for insert_entries (backward compatibility).
    pub async fn insert_documents(&self, entries: &[Entry]) -> Result<(), StoreError> {
        self.insert_entries(entries).await
    }

    /// Alias for entries_table (backward compatibility).
    pub fn documents_table(&self) -> Option<&Table> {
        self.entries_table()
    }

    /// Alias for entry_count (backward compatibility).
    pub async fn document_count(&self) -> Result<usize, StoreError> {
        self.entry_count().await
    }

    /// Alias for delete_entry (backward compatibility).
    pub async fn delete_document(&self, id: &str) -> Result<(), StoreError> {
        self.delete_entry(id).await
    }

    /// Alias for delete_chunks_by_entry (backward compatibility).
    pub async fn delete_chunks_by_document(&self, entry_id: &str) -> Result<(), StoreError> {
        self.delete_chunks_by_entry(entry_id).await
    }

    /// Alias for entry_exists (backward compatibility).
    pub async fn document_exists(&self, id: &str) -> Result<bool, StoreError> {
        self.entry_exists(id).await
    }
}

/// Backward compatibility alias for KixStore.
pub type EipStore = KixStore;
