//! Project-scoped search functionality.
//!
//! This module provides search capabilities within a project's scope,
//! including both issues and linked knowledge entries.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::knowledge::KnowledgeReference;

/// Parameters for project-scoped search.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct ProjectSearchParams {
    /// Project ID or name
    pub project: String,

    /// Search query
    pub query: String,

    /// Search type filter
    #[serde(default)]
    pub search_type: SearchType,

    /// Maximum number of results
    #[serde(default = "default_limit")]
    pub limit: usize,

    /// Include closed issues
    #[serde(default)]
    pub include_closed: bool,
}

fn default_limit() -> usize {
    20
}

/// Type of content to search.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SearchType {
    /// Search all content (issues and knowledge)
    #[default]
    All,
    /// Search only issues
    Issues,
    /// Search only linked knowledge entries
    Knowledge,
}

/// A search result from project-scoped search.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ProjectSearchResult {
    /// Total number of results
    pub total: usize,

    /// Issue results
    pub issues: Vec<IssueSearchResult>,

    /// Knowledge entry results
    pub knowledge: Vec<KnowledgeSearchResult>,
}

impl Default for ProjectSearchResult {
    fn default() -> Self {
        Self {
            total: 0,
            issues: vec![],
            knowledge: vec![],
        }
    }
}

/// An issue search result.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct IssueSearchResult {
    /// Issue ID
    pub id: String,

    /// Issue number
    pub number: u32,

    /// Issue title
    pub title: String,

    /// Issue body excerpt (with match highlighted)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub excerpt: Option<String>,

    /// Issue state
    pub state: String,

    /// Labels
    #[serde(default)]
    pub labels: Vec<String>,

    /// Search score (relevance)
    pub score: f32,

    /// GitHub URL (if synced)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub github_url: Option<String>,
}

/// A knowledge entry search result.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct KnowledgeSearchResult {
    /// Entry ID
    pub entry_id: String,

    /// Entry title
    pub title: String,

    /// Content excerpt (with match highlighted)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub excerpt: Option<String>,

    /// Entry type
    pub entry_type: String,

    /// Source URL
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_url: Option<String>,

    /// Search score (relevance)
    pub score: f32,

    /// Link information
    pub link: KnowledgeReference,
}

/// Search result combiner for merging issue and knowledge results.
pub struct SearchResultMerger {
    issues: Vec<IssueSearchResult>,
    knowledge: Vec<KnowledgeSearchResult>,
}

impl SearchResultMerger {
    /// Create a new result merger.
    pub fn new() -> Self {
        Self {
            issues: vec![],
            knowledge: vec![],
        }
    }

    /// Add issue results.
    pub fn add_issues(&mut self, issues: Vec<IssueSearchResult>) {
        self.issues.extend(issues);
    }

    /// Add knowledge results.
    pub fn add_knowledge(&mut self, knowledge: Vec<KnowledgeSearchResult>) {
        self.knowledge.extend(knowledge);
    }

    /// Merge results into a single response, sorted by score.
    pub fn merge(mut self, limit: usize) -> ProjectSearchResult {
        // Sort by score descending
        self.issues.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
        self.knowledge.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));

        // Interleave results based on score
        let mut result_scores: Vec<(f32, bool, usize)> = vec![];

        for (i, issue) in self.issues.iter().enumerate() {
            result_scores.push((issue.score, true, i));
        }
        for (i, knowledge) in self.knowledge.iter().enumerate() {
            result_scores.push((knowledge.score, false, i));
        }

        // Sort all by score
        result_scores.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));

        // Take top results
        let mut final_issues = Vec::new();
        let mut final_knowledge = Vec::new();
        let mut count = 0;

        for (_, is_issue, idx) in result_scores {
            if count >= limit {
                break;
            }

            if is_issue {
                if let Some(issue) = self.issues.get(idx).cloned() {
                    final_issues.push(issue);
                    count += 1;
                }
            } else if let Some(k) = self.knowledge.get(idx).cloned() {
                final_knowledge.push(k);
                count += 1;
            }
        }

        let total = final_issues.len() + final_knowledge.len();

        ProjectSearchResult {
            total,
            issues: final_issues,
            knowledge: final_knowledge,
        }
    }
}

impl Default for SearchResultMerger {
    fn default() -> Self {
        Self::new()
    }
}

/// Simple text-based search scoring.
/// Returns a score between 0.0 and 1.0 based on query match.
pub fn calculate_text_score(query: &str, title: &str, body: Option<&str>) -> f32 {
    let query_lower = query.to_lowercase();
    let title_lower = title.to_lowercase();

    let mut score = 0.0f32;

    // Title match is weighted highest
    if title_lower == query_lower {
        score += 1.0;
    } else if title_lower.contains(&query_lower) {
        // Partial title match
        let ratio = query_lower.len() as f32 / title_lower.len() as f32;
        score += 0.5 + (ratio * 0.3);
    }

    // Word-based title matching
    let query_words: Vec<&str> = query_lower.split_whitespace().collect();
    let title_words: Vec<&str> = title_lower.split_whitespace().collect();
    let word_matches = query_words.iter().filter(|w| title_words.contains(w)).count();
    if !query_words.is_empty() {
        score += (word_matches as f32 / query_words.len() as f32) * 0.3;
    }

    // Body match
    if let Some(body) = body {
        let body_lower = body.to_lowercase();
        if body_lower.contains(&query_lower) {
            score += 0.2;
        }
        // Word-based body matching
        let body_words: Vec<&str> = body_lower.split_whitespace().collect();
        let body_word_matches = query_words.iter().filter(|w| body_words.contains(w)).count();
        if !query_words.is_empty() {
            score += (body_word_matches as f32 / query_words.len() as f32) * 0.1;
        }
    }

    score.min(1.0)
}

/// Generate an excerpt from text with highlighted match.
pub fn generate_excerpt(text: &str, query: &str, context_chars: usize) -> Option<String> {
    let text_lower = text.to_lowercase();
    let query_lower = query.to_lowercase();

    // Find the first occurrence of the query
    if let Some(pos) = text_lower.find(&query_lower) {
        let start = pos.saturating_sub(context_chars);
        let end = (pos + query.len() + context_chars).min(text.len());

        let mut excerpt = String::new();

        if start > 0 {
            excerpt.push_str("...");
        }

        excerpt.push_str(&text[start..end]);

        if end < text.len() {
            excerpt.push_str("...");
        }

        Some(excerpt)
    } else {
        // No direct match, return beginning of text
        if text.len() > context_chars * 2 {
            Some(format!("{}...", &text[..context_chars * 2]))
        } else {
            Some(text.to_string())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_search_type_default() {
        let search_type = SearchType::default();
        assert_eq!(search_type, SearchType::All);
    }

    #[test]
    fn test_calculate_text_score_exact_match() {
        let score = calculate_text_score("login bug", "login bug", None);
        assert!(score >= 0.8);
    }

    #[test]
    fn test_calculate_text_score_partial_match() {
        let score = calculate_text_score("login", "Fix login bug", None);
        assert!(score > 0.0);
        assert!(score < 1.0);
    }

    #[test]
    fn test_calculate_text_score_body_match() {
        let score = calculate_text_score("authentication", "Bug", Some("The authentication is broken"));
        assert!(score > 0.0);
    }

    #[test]
    fn test_generate_excerpt() {
        let text = "This is a long text that contains the search term somewhere in the middle of the content.";
        let excerpt = generate_excerpt(text, "search term", 20);
        assert!(excerpt.is_some());
        let excerpt = excerpt.unwrap();
        assert!(excerpt.contains("search term"));
    }

    #[test]
    fn test_generate_excerpt_no_match() {
        let text = "This is a short text without the query.";
        let excerpt = generate_excerpt(text, "nonexistent", 20);
        assert!(excerpt.is_some());
    }

    #[test]
    fn test_search_result_merger() {
        let mut merger = SearchResultMerger::new();

        merger.add_issues(vec![
            IssueSearchResult {
                id: "1".to_string(),
                number: 1,
                title: "Issue 1".to_string(),
                excerpt: None,
                state: "open".to_string(),
                labels: vec![],
                score: 0.8,
                github_url: None,
            },
            IssueSearchResult {
                id: "2".to_string(),
                number: 2,
                title: "Issue 2".to_string(),
                excerpt: None,
                state: "open".to_string(),
                labels: vec![],
                score: 0.5,
                github_url: None,
            },
        ]);

        merger.add_knowledge(vec![
            KnowledgeSearchResult {
                entry_id: "k1".to_string(),
                title: "Knowledge 1".to_string(),
                excerpt: None,
                entry_type: "document".to_string(),
                source_url: None,
                score: 0.9,
                link: KnowledgeReference {
                    entry_id: "k1".to_string(),
                    title: "Knowledge 1".to_string(),
                    relevance_score: 0.9,
                    snippet: "Test snippet".to_string(),
                    source: None,
                },
            },
        ]);

        let result = merger.merge(10);
        assert_eq!(result.total, 3);
        assert_eq!(result.issues.len(), 2);
        assert_eq!(result.knowledge.len(), 1);
    }

    #[test]
    fn test_search_result_merger_limit() {
        let mut merger = SearchResultMerger::new();

        merger.add_issues(vec![
            IssueSearchResult {
                id: "1".to_string(),
                number: 1,
                title: "Issue 1".to_string(),
                excerpt: None,
                state: "open".to_string(),
                labels: vec![],
                score: 0.8,
                github_url: None,
            },
            IssueSearchResult {
                id: "2".to_string(),
                number: 2,
                title: "Issue 2".to_string(),
                excerpt: None,
                state: "open".to_string(),
                labels: vec![],
                score: 0.5,
                github_url: None,
            },
        ]);

        // Limit to 1 result
        let result = merger.merge(1);
        assert_eq!(result.total, 1);
    }
}
