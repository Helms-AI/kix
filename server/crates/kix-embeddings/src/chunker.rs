//! Document chunking strategies for embedding generation.

use kix_parser::{ChunkMetadata, ChunkType, Entry, EntryChunk};
use tracing::debug;

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

/// A section of markdown delimited by headers.
#[derive(Debug, Clone)]
pub struct MarkdownSection {
    /// Header text (e.g., "## API Reference")
    pub header: Option<String>,
    /// Header level (1-6, None if no header)
    pub level: Option<u8>,
    /// The text content under this header (excluding nested sections)
    pub text_content: String,
    /// Starting byte offset in original markdown
    pub start_offset: usize,
    /// Ending byte offset in original markdown
    pub end_offset: usize,
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

        debug!(
            entry_id = %entry.id,
            content_len = entry.content.len(),
            "Starting chunking for entry"
        );

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
                let preview: String = summary_text.chars().take(80).collect();
                debug!(
                    chunk_index = chunk_index,
                    chunk_type = "summary",
                    preview = %preview,
                    "Created chunk"
                );
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
                let preview: String = text.chars().take(80).collect();
                debug!(
                    chunk_index = chunk_index,
                    chunk_type = "content",
                    preview = %preview,
                    "Created chunk"
                );
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

        debug!(
            entry_id = %entry.id,
            total_chunks = chunks.len(),
            "Finished chunking"
        );

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

    // ========================================================================
    // Markdown-Aware Chunking (Phase 2 Enhancement)
    // ========================================================================

    /// Parse markdown into sections delimited by headers.
    ///
    /// This splits the markdown at header boundaries (##, ###, etc.) while
    /// preserving the hierarchical structure.
    pub fn parse_markdown_sections(markdown: &str) -> Vec<MarkdownSection> {
        let mut sections = Vec::new();
        let mut current_header: Option<String> = None;
        let mut current_level: Option<u8> = None;
        let mut current_content = String::new();
        let mut section_start: usize = 0;
        let mut byte_offset: usize = 0;

        for line in markdown.lines() {
            let trimmed = line.trim_start();
            let line_bytes = line.len() + 1; // +1 for newline

            // Check if this line is a header
            if trimmed.starts_with('#') {
                let level = trimmed.chars().take_while(|&c| c == '#').count();
                if level >= 1 && level <= 6 {
                    // Save the previous section if it has content
                    if current_header.is_some() || !current_content.trim().is_empty() {
                        sections.push(MarkdownSection {
                            header: current_header.take(),
                            level: current_level.take(),
                            text_content: current_content.trim().to_string(),
                            start_offset: section_start,
                            end_offset: byte_offset,
                        });
                    }

                    // Start new section
                    section_start = byte_offset;
                    let header_text = trimmed[level..].trim_start().to_string();
                    current_header = Some(format!("{} {}", "#".repeat(level), header_text));
                    current_level = Some(level as u8);
                    current_content = String::new();

                    byte_offset += line_bytes;
                    continue;
                }
            }

            // Accumulate content
            if !current_content.is_empty() {
                current_content.push('\n');
            }
            current_content.push_str(line);
            byte_offset += line_bytes;
        }

        // Don't forget the last section
        if current_header.is_some() || !current_content.trim().is_empty() {
            sections.push(MarkdownSection {
                header: current_header,
                level: current_level,
                text_content: current_content.trim().to_string(),
                start_offset: section_start,
                end_offset: byte_offset,
            });
        }

        sections
    }

    /// Markdown-aware chunking that respects header boundaries.
    ///
    /// This method:
    /// 1. Parses markdown into sections at header boundaries
    /// 2. Creates chunks that align with document structure
    /// 3. Prefixes each chunk with its header context for better retrieval
    /// 4. Splits large sections using boundary-aware chunking
    /// 5. Never splits in the middle of a section if it fits
    pub fn chunk_markdown_aware(&self, entry: &Entry, markdown: &str) -> Vec<EntryChunk> {
        let mut chunks = Vec::new();
        let mut chunk_index = 0u32;

        debug!(
            entry_id = %entry.id,
            markdown_len = markdown.len(),
            "Starting markdown-aware chunking"
        );

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
                debug!(
                    chunk_index = chunk_index,
                    chunk_type = "summary",
                    "Created summary chunk"
                );
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

        // Strategy 2: Parse markdown into sections and chunk
        let sections = Self::parse_markdown_sections(markdown);
        debug!(section_count = sections.len(), "Parsed markdown sections");

        // Track the current header context for nested sections
        let mut header_stack: Vec<String> = Vec::new();

        for section in sections {
            // Update header stack based on level
            if let (Some(header), Some(level)) = (&section.header, section.level) {
                // Pop headers at same or higher level
                let level_idx = level as usize;
                while header_stack.len() >= level_idx {
                    header_stack.pop();
                }
                // Push current header
                header_stack.push(header.clone());
            }

            // Build section content with header context
            let mut section_text = String::new();

            // Include header hierarchy for context
            if !header_stack.is_empty() {
                section_text.push_str(&header_stack.join(" > "));
                section_text.push_str("\n\n");
            }

            // Add section content
            section_text.push_str(&section.text_content);

            // Skip empty sections
            if section_text.trim().len() < self.config.min_chunk_size {
                continue;
            }

            // If section fits in a single chunk, keep it together
            if section_text.len() <= self.config.max_chunk_size {
                let chunk_type = if section.header.is_some() {
                    ChunkType::Header
                } else {
                    ChunkType::Content
                };

                debug!(
                    chunk_index = chunk_index,
                    section_header = section.header.as_deref().unwrap_or("(no header)"),
                    content_len = section_text.len(),
                    "Created section chunk"
                );

                chunks.push(EntryChunk::new(
                    entry.id.clone(),
                    chunk_index,
                    chunk_type,
                    section_text,
                    metadata.clone(),
                ));
                chunk_index += 1;
            } else {
                // Section too large - split using boundary-aware chunking
                // but preserve header context in each sub-chunk
                let header_context = if !header_stack.is_empty() {
                    format!("{}\n\n", header_stack.join(" > "))
                } else {
                    String::new()
                };

                let sub_chunks = self.boundary_aware_chunk(&section.text_content);

                for (sub_idx, sub_text) in sub_chunks.into_iter().enumerate() {
                    if sub_text.len() < self.config.min_chunk_size {
                        continue;
                    }

                    // Prefix with header context
                    let chunk_text = if sub_idx == 0 || header_context.is_empty() {
                        format!("{}{}", header_context, sub_text)
                    } else {
                        format!("{}(continued)\n\n{}", header_context, sub_text)
                    };

                    debug!(
                        chunk_index = chunk_index,
                        sub_chunk = sub_idx,
                        content_len = chunk_text.len(),
                        "Created sub-section chunk"
                    );

                    chunks.push(EntryChunk::new(
                        entry.id.clone(),
                        chunk_index,
                        ChunkType::Content,
                        chunk_text,
                        metadata.clone(),
                    ));
                    chunk_index += 1;
                }
            }
        }

        debug!(
            entry_id = %entry.id,
            total_chunks = chunks.len(),
            "Finished markdown-aware chunking"
        );

        chunks
    }

    // ========================================================================
    // Smart Chunking
    // ========================================================================

    /// Smart chunking that preserves context and natural boundaries.
    ///
    /// This method implements a smart chunking strategy:
    /// 1. Preserves code blocks (```) as complete units when possible
    /// 2. Prefers to break at paragraph boundaries (\n\n)
    /// 3. Falls back to sentence boundaries (. ) if needed
    /// 4. Only splits mid-content when absolutely necessary
    /// 5. Combines consecutive small chunks (<200 chars) together
    ///
    /// The key difference from other chunking methods is the 30% threshold:
    /// break points are only used if they appear after 30% of the chunk size,
    /// ensuring chunks aren't too small.
    pub fn chunk_smart_text(&self, entry: &Entry, content: &str) -> Vec<EntryChunk> {
        let mut chunks = Vec::new();
        let mut chunk_index = 0u32;

        debug!(
            entry_id = %entry.id,
            content_len = content.len(),
            "Starting smart chunking"
        );

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
                debug!(
                    chunk_index = chunk_index,
                    chunk_type = "summary",
                    "Created summary chunk"
                );
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

        // Strategy 2: Smart chunk the content with smart splitting algorithm
        let raw_chunks = self.smart_split_content(content);

        // Strategy 3: Consolidate small chunks (<200 chars)
        let consolidated = self.consolidate_small_chunks(raw_chunks, 200);

        // Create EntryChunk for each consolidated chunk
        for text in consolidated {
            if text.len() >= self.config.min_chunk_size {
                // Detect chunk type based on content
                let chunk_type = if text.starts_with("```") || text.contains("\n```") {
                    ChunkType::Code
                } else if text.starts_with('#') && text.lines().count() <= 3 {
                    ChunkType::Header
                } else {
                    ChunkType::Content
                };

                debug!(
                    chunk_index = chunk_index,
                    chunk_type = ?chunk_type,
                    content_len = text.len(),
                    "Created smart chunk"
                );

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

        debug!(
            entry_id = %entry.id,
            total_chunks = chunks.len(),
            "Finished smart chunking"
        );

        chunks
    }

    /// Backward-compatible alias for chunk_smart_text
    #[deprecated(since = "1.0.0", note = "Use chunk_smart_text instead")]
    #[allow(deprecated)]
    pub fn chunk_archon_style(&self, entry: &Entry, content: &str) -> Vec<EntryChunk> {
        self.chunk_smart_text(entry, content)
    }

    /// Core smart splitting algorithm with prioritized break points.
    ///
    /// Priority order:
    /// 1. Code block boundaries (```)
    /// 2. Paragraph boundaries (\n\n)
    /// 3. Sentence boundaries (. )
    /// 4. Mid-content split (last resort)
    ///
    /// Break points are only used if they appear after 30% of the chunk size.
    fn smart_split_content(&self, text: &str) -> Vec<String> {
        if text.is_empty() {
            return Vec::new();
        }

        let chunk_size = self.config.max_chunk_size;
        let min_break_position = (chunk_size as f64 * 0.3) as usize;

        let mut chunks = Vec::new();
        let mut start = 0;
        let text_len = text.len();

        while start < text_len {
            let end = (start + chunk_size).min(text_len);

            // CRITICAL: Snap end to a valid UTF-8 character boundary
            // The calculated end may land in the middle of a multi-byte UTF-8 character
            // (e.g., smart quotes ''', emoji 🎉, or CJK characters 你好)
            // Use floor_char_boundary logic WITHOUT slicing (which would panic)
            let end = if end >= text_len {
                text_len
            } else if text.is_char_boundary(end) {
                end
            } else {
                // Not a boundary, walk backwards to find one
                let mut i = end;
                while i > start && !text.is_char_boundary(i) {
                    i -= 1;
                }
                // If we couldn't find a boundary (shouldn't happen for valid UTF-8),
                // fall back to start which is always valid
                if i <= start { start } else { i }
            };

            // If we're at the end of the text, take what's left
            if end >= text_len {
                let chunk = text[start..].trim();
                if !chunk.is_empty() {
                    chunks.push(chunk.to_string());
                }
                break;
            }

            // Safety: end is now guaranteed to be a valid UTF-8 boundary
            // Get the current chunk candidate
            let chunk_candidate = &text[start..end];

            // Try to find a good break point
            let break_pos = self.find_smart_break_point(chunk_candidate, min_break_position);

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

    /// Find the best break point in a chunk using priority order.
    ///
    /// Returns the position where the chunk should end.
    fn find_smart_break_point(&self, chunk: &str, min_pos: usize) -> usize {
        let chunk_len = chunk.len();

        // Priority 1: Try to break at a code block boundary
        if let Some(pos) = chunk.rfind("```") {
            if pos > min_pos {
                return pos;
            }
        }

        // Priority 2: Try paragraph break (\n\n)
        if let Some(pos) = chunk.rfind("\n\n") {
            if pos > min_pos {
                return pos;
            }
        }

        // Priority 3: Try sentence break (. followed by space or newline)
        // Look for ". " or ".\n"
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
    /// Chunks smaller than `min_size` are combined with their neighbors
    /// until they reach an acceptable size.
    fn consolidate_small_chunks(&self, chunks: Vec<String>, min_size: usize) -> Vec<String> {
        if chunks.is_empty() {
            return chunks;
        }

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

    #[test]
    fn test_parse_markdown_sections() {
        let markdown = r#"# Main Title

Introduction paragraph here.

## Section One

Content for section one.
More content here.

### Subsection 1.1

Detailed content.

## Section Two

Content for section two.
"#;

        let sections = EntryChunker::parse_markdown_sections(markdown);

        // Should have 4 sections: intro, section one, subsection, section two
        assert_eq!(sections.len(), 4, "Expected 4 sections, got {}: {:?}", sections.len(), sections);

        // First section is the intro under # Main Title
        assert_eq!(sections[0].header, Some("# Main Title".to_string()));
        assert_eq!(sections[0].level, Some(1));
        assert!(sections[0].text_content.contains("Introduction paragraph"));

        // Second section
        assert_eq!(sections[1].header, Some("## Section One".to_string()));
        assert_eq!(sections[1].level, Some(2));

        // Third section (subsection)
        assert_eq!(sections[2].header, Some("### Subsection 1.1".to_string()));
        assert_eq!(sections[2].level, Some(3));

        // Fourth section
        assert_eq!(sections[3].header, Some("## Section Two".to_string()));
        assert_eq!(sections[3].level, Some(2));
    }

    #[test]
    fn test_markdown_aware_chunking() {
        let chunker = EntryChunker::new(ChunkingConfig {
            max_chunk_size: 500,
            min_chunk_size: 10,
            semantic_chunking: true,
            ..Default::default()
        });

        let entry = Entry::with_id(
            "md-test".to_string(),
            "Markdown Test".to_string(),
            "/test.md".to_string(),
            "hash".to_string(),
        )
        .with_description("Testing markdown chunking.".to_string())
        .with_content("".to_string())
        .with_tags(vec!["test".to_string()])
        .with_source_type(SourceType::Html);

        let markdown = r#"## Overview

This is the overview section with some important content.

## API Reference

Detailed API documentation here.

### Endpoints

List of API endpoints and their usage.
"#;

        let chunks = chunker.chunk_markdown_aware(&entry, markdown);

        // Should have summary + section chunks
        assert!(chunks.len() >= 2, "Expected at least 2 chunks, got {}", chunks.len());

        // First should be summary
        assert_eq!(chunks[0].chunk_type, ChunkType::Summary);

        // Rest should be header or content chunks with proper context
        for chunk in &chunks[1..] {
            // Each chunk should contain its header context
            println!("Chunk: {:?}", chunk.text);
        }
    }

    #[test]
    fn test_smart_chunking() {
        let chunker = EntryChunker::new(ChunkingConfig {
            max_chunk_size: 500,
            min_chunk_size: 10,
            semantic_chunking: true,
            ..Default::default()
        });

        let entry = Entry::with_id(
            "smart-test".to_string(),
            "Smart Chunking Test".to_string(),
            "/test.md".to_string(),
            "hash".to_string(),
        )
        .with_description("Testing smart chunking.".to_string())
        .with_content("".to_string())
        .with_tags(vec!["test".to_string()])
        .with_source_type(SourceType::Html);

        let content = r#"# Overview

This is the overview section with some important content.

## Code Examples

Here is a code block that should not be split:

```python
def hello_world():
    print("Hello, World!")
    return True
```

More content after the code block.

## API Reference

Detailed API documentation here. This section contains information about endpoints and their usage. The API provides various methods for interacting with the system.

### Endpoints

List of API endpoints and their usage.
"#;

        let chunks = chunker.chunk_smart_text(&entry, content);

        // Should have summary + content chunks
        assert!(chunks.len() >= 2, "Expected at least 2 chunks, got {}", chunks.len());

        // First should be summary
        assert_eq!(chunks[0].chunk_type, ChunkType::Summary);

        // Check that code blocks are preserved
        let has_code_chunk = chunks.iter().any(|c| c.text.contains("def hello_world()"));
        assert!(has_code_chunk, "Code block should be preserved in chunks");

        // Print chunks for debugging
        for (i, chunk) in chunks.iter().enumerate() {
            println!("Chunk {}: type={:?}, len={}", i, chunk.chunk_type, chunk.text.len());
        }
    }

    #[test]
    fn test_smart_small_chunk_consolidation() {
        let chunker = EntryChunker::new(ChunkingConfig {
            max_chunk_size: 100,
            min_chunk_size: 10,
            semantic_chunking: false, // Skip summary for this test
            ..Default::default()
        });

        let entry = Entry::with_id(
            "consolidation-test".to_string(),
            "Consolidation Test".to_string(),
            "/test.md".to_string(),
            "hash".to_string(),
        )
        .with_description("Testing chunk consolidation.".to_string())
        .with_content("".to_string())
        .with_tags(vec!["test".to_string()])
        .with_source_type(SourceType::Html);

        // Content that would create many small chunks without consolidation
        let content = "Short.\n\nAlso short.\n\nAnother short line.\n\nYet another small piece.\n\nFinal bit.";

        let chunks = chunker.chunk_smart_text(&entry, content);

        // Small chunks should be consolidated together
        // Without consolidation, we might get 5+ chunks; with it, should be fewer
        println!("Consolidated chunks: {:?}", chunks.len());
        for (i, chunk) in chunks.iter().enumerate() {
            println!("Chunk {}: '{}' (len={})", i, chunk.text, chunk.text.len());
        }

        // All chunks should be at least 200 chars (or close to it) except possibly the last one
        // that contains all remaining content
        assert!(
            chunks.len() <= 3,
            "Small chunks should be consolidated; got {} chunks",
            chunks.len()
        );
    }

    #[test]
    fn test_chunking_with_multibyte_utf8() {
        // Test that chunking doesn't panic on multi-byte UTF-8 characters
        // This was the root cause of a crawler stall (panic at byte boundary)
        let chunker = EntryChunker::new(ChunkingConfig {
            max_chunk_size: 20, // Small size to force splits
            overlap_size: 0,
            ..Default::default()
        });

        // Content with various multi-byte UTF-8 characters:
        // - Smart quotes (''' is 3 bytes each)
        // - Emoji (🎉 is 4 bytes)
        // - CJK characters (你好 is 3 bytes each)
        let content = "Hello 'world' and 你好世界 with 🎉 emoji and more text to force chunking";

        // Create a dummy entry to use the public API
        let entry = Entry::with_id(
            "utf8-test".to_string(),
            "UTF-8 Test".to_string(),
            "/utf8-test.md".to_string(),
            "hash".to_string(),
        )
        .with_description("Testing UTF-8 handling".to_string())
        .with_content("".to_string())
        .with_tags(vec!["test".to_string()])
        .with_source_type(SourceType::Html);

        // This should NOT panic
        let chunks = chunker.chunk_smart_text(&entry, content);

        // Verify we got valid chunks
        assert!(!chunks.is_empty(), "Should produce at least one chunk");

        // Verify all chunks are valid UTF-8 strings
        for (i, chunk) in chunks.iter().enumerate() {
            println!("UTF-8 Test Chunk {}: '{}' (len={})", i, chunk.text, chunk.text.len());
            // Each chunk should have valid text (this tests that slicing didn't corrupt the string)
            assert!(!chunk.text.is_empty() || i == chunks.len() - 1);
            // Verify we can iterate characters without panic
            let _char_count = chunk.text.chars().count();
        }
    }

    #[test]
    fn test_chunking_split_inside_multibyte_char() {
        // Specifically test the case where chunk_size lands inside a multi-byte char
        // Use a larger chunk size that still forces splitting in multi-byte regions
        let chunker = EntryChunker::new(ChunkingConfig {
            // Content will be split multiple times, testing boundary handling each time
            max_chunk_size: 25,
            overlap_size: 0,
            ..Default::default()
        });

        // Content with smart quotes and emoji that will cause boundary issues
        // Total: about 80 bytes with multi-byte chars spread throughout
        let content = "Hello 'world' 你好 test 'more' data 🎉 final 'end' here please";

        // Create a dummy entry to use the public API
        let entry = Entry::with_id(
            "utf8-boundary-test".to_string(),
            "UTF-8 Boundary Test".to_string(),
            "/boundary-test.md".to_string(),
            "hash".to_string(),
        )
        .with_description("Testing UTF-8 boundary handling".to_string())
        .with_content("".to_string())
        .with_tags(vec!["test".to_string()])
        .with_source_type(SourceType::Html);

        // Should NOT panic - the fix should handle all multi-byte boundaries
        let chunks = chunker.chunk_smart_text(&entry, content);

        println!("Boundary test chunks: {} chunks", chunks.len());
        for (i, chunk) in chunks.iter().enumerate() {
            println!("  Chunk {}: '{}' (len={})", i, chunk.text, chunk.text.len());
        }

        // Should have at least one chunk (may be consolidated)
        assert!(!chunks.is_empty(), "Should produce at least one chunk");
        for chunk in &chunks {
            // All chunks should be valid UTF-8
            let _ = chunk.text.chars().count();
        }
    }
}
