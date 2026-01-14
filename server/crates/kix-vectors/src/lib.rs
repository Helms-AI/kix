//! KIX Vector Store
//!
//! Vector storage using SQLite + sqlite-vec extension.
//! This provides fast, portable vector search without external dependencies.
//!
//! ## Features
//!
//! - Pure SQLite storage (no LanceDB dependency)
//! - sqlite-vec for vector indexing and KNN search
//! - SIMD-accelerated vector operations
//! - Separate database file for vectors (vectors.db)
//!
//! ## Usage
//!
//! ```rust,ignore
//! let store = VectorStore::new("data/sqlite/vectors.db", 768)?;
//!
//! // Insert chunks with embeddings
//! store.insert_chunks(&chunks, &embeddings)?;
//!
//! // Vector search
//! let results = store.vector_search(&query_embedding, 10, None)?;
//! ```

use kix_parser::EntryChunk;
use rusqlite::{ffi::sqlite3_auto_extension, params, Connection, Result as SqliteResult};
use sqlite_vec::sqlite3_vec_init;
use std::path::Path;
use std::sync::{Arc, Mutex};
use thiserror::Error;
use tracing::{debug, info};
use zerocopy::AsBytes;

/// Default embedding dimensions (768 for bge-base-en-v1.5).
pub const DEFAULT_EMBEDDING_DIM: usize = 768;

/// Error type for vector store operations.
#[derive(Error, Debug)]
pub enum VectorError {
    #[error("SQLite error: {0}")]
    Sqlite(#[from] rusqlite::Error),

    #[error("Dimension mismatch: expected {expected}, got {actual}")]
    DimensionMismatch { expected: usize, actual: usize },

    #[error("Serialization error: {0}")]
    Serialization(String),

    #[error("Database error: {0}")]
    Database(String),
}

pub type Result<T> = std::result::Result<T, VectorError>;

/// Search result from vector store.
#[derive(Debug, Clone)]
pub struct VectorSearchResult {
    pub chunk_id: String,
    pub entry_id: String,
    pub page_id: Option<String>,
    pub chunk_index: u32,
    pub chunk_type: String,
    pub text: String,
    pub entry_title: String,
    pub entry_type: String,
    pub source_domain: Option<String>,
    pub tags: Vec<String>,
    pub distance: f32,
    pub score: f32,
}

/// Vector store using SQLite + sqlite-vec.
pub struct VectorStore {
    /// SQLite connection (rusqlite)
    conn: Arc<Mutex<Connection>>,
    /// Embedding dimensions
    embedding_dim: usize,
}

impl VectorStore {
    /// Create a new vector store at the given path.
    ///
    /// Loads the sqlite-vec extension and creates the chunks table if needed.
    pub fn new<P: AsRef<Path>>(db_path: P, embedding_dim: usize) -> Result<Self> {
        // Register sqlite-vec extension before opening connection
        unsafe {
            sqlite3_auto_extension(Some(std::mem::transmute(sqlite3_vec_init as *const ())));
        }

        // Create parent directory if needed
        if let Some(parent) = db_path.as_ref().parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| VectorError::Database(format!("Failed to create directory: {}", e)))?;
        }

        let conn = Connection::open(&db_path)?;

        // Verify sqlite-vec is loaded
        let vec_version: String = conn.query_row("SELECT vec_version()", [], |row| row.get(0))?;
        info!(
            "VectorStore initialized: {} (sqlite-vec {})",
            db_path.as_ref().display(),
            vec_version
        );

        let store = Self {
            conn: Arc::new(Mutex::new(conn)),
            embedding_dim,
        };

        // Create tables
        store.create_tables()?;

        Ok(store)
    }

    /// Create the chunks virtual table and metadata table.
    fn create_tables(&self) -> Result<()> {
        let conn = self.conn.lock().unwrap();

        // Create virtual table for vector search using vec0
        // Note: vec0 uses float[N] syntax for vector dimensions
        let create_vec_sql = format!(
            "CREATE VIRTUAL TABLE IF NOT EXISTS vec_chunks USING vec0(
                chunk_id TEXT PRIMARY KEY,
                embedding float[{}]
            )",
            self.embedding_dim
        );
        conn.execute(&create_vec_sql, [])?;

        // Create metadata table for chunk data
        conn.execute(
            "CREATE TABLE IF NOT EXISTS chunk_metadata (
                chunk_id TEXT PRIMARY KEY,
                entry_id TEXT NOT NULL,
                page_id TEXT,
                chunk_index INTEGER NOT NULL,
                chunk_type TEXT NOT NULL,
                text TEXT NOT NULL,
                entry_title TEXT NOT NULL,
                entry_type TEXT NOT NULL,
                source_domain TEXT,
                tags TEXT,
                created_at TEXT NOT NULL
            )",
            [],
        )?;

        // Create indexes on metadata table
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_chunk_entry_id ON chunk_metadata(entry_id)",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_chunk_page_id ON chunk_metadata(page_id)",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_chunk_entry_type ON chunk_metadata(entry_type)",
            [],
        )?;

        info!(
            "Vector tables created (embedding_dim={})",
            self.embedding_dim
        );
        Ok(())
    }

    /// Returns the embedding dimensions for this store.
    pub fn embedding_dim(&self) -> usize {
        self.embedding_dim
    }

    /// Insert chunks with embeddings into the vector store.
    pub fn insert_chunks(&self, chunks: &[EntryChunk], embeddings: &[Vec<f32>]) -> Result<()> {
        if chunks.is_empty() || embeddings.is_empty() {
            return Ok(());
        }

        if chunks.len() != embeddings.len() {
            return Err(VectorError::Database(format!(
                "Chunks ({}) and embeddings ({}) count mismatch",
                chunks.len(),
                embeddings.len()
            )));
        }

        // Validate embedding dimensions
        for embedding in embeddings.iter() {
            if embedding.len() != self.embedding_dim {
                return Err(VectorError::DimensionMismatch {
                    expected: self.embedding_dim,
                    actual: embedding.len(),
                });
            }
        }

        let conn = self.conn.lock().unwrap();
        let now = chrono::Utc::now().to_rfc3339();

        // Insert into both tables in a transaction
        conn.execute("BEGIN TRANSACTION", [])?;

        for (chunk, embedding) in chunks.iter().zip(embeddings.iter()) {
            // Insert into vec_chunks (vector table)
            // sqlite-vec expects vectors as raw bytes
            conn.execute(
                "INSERT OR REPLACE INTO vec_chunks(chunk_id, embedding) VALUES (?, ?)",
                params![chunk.chunk_id, embedding.as_bytes()],
            )?;

            // Insert into chunk_metadata
            let tags_json = serde_json::to_string(&chunk.metadata.tags).unwrap_or_default();
            let chunk_type_str = chunk.chunk_type.as_str();
            conn.execute(
                "INSERT OR REPLACE INTO chunk_metadata(
                    chunk_id, entry_id, page_id, chunk_index, chunk_type,
                    text, entry_title, entry_type, source_domain, tags, created_at
                ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                params![
                    chunk.chunk_id,
                    chunk.entry_id,
                    chunk.page_id,
                    chunk.chunk_index,
                    chunk_type_str,
                    chunk.text,
                    chunk.metadata.entry_title,
                    chunk.metadata.entry_type,
                    Option::<String>::None, // source_domain not in ChunkMetadata
                    tags_json,
                    now,
                ],
            )?;
        }

        conn.execute("COMMIT", [])?;

        info!("Inserted {} chunks with embeddings", chunks.len());
        Ok(())
    }

    /// Perform vector search (K-Nearest Neighbors).
    ///
    /// Returns the top `limit` chunks ordered by distance to the query embedding.
    pub fn vector_search(
        &self,
        query_embedding: &[f32],
        limit: usize,
        filter: Option<&SearchFilter>,
    ) -> Result<Vec<VectorSearchResult>> {
        if query_embedding.len() != self.embedding_dim {
            return Err(VectorError::DimensionMismatch {
                expected: self.embedding_dim,
                actual: query_embedding.len(),
            });
        }

        let conn = self.conn.lock().unwrap();

        // Build the query with optional filters
        let (filter_clause, filter_params) = match filter {
            Some(f) => f.to_sql_clause(),
            None => (String::new(), vec![]),
        };

        // Build filter clause with numbered parameters starting from ?3
        let numbered_filter_clause = if filter_clause.is_empty() {
            String::new()
        } else {
            // Replace sequential ? placeholders with numbered ones starting at 3
            let mut clause = filter_clause.clone();
            let mut param_num = 3;
            while clause.contains('?') {
                clause = clause.replacen('?', &format!("?{}", param_num), 1);
                param_num += 1;
            }
            format!("AND {}", clause)
        };

        // Use vec_chunks MATCH for KNN search with k constraint, then join with metadata
        // sqlite-vec requires 'k = ?' constraint in the WHERE clause for KNN queries
        let sql = format!(
            "SELECT
                m.chunk_id,
                m.entry_id,
                m.page_id,
                m.chunk_index,
                m.chunk_type,
                m.text,
                m.entry_title,
                m.entry_type,
                m.source_domain,
                m.tags,
                v.distance
            FROM vec_chunks v
            JOIN chunk_metadata m ON v.chunk_id = m.chunk_id
            WHERE v.embedding MATCH ?1
            AND v.k = ?2
            {}
            ORDER BY v.distance",
            numbered_filter_clause
        );

        let mut stmt = conn.prepare(&sql)?;

        // Build params: query vector, k (limit), then filter params
        let query_bytes = query_embedding.as_bytes();

        // Dynamically build params based on filter
        let results: Vec<VectorSearchResult> = if filter_params.is_empty() {
            stmt.query_map(params![query_bytes, limit as i64], |row| {
                Self::row_to_result(row)
            })?
            .collect::<SqliteResult<Vec<_>>>()?
        } else {
            // Build dynamic params with filter values
            let mut param_values: Vec<Box<dyn rusqlite::ToSql>> = vec![
                Box::new(query_bytes.to_vec()),
                Box::new(limit as i64),
            ];
            for p in &filter_params {
                param_values.push(Box::new(p.clone()));
            }
            let params_ref: Vec<&dyn rusqlite::ToSql> =
                param_values.iter().map(|p| p.as_ref()).collect();
            stmt.query_map(params_ref.as_slice(), |row| Self::row_to_result(row))?
                .collect::<SqliteResult<Vec<_>>>()?
        };

        debug!("Vector search returned {} results", results.len());
        Ok(results)
    }

    /// Convert a database row to a VectorSearchResult.
    fn row_to_result(row: &rusqlite::Row) -> SqliteResult<VectorSearchResult> {
        let distance: f64 = row.get(10)?;
        let score = 1.0 / (1.0 + distance as f32);

        let tags_json: String = row.get(9)?;
        let tags: Vec<String> = serde_json::from_str(&tags_json).unwrap_or_default();

        Ok(VectorSearchResult {
            chunk_id: row.get(0)?,
            entry_id: row.get(1)?,
            page_id: row.get(2)?,
            chunk_index: row.get(3)?,
            chunk_type: row.get(4)?,
            text: row.get(5)?,
            entry_title: row.get(6)?,
            entry_type: row.get(7)?,
            source_domain: row.get(8)?,
            tags,
            distance: distance as f32,
            score,
        })
    }

    /// Delete chunks by entry ID.
    pub fn delete_chunks_by_entry(&self, entry_id: &str) -> Result<usize> {
        let conn = self.conn.lock().unwrap();

        // Get chunk IDs to delete from metadata table
        let mut stmt =
            conn.prepare("SELECT chunk_id FROM chunk_metadata WHERE entry_id = ?")?;
        let chunk_ids: Vec<String> = stmt
            .query_map([entry_id], |row| row.get(0))?
            .collect::<SqliteResult<Vec<_>>>()?;
        drop(stmt);

        if chunk_ids.is_empty() {
            return Ok(0);
        }

        conn.execute("BEGIN TRANSACTION", [])?;

        // Delete from vec_chunks
        for chunk_id in &chunk_ids {
            conn.execute("DELETE FROM vec_chunks WHERE chunk_id = ?", [chunk_id])?;
        }

        // Delete from metadata
        let deleted = conn.execute("DELETE FROM chunk_metadata WHERE entry_id = ?", [entry_id])?;

        conn.execute("COMMIT", [])?;

        info!("Deleted {} chunks for entry: {}", deleted, entry_id);
        Ok(deleted)
    }

    /// Delete chunks by page ID.
    pub fn delete_chunks_by_page(&self, page_id: &str) -> Result<usize> {
        let conn = self.conn.lock().unwrap();

        // Get chunk IDs to delete from metadata table
        let mut stmt =
            conn.prepare("SELECT chunk_id FROM chunk_metadata WHERE page_id = ?")?;
        let chunk_ids: Vec<String> = stmt
            .query_map([page_id], |row| row.get(0))?
            .collect::<SqliteResult<Vec<_>>>()?;
        drop(stmt);

        if chunk_ids.is_empty() {
            return Ok(0);
        }

        conn.execute("BEGIN TRANSACTION", [])?;

        // Delete from vec_chunks
        for chunk_id in &chunk_ids {
            conn.execute("DELETE FROM vec_chunks WHERE chunk_id = ?", [chunk_id])?;
        }

        // Delete from metadata
        let deleted = conn.execute("DELETE FROM chunk_metadata WHERE page_id = ?", [page_id])?;

        conn.execute("COMMIT", [])?;

        info!("Deleted {} chunks for page: {}", deleted, page_id);
        Ok(deleted)
    }

    /// Get chunk count.
    pub fn chunk_count(&self) -> Result<usize> {
        let conn = self.conn.lock().unwrap();
        let count: i64 = conn.query_row("SELECT COUNT(*) FROM chunk_metadata", [], |row| {
            row.get(0)
        })?;
        Ok(count as usize)
    }

    /// Get chunk count for a specific entry.
    pub fn chunk_count_by_entry(&self, entry_id: &str) -> Result<usize> {
        let conn = self.conn.lock().unwrap();
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM chunk_metadata WHERE entry_id = ?",
            [entry_id],
            |row| row.get(0),
        )?;
        Ok(count as usize)
    }

    /// Clear all chunks (use with caution!).
    pub fn clear_all(&self) -> Result<()> {
        let conn = self.conn.lock().unwrap();

        conn.execute("BEGIN TRANSACTION", [])?;
        conn.execute("DELETE FROM vec_chunks", [])?;
        conn.execute("DELETE FROM chunk_metadata", [])?;
        conn.execute("COMMIT", [])?;

        info!("Cleared all chunks from vector store");
        Ok(())
    }

    /// Get a chunk by ID.
    pub fn get_chunk(&self, chunk_id: &str) -> Result<Option<VectorSearchResult>> {
        let conn = self.conn.lock().unwrap();

        let result = conn.query_row(
            "SELECT
                chunk_id, entry_id, page_id, chunk_index, chunk_type,
                text, entry_title, entry_type, source_domain, tags
            FROM chunk_metadata WHERE chunk_id = ?",
            [chunk_id],
            |row| {
                let tags_json: String = row.get(9)?;
                let tags: Vec<String> = serde_json::from_str(&tags_json).unwrap_or_default();
                Ok(VectorSearchResult {
                    chunk_id: row.get(0)?,
                    entry_id: row.get(1)?,
                    page_id: row.get(2)?,
                    chunk_index: row.get(3)?,
                    chunk_type: row.get(4)?,
                    text: row.get(5)?,
                    entry_title: row.get(6)?,
                    entry_type: row.get(7)?,
                    source_domain: row.get(8)?,
                    tags,
                    distance: 0.0,
                    score: 1.0,
                })
            },
        );

        match result {
            Ok(r) => Ok(Some(r)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    /// Get chunks by entry ID.
    pub fn get_chunks_by_entry(&self, entry_id: &str) -> Result<Vec<VectorSearchResult>> {
        let conn = self.conn.lock().unwrap();

        let mut stmt = conn.prepare(
            "SELECT
                chunk_id, entry_id, page_id, chunk_index, chunk_type,
                text, entry_title, entry_type, source_domain, tags
            FROM chunk_metadata WHERE entry_id = ? ORDER BY chunk_index",
        )?;

        let results = stmt
            .query_map([entry_id], |row| {
                let tags_json: String = row.get(9)?;
                let tags: Vec<String> = serde_json::from_str(&tags_json).unwrap_or_default();
                Ok(VectorSearchResult {
                    chunk_id: row.get(0)?,
                    entry_id: row.get(1)?,
                    page_id: row.get(2)?,
                    chunk_index: row.get(3)?,
                    chunk_type: row.get(4)?,
                    text: row.get(5)?,
                    entry_title: row.get(6)?,
                    entry_type: row.get(7)?,
                    source_domain: row.get(8)?,
                    tags,
                    distance: 0.0,
                    score: 1.0,
                })
            })?
            .collect::<SqliteResult<Vec<_>>>()?;

        Ok(results)
    }
}

/// Search filter for vector queries.
#[derive(Debug, Default)]
pub struct SearchFilter {
    pub entry_type: Option<String>,
    pub chunk_type: Option<String>,
    pub source_domain: Option<String>,
    pub tag: Option<String>,
}

impl SearchFilter {
    /// Create an empty filter.
    pub fn new() -> Self {
        Self::default()
    }

    /// Filter by entry type.
    pub fn with_entry_type(mut self, entry_type: impl Into<String>) -> Self {
        self.entry_type = Some(entry_type.into());
        self
    }

    /// Filter by chunk type.
    pub fn with_chunk_type(mut self, chunk_type: impl Into<String>) -> Self {
        self.chunk_type = Some(chunk_type.into());
        self
    }

    /// Filter by source domain.
    pub fn with_source_domain(mut self, source_domain: impl Into<String>) -> Self {
        self.source_domain = Some(source_domain.into());
        self
    }

    /// Filter by tag.
    pub fn with_tag(mut self, tag: impl Into<String>) -> Self {
        self.tag = Some(tag.into());
        self
    }

    /// Convert to SQL WHERE clause fragment.
    fn to_sql_clause(&self) -> (String, Vec<String>) {
        let mut conditions = Vec::new();
        let mut params = Vec::new();

        if let Some(ref et) = self.entry_type {
            conditions.push("m.entry_type = ?".to_string());
            params.push(et.clone());
        }

        if let Some(ref ct) = self.chunk_type {
            conditions.push("m.chunk_type = ?".to_string());
            params.push(ct.clone());
        }

        if let Some(ref sd) = self.source_domain {
            conditions.push("(m.source_domain = ? OR m.source_domain LIKE ?)".to_string());
            params.push(sd.clone());
            params.push(format!("%.{}", sd));
        }

        if let Some(ref tag) = self.tag {
            conditions.push("m.tags LIKE ?".to_string());
            params.push(format!("%{}%", tag));
        }

        if conditions.is_empty() {
            (String::new(), vec![])
        } else {
            (conditions.join(" AND "), params)
        }
    }
}

/// Returns the configured embedding dimensions from environment.
pub fn get_embedding_dim() -> usize {
    std::env::var("KIX_EMBEDDING_DIM")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or_else(|| {
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

#[cfg(test)]
mod tests {
    use super::*;
    use kix_parser::ChunkMetadata;
    use tempfile::TempDir;

    fn create_test_chunk(id: &str, entry_id: &str, text: &str) -> EntryChunk {
        EntryChunk {
            chunk_id: id.to_string(),
            entry_id: entry_id.to_string(),
            page_id: Some(format!("page-{}", entry_id)),
            chunk_index: 0,
            chunk_type: kix_parser::ChunkType::Content,
            text: text.to_string(),
            metadata: ChunkMetadata {
                entry_title: "Test Entry".to_string(),
                entry_type: "document".to_string(),
                tags: vec!["test".to_string()],
            },
        }
    }

    fn create_test_embedding(dim: usize, value: f32) -> Vec<f32> {
        vec![value; dim]
    }

    #[test]
    fn test_vector_store_creation() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("vectors.db");

        let store = VectorStore::new(&db_path, 4).unwrap();
        assert_eq!(store.embedding_dim(), 4);
        assert_eq!(store.chunk_count().unwrap(), 0);
    }

    #[test]
    fn test_insert_and_search() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("vectors.db");
        let dim = 4;

        let store = VectorStore::new(&db_path, dim).unwrap();

        // Insert test chunks
        let chunks = vec![
            create_test_chunk("c1", "e1", "First chunk about Rust"),
            create_test_chunk("c2", "e1", "Second chunk about programming"),
            create_test_chunk("c3", "e2", "Third chunk about databases"),
        ];

        let embeddings = vec![
            create_test_embedding(dim, 0.1),
            create_test_embedding(dim, 0.2),
            create_test_embedding(dim, 0.9),
        ];

        store.insert_chunks(&chunks, &embeddings).unwrap();
        assert_eq!(store.chunk_count().unwrap(), 3);

        // Search for similar to first chunk
        let query = create_test_embedding(dim, 0.1);
        let results = store.vector_search(&query, 2, None).unwrap();

        assert_eq!(results.len(), 2);
        // First result should be closest to query
        assert_eq!(results[0].chunk_id, "c1");
    }

    #[test]
    fn test_delete_by_entry() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("vectors.db");
        let dim = 4;

        let store = VectorStore::new(&db_path, dim).unwrap();

        let chunks = vec![
            create_test_chunk("c1", "e1", "Chunk 1"),
            create_test_chunk("c2", "e1", "Chunk 2"),
            create_test_chunk("c3", "e2", "Chunk 3"),
        ];

        let embeddings = vec![
            create_test_embedding(dim, 0.1),
            create_test_embedding(dim, 0.2),
            create_test_embedding(dim, 0.3),
        ];

        store.insert_chunks(&chunks, &embeddings).unwrap();
        assert_eq!(store.chunk_count().unwrap(), 3);

        // Delete chunks for entry e1
        let deleted = store.delete_chunks_by_entry("e1").unwrap();
        assert_eq!(deleted, 2);
        assert_eq!(store.chunk_count().unwrap(), 1);
    }

    #[test]
    fn test_dimension_mismatch() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("vectors.db");

        let store = VectorStore::new(&db_path, 4).unwrap();

        let chunks = vec![create_test_chunk("c1", "e1", "Test")];
        let wrong_dim_embeddings = vec![vec![0.1, 0.2, 0.3]]; // Only 3 dimensions

        let result = store.insert_chunks(&chunks, &wrong_dim_embeddings);
        assert!(matches!(result, Err(VectorError::DimensionMismatch { .. })));
    }

    #[test]
    fn test_get_chunks_by_entry() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("vectors.db");
        let dim = 4;

        let store = VectorStore::new(&db_path, dim).unwrap();

        let mut chunk1 = create_test_chunk("c1", "e1", "Chunk 1");
        chunk1.chunk_index = 0;
        let mut chunk2 = create_test_chunk("c2", "e1", "Chunk 2");
        chunk2.chunk_index = 1;

        let chunks = vec![chunk1, chunk2];
        let embeddings = vec![
            create_test_embedding(dim, 0.1),
            create_test_embedding(dim, 0.2),
        ];

        store.insert_chunks(&chunks, &embeddings).unwrap();

        let results = store.get_chunks_by_entry("e1").unwrap();
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].chunk_index, 0);
        assert_eq!(results[1].chunk_index, 1);
    }

    #[test]
    fn test_clear_all() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("vectors.db");
        let dim = 4;

        let store = VectorStore::new(&db_path, dim).unwrap();

        let chunks = vec![create_test_chunk("c1", "e1", "Test")];
        let embeddings = vec![create_test_embedding(dim, 0.1)];

        store.insert_chunks(&chunks, &embeddings).unwrap();
        assert_eq!(store.chunk_count().unwrap(), 1);

        store.clear_all().unwrap();
        assert_eq!(store.chunk_count().unwrap(), 0);
    }
}
