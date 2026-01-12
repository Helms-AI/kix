//! LanceDB schema definitions.

use arrow_schema::{DataType, Field, Schema};
use std::sync::Arc;

// ============================================================================
// Pages Table Schema (Two-Layer Storage)
// ============================================================================

/// Creates the schema for the pages table.
///
/// The pages table stores full page content for context retrieval.
/// This is the first layer of the two-layer storage pattern:
/// - Pages: Full content for RAG context
/// - Chunks: Smaller pieces for vector search (with page_id FK)
pub fn page_schema() -> Arc<Schema> {
    Arc::new(Schema::new(vec![
        Field::new("page_id", DataType::Utf8, false),      // Unique page identifier
        Field::new("source_id", DataType::Utf8, false),    // FK to sources/entries table
        Field::new("url", DataType::Utf8, false),          // Original URL
        Field::new("title", DataType::Utf8, true),         // Page title
        Field::new("full_content", DataType::Utf8, false), // Complete markdown content
        Field::new("content_hash", DataType::Utf8, false), // Hash for deduplication
        Field::new("content_length", DataType::UInt32, false), // Content length
        Field::new("code_block_count", DataType::UInt32, false), // Number of code blocks
        Field::new("metadata", DataType::Utf8, true),      // JSON metadata
        Field::new("crawl_time_ms", DataType::UInt64, true), // Time to crawl
        Field::new("created_at", DataType::Utf8, false),   // Creation timestamp
    ]))
}

// ============================================================================
// Entries Table Schema
// ============================================================================

/// Creates the schema for the entries table.
pub fn entry_schema() -> Arc<Schema> {
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
        Field::new("source_domain", DataType::Utf8, true),   // Domain for filtering (e.g., "docs.example.com")
        Field::new("slug", DataType::Utf8, false),
        Field::new("source_hash", DataType::Utf8, false),
        Field::new("created_at", DataType::Utf8, false),
        Field::new("updated_at", DataType::Utf8, false),
    ]))
}

// ============================================================================
// Chunks Table Schema
// ============================================================================

/// Creates the schema for the chunks table with embeddings.
///
/// Chunks are the second layer of the two-layer storage pattern.
/// They include a page_id foreign key for context retrieval.
pub fn chunk_schema(embedding_dim: i32) -> Arc<Schema> {
    Arc::new(Schema::new(vec![
        Field::new("chunk_id", DataType::Utf8, false),
        Field::new("entry_id", DataType::Utf8, false),
        Field::new("page_id", DataType::Utf8, true),       // FK to pages table (two-layer storage)
        Field::new("chunk_index", DataType::UInt32, false),
        Field::new("chunk_type", DataType::Utf8, false),
        Field::new("text", DataType::Utf8, false),
        Field::new("entry_title", DataType::Utf8, false),
        Field::new("entry_type", DataType::Utf8, false),
        Field::new("source_domain", DataType::Utf8, true),  // Domain for filtering
        Field::new("tags", DataType::Utf8, true),  // JSON array as string
        // Vector embedding field
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

/// Backward compatibility alias for entry_schema.
pub fn document_schema() -> Arc<Schema> {
    entry_schema()
}

// ============================================================================
// Jobs Table Schema (Job History Persistence)
// ============================================================================

/// Creates the schema for the jobs table.
///
/// The jobs table stores completed job metadata and aggregated statistics
/// for job history persistence. Stored in a separate `jobs.lance` file.
pub fn job_schema() -> Arc<Schema> {
    Arc::new(Schema::new(vec![
        // Identifiers
        Field::new("job_id", DataType::Utf8, false),

        // Job metadata
        Field::new("job_type", DataType::Utf8, false),      // "url", "file_upload", "reindex"
        Field::new("status", DataType::Utf8, false),        // "completed", "failed", "cancelled"

        // Timestamps
        Field::new("created_at", DataType::Utf8, false),    // RFC3339
        Field::new("started_at", DataType::Utf8, true),     // RFC3339
        Field::new("completed_at", DataType::Utf8, false),  // RFC3339

        // Source information
        Field::new("source_url", DataType::Utf8, true),     // For URL jobs
        Field::new("source_domain", DataType::Utf8, true),  // Extracted domain

        // Job configuration (JSON serialized)
        Field::new("config", DataType::Utf8, false),

        // Aggregated statistics
        Field::new("items_processed", DataType::UInt32, false),
        Field::new("items_discovered", DataType::UInt32, false),
        Field::new("chunks_created", DataType::UInt32, false),
        Field::new("embeddings_generated", DataType::UInt32, false),
        Field::new("error_count", DataType::UInt32, false),
        Field::new("duration_ms", DataType::UInt64, false),
        Field::new("processing_rate", DataType::Float64, false), // items/sec

        // Errors (JSON array)
        Field::new("errors", DataType::Utf8, true),
    ]))
}

// ============================================================================
// Job Items Table Schema (Per-Item Details)
// ============================================================================

/// Creates the schema for the job_items table.
///
/// The job_items table stores per-page/file details for job history.
/// Each row represents one processed item (URL or file) with its
/// individual statistics and status.
pub fn job_item_schema() -> Arc<Schema> {
    Arc::new(Schema::new(vec![
        // Identifiers
        Field::new("item_id", DataType::Utf8, false),       // UUID as string
        Field::new("job_id", DataType::Utf8, false),        // FK to jobs table

        // Item details
        Field::new("item_path", DataType::Utf8, false),     // URL or file path
        Field::new("item_type", DataType::Utf8, false),     // "page", "file"
        Field::new("status", DataType::Utf8, false),        // "completed", "error", "skipped"

        // Hierarchy (for crawled pages)
        Field::new("parent_url", DataType::Utf8, true),     // Parent URL for crawls
        Field::new("depth", DataType::UInt32, false),       // Crawl depth

        // Timestamps
        Field::new("discovered_at", DataType::Utf8, false), // RFC3339
        Field::new("started_at", DataType::Utf8, true),     // RFC3339
        Field::new("completed_at", DataType::Utf8, true),   // RFC3339

        // Processing stats
        Field::new("chunks_created", DataType::UInt32, false),
        Field::new("embeddings_generated", DataType::UInt32, false),
        Field::new("duration_ms", DataType::UInt64, false),

        // Error information
        Field::new("error_message", DataType::Utf8, true),
    ]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_entry_schema_fields() {
        let schema = entry_schema();
        assert!(schema.field_with_name("id").is_ok());
        assert!(schema.field_with_name("title").is_ok());
        assert!(schema.field_with_name("entry_type").is_ok());
        assert!(schema.field_with_name("tags").is_ok());
    }

    #[test]
    fn test_chunk_schema_fields() {
        let schema = chunk_schema(384);
        assert!(schema.field_with_name("chunk_id").is_ok());
        assert!(schema.field_with_name("entry_id").is_ok());
        assert!(schema.field_with_name("entry_title").is_ok());
        assert!(schema.field_with_name("vector").is_ok());
    }

    #[test]
    fn test_job_schema_fields() {
        let schema = job_schema();
        assert!(schema.field_with_name("job_id").is_ok());
        assert!(schema.field_with_name("job_type").is_ok());
        assert!(schema.field_with_name("status").is_ok());
        assert!(schema.field_with_name("items_processed").is_ok());
        assert!(schema.field_with_name("chunks_created").is_ok());
        assert!(schema.field_with_name("duration_ms").is_ok());
    }

    #[test]
    fn test_job_item_schema_fields() {
        let schema = job_item_schema();
        assert!(schema.field_with_name("item_id").is_ok());
        assert!(schema.field_with_name("job_id").is_ok());
        assert!(schema.field_with_name("item_path").is_ok());
        assert!(schema.field_with_name("status").is_ok());
        assert!(schema.field_with_name("chunks_created").is_ok());
        assert!(schema.field_with_name("error_message").is_ok());
    }
}
