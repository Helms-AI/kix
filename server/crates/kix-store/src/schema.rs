//! LanceDB schema definitions.

use arrow_schema::{DataType, Field, Schema};
use std::sync::Arc;

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

/// Creates the schema for the chunks table with embeddings.
pub fn chunk_schema(embedding_dim: i32) -> Arc<Schema> {
    Arc::new(Schema::new(vec![
        Field::new("chunk_id", DataType::Utf8, false),
        Field::new("entry_id", DataType::Utf8, false),
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
}
