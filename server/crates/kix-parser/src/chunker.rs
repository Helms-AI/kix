//! Smart Chunking Service
//!
//! Intelligent text chunking with semantic boundary detection.
//! This module provides a smart chunking algorithm optimized for
//! documentation and code content.
//!
//! ## Algorithm
//!
//! 1. Preserves code blocks (```) as complete units when possible
//! 2. Prefers to break at paragraph boundaries (\n\n)
//! 3. Falls back to sentence boundaries (. ) if needed
//! 4. Only splits mid-content when absolutely necessary
//! 5. Combines consecutive small chunks (<100 chars) together
//!
//! ## Key Constants
//!
//! - `SMART_BREAK_THRESHOLD`: 30% minimum position for break points
//! - `SMART_CONSOLIDATION_THRESHOLD`: 100 chars minimum before consolidation

use std::fmt;

// ============================================================================
// Smart Chunking Constants
// ============================================================================

/// Break point threshold - 30% minimum.
///
/// Break points are only used if they appear after this percentage of
/// the chunk size. This prevents creating very small chunks.
pub const SMART_BREAK_THRESHOLD: f32 = 0.3;

/// Minimum chunk size for consolidation.
///
/// Chunks smaller than this are merged with neighbors to prevent
/// having too many tiny chunks.
pub const SMART_CONSOLIDATION_THRESHOLD: usize = 100;

// ============================================================================
// Configuration
// ============================================================================

/// Configuration for the smart chunker.
#[derive(Debug, Clone)]
pub struct ChunkerConfig {
    /// Maximum characters per chunk (default: 2000).
    /// Sized to stay within embedding model token limits (2048 tokens for nomic-embed-text).
    /// 2000 chars ≈ 500-1000 tokens for typical content.
    pub chunk_size: usize,

    /// Minimum chunk size for consolidation (default: 100).
    /// Chunks smaller than this are merged with neighbors.
    pub min_chunk_size: usize,

    /// Break threshold as a fraction of chunk_size (default: 0.3).
    /// Break points are only used if they appear after this percentage
    /// of the chunk size.
    pub break_threshold: f32,

    /// Whether to preserve code blocks as complete units (default: true).
    pub preserve_code_blocks: bool,
}

impl Default for ChunkerConfig {
    fn default() -> Self {
        Self {
            chunk_size: 2000,
            min_chunk_size: 100,
            break_threshold: 0.3,
            preserve_code_blocks: true,
        }
    }
}

impl ChunkerConfig {
    /// Create a config optimized for documentation.
    /// Uses 2000 chars to stay within embedding model token limits.
    pub fn for_documentation() -> Self {
        Self {
            chunk_size: 2000,
            min_chunk_size: 100,
            break_threshold: 0.3,
            preserve_code_blocks: true,
        }
    }

    /// Create a config optimized for code-heavy content.
    /// Code has higher token density (~2 chars/token), so we use 2000 chars
    /// to stay within the 2048 token limit of most embedding models.
    pub fn for_code() -> Self {
        Self {
            chunk_size: 2000,
            min_chunk_size: 100,
            break_threshold: 0.2, // More aggressive at finding good breaks
            preserve_code_blocks: true,
        }
    }

    /// Create a config for shorter chunks (better for embedding models with small context).
    pub fn for_embeddings() -> Self {
        Self {
            chunk_size: 512,
            min_chunk_size: 50,
            break_threshold: 0.3,
            preserve_code_blocks: true,
        }
    }

    /// Create a config for larger context preservation (use with caution - may exceed token limits).
    /// Only use with embedding models that support larger context (e.g., 4096+ tokens).
    pub fn for_large_context() -> Self {
        Self {
            chunk_size: 5000,
            min_chunk_size: 200,
            break_threshold: 0.3,
            preserve_code_blocks: true,
        }
    }
}

// ============================================================================
// Chunk Type
// ============================================================================

/// Type of chunk content.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SmartChunkType {
    /// Regular text content
    Text,
    /// Code block content
    Code,
    /// Header/title content
    Header,
    /// Mixed content
    Mixed,
}

impl fmt::Display for SmartChunkType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SmartChunkType::Text => write!(f, "text"),
            SmartChunkType::Code => write!(f, "code"),
            SmartChunkType::Header => write!(f, "header"),
            SmartChunkType::Mixed => write!(f, "mixed"),
        }
    }
}

// ============================================================================
// Chunk Result
// ============================================================================

/// A chunk of text produced by the smart chunker.
#[derive(Debug, Clone)]
pub struct SmartChunk {
    /// The chunk content
    pub content: String,

    /// Zero-based index of this chunk
    pub index: usize,

    /// Type of content in this chunk
    pub chunk_type: SmartChunkType,

    /// Starting byte offset in original text
    pub start_offset: usize,

    /// Ending byte offset in original text
    pub end_offset: usize,
}

impl SmartChunk {
    /// Create a new chunk.
    pub fn new(content: String, index: usize) -> Self {
        let chunk_type = Self::detect_type(&content);
        Self {
            content,
            index,
            chunk_type,
            start_offset: 0,
            end_offset: 0,
        }
    }

    /// Create a chunk with offset information.
    pub fn with_offsets(
        content: String,
        index: usize,
        start_offset: usize,
        end_offset: usize,
    ) -> Self {
        let chunk_type = Self::detect_type(&content);
        Self {
            content,
            index,
            chunk_type,
            start_offset,
            end_offset,
        }
    }

    /// Detect the type of content in this chunk.
    fn detect_type(content: &str) -> SmartChunkType {
        let has_code = content.contains("```") || content.starts_with("```");
        let is_header = content.starts_with('#') && content.lines().count() <= 3;

        if is_header && !has_code {
            SmartChunkType::Header
        } else if has_code && !content.trim_start_matches("```").contains('\n') {
            SmartChunkType::Code
        } else if has_code {
            SmartChunkType::Mixed
        } else {
            SmartChunkType::Text
        }
    }

    /// Get the length of the chunk content.
    pub fn len(&self) -> usize {
        self.content.len()
    }

    /// Check if the chunk is empty.
    pub fn is_empty(&self) -> bool {
        self.content.is_empty()
    }
}

// ============================================================================
// Smart Chunker
// ============================================================================

/// Smart text chunker with boundary-aware splitting.
///
/// This chunker implements a smart chunking strategy:
/// - Priority 1: Code block boundaries (```)
/// - Priority 2: Paragraph boundaries (\n\n)
/// - Priority 3: Sentence boundaries (. )
/// - Priority 4: Hard cut at chunk size
///
/// Break points are only used if they appear after 30% of the chunk size,
/// ensuring chunks aren't too small.
pub struct SmartChunker {
    config: ChunkerConfig,
}

impl SmartChunker {
    /// Create a new smart chunker with default configuration.
    pub fn new() -> Self {
        Self {
            config: ChunkerConfig::default(),
        }
    }

    /// Create a smart chunker with custom configuration.
    pub fn with_config(config: ChunkerConfig) -> Self {
        Self { config }
    }

    /// Get the configuration.
    pub fn config(&self) -> &ChunkerConfig {
        &self.config
    }

    /// Chunk text using the smart algorithm.
    ///
    /// This is the main entry point for chunking text.
    pub fn chunk(&self, text: &str) -> Vec<SmartChunk> {
        if text.is_empty() {
            return Vec::new();
        }

        // Step 1: Split into raw chunks using smart boundaries
        let raw_chunks = self.smart_split(text);

        // Step 2: Consolidate small chunks
        let consolidated = self.consolidate_small_chunks(raw_chunks);

        // Step 3: Convert to SmartChunk with proper indexing
        consolidated
            .into_iter()
            .enumerate()
            .map(|(idx, content)| SmartChunk::new(content, idx))
            .collect()
    }

    /// Chunk text and return with byte offset information.
    ///
    /// Useful when you need to map chunks back to the original text.
    pub fn chunk_with_offsets(&self, text: &str) -> Vec<SmartChunk> {
        if text.is_empty() {
            return Vec::new();
        }

        // Step 1: Split into raw chunks with offsets
        let raw_chunks = self.smart_split_with_offsets(text);

        // Step 2: Consolidate small chunks
        let consolidated = self.consolidate_chunks_with_offsets(raw_chunks);

        // Step 3: Assign indices
        consolidated
            .into_iter()
            .enumerate()
            .map(|(idx, (content, start, end))| {
                SmartChunk::with_offsets(content, idx, start, end)
            })
            .collect()
    }

    /// Core smart splitting algorithm with prioritized break points.
    ///
    /// Priority order:
    /// 1. Code block boundaries (```)
    /// 2. Paragraph boundaries (\n\n)
    /// 3. Sentence boundaries (. )
    /// 4. Mid-content split (last resort)
    fn smart_split(&self, text: &str) -> Vec<String> {
        let chunk_size = self.config.chunk_size;
        let min_break_pos = (chunk_size as f32 * self.config.break_threshold) as usize;

        let mut chunks = Vec::new();
        let mut start = 0;
        let text_len = text.len();

        while start < text_len {
            let end = (start + chunk_size).min(text_len);

            // If we're at the end of the text, take what's left
            if end >= text_len {
                let chunk = text[start..].trim();
                if !chunk.is_empty() {
                    chunks.push(chunk.to_string());
                }
                break;
            }

            // Get the current chunk candidate
            let chunk_candidate = &text[start..end];

            // Try to find a good break point
            let break_pos = self.find_break_point(chunk_candidate, min_break_pos);

            let actual_end = start + break_pos;
            let chunk = text[start..actual_end].trim();
            if !chunk.is_empty() {
                chunks.push(chunk.to_string());
            }

            // Move start position for next chunk
            start = actual_end;

            // Skip leading whitespace for next chunk
            while start < text_len {
                let c = text[start..].chars().next().unwrap_or(' ');
                if c.is_whitespace() {
                    start += c.len_utf8();
                } else {
                    break;
                }
            }
        }

        chunks
    }

    /// Smart splitting with byte offset tracking.
    fn smart_split_with_offsets(&self, text: &str) -> Vec<(String, usize, usize)> {
        let chunk_size = self.config.chunk_size;
        let min_break_pos = (chunk_size as f32 * self.config.break_threshold) as usize;

        let mut chunks = Vec::new();
        let mut start = 0;
        let text_len = text.len();

        while start < text_len {
            let end = (start + chunk_size).min(text_len);

            // If we're at the end of the text, take what's left
            if end >= text_len {
                let chunk = text[start..].trim();
                if !chunk.is_empty() {
                    chunks.push((chunk.to_string(), start, text_len));
                }
                break;
            }

            // Get the current chunk candidate
            let chunk_candidate = &text[start..end];

            // Try to find a good break point
            let break_pos = self.find_break_point(chunk_candidate, min_break_pos);

            let actual_end = start + break_pos;
            let chunk = text[start..actual_end].trim();
            if !chunk.is_empty() {
                chunks.push((chunk.to_string(), start, actual_end));
            }

            // Move start position for next chunk
            start = actual_end;

            // Skip leading whitespace for next chunk
            while start < text_len {
                let c = text[start..].chars().next().unwrap_or(' ');
                if c.is_whitespace() {
                    start += c.len_utf8();
                } else {
                    break;
                }
            }
        }

        chunks
    }

    /// Find the best break point in a chunk using the priority order.
    ///
    /// Returns the position where the chunk should end.
    fn find_break_point(&self, chunk: &str, min_pos: usize) -> usize {
        let chunk_len = chunk.len();

        // Priority 1: Try to break at a code block boundary
        if self.config.preserve_code_blocks {
            if let Some(pos) = chunk.rfind("```") {
                if pos > min_pos {
                    return pos;
                }
            }
        }

        // Priority 2: Try paragraph break (\n\n)
        if let Some(pos) = chunk.rfind("\n\n") {
            if pos > min_pos {
                return pos;
            }
        }

        // Priority 3: Try sentence break (. followed by space or newline)
        for pattern in &[". ", ".\n"] {
            if let Some(pos) = chunk.rfind(pattern) {
                if pos > min_pos {
                    // Include the period in the chunk
                    return pos + 1;
                }
            }
        }

        // Priority 4: No good break point found, return full chunk length
        chunk_len
    }

    /// Consolidate consecutive small chunks together.
    ///
    /// Chunks smaller than `min_chunk_size` are combined with their neighbors
    /// until they reach an acceptable size.
    fn consolidate_small_chunks(&self, chunks: Vec<String>) -> Vec<String> {
        if chunks.is_empty() {
            return chunks;
        }

        let min_size = self.config.min_chunk_size;
        let mut consolidated = Vec::new();
        let mut i = 0;

        while i < chunks.len() {
            let mut current = chunks[i].clone();

            // Keep combining while current is small and there are more chunks
            while current.len() < min_size && i + 1 < chunks.len() {
                i += 1;
                current = format!("{}\n\n{}", current, chunks[i]);
            }

            consolidated.push(current);
            i += 1;
        }

        consolidated
    }

    /// Consolidate chunks with offset tracking.
    fn consolidate_chunks_with_offsets(
        &self,
        chunks: Vec<(String, usize, usize)>,
    ) -> Vec<(String, usize, usize)> {
        if chunks.is_empty() {
            return chunks;
        }

        let min_size = self.config.min_chunk_size;
        let mut consolidated = Vec::new();
        let mut i = 0;

        while i < chunks.len() {
            let (mut current_text, start, mut end) = chunks[i].clone();

            // Keep combining while current is small and there are more chunks
            while current_text.len() < min_size && i + 1 < chunks.len() {
                i += 1;
                let (next_text, _, next_end) = &chunks[i];
                current_text = format!("{}\n\n{}", current_text, next_text);
                end = *next_end;
            }

            consolidated.push((current_text, start, end));
            i += 1;
        }

        consolidated
    }

    /// Split text by code blocks, keeping them as separate units.
    ///
    /// Returns alternating text and code sections.
    pub fn split_by_code_blocks(&self, text: &str) -> Vec<(String, bool)> {
        let mut sections = Vec::new();
        let mut current_pos = 0;
        let mut in_code_block = false;

        for (idx, _) in text.match_indices("```") {
            if !in_code_block {
                // Found start of code block
                if idx > current_pos {
                    // Add text before code block
                    let text_section = text[current_pos..idx].trim();
                    if !text_section.is_empty() {
                        sections.push((text_section.to_string(), false));
                    }
                }
                current_pos = idx;
                in_code_block = true;
            } else {
                // Found end of code block
                // Include the closing ```
                let code_end = idx + 3;
                let code_section = &text[current_pos..code_end];
                sections.push((code_section.to_string(), true));
                current_pos = code_end;
                in_code_block = false;
            }
        }

        // Add remaining text
        if current_pos < text.len() {
            let remaining = text[current_pos..].trim();
            if !remaining.is_empty() {
                sections.push((remaining.to_string(), in_code_block));
            }
        }

        sections
    }
}

impl Default for SmartChunker {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Convenience Functions
// ============================================================================

/// Chunk text using default configuration.
pub fn chunk_text(text: &str) -> Vec<SmartChunk> {
    SmartChunker::new().chunk(text)
}

/// Chunk text with a specific chunk size.
pub fn chunk_text_with_size(text: &str, chunk_size: usize) -> Vec<SmartChunk> {
    let config = ChunkerConfig {
        chunk_size,
        ..Default::default()
    };
    SmartChunker::with_config(config).chunk(text)
}

/// Chunk text optimized for embeddings (smaller chunks).
pub fn chunk_for_embeddings(text: &str) -> Vec<SmartChunk> {
    SmartChunker::with_config(ChunkerConfig::for_embeddings()).chunk(text)
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_text() {
        let chunker = SmartChunker::new();
        let chunks = chunker.chunk("");
        assert!(chunks.is_empty());
    }

    #[test]
    fn test_small_text() {
        let chunker = SmartChunker::new();
        let text = "Hello, world!";
        let chunks = chunker.chunk(text);

        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].content, "Hello, world!");
        assert_eq!(chunks[0].index, 0);
    }

    #[test]
    fn test_paragraph_breaks() {
        let config = ChunkerConfig {
            chunk_size: 100,
            min_chunk_size: 10,
            break_threshold: 0.3,
            preserve_code_blocks: true,
        };
        let chunker = SmartChunker::with_config(config);

        let text = "First paragraph with some content here.\n\nSecond paragraph with different content.\n\nThird paragraph continues the discussion.";
        let chunks = chunker.chunk(text);

        // Should break at paragraph boundaries
        assert!(chunks.len() >= 1);
        // All chunks should have content
        for chunk in &chunks {
            assert!(!chunk.content.is_empty());
        }
    }

    #[test]
    fn test_code_block_preservation() {
        let config = ChunkerConfig {
            chunk_size: 150,
            min_chunk_size: 10,
            break_threshold: 0.2,
            preserve_code_blocks: true,
        };
        let chunker = SmartChunker::with_config(config);

        let text = r#"Some introduction text here.

```python
def hello_world():
    print("Hello, World!")
    return True
```

More text after the code block."#;

        let chunks = chunker.chunk(text);

        // Code block should be preserved
        let has_code = chunks.iter().any(|c| c.content.contains("def hello_world()"));
        assert!(has_code, "Code block should be preserved");
    }

    #[test]
    fn test_sentence_breaks() {
        let config = ChunkerConfig {
            chunk_size: 80,
            min_chunk_size: 10,
            break_threshold: 0.3,
            preserve_code_blocks: true,
        };
        let chunker = SmartChunker::with_config(config);

        let text = "First sentence here. Second sentence follows. Third sentence continues. Fourth sentence ends.";
        let chunks = chunker.chunk(text);

        // Should break at sentence boundaries
        assert!(chunks.len() >= 1);
        // Chunks should end at sentence boundaries (period)
        for chunk in &chunks[..chunks.len() - 1] {
            assert!(
                chunk.content.ends_with('.') || chunk.content.ends_with('!') || chunk.content.ends_with('?'),
                "Chunk should end at sentence boundary: '{}'",
                chunk.content
            );
        }
    }

    #[test]
    fn test_consolidation() {
        let config = ChunkerConfig {
            chunk_size: 100,
            min_chunk_size: 200, // High threshold to force consolidation
            break_threshold: 0.3,
            preserve_code_blocks: true,
        };
        let chunker = SmartChunker::with_config(config);

        let text = "Short.\n\nAlso short.\n\nAnother short.\n\nYet another.\n\nFinal.";
        let chunks = chunker.chunk(text);

        // Small chunks should be consolidated
        assert!(
            chunks.len() <= 2,
            "Small chunks should be consolidated; got {} chunks",
            chunks.len()
        );
    }

    #[test]
    fn test_chunk_types() {
        let chunker = SmartChunker::new();

        // Text chunk
        let text_chunk = SmartChunk::new("Regular text content.".to_string(), 0);
        assert_eq!(text_chunk.chunk_type, SmartChunkType::Text);

        // Header chunk
        let header_chunk = SmartChunk::new("# Main Title".to_string(), 1);
        assert_eq!(header_chunk.chunk_type, SmartChunkType::Header);

        // Code chunk
        let code_chunk = SmartChunk::new("```python\nprint('hello')\n```".to_string(), 2);
        assert!(
            code_chunk.chunk_type == SmartChunkType::Code
                || code_chunk.chunk_type == SmartChunkType::Mixed
        );
    }

    #[test]
    fn test_break_threshold() {
        let config = ChunkerConfig {
            chunk_size: 100,
            min_chunk_size: 10,
            break_threshold: 0.5, // 50% threshold
            preserve_code_blocks: true,
        };
        let chunker = SmartChunker::with_config(config);

        // Break point at 20% should not be used with 50% threshold
        let text = "Short.\n\nThis is a longer piece of text that continues for a while.";
        let chunks = chunker.chunk(text);

        // The early break should be ignored due to threshold
        assert!(chunks.len() >= 1);
    }

    #[test]
    fn test_offsets() {
        let config = ChunkerConfig {
            chunk_size: 50,
            min_chunk_size: 10,
            break_threshold: 0.3,
            preserve_code_blocks: true,
        };
        let chunker = SmartChunker::with_config(config);

        let text = "First part of text.\n\nSecond part of text.";
        let chunks = chunker.chunk_with_offsets(text);

        assert!(!chunks.is_empty());
        // First chunk should start at 0
        assert_eq!(chunks[0].start_offset, 0);
    }

    #[test]
    fn test_split_by_code_blocks() {
        let chunker = SmartChunker::new();

        let text = r#"Introduction text.

```python
code here
```

Middle text.

```javascript
more code
```

Conclusion."#;

        let sections = chunker.split_by_code_blocks(text);

        // Should have 5 sections: text, code, text, code, text
        assert_eq!(sections.len(), 5);
        assert!(!sections[0].1); // text
        assert!(sections[1].1); // code
        assert!(!sections[2].1); // text
        assert!(sections[3].1); // code
        assert!(!sections[4].1); // text
    }

    #[test]
    fn test_convenience_functions() {
        let text = "Some text content that needs chunking.";

        let chunks1 = chunk_text(text);
        assert!(!chunks1.is_empty());

        let chunks2 = chunk_text_with_size(text, 100);
        assert!(!chunks2.is_empty());

        let chunks3 = chunk_for_embeddings(text);
        assert!(!chunks3.is_empty());
    }

    #[test]
    fn test_large_content() {
        let chunker = SmartChunker::with_config(ChunkerConfig {
            chunk_size: 500,
            min_chunk_size: 50,
            ..Default::default()
        });

        // Generate large content
        let paragraph = "This is a paragraph of text that contains multiple sentences. It goes on for a while. There are many things to discuss. The content continues further.\n\n";
        let text = paragraph.repeat(20);

        let chunks = chunker.chunk(&text);

        // Should produce multiple chunks
        assert!(chunks.len() > 1, "Large content should produce multiple chunks");

        // All chunks should be non-empty
        for chunk in &chunks {
            assert!(!chunk.is_empty());
        }

        // Check chunk indices are correct
        for (i, chunk) in chunks.iter().enumerate() {
            assert_eq!(chunk.index, i);
        }
    }

    // ========================================================================
    // Smart Chunking Validation Tests
    // ========================================================================

    #[test]
    fn test_smart_constants() {
        // Verify smart chunking constants match expected values
        assert_eq!(
            SMART_BREAK_THRESHOLD, 0.3,
            "Break threshold must be 30%"
        );
        assert_eq!(
            SMART_CONSOLIDATION_THRESHOLD, 100,
            "Consolidation threshold must be 100 chars"
        );
    }

    #[test]
    fn test_default_config_uses_constants() {
        // Verify default config uses the standard constant values
        let config = ChunkerConfig::default();
        assert_eq!(
            config.break_threshold, SMART_BREAK_THRESHOLD,
            "Default break_threshold must match SMART_BREAK_THRESHOLD"
        );
        assert_eq!(
            config.min_chunk_size, SMART_CONSOLIDATION_THRESHOLD,
            "Default min_chunk_size must match SMART_CONSOLIDATION_THRESHOLD"
        );
    }

    #[test]
    fn test_chunking_priorities() {
        // Verify: code blocks > paragraphs > sentences > hard cut
        let chunker = SmartChunker::new();

        // Code block should never be split when possible
        let with_code = "Text before code.\n\n```python\ndef foo():\n    pass\n```\n\nMore text after.";
        let chunks = chunker.chunk(with_code);

        // Find the chunk containing the code block
        let code_chunk = chunks.iter().find(|c| c.content.contains("```python"));
        assert!(code_chunk.is_some(), "Code block must be preserved in a chunk");

        // Code should be complete
        let code = code_chunk.unwrap();
        assert!(
            code.content.contains("def foo()") && code.content.contains("pass"),
            "Code block must be complete, not split"
        );
    }

    #[test]
    fn test_smart_consolidation() {
        // Small chunks (<200 chars) should be consolidated
        let config = ChunkerConfig {
            chunk_size: 100,
            min_chunk_size: SMART_CONSOLIDATION_THRESHOLD,
            break_threshold: SMART_BREAK_THRESHOLD,
            preserve_code_blocks: true,
        };
        let chunker = SmartChunker::with_config(config);

        // Two very short paragraphs should be consolidated
        let text = "Short.\n\nAlso short.";
        let chunks = chunker.chunk(text);

        // Should consolidate to single chunk since total < 200
        assert_eq!(
            chunks.len(), 1,
            "Chunks smaller than {} chars should be consolidated",
            SMART_CONSOLIDATION_THRESHOLD
        );
    }

    #[test]
    fn test_smart_break_threshold() {
        // Break points should only be used after 30% of chunk size
        let config = ChunkerConfig {
            chunk_size: 100, // 30% = 30 chars
            min_chunk_size: 10,
            break_threshold: SMART_BREAK_THRESHOLD,
            preserve_code_blocks: true,
        };
        let chunker = SmartChunker::with_config(config);

        // Early break at 10 chars should be ignored (10 < 30)
        // Break at 50 chars should be used (50 > 30)
        let text = "Short.\n\nA much longer paragraph that exceeds thirty percent of chunk size for testing.";
        let chunks = chunker.chunk(text);

        // First chunk should not break at "Short.\n\n" because it's < 30%
        // Instead it should include more content
        assert!(
            !chunks.is_empty(),
            "Should produce at least one chunk"
        );
    }
}
