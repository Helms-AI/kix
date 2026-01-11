//! Search operations for the Knowledge Indexer store.

use arrow_array::{Array, RecordBatch, StringArray, Float32Array, Int32Array};
use futures::TryStreamExt;
use lancedb::index::scalar::FullTextSearchQuery;
use lancedb::query::{ExecutableQuery, QueryBase};
use serde::{Deserialize, Serialize};
use tracing::debug;

use crate::error::StoreError;
use crate::store::KixStore;

/// Search result from the chunks table.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    /// Chunk ID
    pub chunk_id: String,
    /// Parent entry ID
    pub entry_id: String,
    /// Entry title
    pub entry_title: String,
    /// Chunk text content
    pub text: String,
    /// Relevance score (0-1, higher is better)
    pub score: f32,
    /// Entry type (document, article, pdf, etc.)
    pub entry_type: String,
    /// Tags
    pub tags: Vec<String>,
    /// Chunk type (summary, content, code, header)
    pub chunk_type: Option<String>,
    /// Source domain (e.g., "docs.example.com")
    pub source_domain: Option<String>,
}

/// Filters for search queries.
#[derive(Debug, Clone, Default)]
pub struct SearchFilters {
    /// Filter by entry type (e.g., "document", "article", "pdf")
    pub entry_type: Option<String>,
    /// Filter by chunk type (e.g., "summary", "content")
    pub chunk_type: Option<String>,
    /// Filter by tag
    pub tag: Option<String>,
    /// Filter by source domain (e.g., "docs.example.com")
    pub source_domain: Option<String>,
}

impl SearchFilters {
    /// Creates a new empty filter.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the entry type filter.
    pub fn with_entry_type(mut self, entry_type: &str) -> Self {
        self.entry_type = Some(entry_type.to_string());
        self
    }

    /// Sets the chunk type filter.
    pub fn with_chunk_type(mut self, chunk_type: &str) -> Self {
        self.chunk_type = Some(chunk_type.to_string());
        self
    }

    /// Sets the tag filter.
    pub fn with_tag(mut self, tag: &str) -> Self {
        self.tag = Some(tag.to_string());
        self
    }

    /// Sets the source domain filter.
    pub fn with_source_domain(mut self, domain: &str) -> Self {
        self.source_domain = Some(domain.to_string());
        self
    }

    /// Builds a SQL filter string.
    fn to_filter_string(&self) -> Option<String> {
        let mut conditions = Vec::new();

        if let Some(ref et) = self.entry_type {
            conditions.push(format!("entry_type = '{}'", et));
        }

        if let Some(ref ct) = self.chunk_type {
            conditions.push(format!("chunk_type = '{}'", ct));
        }

        if let Some(ref tag) = self.tag {
            // Tags is stored as JSON, use LIKE for partial match
            conditions.push(format!("tags LIKE '%{}%'", tag));
        }

        if let Some(ref domain) = self.source_domain {
            // Exact match or subdomain match for source_domain
            conditions.push(format!(
                "(source_domain = '{}' OR source_domain LIKE '%.{}')",
                domain, domain
            ));
        }

        if conditions.is_empty() {
            None
        } else {
            Some(conditions.join(" AND "))
        }
    }
}

/// Entry summary for list operations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntrySummary {
    /// Entry ID
    pub id: String,
    /// Entry title
    pub title: String,
    /// Short description
    pub description: String,
    /// Entry type
    pub entry_type: String,
    /// Tags
    pub tags: Vec<String>,
    /// Source type (url, file, etc.)
    pub source_type: Option<String>,
    /// Source domain (e.g., "docs.example.com")
    pub source_domain: Option<String>,
    /// Source path
    pub source_path: Option<String>,
    /// Created timestamp
    pub created_at: Option<String>,
    /// Updated timestamp
    pub updated_at: Option<String>,
}

/// Chunk data for entry content display.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntryChunk {
    /// Chunk ID
    pub chunk_id: String,
    /// Parent entry ID
    pub entry_id: String,
    /// Chunk text content
    pub text: String,
    /// Chunk type (summary, content, code, header)
    pub chunk_type: Option<String>,
    /// Chunk index/order
    pub chunk_index: Option<i32>,
}

/// Search optimization parameters
/// nprobes: Number of IVF partitions to search (higher = better recall, slower)
const DEFAULT_NPROBES: usize = 20;
/// refine_factor: Multiplier for candidates to re-rank (higher = better quality)
const DEFAULT_REFINE_FACTOR: u32 = 10;

impl KixStore {
    /// Performs semantic vector search with optimized parameters.
    pub async fn vector_search(
        &self,
        query_embedding: &[f32],
        limit: usize,
        filters: &SearchFilters,
    ) -> Result<Vec<SearchResult>, StoreError> {
        let table = self
            .chunks_table()
            .ok_or_else(|| StoreError::Database("Chunks table not initialized".to_string()))?;

        debug!("Performing optimized vector search with limit {}, nprobes={}, refine={}",
               limit, DEFAULT_NPROBES, DEFAULT_REFINE_FACTOR);

        let mut query = table
            .vector_search(query_embedding)
            .map_err(|e| StoreError::Query(e.to_string()))?
            // Search more IVF partitions for better recall
            .nprobes(DEFAULT_NPROBES)
            // Re-rank more candidates for better quality
            .refine_factor(DEFAULT_REFINE_FACTOR)
            .limit(limit);

        // Apply filters (post-filter for IVF indexes)
        if let Some(filter_str) = filters.to_filter_string() {
            query = query.only_if(filter_str);
        }

        let batches = query
            .execute()
            .await
            .map_err(|e| StoreError::Query(e.to_string()))?
            .try_collect::<Vec<_>>()
            .await
            .map_err(|e| StoreError::Query(e.to_string()))?;

        let mut search_results = Vec::new();
        for batch in batches {
            search_results.extend(Self::batch_to_results(&batch)?);
        }

        Ok(search_results)
    }

    /// Performs full-text search.
    pub async fn text_search(
        &self,
        query_text: &str,
        limit: usize,
        filters: &SearchFilters,
    ) -> Result<Vec<SearchResult>, StoreError> {
        let table = self
            .chunks_table()
            .ok_or_else(|| StoreError::Database("Chunks table not initialized".to_string()))?;

        debug!("Performing FTS search for: {}", query_text);

        let mut query = table
            .query()
            .full_text_search(FullTextSearchQuery::new(query_text.to_owned()))
            .limit(limit);

        // Apply filters
        if let Some(filter_str) = filters.to_filter_string() {
            query = query.only_if(filter_str);
        }

        let batches = query
            .execute()
            .await
            .map_err(|e| StoreError::Query(e.to_string()))?
            .try_collect::<Vec<_>>()
            .await
            .map_err(|e| StoreError::Query(e.to_string()))?;

        let mut search_results = Vec::new();
        for batch in batches {
            // FTS results don't have distance, use a default score
            search_results.extend(Self::batch_to_results_with_score(&batch, 1.0)?);
        }

        Ok(search_results)
    }

    /// Performs hybrid search combining vector and full-text search.
    /// Falls back to vector-only search if FTS index is not available.
    pub async fn hybrid_search(
        &self,
        query_text: &str,
        query_embedding: &[f32],
        limit: usize,
        filters: &SearchFilters,
    ) -> Result<Vec<SearchResult>, StoreError> {
        let table = self
            .chunks_table()
            .ok_or_else(|| StoreError::Database("Chunks table not initialized".to_string()))?;

        debug!("Performing hybrid search for: {}", query_text);

        // Try hybrid search first (requires FTS index)
        let hybrid_result = async {
            let mut query = table
                .query()
                .full_text_search(FullTextSearchQuery::new(query_text.to_owned()))
                .nearest_to(query_embedding)
                .map_err(|e| StoreError::Query(e.to_string()))?
                .limit(limit);

            // Apply filters
            if let Some(filter_str) = filters.to_filter_string() {
                query = query.only_if(filter_str);
            }

            let batches = query
                .execute()
                .await
                .map_err(|e| StoreError::Query(e.to_string()))?
                .try_collect::<Vec<_>>()
                .await
                .map_err(|e| StoreError::Query(e.to_string()))?;

            let mut search_results = Vec::new();
            for batch in batches {
                search_results.extend(Self::batch_to_results(&batch)?);
            }

            Ok::<Vec<SearchResult>, StoreError>(search_results)
        }
        .await;

        // If hybrid search fails (likely no FTS index), fall back to vector search
        let mut search_results = match hybrid_result {
            Ok(results) => results,
            Err(e) => {
                let error_msg = e.to_string();
                if error_msg.contains("INVERTED index") || error_msg.contains("full text search") {
                    debug!("FTS index not available, falling back to vector search");
                    self.vector_search(query_embedding, limit, filters).await?
                } else {
                    return Err(e);
                }
            }
        };

        // Normalize scores so the max score becomes 1.0 (100%)
        if !search_results.is_empty() {
            let max_score = search_results
                .iter()
                .map(|r| r.score)
                .fold(0.0f32, |a, b| a.max(b));
            if max_score > 0.0 {
                for result in &mut search_results {
                    result.score = result.score / max_score;
                }
            }
        }

        Ok(search_results)
    }

    /// Gets an entry by exact title.
    pub async fn get_entry_by_title(&self, title: &str) -> Result<Option<EntrySummary>, StoreError> {
        let table = self
            .entries_table()
            .ok_or_else(|| StoreError::Database("Entries table not initialized".to_string()))?;

        let filter = format!("title = '{}'", title.replace('\'', "''"));

        let batches = table
            .query()
            .only_if(filter)
            .limit(1)
            .execute()
            .await
            .map_err(|e| StoreError::Query(e.to_string()))?
            .try_collect::<Vec<_>>()
            .await
            .map_err(|e| StoreError::Query(e.to_string()))?;

        for batch in batches {
            let summaries = Self::batch_to_entry_summaries(&batch)?;
            if let Some(summary) = summaries.into_iter().next() {
                return Ok(Some(summary));
            }
        }
        Ok(None)
    }

    /// Gets an entry by ID.
    pub async fn get_entry_by_id(&self, id: &str) -> Result<Option<EntrySummary>, StoreError> {
        let table = self
            .entries_table()
            .ok_or_else(|| StoreError::Database("Entries table not initialized".to_string()))?;

        let filter = format!("id = '{}'", id.replace('\'', "''"));

        let batches = table
            .query()
            .only_if(filter)
            .limit(1)
            .execute()
            .await
            .map_err(|e| StoreError::Query(e.to_string()))?
            .try_collect::<Vec<_>>()
            .await
            .map_err(|e| StoreError::Query(e.to_string()))?;

        for batch in batches {
            let summaries = Self::batch_to_entry_summaries(&batch)?;
            if let Some(summary) = summaries.into_iter().next() {
                return Ok(Some(summary));
            }
        }
        Ok(None)
    }

    /// Lists entries by tag.
    pub async fn list_by_tag(&self, tag: &str) -> Result<Vec<EntrySummary>, StoreError> {
        let table = self
            .entries_table()
            .ok_or_else(|| StoreError::Database("Entries table not initialized".to_string()))?;

        let filter = format!("tags LIKE '%{}%'", tag.replace('\'', "''"));

        let batches = table
            .query()
            .only_if(filter)
            .execute()
            .await
            .map_err(|e| StoreError::Query(e.to_string()))?
            .try_collect::<Vec<_>>()
            .await
            .map_err(|e| StoreError::Query(e.to_string()))?;

        let mut summaries = Vec::new();
        for batch in batches {
            summaries.extend(Self::batch_to_entry_summaries(&batch)?);
        }

        Ok(summaries)
    }

    /// Lists entries by type.
    pub async fn list_by_entry_type(&self, entry_type: &str) -> Result<Vec<EntrySummary>, StoreError> {
        let table = self
            .entries_table()
            .ok_or_else(|| StoreError::Database("Entries table not initialized".to_string()))?;

        let filter = format!("entry_type = '{}'", entry_type.replace('\'', "''"));

        let batches = table
            .query()
            .only_if(filter)
            .execute()
            .await
            .map_err(|e| StoreError::Query(e.to_string()))?
            .try_collect::<Vec<_>>()
            .await
            .map_err(|e| StoreError::Query(e.to_string()))?;

        let mut summaries = Vec::new();
        for batch in batches {
            summaries.extend(Self::batch_to_entry_summaries(&batch)?);
        }

        Ok(summaries)
    }

    /// Lists all entries.
    pub async fn list_all_entries(&self) -> Result<Vec<EntrySummary>, StoreError> {
        let table = self
            .entries_table()
            .ok_or_else(|| StoreError::Database("Entries table not initialized".to_string()))?;

        let batches = table
            .query()
            .execute()
            .await
            .map_err(|e| StoreError::Query(e.to_string()))?
            .try_collect::<Vec<_>>()
            .await
            .map_err(|e| StoreError::Query(e.to_string()))?;

        let mut summaries = Vec::new();
        for batch in batches {
            summaries.extend(Self::batch_to_entry_summaries(&batch)?);
        }

        Ok(summaries)
    }

    /// Gets all chunks for a specific entry ID.
    pub async fn get_chunks_by_entry_id(&self, entry_id: &str) -> Result<Vec<EntryChunk>, StoreError> {
        let table = self
            .chunks_table()
            .ok_or_else(|| StoreError::Database("Chunks table not initialized".to_string()))?;

        let filter = format!("entry_id = '{}'", entry_id.replace('\'', "''"));

        let batches = table
            .query()
            .only_if(filter)
            .execute()
            .await
            .map_err(|e| StoreError::Query(e.to_string()))?
            .try_collect::<Vec<_>>()
            .await
            .map_err(|e| StoreError::Query(e.to_string()))?;

        let mut chunks = Vec::new();
        for batch in batches {
            chunks.extend(Self::batch_to_entry_chunks(&batch)?);
        }

        // Sort by chunk_index if available
        chunks.sort_by(|a, b| {
            match (a.chunk_index, b.chunk_index) {
                (Some(ai), Some(bi)) => ai.cmp(&bi),
                (Some(_), None) => std::cmp::Ordering::Less,
                (None, Some(_)) => std::cmp::Ordering::Greater,
                (None, None) => a.chunk_id.cmp(&b.chunk_id),
            }
        });

        Ok(chunks)
    }

    /// Converts a RecordBatch to entry chunks.
    fn batch_to_entry_chunks(batch: &RecordBatch) -> Result<Vec<EntryChunk>, StoreError> {
        let chunk_ids = Self::get_string_column(batch, "chunk_id")?;
        let entry_ids = Self::get_string_column(batch, "entry_id")?;
        let texts = Self::get_string_column(batch, "text")?;
        let chunk_types = Self::get_optional_string_column(batch, "chunk_type");
        let chunk_indices = Self::get_optional_int_column(batch, "chunk_index");

        let mut chunks = Vec::new();

        for i in 0..batch.num_rows() {
            chunks.push(EntryChunk {
                chunk_id: chunk_ids[i].clone(),
                entry_id: entry_ids[i].clone(),
                text: texts[i].clone(),
                chunk_type: chunk_types[i].clone(),
                chunk_index: chunk_indices[i],
            });
        }

        Ok(chunks)
    }

    /// Converts a RecordBatch to search results with distance-based scores.
    fn batch_to_results(batch: &RecordBatch) -> Result<Vec<SearchResult>, StoreError> {
        let chunk_ids = Self::get_string_column(batch, "chunk_id")?;
        let entry_ids = Self::get_string_column(batch, "entry_id")?;
        let entry_titles = Self::get_string_column(batch, "entry_title")?;
        let texts = Self::get_string_column(batch, "text")?;
        let entry_types = Self::get_string_column(batch, "entry_type")?;
        let tags_json = Self::get_string_column(batch, "tags")?;
        let chunk_types = Self::get_optional_string_column(batch, "chunk_type");
        let source_domains = Self::get_optional_string_column(batch, "source_domain");

        // Get scores - try _relevance_score first (hybrid), then _score, then _distance
        let relevance_scores = Self::get_float_column(batch, "_relevance_score").ok();
        let scores = Self::get_float_column(batch, "_score").ok();
        let distances = Self::get_float_column(batch, "_distance").ok();

        let mut results = Vec::new();

        for i in 0..batch.num_rows() {
            let tags: Vec<String> = serde_json::from_str(&tags_json[i]).unwrap_or_default();

            // Use _relevance_score (hybrid), _score, or convert _distance to score
            let score = if let Some(ref rs) = relevance_scores {
                rs[i]
            } else if let Some(ref s) = scores {
                s[i]
            } else if let Some(ref d) = distances {
                1.0 / (1.0 + d[i])
            } else {
                0.5
            };

            results.push(SearchResult {
                chunk_id: chunk_ids[i].clone(),
                entry_id: entry_ids[i].clone(),
                entry_title: entry_titles[i].clone(),
                text: texts[i].clone(),
                score,
                entry_type: entry_types[i].clone(),
                tags,
                chunk_type: chunk_types[i].clone(),
                source_domain: source_domains[i].clone(),
            });
        }

        Ok(results)
    }

    /// Converts a RecordBatch to search results with a fixed score.
    fn batch_to_results_with_score(
        batch: &RecordBatch,
        default_score: f32,
    ) -> Result<Vec<SearchResult>, StoreError> {
        let chunk_ids = Self::get_string_column(batch, "chunk_id")?;
        let entry_ids = Self::get_string_column(batch, "entry_id")?;
        let entry_titles = Self::get_string_column(batch, "entry_title")?;
        let texts = Self::get_string_column(batch, "text")?;
        let entry_types = Self::get_string_column(batch, "entry_type")?;
        let tags_json = Self::get_string_column(batch, "tags")?;
        let chunk_types = Self::get_optional_string_column(batch, "chunk_type");
        let source_domains = Self::get_optional_string_column(batch, "source_domain");

        let mut results = Vec::new();

        for i in 0..batch.num_rows() {
            let tags: Vec<String> = serde_json::from_str(&tags_json[i]).unwrap_or_default();

            results.push(SearchResult {
                chunk_id: chunk_ids[i].clone(),
                entry_id: entry_ids[i].clone(),
                entry_title: entry_titles[i].clone(),
                text: texts[i].clone(),
                score: default_score - (i as f32 * 0.01), // Slight decay for rank
                entry_type: entry_types[i].clone(),
                tags,
                chunk_type: chunk_types[i].clone(),
                source_domain: source_domains[i].clone(),
            });
        }

        Ok(results)
    }

    /// Converts a RecordBatch to entry summaries.
    fn batch_to_entry_summaries(batch: &RecordBatch) -> Result<Vec<EntrySummary>, StoreError> {
        let ids = Self::get_string_column(batch, "id")?;
        let titles = Self::get_string_column(batch, "title")?;
        let descriptions = Self::get_string_column(batch, "description")?;
        let entry_types = Self::get_string_column(batch, "entry_type")?;
        let tags_json = Self::get_string_column(batch, "tags")?;
        let source_types = Self::get_optional_string_column(batch, "source_type");
        let source_domains = Self::get_optional_string_column(batch, "source_domain");
        let source_paths = Self::get_optional_string_column(batch, "source_path");
        let created_ats = Self::get_optional_string_column(batch, "created_at");
        let updated_ats = Self::get_optional_string_column(batch, "updated_at");

        let mut summaries = Vec::new();

        for i in 0..batch.num_rows() {
            let tags: Vec<String> = serde_json::from_str(&tags_json[i]).unwrap_or_default();

            summaries.push(EntrySummary {
                id: ids[i].clone(),
                title: titles[i].clone(),
                description: descriptions[i].clone(),
                entry_type: entry_types[i].clone(),
                tags,
                source_type: source_types[i].clone(),
                source_domain: source_domains[i].clone(),
                source_path: source_paths[i].clone(),
                created_at: created_ats[i].clone(),
                updated_at: updated_ats[i].clone(),
            });
        }

        Ok(summaries)
    }

    /// Helper to get a string column from a RecordBatch.
    fn get_string_column(batch: &RecordBatch, name: &str) -> Result<Vec<String>, StoreError> {
        let column = batch
            .column_by_name(name)
            .ok_or_else(|| StoreError::Query(format!("Column '{}' not found", name)))?;

        let array = column
            .as_any()
            .downcast_ref::<StringArray>()
            .ok_or_else(|| StoreError::Query(format!("Column '{}' is not a string", name)))?;

        Ok((0..array.len())
            .map(|i| array.value(i).to_string())
            .collect())
    }

    /// Helper to get a float column from a RecordBatch.
    fn get_float_column(batch: &RecordBatch, name: &str) -> Result<Vec<f32>, StoreError> {
        let column = batch
            .column_by_name(name)
            .ok_or_else(|| StoreError::Query(format!("Column '{}' not found", name)))?;

        let array = column
            .as_any()
            .downcast_ref::<Float32Array>()
            .ok_or_else(|| StoreError::Query(format!("Column '{}' is not a float", name)))?;

        Ok((0..array.len()).map(|i| array.value(i)).collect())
    }

    /// Helper to get an optional string column from a RecordBatch.
    fn get_optional_string_column(batch: &RecordBatch, name: &str) -> Vec<Option<String>> {
        batch
            .column_by_name(name)
            .and_then(|col| col.as_any().downcast_ref::<StringArray>())
            .map(|array| {
                (0..array.len())
                    .map(|i| {
                        if array.is_null(i) {
                            None
                        } else {
                            let val = array.value(i);
                            if val.is_empty() {
                                None
                            } else {
                                Some(val.to_string())
                            }
                        }
                    })
                    .collect()
            })
            .unwrap_or_else(|| vec![None; batch.num_rows()])
    }

    /// Helper to get an optional int column from a RecordBatch.
    fn get_optional_int_column(batch: &RecordBatch, name: &str) -> Vec<Option<i32>> {
        batch
            .column_by_name(name)
            .and_then(|col| col.as_any().downcast_ref::<Int32Array>())
            .map(|array| {
                (0..array.len())
                    .map(|i| {
                        if array.is_null(i) {
                            None
                        } else {
                            Some(array.value(i))
                        }
                    })
                    .collect()
            })
            .unwrap_or_else(|| vec![None; batch.num_rows()])
    }

    // Backward compatibility aliases
    /// Alias for get_entry_by_title (backward compatibility).
    pub async fn get_pattern_by_name(&self, name: &str) -> Result<Option<EntrySummary>, StoreError> {
        self.get_entry_by_title(name).await
    }

    /// Alias for get_entry_by_id (backward compatibility).
    pub async fn get_pattern_by_id(&self, id: &str) -> Result<Option<EntrySummary>, StoreError> {
        self.get_entry_by_id(id).await
    }

    /// Alias for list_by_tag (backward compatibility).
    pub async fn list_by_category(&self, category: &str) -> Result<Vec<EntrySummary>, StoreError> {
        self.list_by_tag(category).await
    }

    /// Alias for list_by_entry_type (backward compatibility).
    pub async fn list_by_pattern_type(&self, pattern_type: &str) -> Result<Vec<EntrySummary>, StoreError> {
        self.list_by_entry_type(pattern_type).await
    }

    /// Alias for list_all_entries (backward compatibility).
    pub async fn list_all_patterns(&self) -> Result<Vec<EntrySummary>, StoreError> {
        self.list_all_entries().await
    }
}

/// Backward compatibility alias for EntrySummary.
pub type PatternSummary = EntrySummary;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_filter_building() {
        let filters = SearchFilters::new()
            .with_entry_type("document")
            .with_chunk_type("content");

        let filter_str = filters.to_filter_string().unwrap();
        assert!(filter_str.contains("entry_type = 'document'"));
        assert!(filter_str.contains("chunk_type = 'content'"));
    }

    #[test]
    fn test_empty_filters() {
        let filters = SearchFilters::new();
        assert!(filters.to_filter_string().is_none());
    }

    #[test]
    fn test_tag_filter() {
        let filters = SearchFilters::new().with_tag("api");
        let filter_str = filters.to_filter_string().unwrap();
        assert!(filter_str.contains("tags LIKE '%api%'"));
    }

    #[test]
    fn test_source_domain_filter() {
        let filters = SearchFilters::new().with_source_domain("docs.example.com");
        let filter_str = filters.to_filter_string().unwrap();
        assert!(filter_str.contains("source_domain = 'docs.example.com'"));
        assert!(filter_str.contains("source_domain LIKE '%.docs.example.com'"));
    }

    #[test]
    fn test_combined_filters() {
        let filters = SearchFilters::new()
            .with_entry_type("document")
            .with_source_domain("docs.anthropic.com")
            .with_tag("api");

        let filter_str = filters.to_filter_string().unwrap();
        assert!(filter_str.contains("entry_type = 'document'"));
        assert!(filter_str.contains("source_domain"));
        assert!(filter_str.contains("tags LIKE '%api%'"));
        assert!(filter_str.contains(" AND "));
    }
}
