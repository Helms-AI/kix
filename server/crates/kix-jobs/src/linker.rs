//! Pattern linking system for EIP Knowledge System
//!
//! Automatically finds and suggests related patterns based on semantic similarity.

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tokio::sync::{Mutex, RwLock};
use tracing::{debug, info};

use kix_embeddings::EmbeddingGenerator;
use kix_store::search::SearchFilters;
use kix_store::KixStore;

use crate::JobError;

/// Configuration for pattern linking
#[derive(Clone, Debug)]
pub struct LinkingConfig {
    /// Minimum similarity score (0.0-1.0) for automatic linking
    pub similarity_threshold: f32,
    /// Maximum number of related patterns to suggest
    pub max_links: usize,
    /// Whether to include the same pattern type only
    pub same_type_only: bool,
    /// Minimum similarity for "strong" links
    pub strong_link_threshold: f32,
}

impl Default for LinkingConfig {
    fn default() -> Self {
        Self {
            similarity_threshold: 0.5,
            max_links: 10,
            same_type_only: false,
            strong_link_threshold: 0.75,
        }
    }
}

/// A suggested pattern link
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatternLink {
    /// Target pattern ID
    pub pattern_id: String,
    /// Target pattern name
    pub pattern_name: String,
    /// Similarity score (0.0-1.0)
    pub similarity: f32,
    /// Link strength classification
    pub strength: LinkStrength,
    /// Reason for the link (e.g., "semantic similarity")
    pub reason: String,
    /// Whether this link was manually added
    pub manual: bool,
}

/// Link strength classification
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum LinkStrength {
    /// Very strong similarity (>= strong_link_threshold)
    Strong,
    /// Moderate similarity (between thresholds)
    Moderate,
    /// Weak similarity (close to threshold)
    Weak,
}

impl LinkStrength {
    fn from_score(score: f32, strong_threshold: f32, base_threshold: f32) -> Self {
        if score >= strong_threshold {
            LinkStrength::Strong
        } else if score >= (strong_threshold + base_threshold) / 2.0 {
            LinkStrength::Moderate
        } else {
            LinkStrength::Weak
        }
    }
}

impl std::fmt::Display for LinkStrength {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LinkStrength::Strong => write!(f, "strong"),
            LinkStrength::Moderate => write!(f, "moderate"),
            LinkStrength::Weak => write!(f, "weak"),
        }
    }
}

/// Pattern linker finds related patterns using semantic similarity
pub struct PatternLinker {
    store: Arc<RwLock<KixStore>>,
    embedder: Arc<Mutex<EmbeddingGenerator>>,
    config: LinkingConfig,
}

impl PatternLinker {
    /// Create a new pattern linker
    pub fn new(
        store: Arc<RwLock<KixStore>>,
        embedder: Arc<Mutex<EmbeddingGenerator>>,
        config: LinkingConfig,
    ) -> Self {
        Self {
            store,
            embedder,
            config,
        }
    }

    /// Create with default configuration
    pub fn with_defaults(
        store: Arc<RwLock<KixStore>>,
        embedder: Arc<Mutex<EmbeddingGenerator>>,
    ) -> Self {
        Self::new(store, embedder, LinkingConfig::default())
    }

    /// Find related patterns for a given text query
    pub async fn find_related(
        &self,
        query_text: &str,
        exclude_pattern_id: Option<&str>,
    ) -> Result<Vec<PatternLink>, JobError> {
        if query_text.trim().is_empty() {
            return Ok(Vec::new());
        }

        debug!(query_len = query_text.len(), "Finding related patterns");

        // Generate embedding for query
        let embedding = {
            let mut embedder = self.embedder.lock().await;
            embedder
                .embed_query(query_text)
                .map_err(|e| JobError::Processing(format!("Failed to embed query: {}", e)))?
        };

        self.find_by_embedding(&embedding, exclude_pattern_id).await
    }

    /// Find related patterns using a pre-computed embedding
    pub async fn find_by_embedding(
        &self,
        embedding: &[f32],
        exclude_pattern_id: Option<&str>,
    ) -> Result<Vec<PatternLink>, JobError> {
        // Use read lock for concurrent read access
        let store = self.store.read().await;

        // Search for similar patterns
        let filters = SearchFilters::new();
        let results = store
            .vector_search(embedding, self.config.max_links * 2, &filters)
            .await
            .map_err(|e| JobError::Processing(format!("Vector search failed: {}", e)))?;

        let mut links = Vec::new();
        let mut seen_patterns: std::collections::HashSet<String> = std::collections::HashSet::new();

        for result in results {
            // Skip excluded pattern
            if let Some(exclude_id) = exclude_pattern_id {
                if result.entry_id == exclude_id {
                    continue;
                }
            }

            // Skip if we've already seen this pattern
            if seen_patterns.contains(&result.entry_id) {
                continue;
            }
            seen_patterns.insert(result.entry_id.clone());

            // Check similarity threshold
            if result.score < self.config.similarity_threshold {
                continue;
            }

            let strength = LinkStrength::from_score(
                result.score,
                self.config.strong_link_threshold,
                self.config.similarity_threshold,
            );

            links.push(PatternLink {
                pattern_id: result.entry_id,
                pattern_name: result.entry_title,
                similarity: result.score,
                strength,
                reason: "Semantic similarity".to_string(),
                manual: false,
            });

            if links.len() >= self.config.max_links {
                break;
            }
        }

        info!(
            found = links.len(),
            threshold = self.config.similarity_threshold,
            "Found related patterns"
        );

        Ok(links)
    }

    /// Find related patterns for a document by combining title, description, and problem
    pub async fn find_related_for_document(
        &self,
        title: &str,
        description: &str,
        problem: Option<&str>,
        solution: Option<&str>,
        exclude_pattern_id: Option<&str>,
    ) -> Result<Vec<PatternLink>, JobError> {
        // Build a combined query text
        let mut query_parts = vec![title.to_string()];

        if !description.is_empty() {
            query_parts.push(description.to_string());
        }

        if let Some(prob) = problem {
            if !prob.is_empty() {
                query_parts.push(prob.to_string());
            }
        }

        if let Some(sol) = solution {
            if !sol.is_empty() {
                query_parts.push(sol.to_string());
            }
        }

        let query_text = query_parts.join(" ");

        // Truncate to reasonable size for embedding
        let query_text = if query_text.len() > 2000 {
            query_text.chars().take(2000).collect()
        } else {
            query_text
        };

        self.find_related(&query_text, exclude_pattern_id).await
    }

    /// Filter links to only include strong links
    pub fn filter_strong_links(links: &[PatternLink]) -> Vec<PatternLink> {
        links
            .iter()
            .filter(|l| l.strength == LinkStrength::Strong)
            .cloned()
            .collect()
    }

    /// Filter links to include strong and moderate links
    pub fn filter_significant_links(links: &[PatternLink]) -> Vec<PatternLink> {
        links
            .iter()
            .filter(|l| l.strength != LinkStrength::Weak)
            .cloned()
            .collect()
    }

    /// Get link names only (for storing in related_patterns field)
    pub fn link_names(links: &[PatternLink]) -> Vec<String> {
        links.iter().map(|l| l.pattern_name.clone()).collect()
    }

    /// Get configuration
    pub fn config(&self) -> &LinkingConfig {
        &self.config
    }

    /// Update configuration
    pub fn set_config(&mut self, config: LinkingConfig) {
        self.config = config;
    }

    /// Update similarity threshold
    pub fn set_threshold(&mut self, threshold: f32) {
        self.config.similarity_threshold = threshold.clamp(0.0, 1.0);
    }

    /// Update max links
    pub fn set_max_links(&mut self, max_links: usize) {
        self.config.max_links = max_links;
    }
}

/// Link suggestion result for API responses
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LinkSuggestions {
    /// Source document ID
    pub source_id: String,
    /// All suggested links
    pub links: Vec<PatternLink>,
    /// Strong links only
    pub strong_links: Vec<String>,
    /// Configuration used
    pub threshold: f32,
}

impl LinkSuggestions {
    pub fn new(source_id: String, links: Vec<PatternLink>, threshold: f32) -> Self {
        let strong_links = PatternLinker::filter_strong_links(&links)
            .iter()
            .map(|l| l.pattern_name.clone())
            .collect();

        Self {
            source_id,
            links,
            strong_links,
            threshold,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_link_strength_classification() {
        let strong_threshold = 0.75;
        let base_threshold = 0.5;

        assert_eq!(
            LinkStrength::from_score(0.9, strong_threshold, base_threshold),
            LinkStrength::Strong
        );
        assert_eq!(
            LinkStrength::from_score(0.75, strong_threshold, base_threshold),
            LinkStrength::Strong
        );
        assert_eq!(
            LinkStrength::from_score(0.7, strong_threshold, base_threshold),
            LinkStrength::Moderate
        );
        assert_eq!(
            LinkStrength::from_score(0.55, strong_threshold, base_threshold),
            LinkStrength::Weak
        );
    }

    #[test]
    fn test_link_names() {
        let links = vec![
            PatternLink {
                pattern_id: "1".to_string(),
                pattern_name: "Aggregator".to_string(),
                similarity: 0.8,
                strength: LinkStrength::Strong,
                reason: "test".to_string(),
                manual: false,
            },
            PatternLink {
                pattern_id: "2".to_string(),
                pattern_name: "Splitter".to_string(),
                similarity: 0.6,
                strength: LinkStrength::Moderate,
                reason: "test".to_string(),
                manual: false,
            },
        ];

        let names = PatternLinker::link_names(&links);
        assert_eq!(names, vec!["Aggregator", "Splitter"]);
    }

    #[test]
    fn test_filter_strong_links() {
        let links = vec![
            PatternLink {
                pattern_id: "1".to_string(),
                pattern_name: "Strong".to_string(),
                similarity: 0.8,
                strength: LinkStrength::Strong,
                reason: "test".to_string(),
                manual: false,
            },
            PatternLink {
                pattern_id: "2".to_string(),
                pattern_name: "Weak".to_string(),
                similarity: 0.5,
                strength: LinkStrength::Weak,
                reason: "test".to_string(),
                manual: false,
            },
        ];

        let strong = PatternLinker::filter_strong_links(&links);
        assert_eq!(strong.len(), 1);
        assert_eq!(strong[0].pattern_name, "Strong");
    }

    #[test]
    fn test_default_config() {
        let config = LinkingConfig::default();
        assert_eq!(config.similarity_threshold, 0.5);
        assert_eq!(config.max_links, 10);
        assert!(!config.same_type_only);
        assert_eq!(config.strong_link_threshold, 0.75);
    }
}
