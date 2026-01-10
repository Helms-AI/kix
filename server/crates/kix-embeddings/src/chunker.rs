//! Document chunking strategies for embedding generation.

use kix_parser::{ChunkMetadata, ChunkType, Entry, EntryChunk};

use crate::tagger::{TagExtractionConfig, TagExtractor};

// ============================================================================
// Smart Chunking Types
// ============================================================================

/// A code block for smart chunking (mirrors crawler's CodeBlock)
#[derive(Debug, Clone)]
pub struct CodeBlockInput {
    /// Programming language (if detected)
    pub language: Option<String>,
    /// The code content
    pub content: String,
}

/// A header for smart chunking (mirrors crawler's ExtractedHeader)
#[derive(Debug, Clone)]
pub struct HeaderInput {
    /// Header level (1-6)
    pub level: u8,
    /// Header text
    pub text: String,
}

/// Structured content for smart chunking
#[derive(Debug, Clone, Default)]
pub struct SmartChunkingInput {
    /// Content converted to markdown (or plain text)
    pub markdown: String,
    /// Extracted code blocks (preserved separately)
    pub code_blocks: Vec<CodeBlockInput>,
    /// Extracted headers for hierarchy
    pub headers: Vec<HeaderInput>,
    /// Plain text fallback
    pub plain_text: String,
}

/// Configuration for document chunking.
#[derive(Debug, Clone)]
pub struct ChunkingConfig {
    /// Maximum characters per chunk.
    pub max_chunk_size: usize,

    /// Overlap between consecutive chunks (in characters).
    pub overlap_size: usize,

    /// Whether to create semantic chunks (title + description summary).
    pub semantic_chunking: bool,

    /// Minimum chunk size to include.
    pub min_chunk_size: usize,

    /// Whether to auto-extract tags from content (default: true).
    pub auto_tagging: bool,

    /// Configuration for tag extraction.
    pub tag_config: TagExtractionConfig,
}

impl Default for ChunkingConfig {
    fn default() -> Self {
        Self {
            max_chunk_size: 512,
            overlap_size: 50,
            semantic_chunking: true,
            min_chunk_size: 50,
            auto_tagging: true,
            tag_config: TagExtractionConfig::default(),
        }
    }
}

/// Entry chunker that splits entries into embeddable chunks.
pub struct EntryChunker {
    config: ChunkingConfig,
    tag_extractor: Option<TagExtractor>,
}

impl EntryChunker {
    /// Creates a new entry chunker with the given configuration.
    pub fn new(config: ChunkingConfig) -> Self {
        let tag_extractor = if config.auto_tagging {
            Some(TagExtractor::new(config.tag_config.clone()))
        } else {
            None
        };
        Self { config, tag_extractor }
    }

    /// Creates a chunker with default configuration.
    pub fn with_defaults() -> Self {
        Self::new(ChunkingConfig::default())
    }

    /// Chunks an entry into embeddable pieces.
    ///
    /// If auto_tagging is enabled, this will extract additional tags from
    /// the entry content and merge them with existing tags.
    pub fn chunk(&self, entry: &Entry) -> Vec<EntryChunk> {
        let mut chunks = Vec::new();
        let mut chunk_index = 0u32;

        // Extract and merge tags if auto-tagging is enabled
        let tags = self.extract_tags(entry);

        let metadata = ChunkMetadata {
            entry_title: entry.title.clone(),
            tags,
            entry_type: entry.entry_type.to_string(),
        };

        // Strategy 1: Semantic chunks for discovery
        if self.config.semantic_chunking {
            // Chunk 1: Summary (Title + Description for discovery searches)
            let summary_text = self.build_summary_chunk(entry);
            if summary_text.len() >= self.config.min_chunk_size {
                chunks.push(EntryChunk::new(
                    entry.id.clone(),
                    chunk_index,
                    ChunkType::Summary,
                    summary_text,
                    metadata.clone(),
                ));
                chunk_index += 1;
            }
        }

        // Strategy 2: Sliding window for long content
        let content_chunks = self.sliding_window_chunk(&entry.content);
        for text in content_chunks {
            if text.len() >= self.config.min_chunk_size {
                chunks.push(EntryChunk::new(
                    entry.id.clone(),
                    chunk_index,
                    ChunkType::Content,
                    text,
                    metadata.clone(),
                ));
                chunk_index += 1;
            }
        }

        chunks
    }

    /// Extracts tags from entry content using auto-tagging.
    ///
    /// If auto-tagging is disabled, returns the entry's existing tags.
    /// If enabled, extracts additional tags and merges with existing ones.
    fn extract_tags(&self, entry: &Entry) -> Vec<String> {
        match &self.tag_extractor {
            Some(extractor) => {
                // Combine title, description, and content for tag extraction
                let full_text = format!(
                    "{} {} {}",
                    entry.title,
                    entry.description,
                    entry.content
                );

                // Extract and merge with existing tags
                let extracted = extractor.extract_and_merge(&full_text, &entry.tags);
                TagExtractor::to_string_list(&extracted)
            }
            None => entry.tags.clone(),
        }
    }

    /// Builds a summary chunk combining title and description.
    fn build_summary_chunk(&self, entry: &Entry) -> String {
        let mut parts = Vec::new();
        parts.push(entry.title.clone());

        if !entry.description.is_empty() {
            parts.push(entry.description.clone());
        }

        // Include tags in summary for better search matching
        if !entry.tags.is_empty() {
            parts.push(format!("Tags: {}", entry.tags.join(", ")));
        }

        let combined = parts.join(". ");

        // Truncate if too long
        if combined.len() > self.config.max_chunk_size * 2 {
            combined.chars().take(self.config.max_chunk_size * 2).collect()
        } else {
            combined
        }
    }

    /// Splits text using a sliding window with overlap.
    fn sliding_window_chunk(&self, text: &str) -> Vec<String> {
        let chars: Vec<char> = text.chars().collect();
        let mut chunks = Vec::new();

        if chars.is_empty() {
            return chunks;
        }

        let mut start = 0;

        while start < chars.len() {
            let end = (start + self.config.max_chunk_size).min(chars.len());
            let chunk: String = chars[start..end].iter().collect();

            if !chunk.trim().is_empty() {
                chunks.push(chunk.trim().to_string());
            }

            if end >= chars.len() {
                break;
            }

            // Move start forward, keeping some overlap
            start = if self.config.overlap_size < self.config.max_chunk_size {
                end.saturating_sub(self.config.overlap_size)
            } else {
                end
            };
        }

        chunks
    }

    /// Smart content-aware chunking that respects code blocks and boundaries.
    ///
    /// This method:
    /// 1. Preserves code blocks as separate chunks (never splits them)
    /// 2. Splits text at paragraph boundaries (\n\n)
    /// 3. Falls back to sentence boundaries (. )
    /// 4. Uses sliding window only when necessary
    pub fn chunk_smart(&self, entry: &Entry, structured: &SmartChunkingInput) -> Vec<EntryChunk> {
        let mut chunks = Vec::new();
        let mut chunk_index = 0u32;

        // Extract and merge tags if auto-tagging is enabled
        let tags = self.extract_tags(entry);

        let metadata = ChunkMetadata {
            entry_title: entry.title.clone(),
            tags,
            entry_type: entry.entry_type.to_string(),
        };

        // Strategy 1: Semantic summary chunk for discovery
        if self.config.semantic_chunking {
            let summary_text = self.build_summary_chunk(entry);
            if summary_text.len() >= self.config.min_chunk_size {
                chunks.push(EntryChunk::new(
                    entry.id.clone(),
                    chunk_index,
                    ChunkType::Summary,
                    summary_text,
                    metadata.clone(),
                ));
                chunk_index += 1;
            }
        }

        // Strategy 2: Code blocks as separate chunks (NEVER split)
        for code_block in &structured.code_blocks {
            if code_block.content.len() >= self.config.min_chunk_size {
                // Add language prefix for better semantic search
                let code_text = if let Some(lang) = &code_block.language {
                    format!("```{}\n{}\n```", lang, code_block.content)
                } else {
                    format!("```\n{}\n```", code_block.content)
                };

                chunks.push(EntryChunk::new(
                    entry.id.clone(),
                    chunk_index,
                    ChunkType::Code,
                    code_text,
                    metadata.clone(),
                ));
                chunk_index += 1;
            }
        }

        // Strategy 3: Split markdown content at boundaries
        let text_chunks = self.boundary_aware_chunk(&structured.markdown);
        for text in text_chunks {
            if text.len() >= self.config.min_chunk_size {
                // Detect if chunk is mostly a header
                let chunk_type = if text.starts_with('#') && text.lines().count() <= 3 {
                    ChunkType::Header
                } else {
                    ChunkType::Content
                };

                chunks.push(EntryChunk::new(
                    entry.id.clone(),
                    chunk_index,
                    chunk_type,
                    text,
                    metadata.clone(),
                ));
                chunk_index += 1;
            }
        }

        chunks
    }

    /// Splits text respecting natural boundaries (paragraphs, then sentences).
    fn boundary_aware_chunk(&self, text: &str) -> Vec<String> {
        let mut chunks = Vec::new();
        let max_size = self.config.max_chunk_size;

        // First split by paragraphs (double newlines)
        let paragraphs: Vec<&str> = text.split("\n\n").collect();

        let mut current_chunk = String::new();

        for para in paragraphs {
            let para = para.trim();
            if para.is_empty() {
                continue;
            }

            // If adding this paragraph would exceed max_size
            if !current_chunk.is_empty()
                && current_chunk.len() + para.len() + 2 > max_size
            {
                // Save current chunk and start new one
                if !current_chunk.is_empty() {
                    chunks.push(current_chunk.clone());
                }
                current_chunk = String::new();
            }

            // If paragraph itself is too large, split by sentences
            if para.len() > max_size {
                // Save any accumulated content first
                if !current_chunk.is_empty() {
                    chunks.push(current_chunk.clone());
                    current_chunk = String::new();
                }

                // Split large paragraph by sentences
                let sentence_chunks = self.split_by_sentences(para);
                chunks.extend(sentence_chunks);
            } else {
                // Add paragraph to current chunk
                if !current_chunk.is_empty() {
                    current_chunk.push_str("\n\n");
                }
                current_chunk.push_str(para);
            }
        }

        // Don't forget the last chunk
        if !current_chunk.is_empty() {
            chunks.push(current_chunk);
        }

        chunks
    }

    /// Splits text by sentence boundaries when paragraphs are too large.
    fn split_by_sentences(&self, text: &str) -> Vec<String> {
        let mut chunks = Vec::new();
        let max_size = self.config.max_chunk_size;

        // Simple sentence splitting on ". ", "! ", "? "
        let sentences: Vec<&str> = text
            .split_inclusive(&['.', '!', '?'][..])
            .collect();

        let mut current_chunk = String::new();

        for sentence in sentences {
            let sentence = sentence.trim();
            if sentence.is_empty() {
                continue;
            }

            // If adding this sentence would exceed max_size
            if !current_chunk.is_empty()
                && current_chunk.len() + sentence.len() + 1 > max_size
            {
                chunks.push(current_chunk.clone());
                current_chunk = String::new();
            }

            // If sentence itself is too large, fall back to sliding window
            if sentence.len() > max_size {
                if !current_chunk.is_empty() {
                    chunks.push(current_chunk.clone());
                    current_chunk = String::new();
                }
                // Use sliding window for oversized sentences
                chunks.extend(self.sliding_window_chunk(sentence));
            } else {
                if !current_chunk.is_empty() {
                    current_chunk.push(' ');
                }
                current_chunk.push_str(sentence);
            }
        }

        if !current_chunk.is_empty() {
            chunks.push(current_chunk);
        }

        chunks
    }
}

/// Backward compatibility alias - use EntryChunker instead.
pub type DocumentChunker = EntryChunker;

#[cfg(test)]
mod tests {
    use super::*;
    use kix_parser::SourceType;

    fn create_test_entry() -> Entry {
        Entry::with_id(
            "test-entry-123".to_string(),
            "API Documentation Guide".to_string(),
            "/docs/api-guide.html".to_string(),
            "abc123".to_string(),
        )
        .with_description("A comprehensive guide to using our REST API for application integration.".to_string())
        .with_content("This is the main content of the guide. ".repeat(50))
        .with_tags(vec!["api".to_string(), "documentation".to_string(), "rest".to_string()])
        .with_source_type(SourceType::Html)
    }

    #[test]
    fn test_basic_chunking() {
        let chunker = EntryChunker::with_defaults();
        let entry = create_test_entry();
        let chunks = chunker.chunk(&entry);

        // Should have at least summary and content chunks
        assert!(chunks.len() >= 2);

        // First chunk should be summary type
        assert_eq!(chunks[0].chunk_type, ChunkType::Summary);
        assert!(chunks[0].text.contains("API Documentation"));
    }

    #[test]
    fn test_sliding_window() {
        let chunker = EntryChunker::new(ChunkingConfig {
            max_chunk_size: 100,
            overlap_size: 20,
            ..Default::default()
        });

        let text = "a".repeat(250);
        let chunks = chunker.sliding_window_chunk(&text);

        // Should produce multiple chunks
        assert!(chunks.len() >= 2);

        // Each chunk should be at most max_chunk_size
        for chunk in &chunks {
            assert!(chunk.len() <= 100);
        }
    }

    #[test]
    fn test_chunk_ids() {
        let chunker = EntryChunker::with_defaults();
        let entry = create_test_entry();
        let chunks = chunker.chunk(&entry);

        // Verify chunk IDs are unique and properly formatted
        for (i, chunk) in chunks.iter().enumerate() {
            assert!(chunk.chunk_id.starts_with("test-entry-123#"));
            assert_eq!(chunk.chunk_index, i as u32);
        }
    }

    #[test]
    fn test_summary_includes_tags() {
        let chunker = EntryChunker::with_defaults();
        let entry = create_test_entry();
        let chunks = chunker.chunk(&entry);

        // Summary chunk should include tags
        let summary = &chunks[0];
        assert!(summary.text.contains("api"));
        assert!(summary.text.contains("documentation"));
    }

    #[test]
    fn test_auto_tagging_extracts_keywords() {
        let chunker = EntryChunker::with_defaults();

        // Create entry with meaningful content but minimal tags
        let entry = Entry::with_id(
            "ml-guide".to_string(),
            "Machine Learning Introduction".to_string(),
            "/docs/ml-guide.html".to_string(),
            "xyz789".to_string(),
        )
        .with_description("Learn about neural networks and deep learning fundamentals.".to_string())
        .with_content("Machine learning is transforming software development. \
                       Neural networks enable pattern recognition. \
                       Deep learning models process complex data structures.".to_string())
        .with_tags(vec!["tutorial".to_string()]) // Only one initial tag
        .with_source_type(SourceType::Html);

        let chunks = chunker.chunk(&entry);

        // Check that auto-tagging extracted additional tags
        let chunk_tags = &chunks[0].metadata.tags;
        println!("Extracted tags: {:?}", chunk_tags);

        // Should have more than the initial 1 tag
        assert!(chunk_tags.len() > 1, "Auto-tagging should extract additional tags");

        // Original tag should be preserved
        assert!(chunk_tags.contains(&"tutorial".to_string()));
    }

    #[test]
    fn test_auto_tagging_disabled() {
        let config = ChunkingConfig {
            auto_tagging: false,
            ..Default::default()
        };
        let chunker = EntryChunker::new(config);

        let entry = Entry::with_id(
            "test".to_string(),
            "Test Entry with Meaningful Description".to_string(),
            "/test.html".to_string(),
            "hash".to_string(),
        )
        .with_description("This is a comprehensive description about machine learning and neural networks for deep learning applications.".to_string())
        .with_content("Machine learning neural networks deep learning are important topics in artificial intelligence research.".to_string())
        .with_tags(vec!["original".to_string()])
        .with_source_type(SourceType::Html);

        let chunks = chunker.chunk(&entry);

        // Should have at least one chunk (summary)
        assert!(!chunks.is_empty(), "Should produce at least one chunk");

        // Should only have the original tag when auto-tagging is disabled
        let chunk_tags = &chunks[0].metadata.tags;
        assert_eq!(chunk_tags.len(), 1);
        assert_eq!(chunk_tags[0], "original");
    }
}
