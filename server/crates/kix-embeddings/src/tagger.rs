//! Auto-tagging via keyword extraction using RAKE and TF-IDF algorithms.
//!
//! This module provides automatic tag extraction from text content using:
//! - RAKE (Rapid Automatic Keyword Extraction) for multi-word phrases
//! - TF-IDF for significant single terms

use keyword_extraction::rake::{Rake, RakeParams};
use keyword_extraction::tf_idf::{TextSplit, TfIdf, TfIdfParams};
use stop_words::{get, LANGUAGE};

/// Configuration for tag extraction.
#[derive(Debug, Clone)]
pub struct TagExtractionConfig {
    /// Maximum number of tags to extract (default: 10).
    pub max_tags: usize,

    /// Minimum score threshold for including a RAKE tag (default: 1.0).
    pub min_rake_score: f32,

    /// Maximum words in a phrase for RAKE (default: 4).
    pub max_phrase_words: usize,

    /// Convert all tags to lowercase (default: true).
    pub lowercase: bool,

    /// Include TF-IDF single terms in addition to RAKE phrases (default: true).
    pub include_tfidf_terms: bool,

    /// Number of top TF-IDF terms to include (default: 5).
    pub tfidf_term_count: usize,

    /// Number of top RAKE phrases to consider (default: 20).
    pub rake_phrase_count: usize,
}

impl Default for TagExtractionConfig {
    fn default() -> Self {
        Self {
            max_tags: 10,
            min_rake_score: 1.0,
            max_phrase_words: 4,
            lowercase: true,
            include_tfidf_terms: true,
            tfidf_term_count: 5,
            rake_phrase_count: 20,
        }
    }
}

/// Source of an extracted tag.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TagSource {
    /// Extracted using RAKE algorithm.
    Rake,
    /// Extracted using TF-IDF algorithm.
    TfIdf,
    /// From document metadata (e.g., HTML meta tags).
    MetaTag,
    /// User-defined tag.
    UserDefined,
}

/// A tag extracted from content with its score and source.
#[derive(Debug, Clone)]
pub struct ExtractedTag {
    /// The tag text.
    pub text: String,
    /// Relevance score (higher is more relevant).
    pub score: f32,
    /// How this tag was extracted.
    pub source: TagSource,
}

impl ExtractedTag {
    /// Creates a new extracted tag.
    pub fn new(text: String, score: f32, source: TagSource) -> Self {
        Self { text, score, source }
    }
}

/// Tag extractor using RAKE and TF-IDF algorithms.
pub struct TagExtractor {
    config: TagExtractionConfig,
    stopwords: Vec<String>,
}

impl TagExtractor {
    /// Creates a new tag extractor with the given configuration.
    pub fn new(config: TagExtractionConfig) -> Self {
        // Get English stopwords
        let stopwords: Vec<String> = get(LANGUAGE::English)
            .iter()
            .map(|s| s.to_string())
            .collect();

        Self { config, stopwords }
    }

    /// Creates a tag extractor with default configuration.
    pub fn with_defaults() -> Self {
        Self::new(TagExtractionConfig::default())
    }

    /// Extracts tags from the given text content.
    ///
    /// Returns a list of tags sorted by score (highest first), limited to max_tags.
    pub fn extract(&self, content: &str) -> Vec<ExtractedTag> {
        if content.trim().is_empty() {
            return Vec::new();
        }

        let mut all_tags = Vec::new();

        // Extract using RAKE
        let rake_tags = self.extract_rake(content);
        all_tags.extend(rake_tags);

        // Extract using TF-IDF if enabled
        if self.config.include_tfidf_terms {
            let tfidf_tags = self.extract_tfidf(content);
            all_tags.extend(tfidf_tags);
        }

        // Deduplicate by text (keep highest score)
        all_tags = self.deduplicate(all_tags);

        // Sort by score descending
        all_tags.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));

        // Limit to max_tags
        all_tags.truncate(self.config.max_tags);

        all_tags
    }

    /// Extracts tags and merges with existing tags from metadata.
    ///
    /// Existing tags are preserved with UserDefined source and highest priority.
    /// Returns combined list limited to max_tags.
    pub fn extract_and_merge(&self, content: &str, existing_tags: &[String]) -> Vec<ExtractedTag> {
        let mut all_tags = Vec::new();

        // Add existing tags with high score to preserve them
        for tag in existing_tags {
            let text = if self.config.lowercase {
                tag.to_lowercase()
            } else {
                tag.clone()
            };
            all_tags.push(ExtractedTag::new(text, 1000.0, TagSource::UserDefined));
        }

        // Extract new tags
        let extracted = self.extract(content);
        all_tags.extend(extracted);

        // Deduplicate (existing tags win due to higher score)
        all_tags = self.deduplicate(all_tags);

        // Sort by score descending
        all_tags.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));

        // Limit to max_tags
        all_tags.truncate(self.config.max_tags);

        all_tags
    }

    /// Extracts keywords using RAKE algorithm.
    fn extract_rake(&self, content: &str) -> Vec<ExtractedTag> {
        if content.trim().is_empty() {
            return Vec::new();
        }

        let rake = Rake::new(RakeParams::WithDefaults(content, &self.stopwords));
        let keyword_scores = rake.get_ranked_keyword_scores(self.config.rake_phrase_count);

        keyword_scores
            .into_iter()
            .filter(|(phrase, score)| {
                let word_count = phrase.split_whitespace().count();
                word_count <= self.config.max_phrase_words
                    && *score >= self.config.min_rake_score
            })
            .map(|(phrase, score)| {
                let text = if self.config.lowercase {
                    phrase.to_lowercase()
                } else {
                    phrase
                };
                ExtractedTag::new(text, score, TagSource::Rake)
            })
            .collect()
    }

    /// Extracts top terms using TF-IDF algorithm.
    fn extract_tfidf(&self, content: &str) -> Vec<ExtractedTag> {
        if content.trim().is_empty() {
            return Vec::new();
        }

        let tfidf = TfIdf::new(TfIdfParams::TextBlock(
            content,
            &self.stopwords,
            None,
            TextSplit::Sentences,
        ));

        // Get ranked words (returns Vec<String>)
        let words = tfidf.get_ranked_words(self.config.tfidf_term_count);

        // Convert to extracted tags with decreasing scores
        words
            .into_iter()
            .enumerate()
            .filter(|(_, word)| word.len() >= 3)
            .map(|(i, word)| {
                let text = if self.config.lowercase {
                    word.to_lowercase()
                } else {
                    word
                };
                // Score decreases with rank (first = highest)
                let score = (self.config.tfidf_term_count - i) as f32 / self.config.tfidf_term_count as f32;
                ExtractedTag::new(text, score, TagSource::TfIdf)
            })
            .collect()
    }

    /// Deduplicates tags by text, keeping the one with highest score.
    fn deduplicate(&self, tags: Vec<ExtractedTag>) -> Vec<ExtractedTag> {
        use std::collections::HashMap;

        let mut seen: HashMap<String, ExtractedTag> = HashMap::new();

        for tag in tags {
            let key = tag.text.to_lowercase();
            match seen.get(&key) {
                Some(existing) if existing.score >= tag.score => {
                    // Keep existing, it has higher or equal score
                }
                _ => {
                    seen.insert(key, tag);
                }
            }
        }

        seen.into_values().collect()
    }

    /// Converts extracted tags to simple string list.
    pub fn to_string_list(tags: &[ExtractedTag]) -> Vec<String> {
        tags.iter().map(|t| t.text.clone()).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_extraction() {
        let extractor = TagExtractor::with_defaults();
        let content = "Machine learning is a branch of artificial intelligence. \
                       Deep learning uses neural networks for pattern recognition. \
                       Natural language processing enables text analysis and understanding.";

        let tags = extractor.extract(content);

        assert!(!tags.is_empty());
        let tag_texts: Vec<&str> = tags.iter().map(|t| t.text.as_str()).collect();
        println!("Extracted tags: {:?}", tag_texts);
    }

    #[test]
    fn test_empty_content() {
        let extractor = TagExtractor::with_defaults();
        let tags = extractor.extract("");
        assert!(tags.is_empty());

        let tags = extractor.extract("   ");
        assert!(tags.is_empty());
    }

    #[test]
    fn test_merge_with_existing() {
        let extractor = TagExtractor::with_defaults();
        let content = "This article discusses web development and JavaScript frameworks for building modern applications.";
        let existing = vec!["programming".to_string(), "tutorial".to_string()];

        let tags = extractor.extract_and_merge(content, &existing);

        // Existing tags should be preserved
        let tag_texts: Vec<&str> = tags.iter().map(|t| t.text.as_str()).collect();
        assert!(tag_texts.contains(&"programming"));
        assert!(tag_texts.contains(&"tutorial"));
    }

    #[test]
    fn test_deduplication() {
        let extractor = TagExtractor::with_defaults();
        let tags = vec![
            ExtractedTag::new("machine learning".to_string(), 5.0, TagSource::Rake),
            ExtractedTag::new("Machine Learning".to_string(), 3.0, TagSource::TfIdf),
        ];

        let deduped = extractor.deduplicate(tags);
        assert_eq!(deduped.len(), 1);
        assert_eq!(deduped[0].score, 5.0); // Higher score wins
    }

    #[test]
    fn test_max_tags_limit() {
        let config = TagExtractionConfig {
            max_tags: 3,
            ..Default::default()
        };
        let extractor = TagExtractor::new(config);

        let content = "Software engineering principles include design patterns, \
                       code review, testing methodologies, continuous integration, \
                       agile development, microservices architecture, and DevOps practices.";

        let tags = extractor.extract(content);
        assert!(tags.len() <= 3);
    }

    #[test]
    fn test_to_string_list() {
        let tags = vec![
            ExtractedTag::new("api".to_string(), 5.0, TagSource::Rake),
            ExtractedTag::new("rest".to_string(), 3.0, TagSource::TfIdf),
        ];

        let strings = TagExtractor::to_string_list(&tags);
        assert_eq!(strings, vec!["api", "rest"]);
    }

    #[test]
    fn test_rake_extraction() {
        let extractor = TagExtractor::with_defaults();
        let content = "Rust programming language is fast and memory safe. \
                       Systems programming requires understanding of memory management.";

        let tags = extractor.extract(content);
        let tag_texts: Vec<&str> = tags.iter().map(|t| t.text.as_str()).collect();
        println!("RAKE extracted tags: {:?}", tag_texts);

        // Should extract meaningful phrases
        assert!(!tags.is_empty());
    }
}
