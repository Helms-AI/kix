//! Index write operations for KIX search.
//!
//! Provides functions for adding, updating, and deleting documents in Tantivy indexes.

use tantivy::schema::Schema;
use tantivy::{DateTime, Index, TantivyDocument, Term};
use tracing::{debug, info};

use crate::error::{SearchError, SearchResult};
use crate::schema::{EntryFields, IssueFields, PageFields};

/// Memory budget for index writer (50MB default).
pub const DEFAULT_MEMORY_BUDGET: usize = 50_000_000;

/// Entry document for indexing.
#[derive(Debug, Clone)]
pub struct EntryDocument {
    pub id: String,
    pub title: String,
    pub description: Option<String>,
    pub content: Option<String>,
    pub entry_type: String,
    pub source_domain: Option<String>,
    pub source_path: String,
    pub tags: Vec<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

/// Page document for indexing.
#[derive(Debug, Clone)]
pub struct PageDocument {
    pub page_id: String,
    pub entry_id: String,
    pub url: String,
    pub title: Option<String>,
    pub content: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

/// Issue document for indexing.
#[derive(Debug, Clone)]
pub struct IssueDocument {
    pub id: String,
    pub project_id: String,
    pub number: i64,
    pub title: String,
    pub body: Option<String>,
    pub state: String,
    pub labels: Vec<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

/// Entry index writer for managing entry documents.
pub struct EntryIndexWriter {
    index: Index,
    fields: EntryFields,
    schema: Schema,
}

impl EntryIndexWriter {
    /// Create a new entry index writer.
    pub fn new(index: Index, schema: Schema, fields: EntryFields) -> Self {
        Self {
            index,
            fields,
            schema,
        }
    }

    /// Index a single entry document.
    pub fn index_entry(&self, entry: &EntryDocument) -> SearchResult<()> {
        let mut writer = self
            .index
            .writer::<TantivyDocument>(DEFAULT_MEMORY_BUDGET)
            .map_err(|e| SearchError::Index(format!("Failed to create writer: {}", e)))?;

        // Delete existing document if it exists
        writer.delete_term(Term::from_field_text(self.fields.id, &entry.id));

        // Create new document
        let doc = self.create_document(entry);
        writer.add_document(doc)?;
        writer.commit()?;

        debug!(entry_id = %entry.id, "Indexed entry");
        Ok(())
    }

    /// Index multiple entries in a batch.
    pub fn index_batch(&self, entries: &[EntryDocument]) -> SearchResult<usize> {
        if entries.is_empty() {
            return Ok(0);
        }

        let mut writer = self
            .index
            .writer::<TantivyDocument>(DEFAULT_MEMORY_BUDGET)
            .map_err(|e| SearchError::Index(format!("Failed to create writer: {}", e)))?;

        for entry in entries {
            // Delete existing document if it exists
            writer.delete_term(Term::from_field_text(self.fields.id, &entry.id));

            // Create and add new document
            let doc = self.create_document(entry);
            writer.add_document(doc)?;
        }

        writer.commit()?;
        info!(count = entries.len(), "Indexed entry batch");
        Ok(entries.len())
    }

    /// Delete an entry from the index.
    pub fn delete_entry(&self, entry_id: &str) -> SearchResult<()> {
        let mut writer = self
            .index
            .writer::<TantivyDocument>(DEFAULT_MEMORY_BUDGET)
            .map_err(|e| SearchError::Index(format!("Failed to create writer: {}", e)))?;

        writer.delete_term(Term::from_field_text(self.fields.id, entry_id));
        writer.commit()?;

        debug!(entry_id = %entry_id, "Deleted entry from index");
        Ok(())
    }

    /// Clear all documents from the index.
    pub fn clear_all(&self) -> SearchResult<()> {
        let mut writer = self
            .index
            .writer::<TantivyDocument>(DEFAULT_MEMORY_BUDGET)
            .map_err(|e| SearchError::Index(format!("Failed to create writer: {}", e)))?;

        writer.delete_all_documents()?;
        writer.commit()?;

        info!("Cleared all entries from index");
        Ok(())
    }

    fn create_document(&self, entry: &EntryDocument) -> TantivyDocument {
        let mut doc = TantivyDocument::default();

        doc.add_text(self.fields.id, &entry.id);
        doc.add_text(self.fields.title, &entry.title);

        if let Some(ref desc) = entry.description {
            doc.add_text(self.fields.description, desc);
        }

        if let Some(ref content) = entry.content {
            doc.add_text(self.fields.content, content);
        }

        doc.add_text(self.fields.entry_type, &entry.entry_type);

        if let Some(ref domain) = entry.source_domain {
            doc.add_text(self.fields.source_domain, domain);
        }

        doc.add_text(self.fields.source_path, &entry.source_path);

        // Add tags as space-separated for search
        if !entry.tags.is_empty() {
            doc.add_text(self.fields.tags, &entry.tags.join(" "));
        }

        // Convert chrono DateTime to tantivy DateTime
        let tantivy_dt = DateTime::from_timestamp_secs(entry.created_at.timestamp());
        doc.add_date(self.fields.created_at, tantivy_dt);

        doc
    }
}

/// Page index writer for managing page documents.
pub struct PageIndexWriter {
    index: Index,
    fields: PageFields,
    schema: Schema,
}

impl PageIndexWriter {
    /// Create a new page index writer.
    pub fn new(index: Index, schema: Schema, fields: PageFields) -> Self {
        Self {
            index,
            fields,
            schema,
        }
    }

    /// Index a single page document.
    pub fn index_page(&self, page: &PageDocument) -> SearchResult<()> {
        let mut writer = self
            .index
            .writer::<TantivyDocument>(DEFAULT_MEMORY_BUDGET)
            .map_err(|e| SearchError::Index(format!("Failed to create writer: {}", e)))?;

        writer.delete_term(Term::from_field_text(self.fields.page_id, &page.page_id));

        let doc = self.create_document(page);
        writer.add_document(doc)?;
        writer.commit()?;

        debug!(page_id = %page.page_id, "Indexed page");
        Ok(())
    }

    /// Index multiple pages in a batch.
    pub fn index_batch(&self, pages: &[PageDocument]) -> SearchResult<usize> {
        if pages.is_empty() {
            return Ok(0);
        }

        let mut writer = self
            .index
            .writer::<TantivyDocument>(DEFAULT_MEMORY_BUDGET)
            .map_err(|e| SearchError::Index(format!("Failed to create writer: {}", e)))?;

        for page in pages {
            writer.delete_term(Term::from_field_text(self.fields.page_id, &page.page_id));
            let doc = self.create_document(page);
            writer.add_document(doc)?;
        }

        writer.commit()?;
        info!(count = pages.len(), "Indexed page batch");
        Ok(pages.len())
    }

    /// Delete a page from the index.
    pub fn delete_page(&self, page_id: &str) -> SearchResult<()> {
        let mut writer = self
            .index
            .writer::<TantivyDocument>(DEFAULT_MEMORY_BUDGET)
            .map_err(|e| SearchError::Index(format!("Failed to create writer: {}", e)))?;

        writer.delete_term(Term::from_field_text(self.fields.page_id, page_id));
        writer.commit()?;

        debug!(page_id = %page_id, "Deleted page from index");
        Ok(())
    }

    /// Clear all documents from the index.
    pub fn clear_all(&self) -> SearchResult<()> {
        let mut writer = self
            .index
            .writer::<TantivyDocument>(DEFAULT_MEMORY_BUDGET)
            .map_err(|e| SearchError::Index(format!("Failed to create writer: {}", e)))?;

        writer.delete_all_documents()?;
        writer.commit()?;

        info!("Cleared all pages from index");
        Ok(())
    }

    fn create_document(&self, page: &PageDocument) -> TantivyDocument {
        let mut doc = TantivyDocument::default();

        doc.add_text(self.fields.page_id, &page.page_id);
        doc.add_text(self.fields.entry_id, &page.entry_id);
        doc.add_text(self.fields.url, &page.url);

        if let Some(ref title) = page.title {
            doc.add_text(self.fields.title, title);
        }

        doc.add_text(self.fields.content, &page.content);

        let tantivy_dt = DateTime::from_timestamp_secs(page.created_at.timestamp());
        doc.add_date(self.fields.created_at, tantivy_dt);

        doc
    }
}

/// Issue index writer for managing issue documents.
pub struct IssueIndexWriter {
    index: Index,
    fields: IssueFields,
    schema: Schema,
}

impl IssueIndexWriter {
    /// Create a new issue index writer.
    pub fn new(index: Index, schema: Schema, fields: IssueFields) -> Self {
        Self {
            index,
            fields,
            schema,
        }
    }

    /// Index a single issue document.
    pub fn index_issue(&self, issue: &IssueDocument) -> SearchResult<()> {
        let mut writer = self
            .index
            .writer::<TantivyDocument>(DEFAULT_MEMORY_BUDGET)
            .map_err(|e| SearchError::Index(format!("Failed to create writer: {}", e)))?;

        writer.delete_term(Term::from_field_text(self.fields.id, &issue.id));

        let doc = self.create_document(issue);
        writer.add_document(doc)?;
        writer.commit()?;

        debug!(issue_id = %issue.id, "Indexed issue");
        Ok(())
    }

    /// Index multiple issues in a batch.
    pub fn index_batch(&self, issues: &[IssueDocument]) -> SearchResult<usize> {
        if issues.is_empty() {
            return Ok(0);
        }

        let mut writer = self
            .index
            .writer::<TantivyDocument>(DEFAULT_MEMORY_BUDGET)
            .map_err(|e| SearchError::Index(format!("Failed to create writer: {}", e)))?;

        for issue in issues {
            writer.delete_term(Term::from_field_text(self.fields.id, &issue.id));
            let doc = self.create_document(issue);
            writer.add_document(doc)?;
        }

        writer.commit()?;
        info!(count = issues.len(), "Indexed issue batch");
        Ok(issues.len())
    }

    /// Delete an issue from the index.
    pub fn delete_issue(&self, issue_id: &str) -> SearchResult<()> {
        let mut writer = self
            .index
            .writer::<TantivyDocument>(DEFAULT_MEMORY_BUDGET)
            .map_err(|e| SearchError::Index(format!("Failed to create writer: {}", e)))?;

        writer.delete_term(Term::from_field_text(self.fields.id, issue_id));
        writer.commit()?;

        debug!(issue_id = %issue_id, "Deleted issue from index");
        Ok(())
    }

    /// Clear all documents from the index.
    pub fn clear_all(&self) -> SearchResult<()> {
        let mut writer = self
            .index
            .writer::<TantivyDocument>(DEFAULT_MEMORY_BUDGET)
            .map_err(|e| SearchError::Index(format!("Failed to create writer: {}", e)))?;

        writer.delete_all_documents()?;
        writer.commit()?;

        info!("Cleared all issues from index");
        Ok(())
    }

    fn create_document(&self, issue: &IssueDocument) -> TantivyDocument {
        let mut doc = TantivyDocument::default();

        doc.add_text(self.fields.id, &issue.id);
        doc.add_text(self.fields.project_id, &issue.project_id);
        doc.add_i64(self.fields.number, issue.number);
        doc.add_text(self.fields.title, &issue.title);

        if let Some(ref body) = issue.body {
            doc.add_text(self.fields.body, body);
        }

        doc.add_text(self.fields.state, &issue.state);

        if !issue.labels.is_empty() {
            doc.add_text(self.fields.labels, &issue.labels.join(" "));
        }

        let tantivy_dt = DateTime::from_timestamp_secs(issue.created_at.timestamp());
        doc.add_date(self.fields.created_at, tantivy_dt);

        doc
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::{build_entry_schema, build_issue_schema, build_page_schema};
    use tempfile::tempdir;

    fn create_test_entry() -> EntryDocument {
        EntryDocument {
            id: "test-entry-1".to_string(),
            title: "Test Entry".to_string(),
            description: Some("A test description".to_string()),
            content: Some("Test content for searching".to_string()),
            entry_type: "document".to_string(),
            source_domain: Some("example.com".to_string()),
            source_path: "/test".to_string(),
            tags: vec!["test".to_string(), "example".to_string()],
            created_at: chrono::Utc::now(),
        }
    }

    #[test]
    fn test_entry_index_single() {
        let dir = tempdir().unwrap();
        let (schema, fields) = build_entry_schema();
        let index = Index::create_in_dir(dir.path(), schema.clone()).unwrap();
        let writer = EntryIndexWriter::new(index, schema, fields);

        let entry = create_test_entry();
        writer.index_entry(&entry).unwrap();
    }

    #[test]
    fn test_entry_index_batch() {
        let dir = tempdir().unwrap();
        let (schema, fields) = build_entry_schema();
        let index = Index::create_in_dir(dir.path(), schema.clone()).unwrap();
        let writer = EntryIndexWriter::new(index, schema, fields);

        let entries = vec![
            create_test_entry(),
            EntryDocument {
                id: "test-entry-2".to_string(),
                ..create_test_entry()
            },
        ];

        let count = writer.index_batch(&entries).unwrap();
        assert_eq!(count, 2);
    }

    #[test]
    fn test_entry_delete() {
        let dir = tempdir().unwrap();
        let (schema, fields) = build_entry_schema();
        let index = Index::create_in_dir(dir.path(), schema.clone()).unwrap();
        let writer = EntryIndexWriter::new(index, schema, fields);

        let entry = create_test_entry();
        writer.index_entry(&entry).unwrap();
        writer.delete_entry(&entry.id).unwrap();
    }

    #[test]
    fn test_entry_clear_all() {
        let dir = tempdir().unwrap();
        let (schema, fields) = build_entry_schema();
        let index = Index::create_in_dir(dir.path(), schema.clone()).unwrap();
        let writer = EntryIndexWriter::new(index, schema, fields);

        let entry = create_test_entry();
        writer.index_entry(&entry).unwrap();
        writer.clear_all().unwrap();
    }
}
