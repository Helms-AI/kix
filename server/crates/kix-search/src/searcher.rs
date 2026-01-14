//! Search query execution for KIX.
//!
//! Provides search functionality using Tantivy's BM25 ranking and query parsing.

use serde::{Deserialize, Serialize};
use tantivy::collector::TopDocs;
use tantivy::query::{BooleanQuery, Occur, Query, QueryParser, TermQuery};
use tantivy::schema::{Field, IndexRecordOption, Schema, Value};
use tantivy::{Index, IndexReader, ReloadPolicy, SnippetGenerator, TantivyDocument, Term};
use tracing::debug;

use crate::error::{SearchError, SearchResult};
use crate::schema::{EntryFields, IssueFields, PageFields};

/// Search result with score and snippet.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextSearchResult {
    /// Document ID.
    pub id: String,
    /// Document title.
    pub title: String,
    /// Search relevance score (BM25).
    pub score: f32,
    /// Highlighted snippet from matching content.
    pub snippet: Option<String>,
    /// Entry type (for entries) or state (for issues).
    pub doc_type: Option<String>,
    /// Source domain (for entries).
    pub source_domain: Option<String>,
}

/// Search filters for entries.
#[derive(Debug, Clone, Default)]
pub struct EntrySearchFilters {
    pub entry_type: Option<String>,
    pub source_domain: Option<String>,
    pub tag: Option<String>,
}

/// Search filters for issues.
#[derive(Debug, Clone, Default)]
pub struct IssueSearchFilters {
    pub project_id: Option<String>,
    pub state: Option<String>,
}

/// Entry searcher for full-text search.
pub struct EntrySearcher {
    index: Index,
    reader: IndexReader,
    schema: Schema,
    fields: EntryFields,
}

impl EntrySearcher {
    /// Create a new entry searcher.
    pub fn new(index: Index, schema: Schema, fields: EntryFields) -> SearchResult<Self> {
        let reader = index
            .reader_builder()
            .reload_policy(ReloadPolicy::OnCommitWithDelay)
            .try_into()
            .map_err(|e| SearchError::Index(format!("Failed to create reader: {}", e)))?;

        Ok(Self {
            index,
            reader,
            schema,
            fields,
        })
    }

    /// Search entries with a query string.
    pub fn search(
        &self,
        query_text: &str,
        limit: usize,
        filters: &EntrySearchFilters,
    ) -> SearchResult<Vec<TextSearchResult>> {
        let searcher = self.reader.searcher();

        // Create query parser for title, description, content, and tags
        let query_parser = QueryParser::for_index(
            &self.index,
            vec![
                self.fields.title,
                self.fields.description,
                self.fields.content,
                self.fields.tags,
            ],
        );

        // Parse the user query
        let user_query = query_parser.parse_query(query_text)?;

        // Build filter queries
        let mut filter_queries: Vec<(Occur, Box<dyn Query>)> = vec![(Occur::Must, user_query)];

        if let Some(ref entry_type) = filters.entry_type {
            let term = Term::from_field_text(self.fields.entry_type, entry_type);
            filter_queries.push((
                Occur::Must,
                Box::new(TermQuery::new(term, IndexRecordOption::Basic)),
            ));
        }

        if let Some(ref source_domain) = filters.source_domain {
            let term = Term::from_field_text(self.fields.source_domain, source_domain);
            filter_queries.push((
                Occur::Must,
                Box::new(TermQuery::new(term, IndexRecordOption::Basic)),
            ));
        }

        // Combine queries
        let combined_query = BooleanQuery::new(filter_queries);

        // Execute search
        let top_docs = searcher.search(&combined_query, &TopDocs::with_limit(limit))?;

        // Collect results with snippets
        let mut results = Vec::new();
        let snippet_generator =
            SnippetGenerator::create(&searcher, &combined_query, self.fields.description)?;

        for (score, doc_address) in top_docs {
            let doc: TantivyDocument = searcher.doc(doc_address)?;

            let id = self.get_text_field(&doc, self.fields.id);
            let title = self.get_text_field(&doc, self.fields.title);
            let entry_type = self.get_text_field_opt(&doc, self.fields.entry_type);
            let source_domain = self.get_text_field_opt(&doc, self.fields.source_domain);

            // Generate snippet from description
            let snippet = snippet_generator.snippet_from_doc(&doc);
            let snippet_text = if snippet.is_empty() {
                None
            } else {
                Some(snippet.to_html())
            };

            results.push(TextSearchResult {
                id,
                title,
                score,
                snippet: snippet_text,
                doc_type: entry_type,
                source_domain,
            });
        }

        debug!(
            query = %query_text,
            results = results.len(),
            "Entry search completed"
        );

        Ok(results)
    }

    /// Get document count in the index.
    pub fn doc_count(&self) -> u64 {
        self.reader.searcher().num_docs()
    }

    /// Reload the reader to see new commits.
    pub fn reload(&self) -> SearchResult<()> {
        self.reader
            .reload()
            .map_err(|e| SearchError::Index(format!("Failed to reload reader: {}", e)))
    }

    fn get_text_field(&self, doc: &TantivyDocument, field: Field) -> String {
        doc.get_first(field)
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string()
    }

    fn get_text_field_opt(&self, doc: &TantivyDocument, field: Field) -> Option<String> {
        doc.get_first(field)
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
    }
}

/// Page searcher for full-text search.
pub struct PageSearcher {
    index: Index,
    reader: IndexReader,
    schema: Schema,
    fields: PageFields,
}

impl PageSearcher {
    /// Create a new page searcher.
    pub fn new(index: Index, schema: Schema, fields: PageFields) -> SearchResult<Self> {
        let reader = index
            .reader_builder()
            .reload_policy(ReloadPolicy::OnCommitWithDelay)
            .try_into()
            .map_err(|e| SearchError::Index(format!("Failed to create reader: {}", e)))?;

        Ok(Self {
            index,
            reader,
            schema,
            fields,
        })
    }

    /// Search pages with a query string.
    pub fn search(&self, query_text: &str, limit: usize) -> SearchResult<Vec<TextSearchResult>> {
        let searcher = self.reader.searcher();

        let query_parser =
            QueryParser::for_index(&self.index, vec![self.fields.title, self.fields.content]);

        let query = query_parser.parse_query(query_text)?;
        let top_docs = searcher.search(&query, &TopDocs::with_limit(limit))?;

        let mut results = Vec::new();

        for (score, doc_address) in top_docs {
            let doc: TantivyDocument = searcher.doc(doc_address)?;

            let id = self.get_text_field(&doc, self.fields.page_id);
            let title = self
                .get_text_field_opt(&doc, self.fields.title)
                .unwrap_or_else(|| self.get_text_field(&doc, self.fields.url));
            let entry_id = self.get_text_field_opt(&doc, self.fields.entry_id);

            results.push(TextSearchResult {
                id,
                title,
                score,
                snippet: None,
                doc_type: entry_id,
                source_domain: None,
            });
        }

        debug!(
            query = %query_text,
            results = results.len(),
            "Page search completed"
        );

        Ok(results)
    }

    /// Get document count in the index.
    pub fn doc_count(&self) -> u64 {
        self.reader.searcher().num_docs()
    }

    /// Reload the reader to see new commits.
    pub fn reload(&self) -> SearchResult<()> {
        self.reader
            .reload()
            .map_err(|e| SearchError::Index(format!("Failed to reload reader: {}", e)))
    }

    fn get_text_field(&self, doc: &TantivyDocument, field: Field) -> String {
        doc.get_first(field)
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string()
    }

    fn get_text_field_opt(&self, doc: &TantivyDocument, field: Field) -> Option<String> {
        doc.get_first(field)
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
    }
}

/// Issue searcher for full-text search.
pub struct IssueSearcher {
    index: Index,
    reader: IndexReader,
    schema: Schema,
    fields: IssueFields,
}

impl IssueSearcher {
    /// Create a new issue searcher.
    pub fn new(index: Index, schema: Schema, fields: IssueFields) -> SearchResult<Self> {
        let reader = index
            .reader_builder()
            .reload_policy(ReloadPolicy::OnCommitWithDelay)
            .try_into()
            .map_err(|e| SearchError::Index(format!("Failed to create reader: {}", e)))?;

        Ok(Self {
            index,
            reader,
            schema,
            fields,
        })
    }

    /// Search issues with a query string.
    pub fn search(
        &self,
        query_text: &str,
        limit: usize,
        filters: &IssueSearchFilters,
    ) -> SearchResult<Vec<TextSearchResult>> {
        let searcher = self.reader.searcher();

        let query_parser = QueryParser::for_index(
            &self.index,
            vec![self.fields.title, self.fields.body, self.fields.labels],
        );

        let user_query = query_parser.parse_query(query_text)?;

        // Build filter queries
        let mut filter_queries: Vec<(Occur, Box<dyn Query>)> = vec![(Occur::Must, user_query)];

        if let Some(ref project_id) = filters.project_id {
            let term = Term::from_field_text(self.fields.project_id, project_id);
            filter_queries.push((
                Occur::Must,
                Box::new(TermQuery::new(term, IndexRecordOption::Basic)),
            ));
        }

        if let Some(ref state) = filters.state {
            let term = Term::from_field_text(self.fields.state, state);
            filter_queries.push((
                Occur::Must,
                Box::new(TermQuery::new(term, IndexRecordOption::Basic)),
            ));
        }

        let combined_query = BooleanQuery::new(filter_queries);
        let top_docs = searcher.search(&combined_query, &TopDocs::with_limit(limit))?;

        let mut results = Vec::new();
        let snippet_generator =
            SnippetGenerator::create(&searcher, &combined_query, self.fields.title)?;

        for (score, doc_address) in top_docs {
            let doc: TantivyDocument = searcher.doc(doc_address)?;

            let id = self.get_text_field(&doc, self.fields.id);
            let title = self.get_text_field(&doc, self.fields.title);
            let state = self.get_text_field_opt(&doc, self.fields.state);
            let project_id = self.get_text_field_opt(&doc, self.fields.project_id);

            let snippet = snippet_generator.snippet_from_doc(&doc);
            let snippet_text = if snippet.is_empty() {
                None
            } else {
                Some(snippet.to_html())
            };

            results.push(TextSearchResult {
                id,
                title,
                score,
                snippet: snippet_text,
                doc_type: state,
                source_domain: project_id,
            });
        }

        debug!(
            query = %query_text,
            results = results.len(),
            "Issue search completed"
        );

        Ok(results)
    }

    /// Get document count in the index.
    pub fn doc_count(&self) -> u64 {
        self.reader.searcher().num_docs()
    }

    /// Reload the reader to see new commits.
    pub fn reload(&self) -> SearchResult<()> {
        self.reader
            .reload()
            .map_err(|e| SearchError::Index(format!("Failed to reload reader: {}", e)))
    }

    fn get_text_field(&self, doc: &TantivyDocument, field: Field) -> String {
        doc.get_first(field)
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string()
    }

    fn get_text_field_opt(&self, doc: &TantivyDocument, field: Field) -> Option<String> {
        doc.get_first(field)
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::indexer::{EntryDocument, EntryIndexWriter};
    use crate::schema::build_entry_schema;
    use tempfile::tempdir;

    fn create_test_entries() -> Vec<EntryDocument> {
        vec![
            EntryDocument {
                id: "1".to_string(),
                title: "Content Based Router".to_string(),
                description: Some("Routes messages based on content".to_string()),
                content: Some(
                    "A content based router examines the message content and routes to different channels"
                        .to_string(),
                ),
                entry_type: "pattern".to_string(),
                source_domain: Some("eips.example.com".to_string()),
                source_path: "/cbr".to_string(),
                tags: vec!["routing".to_string(), "integration".to_string()],
                created_at: chrono::Utc::now(),
            },
            EntryDocument {
                id: "2".to_string(),
                title: "Message Filter".to_string(),
                description: Some("Filters messages based on criteria".to_string()),
                content: Some(
                    "A message filter removes unwanted messages from a channel".to_string(),
                ),
                entry_type: "pattern".to_string(),
                source_domain: Some("eips.example.com".to_string()),
                source_path: "/filter".to_string(),
                tags: vec!["filtering".to_string(), "integration".to_string()],
                created_at: chrono::Utc::now(),
            },
            EntryDocument {
                id: "3".to_string(),
                title: "Aggregator".to_string(),
                description: Some("Combines multiple messages into one".to_string()),
                content: Some(
                    "An aggregator collects and stores individual messages until a complete set arrives"
                        .to_string(),
                ),
                entry_type: "pattern".to_string(),
                source_domain: Some("eips.example.com".to_string()),
                source_path: "/aggregator".to_string(),
                tags: vec!["aggregation".to_string(), "integration".to_string()],
                created_at: chrono::Utc::now(),
            },
        ]
    }

    #[test]
    fn test_entry_search_basic() {
        let dir = tempdir().unwrap();
        let (schema, fields) = build_entry_schema();
        let index = Index::create_in_dir(dir.path(), schema.clone()).unwrap();

        // Index test documents
        let writer = EntryIndexWriter::new(index.clone(), schema.clone(), fields.clone());
        let entries = create_test_entries();
        writer.index_batch(&entries).unwrap();

        // Search
        let searcher = EntrySearcher::new(index, schema, fields).unwrap();
        let results = searcher
            .search("router", 10, &EntrySearchFilters::default())
            .unwrap();

        assert!(!results.is_empty());
        assert_eq!(results[0].title, "Content Based Router");
    }

    #[test]
    fn test_entry_search_with_filter() {
        let dir = tempdir().unwrap();
        let (schema, fields) = build_entry_schema();
        let index = Index::create_in_dir(dir.path(), schema.clone()).unwrap();

        let writer = EntryIndexWriter::new(index.clone(), schema.clone(), fields.clone());
        let entries = create_test_entries();
        writer.index_batch(&entries).unwrap();

        let searcher = EntrySearcher::new(index, schema, fields).unwrap();

        // Search with entry_type filter
        let filters = EntrySearchFilters {
            entry_type: Some("pattern".to_string()),
            ..Default::default()
        };
        let results = searcher.search("message", 10, &filters).unwrap();

        assert!(!results.is_empty());
        // All results should be patterns
        for result in &results {
            assert_eq!(result.doc_type, Some("pattern".to_string()));
        }
    }

    #[test]
    fn test_entry_doc_count() {
        let dir = tempdir().unwrap();
        let (schema, fields) = build_entry_schema();
        let index = Index::create_in_dir(dir.path(), schema.clone()).unwrap();

        let writer = EntryIndexWriter::new(index.clone(), schema.clone(), fields.clone());
        let entries = create_test_entries();
        writer.index_batch(&entries).unwrap();

        let searcher = EntrySearcher::new(index, schema, fields).unwrap();
        searcher.reload().unwrap();

        assert_eq!(searcher.doc_count(), 3);
    }
}
