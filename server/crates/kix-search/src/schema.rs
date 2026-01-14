//! Tantivy schema definitions for KIX search indexes.
//!
//! Defines the field schemas for entries, pages, and issues.

use tantivy::schema::{
    DateOptions, Field, Schema, SchemaBuilder, TextFieldIndexing, TextOptions, FAST, INDEXED,
    STORED, STRING,
};

/// Field references for the entry schema.
#[derive(Clone, Debug)]
pub struct EntryFields {
    pub id: Field,
    pub title: Field,
    pub description: Field,
    pub content: Field,
    pub entry_type: Field,
    pub source_domain: Field,
    pub source_path: Field,
    pub tags: Field,
    pub created_at: Field,
}

/// Field references for the page schema.
#[derive(Clone, Debug)]
pub struct PageFields {
    pub page_id: Field,
    pub entry_id: Field,
    pub url: Field,
    pub title: Field,
    pub content: Field,
    pub created_at: Field,
}

/// Field references for the issue schema.
#[derive(Clone, Debug)]
pub struct IssueFields {
    pub id: Field,
    pub project_id: Field,
    pub number: Field,
    pub title: Field,
    pub body: Field,
    pub state: Field,
    pub labels: Field,
    pub created_at: Field,
}

/// Build the entry schema for full-text search.
///
/// Entry schema supports searching across titles, descriptions, and content
/// with BM25 ranking. Also supports filtering by entry_type and source_domain.
pub fn build_entry_schema() -> (Schema, EntryFields) {
    let mut schema_builder = SchemaBuilder::default();

    // Text options for searchable fields with BM25 scoring
    let text_options = TextOptions::default()
        .set_indexing_options(
            TextFieldIndexing::default()
                .set_tokenizer("en_stem")
                .set_index_option(tantivy::schema::IndexRecordOption::WithFreqsAndPositions),
        )
        .set_stored();

    // Stored + Indexed fields
    let id = schema_builder.add_text_field("id", STRING | STORED);
    let title = schema_builder.add_text_field("title", text_options.clone());
    let description = schema_builder.add_text_field("description", text_options.clone());

    // Content is indexed but NOT stored (too large, retrieve from DB)
    let content_options = TextOptions::default().set_indexing_options(
        TextFieldIndexing::default()
            .set_tokenizer("en_stem")
            .set_index_option(tantivy::schema::IndexRecordOption::WithFreqsAndPositions),
    );
    let content = schema_builder.add_text_field("content", content_options);

    // Filter fields (stored + fast for filtering)
    let entry_type = schema_builder.add_text_field("entry_type", STRING | STORED | FAST);
    let source_domain = schema_builder.add_text_field("source_domain", STRING | STORED | FAST);
    let source_path = schema_builder.add_text_field("source_path", STRING | STORED);

    // Tags as searchable text
    let tags = schema_builder.add_text_field("tags", text_options);

    // Date field for sorting and filtering
    let date_options = DateOptions::default().set_indexed().set_stored().set_fast();
    let created_at = schema_builder.add_date_field("created_at", date_options);

    let fields = EntryFields {
        id,
        title,
        description,
        content,
        entry_type,
        source_domain,
        source_path,
        tags,
        created_at,
    };

    (schema_builder.build(), fields)
}

/// Build the page schema for full-text search.
///
/// Page schema supports searching across full page content for RAG context.
pub fn build_page_schema() -> (Schema, PageFields) {
    let mut schema_builder = SchemaBuilder::default();

    let text_options = TextOptions::default()
        .set_indexing_options(
            TextFieldIndexing::default()
                .set_tokenizer("en_stem")
                .set_index_option(tantivy::schema::IndexRecordOption::WithFreqsAndPositions),
        )
        .set_stored();

    // Content indexed but not stored (retrieve from DB)
    let content_options = TextOptions::default().set_indexing_options(
        TextFieldIndexing::default()
            .set_tokenizer("en_stem")
            .set_index_option(tantivy::schema::IndexRecordOption::WithFreqsAndPositions),
    );

    let page_id = schema_builder.add_text_field("page_id", STRING | STORED);
    let entry_id = schema_builder.add_text_field("entry_id", STRING | STORED | FAST);
    let url = schema_builder.add_text_field("url", STRING | STORED);
    let title = schema_builder.add_text_field("title", text_options);
    let content = schema_builder.add_text_field("content", content_options);

    let date_options = DateOptions::default().set_indexed().set_stored().set_fast();
    let created_at = schema_builder.add_date_field("created_at", date_options);

    let fields = PageFields {
        page_id,
        entry_id,
        url,
        title,
        content,
        created_at,
    };

    (schema_builder.build(), fields)
}

/// Build the issue schema for full-text search.
///
/// Issue schema supports searching across titles and bodies with filtering by state.
pub fn build_issue_schema() -> (Schema, IssueFields) {
    let mut schema_builder = SchemaBuilder::default();

    let text_options = TextOptions::default()
        .set_indexing_options(
            TextFieldIndexing::default()
                .set_tokenizer("en_stem")
                .set_index_option(tantivy::schema::IndexRecordOption::WithFreqsAndPositions),
        )
        .set_stored();

    // Body indexed but not stored
    let body_options = TextOptions::default().set_indexing_options(
        TextFieldIndexing::default()
            .set_tokenizer("en_stem")
            .set_index_option(tantivy::schema::IndexRecordOption::WithFreqsAndPositions),
    );

    let id = schema_builder.add_text_field("id", STRING | STORED);
    let project_id = schema_builder.add_text_field("project_id", STRING | STORED | FAST);
    let number = schema_builder.add_i64_field("number", INDEXED | STORED | FAST);
    let title = schema_builder.add_text_field("title", text_options.clone());
    let body = schema_builder.add_text_field("body", body_options);
    let state = schema_builder.add_text_field("state", STRING | STORED | FAST);
    let labels = schema_builder.add_text_field("labels", text_options);

    let date_options = DateOptions::default().set_indexed().set_stored().set_fast();
    let created_at = schema_builder.add_date_field("created_at", date_options);

    let fields = IssueFields {
        id,
        project_id,
        number,
        title,
        body,
        state,
        labels,
        created_at,
    };

    (schema_builder.build(), fields)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_entry_schema_fields() {
        let (schema, fields) = build_entry_schema();

        // Verify all fields exist
        assert!(schema.get_field("id").is_ok());
        assert!(schema.get_field("title").is_ok());
        assert!(schema.get_field("description").is_ok());
        assert!(schema.get_field("content").is_ok());
        assert!(schema.get_field("entry_type").is_ok());
        assert!(schema.get_field("source_domain").is_ok());
        assert!(schema.get_field("tags").is_ok());
        assert!(schema.get_field("created_at").is_ok());
    }

    #[test]
    fn test_page_schema_fields() {
        let (schema, _fields) = build_page_schema();

        assert!(schema.get_field("page_id").is_ok());
        assert!(schema.get_field("entry_id").is_ok());
        assert!(schema.get_field("url").is_ok());
        assert!(schema.get_field("title").is_ok());
        assert!(schema.get_field("content").is_ok());
    }

    #[test]
    fn test_issue_schema_fields() {
        let (schema, _fields) = build_issue_schema();

        assert!(schema.get_field("id").is_ok());
        assert!(schema.get_field("project_id").is_ok());
        assert!(schema.get_field("number").is_ok());
        assert!(schema.get_field("title").is_ok());
        assert!(schema.get_field("body").is_ok());
        assert!(schema.get_field("state").is_ok());
    }
}
