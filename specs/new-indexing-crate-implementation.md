# Product Requirements Document: kix-indexing

## AutoRAG Indexing Engine for Rust

**Version:** 1.0.0  
**Author:** Helms AI  
**Date:** January 2026  
**Status:** Draft

---

## Executive Summary

**kix-indexing** is a high-performance, Rust-native AutoRAG indexing engine designed to automate the entire content-to-vector pipeline. It combines intelligent web crawling, content-type detection, AST-aware code parsing, semantic chunking, and multi-vector embedding strategies into a single, embeddable crate.

**Zero external dependencies by default:**
- **Embeddings:** Ollama with `nomic-embed-text` (local inference, no API costs)
- **Storage:** SQLite + sqlite-vec + SeaORM (single database file, type-safe queries, no server required)
- **Enrichment:** Summaries, HyDE, and entity extraction disabled by default for fast indexing

The goal is to eliminate the complexity of building RAG pipelines by providing a "point-and-index" solution that automatically optimizes chunking strategies, generates rich metadata, and produces embedding-ready chunks with minimal configuration. Alternative storage backends (Qdrant, pgvector, Pinecone) are supported via the `VectorStore` trait abstraction.

---

## Problem Statement

Building production-grade RAG systems requires solving multiple interconnected challenges:

1. **Content Acquisition Complexity** — Crawling websites, parsing repositories, and ingesting documents each require different tooling
2. **Naive Chunking Fails** — Fixed-size character splitting destroys semantic meaning, splits code mid-function, and creates poor retrieval quality
3. **Missing Context** — Chunks lose document hierarchy, cross-references, and structural relationships
4. **One-Size-Fits-All Doesn't Work** — Code, prose, tables, and structured data each need different chunking strategies
5. **Embedding Inefficiency** — No deduplication, no caching, no incremental updates
6. **Quality Blindness** — No visibility into chunk quality, coverage, or indexing health

---

## Solution Overview

kix-indexing provides an end-to-end AutoRAG pipeline with **zero external dependencies** - just Ollama for embeddings and a single SQLite database file for storage:

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                           kix-indexing Pipeline                             │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  ┌──────────┐    ┌──────────┐    ┌──────────┐    ┌──────────┐    ┌────────┐│
│  │ Acquire  │───▶│ Analyze  │───▶│  Chunk   │───▶│ Enrich   │───▶│ Output ││
│  └──────────┘    └──────────┘    └──────────┘    └──────────┘    └────────┘│
│       │              │               │               │               │      │
│   - Spider       - Detect        - Semantic      - Metadata      - SQLite  │
│   - Git          - Classify      - AST-aware     - Relations     - Vectors │
│   - FileSystem   - Extract       - Hierarchical  - HyDE          - Tantivy │
│   - API          - Structure     - Sliding       - Summary       - Export  │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## Prerequisites

### Ollama (Assumed Available)

kix-indexing uses a local Ollama server with `nomic-embed-text` for embeddings. **Ollama is assumed to be installed and running** - see your environment setup for Ollama configuration.

**Embedding Model:** `nomic-embed-text` (768 dimensions, 8192 token context, Matryoshka support)

### SQLite + sqlite-vec + SeaORM (Included)

kix-indexing bundles SQLite, the sqlite-vec extension, and uses **SeaORM** for type-safe database operations - **no additional setup required**.

The vector database is a single file that you specify:
```bash
# Creates/opens database at ./my-index.db (migrations run automatically)
kix index web https://docs.example.com --output ./my-index.db
```

**SQLite-vec Specifications:**
| Property | Value |
|----------|-------|
| Max Vectors | ~10M (practical limit) |
| Dimensions | Up to 65,535 |
| Distance Metrics | Cosine, L2, Inner Product |
| Index Type | Flat (brute-force), IVF (approximate) |
| File Format | Standard SQLite database |

**SeaORM Benefits:**
- Type-safe entity definitions and queries
- Automatic migrations on startup
- Transaction support with proper rollback
- Easy testing with in-memory databases

---

## Core Features

### 1. Multi-Source Content Acquisition

#### 1.1 Web Crawler (`spider` integration + Framework-Aware Code Extraction)

kix-indexing uses a **hybrid architecture** for web crawling:

1. **spider** handles crawling, discovery, and basic content extraction
2. **CodeExtractor** runs 30+ framework-specific patterns on raw HTML for high-quality code block extraction

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                    Web Crawling Pipeline                                     │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  spider.crawl(url)                                                          │
│         │                                                                    │
│         ▼                                                                    │
│  ┌─────────────────────────────────────────────────────────────────────┐    │
│  │  spider::Page { raw_html, url, status }                             │    │
│  └─────────────────────────────────────────────────────────────────────┘    │
│                │                              │                              │
│       ┌───────┴───────┐              ┌───────┴───────┐                      │
│       ▼               ▼              ▼               │                      │
│  ┌──────────────┐  ┌─────────────────────────────────────────────────┐     │
│  │ spider_      │  │  CodeExtractor (30+ patterns)                   │     │
│  │ transform    │  │  - Docusaurus, MkDocs, Sphinx, VuePress         │     │
│  │              │  │  - Prism.js, Highlight.js, Shiki                │     │
│  │ HTML →       │  │  - GitHub, GitLab, Stack Overflow               │     │
│  │ Markdown     │  │  - Monaco, CodeMirror, Ace Editor               │     │
│  │ (prose)      │  │  + Multi-layer language detection               │     │
│  └──────┬───────┘  │  + Code validation & deduplication              │     │
│         │          └──────────────────────────┬──────────────────────┘     │
│         │                                     │                             │
│         ▼                                     ▼                             │
│  ┌─────────────┐                   ┌─────────────────────┐                  │
│  │  Markdown   │                   │  Vec<CodeBlock>     │                  │
│  │  (prose)    │                   │  with rich metadata │                  │
│  └──────┬──────┘                   └──────────┬──────────┘                  │
│         │                                     │                             │
│         └─────────────────┬───────────────────┘                             │
│                           ▼                                                  │
│                 ┌─────────────────────┐                                     │
│                 │  ProcessedPage      │                                     │
│                 │  ├─ markdown        │                                     │
│                 │  ├─ code_blocks[]   │                                     │
│                 │  └─ metadata        │                                     │
│                 └─────────────────────┘                                     │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

```rust
use kix_indexing::sources::WebSource;

let source = WebSource::builder()
    .url("https://docs.example.com")
    .max_depth(3)
    .respect_robots_txt(true)
    .concurrent_requests(10)
    .follow_subdomains(false)
    .include_patterns(vec![r"/docs/.*", r"/api/.*"])
    .exclude_patterns(vec![r"/blog/.*", r".*\.pdf$"])
    .javascript_rendering(false) // Optional: spider's "smart" mode (HTTP first, JS fallback)
    .rate_limit(Duration::from_millis(100))
    // Code extraction configuration
    .code_extraction(CodeExtractionConfig {
        enabled: true,
        min_length: 10,
        max_prose_ratio: 0.6,
        validate_syntax: true,  // Optional tree-sitter validation
    })
    .build();
```

**spider Features:**
- Async crawling with configurable concurrency
- Sitemap.xml and robots.txt compliance
- URL pattern filtering (include/exclude regex)
- Smart mode: HTTP first, JavaScript rendering only when needed
- Incremental crawling with ETag/Last-Modified HTTP caching
- Custom request headers and authentication
- Built-in rate limiting and crawl budgets

**CodeExtractor Features (30+ patterns):**
- Framework-aware extraction (Docusaurus, MkDocs, Sphinx, Hugo, Jekyll, VuePress, Gatsby, Astro)
- Syntax highlighter support (Prism.js, Highlight.js, Shiki, Rouge, SyntaxHighlighter)
- Platform patterns (GitHub, GitLab, Bitbucket, Stack Overflow)
- Editor components (Monaco, CodeMirror, Ace)
- Multi-layer language detection (class, data attributes, parent elements)
- Language normalization (js→JavaScript, rs→Rust, py→Python)
- Code validation (structure check, prose ratio filtering)
- Deduplication via content hashing

See **Appendix E** for the complete list of 30+ supported code extraction patterns.

#### 1.2 Git Repository Indexer

```rust
use kix_indexing::sources::GitSource;

let source = GitSource::builder()
    .repo("https://github.com/org/repo")
    .branch("main")
    .include_patterns(vec![r".*\.rs$", r".*\.md$"])
    .exclude_patterns(vec![r"target/.*", r"node_modules/.*"])
    .include_history(true) // Index git blame/history
    .max_file_size(1_000_000) // 1MB limit
    .build();
```

**Features:**
- Clone or fetch existing repos
- Branch/tag/commit targeting
- File pattern filtering
- Git blame integration for authorship metadata
- Commit history for temporal context
- Submodule support

#### 1.3 File System Scanner

```rust
use kix_indexing::sources::FileSystemSource;

let source = FileSystemSource::builder()
    .path("/path/to/docs")
    .recursive(true)
    .follow_symlinks(false)
    .include_hidden(false)
    .watch(true) // File system watcher for incremental updates
    .build();
```

**Features:**
- Recursive directory traversal
- Real-time file watching (notify crate)
- Symlink handling
- File metadata extraction (created, modified, size)

#### 1.4 API/Feed Ingestion

```rust
use kix_indexing::sources::ApiSource;

let source = ApiSource::builder()
    .endpoint("https://api.example.com/docs")
    .method(Method::GET)
    .headers(vec![("Authorization", "Bearer {token}")])
    .pagination(PaginationStrategy::Cursor { param: "cursor" })
    .rate_limit(100, Duration::from_secs(60))
    .build();
```

**Features:**
- REST API pagination (cursor, offset, page)
- Authentication (Bearer, API Key, OAuth2)
- Rate limiting
- Response transformation

---

### 2. Content Analysis & Classification

#### 2.1 Content Type Detection

Automatic detection of content type for optimal processing:

```rust
pub enum ContentType {
    // Code
    Code(Language),
    
    // Documents
    Markdown,
    Html,
    PlainText,
    Pdf,
    Docx,
    
    // Structured Data
    Json,
    Yaml,
    Toml,
    Xml,
    Csv,
    
    // Mixed
    Notebook(NotebookFormat), // Jupyter, R Markdown
    
    // Binary (skip or OCR)
    Image,
    Binary,
}

pub enum Language {
    Rust,
    Python,
    JavaScript,
    TypeScript,
    Go,
    Java,
    CSharp,
    Cpp,
    C,
    Ruby,
    Php,
    Swift,
    Kotlin,
    Sql,
    Shell,
    Other(String),
}
```

**Detection Strategy:**
1. File extension mapping
2. MIME type detection (magic bytes)
3. Content heuristics (shebang, language markers)
4. Tree-sitter parse attempt

#### 2.2 Structure Extraction

Extract document structure for hierarchical chunking:

```rust
pub struct DocumentStructure {
    pub title: Option<String>,
    pub headings: Vec<Heading>,
    pub sections: Vec<Section>,
    pub code_blocks: Vec<CodeBlock>,
    pub tables: Vec<Table>,
    pub links: Vec<Link>,
    pub images: Vec<ImageRef>,
    pub metadata: HashMap<String, String>,
}

pub struct Heading {
    pub level: u8,          // h1=1, h2=2, etc.
    pub text: String,
    pub anchor: Option<String>,
    pub start_offset: usize,
    pub end_offset: usize,
}

pub struct Section {
    pub heading: Option<Heading>,
    pub content: String,
    pub children: Vec<Section>,
    pub depth: u8,
}
```

**Extractors:**
- HTML: `scraper` + `readability` for main content extraction
- Markdown: `pulldown-cmark` with custom AST walker
- Code: `tree-sitter` for AST extraction (source files)
- Code: `CodeExtractor` for HTML code block extraction (web pages)
- PDF: `pdf-extract` or `lopdf` for text extraction
- DOCX: `docx-rs` for paragraph/table extraction

#### 2.3 Framework-Aware Code Extraction (Web Pages)

When crawling documentation websites, code blocks are embedded in HTML with framework-specific markup. The `CodeExtractor` module uses 30+ CSS patterns to extract code blocks with high fidelity.

**Why Framework-Aware Extraction?**

Different documentation frameworks render code blocks differently:

```html
<!-- Docusaurus -->
<div class="prism-code language-rust">
  <pre><code>fn main() { ... }</code></pre>
</div>

<!-- MkDocs -->
<div class="highlight"><pre><code class="language-python">def main(): ...</code></pre></div>

<!-- GitHub -->
<div class="highlight highlight-source-rust">
  <pre><code>fn main() { ... }</code></pre>
</div>
```

Generic HTML→Markdown conversion misses these patterns. `CodeExtractor` understands them all.

**CodeExtractor API:**

```rust
use kix_indexing::extraction::{CodeExtractor, CodeBlock, CodePattern};

let extractor = CodeExtractor::builder()
    .min_length(10)           // Minimum code length
    .max_prose_ratio(0.6)     // Filter non-code content
    .validate_syntax(true)    // Optional tree-sitter validation
    .build();

// Extract from raw HTML (from spider::Page)
let code_blocks: Vec<CodeBlock> = extractor.extract(&raw_html);

for block in code_blocks {
    println!("Language: {:?}", block.language);      // Language::Rust
    println!("Pattern: {:?}", block.pattern);        // CodePattern::DocusaurusCodeBlock
    println!("Content: {}", block.content);          // fn main() { ... }
    println!("Lines: {}", block.line_count);         // 3
}
```

**CodeBlock Structure:**

```rust
pub struct CodeBlock {
    /// The extracted code content
    pub content: String,

    /// Detected programming language
    pub language: Language,

    /// Which pattern matched (for debugging/analytics)
    pub pattern: CodePattern,

    /// Content hash for deduplication
    pub hash: u64,

    /// Line count
    pub line_count: usize,

    /// Original HTML (for debugging)
    pub source_html: Option<String>,
}
```

**Language Detection Strategy:**

The extractor uses a multi-layer detection strategy:

```rust
// Detection priority (highest to lowest):
1. class="language-rust" or class="lang-rust"
2. data-language="rust" or data-lang="rust"
3. Parent element's class/data attributes
4. Known language class names (e.g., class="rust")
5. Tree-sitter parse validation (optional)

// Normalization:
"js" → Language::JavaScript
"ts" → Language::TypeScript
"rs" → Language::Rust
"py" → Language::Python
"c++" → Language::Cpp
// ... etc.
```

**Code Validation:**

Not everything in a `<pre>` tag is code. The extractor validates content:

```rust
// Validation checks:
1. Minimum length (default: 10 chars)
2. Not placeholder text ("Loading...", "...", "Copy")
3. Has code structure ({}, [], (), ;, =, etc.)
4. Prose ratio below threshold (filters English text)
5. Optional: valid syntax per tree-sitter
```

**Supported Patterns (30+):**

| Category | Patterns |
|----------|----------|
| **Documentation Frameworks** | Docusaurus, MkDocs, Sphinx, ReadTheDocs, Jekyll, Hugo, VuePress, Gatsby, Next.js, Astro |
| **Syntax Highlighters** | Prism.js, Highlight.js, SyntaxHighlighter, Rouge, Shiki |
| **Platforms** | GitHub, GitLab, Bitbucket, Stack Overflow |
| **Editor Components** | Monaco, CodeMirror, Ace |
| **Generic** | Pre+Code, data-language, class prefixes |

See **Appendix E** for the complete pattern list with CSS selectors.

---

### 3. Intelligent Chunking Engine

The heart of kix-indexing: content-aware chunking strategies.

#### 3.1 Chunking Strategy Selection

```rust
pub enum ChunkingStrategy {
    /// Semantic sentence-based chunking with overlap
    Semantic {
        target_tokens: usize,
        overlap_tokens: usize,
        sentence_splitter: SentenceSplitter,
    },
    
    /// AST-aware code chunking
    CodeAst {
        granularity: CodeGranularity,
        include_context: bool, // Include imports, class context
    },
    
    /// Hierarchical document chunking (preserves structure)
    Hierarchical {
        max_depth: u8,
        include_parent_context: bool,
    },
    
    /// Sliding window with smart boundaries
    SlidingWindow {
        window_tokens: usize,
        stride_tokens: usize,
        boundary_preference: BoundaryPreference,
    },
    
    /// Recursive character splitting (fallback)
    Recursive {
        chunk_size: usize,
        chunk_overlap: usize,
        separators: Vec<String>,
    },
    
    /// Table-aware chunking
    Tabular {
        rows_per_chunk: usize,
        include_headers: bool,
    },
    
    /// Auto-select based on content type
    Auto,
}

pub enum CodeGranularity {
    File,           // Entire file as chunk
    Module,         // Module/namespace level
    Class,          // Class/struct/impl level
    Function,       // Function/method level
    Block,          // Significant code blocks
}

pub enum BoundaryPreference {
    Paragraph,
    Sentence,
    Line,
    None,
}
```

#### 3.2 Semantic Chunking Implementation

```rust
use kix_indexing::chunking::{SemanticChunker, ChunkConfig};

let chunker = SemanticChunker::builder()
    .target_tokens(512)
    .max_tokens(1024)
    .overlap_tokens(50)
    .tokenizer(Tokenizer::Tiktoken("cl100k_base")) // or HuggingFace
    .sentence_splitter(SentenceSplitter::Unicode) // or Nltk, Spacy
    .preserve_paragraphs(true)
    .build();

let chunks = chunker.chunk(&document);
```

**Algorithm:**
1. Split into sentences using Unicode segmentation or NLP
2. Accumulate sentences until target token count
3. Find optimal break point (paragraph > sentence > word)
4. Add overlap from previous chunk
5. Track source positions for citation

#### 3.3 AST-Aware Code Chunking

```rust
use kix_indexing::chunking::CodeChunker;

let chunker = CodeChunker::builder()
    .language(Language::Rust)
    .granularity(CodeGranularity::Function)
    .include_signature(true)      // Always include function signature
    .include_docstring(true)      // Include doc comments
    .include_imports(true)        // Add relevant imports as context
    .include_class_context(true)  // Add class/impl name as context
    .max_tokens(1024)
    .build();

let chunks = chunker.chunk(&source_code);
```

**Code Chunk Structure:**
```rust
pub struct CodeChunk {
    pub content: String,
    pub language: Language,
    pub chunk_type: CodeChunkType,
    pub context: CodeContext,
    pub signature: Option<String>,
    pub docstring: Option<String>,
    pub imports: Vec<String>,
    pub references: Vec<String>,     // Called functions/types
    pub definitions: Vec<String>,    // Defined symbols
    pub complexity: Option<u32>,     // Cyclomatic complexity
    pub lines: Range<usize>,
}

pub enum CodeChunkType {
    Function { name: String, params: Vec<String>, return_type: Option<String> },
    Method { class: String, name: String, is_async: bool },
    Class { name: String, bases: Vec<String> },
    Struct { name: String, fields: Vec<String> },
    Enum { name: String, variants: Vec<String> },
    Trait { name: String },
    Impl { struct_name: String, trait_name: Option<String> },
    Module { name: String },
    Constant { name: String, type_: String },
    Test { name: String },
}

pub struct CodeContext {
    pub file_path: String,
    pub module_path: Vec<String>,
    pub parent_class: Option<String>,
    pub namespace: Option<String>,
}
```

**Tree-sitter Integration:**
```rust
// Language-specific queries for symbol extraction
const RUST_FUNCTION_QUERY: &str = r#"
    (function_item
        name: (identifier) @func.name
        parameters: (parameters) @func.params
        return_type: (_)? @func.return
        body: (block) @func.body
    ) @func.def
    
    (impl_item
        type: (type_identifier) @impl.type
        trait: (type_identifier)? @impl.trait
        body: (declaration_list) @impl.body
    ) @impl.def
"#;
```

#### 3.4 Hierarchical Document Chunking

Preserves document structure with parent-child relationships:

```rust
pub struct HierarchicalChunk {
    pub id: ChunkId,
    pub content: String,
    pub level: u8,
    pub heading: Option<String>,
    pub parent_id: Option<ChunkId>,
    pub children_ids: Vec<ChunkId>,
    pub path: Vec<String>,           // ["Chapter 1", "Section 1.1", "Subsection"]
    pub summary: Option<String>,     // Generated summary of children
}

// Example output structure:
// Document
// ├── Chapter 1 (level 0, summary of children)
// │   ├── Section 1.1 (level 1)
// │   │   ├── Chunk 1 (level 2, content)
// │   │   └── Chunk 2 (level 2, content)
// │   └── Section 1.2 (level 1)
// │       └── Chunk 3 (level 2, content)
// └── Chapter 2 (level 0)
```

#### 3.5 Late Chunking (Contextual Embeddings)

Advanced technique: embed full document first, then chunk embeddings:

```rust
pub struct LateChunker {
    embedding_model: Box<dyn EmbeddingModel>,
    max_context_length: usize,
}

impl LateChunker {
    /// 1. Embed entire document (up to max context)
    /// 2. Split into chunks with token boundaries
    /// 3. Extract corresponding embedding spans
    /// 4. Each chunk retains full document context in its embedding
    pub async fn chunk(&self, document: &str) -> Vec<ContextualChunk>;
}
```

**Late Chunking Configuration:**

```rust
use kix_indexing::chunking::LateChunker;

let late_chunker = LateChunker::builder()
    .embedding_provider(ollama_embeddings)
    .max_context_length(8192)  // nomic-embed-text limit
    .chunk_size(512)           // Target chunk size in tokens
    .overlap(64)               // Overlap between chunks
    .pooling_strategy(PoolingStrategy::Mean)
    .build();

// Process document
let contextual_chunks = late_chunker.chunk(&document).await?;

// Each chunk now has embeddings that "know" the full document context
for chunk in contextual_chunks {
    println!("Chunk: {} tokens, context-aware embedding", chunk.token_count);
}
```

**When to Use Late Chunking:**
- Documents where local context is insufficient
- Technical documentation with forward/backward references
- Legal documents where clause context matters
- Research papers with interconnected sections

#### 3.6 Table-Aware Chunking

Specialized chunking for tabular data that preserves structure:

```rust
use kix_indexing::chunking::TableChunker;

let table_chunker = TableChunker::builder()
    .rows_per_chunk(25)
    .include_headers(true)         // Repeat headers in each chunk
    .include_column_descriptions(true)
    .preserve_row_integrity(true)  // Never split a row
    .output_format(TableOutputFormat::Markdown)
    .build();
```

**Table Chunk Structure:**

```rust
pub struct TableChunk {
    pub id: ChunkId,
    pub content: String,
    pub table_metadata: TableMetadata,
    pub row_range: Range<usize>,
    pub columns: Vec<ColumnInfo>,
}

pub struct TableMetadata {
    pub table_id: String,
    pub title: Option<String>,
    pub total_rows: usize,
    pub total_columns: usize,
    pub has_header: bool,
    pub source_format: TableSourceFormat,
}

pub struct ColumnInfo {
    pub name: String,
    pub data_type: ColumnDataType,
    pub description: Option<String>,
    pub statistics: Option<ColumnStats>,
}

pub enum TableSourceFormat {
    Csv,
    Html,
    Markdown,
    Excel,
    Pdf,
}

pub enum ColumnDataType {
    Text,
    Number,
    Date,
    Boolean,
    Currency,
    Percentage,
    Mixed,
}
```

**Table Detection & Extraction:**

```rust
pub struct TableExtractor {
    /// Detect tables in various document formats
    pub fn extract_tables(&self, document: &Document) -> Vec<ExtractedTable>;
}

// Supports extraction from:
// - HTML: <table> elements with proper header detection
// - Markdown: Pipe-delimited tables
// - PDF: Detected via layout analysis (borders, alignment)
// - CSV/TSV: Native parsing with type inference
// - Excel: Sheet-by-sheet extraction with formulas resolved
```

#### 3.7 PDF Document Parsing

Comprehensive PDF parsing with structure preservation:

```rust
use kix_indexing::parsers::PdfParser;

let pdf_parser = PdfParser::builder()
    .extract_text(true)
    .extract_tables(true)
    .extract_images(false)       // OCR not enabled by default
    .preserve_layout(true)       // Maintain reading order
    .detect_headers(true)        // Identify section headings
    .detect_footnotes(true)
    .page_range(None)            // All pages, or Some(1..=10)
    .build();

let parsed = pdf_parser.parse(&pdf_bytes)?;
```

**PDF Structure:**

```rust
pub struct ParsedPdf {
    pub metadata: PdfMetadata,
    pub pages: Vec<PdfPage>,
    pub outline: Option<PdfOutline>,  // Table of contents
    pub tables: Vec<ExtractedTable>,
    pub images: Vec<ExtractedImage>,
}

pub struct PdfMetadata {
    pub title: Option<String>,
    pub author: Option<String>,
    pub subject: Option<String>,
    pub keywords: Vec<String>,
    pub created: Option<DateTime<Utc>>,
    pub modified: Option<DateTime<Utc>>,
    pub page_count: usize,
}

pub struct PdfPage {
    pub number: usize,
    pub content: String,
    pub sections: Vec<PdfSection>,
    pub tables: Vec<TableRef>,
    pub images: Vec<ImageRef>,
}

pub struct PdfSection {
    pub heading: Option<String>,
    pub level: u8,
    pub content: String,
    pub bounding_box: Option<BoundingBox>,
}

pub struct PdfOutline {
    pub entries: Vec<OutlineEntry>,
}

pub struct OutlineEntry {
    pub title: String,
    pub page: usize,
    pub level: u8,
    pub children: Vec<OutlineEntry>,
}
```

**PDF Chunking Strategy:**

```rust
// PDF-aware chunking respects document structure
let pdf_chunker = PdfChunker::builder()
    .respect_page_boundaries(false)  // Allow chunks to span pages
    .respect_section_boundaries(true)
    .use_outline_for_hierarchy(true)
    .include_page_numbers(true)      // Add page refs to metadata
    .table_handling(TableHandling::SeparateChunks)
    .build();
```

#### 3.8 DOCX Document Parsing

Microsoft Word document parsing with full structure extraction:

```rust
use kix_indexing::parsers::DocxParser;

let docx_parser = DocxParser::builder()
    .extract_text(true)
    .extract_tables(true)
    .extract_images(false)
    .preserve_formatting(true)    // Track bold, italic, etc.
    .extract_comments(true)       // Include document comments
    .extract_track_changes(false) // Include revision history
    .include_headers_footers(true)
    .build();

let parsed = docx_parser.parse(&docx_bytes)?;
```

**DOCX Structure:**

```rust
pub struct ParsedDocx {
    pub metadata: DocxMetadata,
    pub body: Vec<DocxElement>,
    pub styles: HashMap<String, DocxStyle>,
    pub comments: Vec<DocxComment>,
    pub headers: Vec<HeaderFooter>,
    pub footers: Vec<HeaderFooter>,
}

pub struct DocxMetadata {
    pub title: Option<String>,
    pub author: Option<String>,
    pub last_modified_by: Option<String>,
    pub created: Option<DateTime<Utc>>,
    pub modified: Option<DateTime<Utc>>,
    pub word_count: Option<usize>,
    pub page_count: Option<usize>,
}

pub enum DocxElement {
    Paragraph(DocxParagraph),
    Table(DocxTable),
    Image(DocxImage),
    List(DocxList),
    Heading(DocxHeading),
}

pub struct DocxParagraph {
    pub text: String,
    pub style: Option<String>,
    pub runs: Vec<DocxRun>,  // Formatted text segments
    pub alignment: Alignment,
}

pub struct DocxRun {
    pub text: String,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub font: Option<String>,
    pub size: Option<f32>,
    pub color: Option<String>,
}

pub struct DocxHeading {
    pub text: String,
    pub level: u8,  // 1-9
    pub style: String,
}

pub struct DocxTable {
    pub rows: Vec<DocxTableRow>,
    pub column_widths: Vec<f32>,
    pub style: Option<String>,
}

pub struct DocxComment {
    pub id: String,
    pub author: String,
    pub date: DateTime<Utc>,
    pub content: String,
    pub anchor_text: String,  // The text being commented on
}
```

**DOCX Chunking Strategy:**

```rust
// DOCX-aware chunking uses document structure
let docx_chunker = DocxChunker::builder()
    .chunk_by_heading(true)       // New chunk at each heading
    .min_heading_level(2)         // Only split at H1, H2
    .include_style_context(true)  // Preserve formatting info
    .table_handling(TableHandling::SeparateChunks)
    .list_handling(ListHandling::KeepTogether)
    .include_comments_as_metadata(true)
    .build();
```

---

### 4. Metadata Enrichment

#### 4.1 Automatic Metadata Extraction

```rust
pub struct ChunkMetadata {
    // Source Information
    pub source_url: Option<String>,
    pub source_path: Option<String>,
    pub source_type: SourceType,
    
    // Document Context
    pub document_title: Option<String>,
    pub document_id: String,
    pub section_title: Option<String>,
    pub section_path: Vec<String>,
    
    // Position
    pub chunk_index: usize,
    pub total_chunks: usize,
    pub start_offset: usize,
    pub end_offset: usize,
    pub line_range: Option<Range<usize>>,
    
    // Content Analysis
    pub content_type: ContentType,
    pub language: Option<Language>,
    pub token_count: usize,
    pub char_count: usize,
    
    // Temporal
    pub created_at: Option<DateTime<Utc>>,
    pub modified_at: Option<DateTime<Utc>>,
    pub indexed_at: DateTime<Utc>,
    
    // Quality Signals
    pub quality_score: f32,
    pub information_density: f32,
    
    // Relationships
    pub parent_chunk_id: Option<ChunkId>,
    pub related_chunk_ids: Vec<ChunkId>,
    
    // Custom
    pub custom: HashMap<String, Value>,
}
```

#### 4.2 Entity Extraction

Automatic entity recognition for enhanced retrieval:

```rust
pub struct ExtractedEntities {
    pub people: Vec<Entity>,
    pub organizations: Vec<Entity>,
    pub locations: Vec<Entity>,
    pub technologies: Vec<Entity>,
    pub concepts: Vec<Entity>,
    pub code_symbols: Vec<CodeSymbol>,
}

pub struct Entity {
    pub text: String,
    pub entity_type: EntityType,
    pub start: usize,
    pub end: usize,
    pub confidence: f32,
    pub normalized: Option<String>, // Canonical form
}
```

#### 4.3 Relationship Extraction

Build knowledge graph from chunks:

```rust
pub struct ChunkRelationship {
    pub source_chunk_id: ChunkId,
    pub target_chunk_id: ChunkId,
    pub relationship_type: RelationshipType,
    pub confidence: f32,
}

pub enum RelationshipType {
    // Structural
    ParentOf,
    ChildOf,
    SiblingOf,
    Continues,
    
    // Semantic
    References,
    Defines,
    Implements,
    Examples,
    Contradicts,
    Supports,
    
    // Code-specific
    Calls,
    CalledBy,
    Imports,
    ImportedBy,
    Inherits,
    Overrides,
}
```

#### 4.4 Summary Generation (Optional)

Chunk summarization is **disabled by default** and can be easily enabled when needed. Summaries add latency and LLM costs but improve retrieval quality for certain use cases.

**Quick Toggle:**

```rust
// Disabled (default) - fastest indexing
let indexer = Indexer::builder()
    .add_source(source)
    .enable_summaries(false)  // Default
    .build();

// Enabled - generates summaries for each chunk
let indexer = Indexer::builder()
    .add_source(source)
    .enable_summaries(true)
    .build();
```

**CLI Toggle:**

```bash
# Without summaries (default, fast)
kix index web https://docs.example.com --output ./docs.db

# With summaries (slower, better retrieval)
kix index web https://docs.example.com --output ./docs.db --summaries

# With specific summary levels
kix index web https://docs.example.com --output ./docs.db \
    --summaries \
    --summary-levels chunk,section
```

**TOML Configuration:**

```toml
[enrichment]
# Easy on/off toggle
enable_summaries = false  # Default: disabled

# Fine-grained control (only applies when enabled)
summary_levels = ["chunk", "section"]  # Options: chunk, section, document, corpus
summary_style = "concise"              # Options: concise, descriptive, technical, simplified
summary_model = "llama3.2"             # LLM model for generation
chunk_summary_tokens = 50              # Max tokens per chunk summary
embed_summaries = true                 # Create separate summary vectors
```

**Detailed Configuration:**

```rust
use kix_indexing::enrichment::{SummaryGenerator, SummaryConfig};

// Fine-grained control when summaries are needed
let summary_config = SummaryConfig {
    enabled: true,
    levels: vec![SummaryLevel::Chunk, SummaryLevel::Section],
    style: SummaryStyle::Technical,
    chunk_summary_tokens: 50,
    section_summary_tokens: 150,
    document_summary_tokens: 300,
    embed_summaries: true,  // Create separate summary vectors for retrieval
    
    // LLM configuration
    llm_provider: LlmProvider::Ollama {
        base_url: "http://localhost:11434".to_string(),
        model: "llama3.2".to_string(),
    },
    
    // Performance tuning
    batch_size: 10,          // Chunks per LLM batch
    max_concurrent: 3,       // Parallel LLM requests
    timeout_secs: 30,
};

let indexer = Indexer::builder()
    .add_source(source)
    .summary_config(summary_config)
    .build();
```

**When to Enable Summaries:**

| Use Case | Enable Summaries? | Why |
|----------|-------------------|-----|
| Code repositories | ❌ No | Code structure is self-explanatory |
| API documentation | ❌ No | Already concise and structured |
| Long-form articles | ✅ Yes | Helps with high-level retrieval |
| Legal/policy docs | ✅ Yes | Summaries aid navigation |
| Research papers | ✅ Yes | Abstract + section summaries valuable |
| Chat/support logs | ❌ No | Short messages don't need summarization |

**Summary Types:**

```rust
pub struct ChunkSummaries {
    /// One-line summary of chunk content (when enabled)
    pub chunk_summary: Option<String>,
    
    /// Key points extracted from chunk
    pub key_points: Vec<String>,
    
    /// Questions this chunk answers
    pub answered_questions: Vec<String>,
    
    /// Parent section summary (if hierarchical + enabled)
    pub section_summary: Option<String>,
    
    /// Full document summary
    pub document_summary: Option<String>,
}

pub enum SummaryLevel {
    Chunk,     // Summary per chunk (~50 tokens)
    Section,   // Summary per document section (~150 tokens)
    Document,  // Summary per document (~300 tokens)
    Corpus,    // Summary across all documents
}

pub enum SummaryStyle {
    Concise,      // Brief, factual (default)
    Descriptive,  // More detailed
    Technical,    // Preserve technical terms
    Simplified,   // Plain language
}
```

**Summary Pipeline Integration:**

```rust
// Summaries integrate with multi-vector storage when enabled
let indexer = Indexer::builder()
    .add_source(source)
    .enable_summaries(true)
    .summary_config(SummaryConfig {
        levels: vec![SummaryLevel::Chunk, SummaryLevel::Section],
        embed_summaries: true,  // Creates kix_vectors entries with vector_type='summary'
        ..Default::default()
    })
    .build();

// Search can then use summary vectors for high-level queries
let results = store.search_by_vector_type(query, VectorType::Summary, limit).await?;
```

**Hierarchical Summary Aggregation:**

```rust
pub struct HierarchicalSummarizer {
    /// Builds summaries bottom-up (when enabled):
    /// 1. Summarize individual chunks
    /// 2. Aggregate chunk summaries into section summaries
    /// 3. Aggregate section summaries into document summary
    pub async fn summarize_document(
        &self,
        chunks: &[Chunk],
        structure: &DocumentStructure,
    ) -> DocumentSummaries;
}

pub struct DocumentSummaries {
    pub document_id: DocumentId,
    pub document_summary: Option<String>,
    pub section_summaries: HashMap<String, String>,
    pub chunk_summaries: HashMap<ChunkId, String>,
}
```

---

### 5. Advanced RAG Techniques

#### 5.1 Multi-Vector Representations

Generate multiple embeddings per chunk for different retrieval scenarios:

```rust
pub struct MultiVectorChunk {
    pub chunk_id: ChunkId,
    pub content: String,
    pub vectors: MultiVectorSet,
}

pub struct MultiVectorSet {
    /// Original content embedding
    pub content_vector: Vector,
    
    /// Summary embedding (for high-level search)
    pub summary_vector: Option<Vector>,
    
    /// Title/heading embedding (for navigation)
    pub title_vector: Option<Vector>,
    
    /// Hypothetical questions embedding (HyDE)
    pub questions_vector: Option<Vector>,
    
    /// Code signature embedding (for code search)
    pub signature_vector: Option<Vector>,
}
```

#### 5.2 Hypothetical Document Embeddings (HyDE)

Generate hypothetical questions for improved retrieval:

```rust
use kix_indexing::enrichment::HydeGenerator;

let hyde = HydeGenerator::builder()
    .llm_client(ollama_llm)  // Uses Ollama for LLM generation
    .questions_per_chunk(3)
    .include_answer_style(true)
    .build();

// For chunk: "Rust's ownership system prevents data races..."
// Generates:
// - "How does Rust prevent data races?"
// - "What is Rust's ownership system?"
// - "Why doesn't Rust have a garbage collector?"
```

#### 5.3 Parent Document Retrieval

Store both detailed chunks and parent summaries:

```rust
pub struct ParentDocumentStore {
    /// Small chunks for precise retrieval
    pub chunks: Vec<DetailedChunk>,
    
    /// Parent documents with summaries
    pub parents: Vec<ParentDocument>,
}

pub struct DetailedChunk {
    pub id: ChunkId,
    pub parent_id: DocumentId,
    pub content: String,
    pub vector: Vector,
}

pub struct ParentDocument {
    pub id: DocumentId,
    pub full_content: String,
    pub summary: String,
    pub summary_vector: Vector,
}

// Retrieval strategy:
// 1. Search chunks for relevant matches
// 2. Retrieve parent document for full context
// 3. Return expanded context window
```

#### 5.4 Contextual Compression

Compress chunks at retrieval time for optimal context:

```rust
pub struct ContextualCompressor {
    llm_client: Box<dyn LlmClient>,
    max_output_tokens: usize,
}

impl ContextualCompressor {
    /// Given a query and retrieved chunks, compress to relevant portions
    pub async fn compress(
        &self,
        query: &str,
        chunks: &[Chunk],
    ) -> Vec<CompressedChunk>;
}
```

#### 5.5 Chunk Deduplication

Avoid indexing duplicate or near-duplicate content:

```rust
pub struct Deduplicator {
    /// MinHash for near-duplicate detection
    minhash_bands: usize,
    minhash_rows: usize,
    
    /// Similarity threshold (0.0-1.0)
    similarity_threshold: f32,
    
    /// Existing hash index
    hash_index: HashIndex,
}

impl Deduplicator {
    /// Check if chunk is duplicate of existing content
    pub fn is_duplicate(&self, chunk: &Chunk) -> Option<ChunkId>;
    
    /// Find all near-duplicates
    pub fn find_similar(&self, chunk: &Chunk) -> Vec<(ChunkId, f32)>;
    
    /// Merge duplicate chunks, keeping best quality
    pub fn merge_duplicates(&mut self, chunks: Vec<Chunk>) -> Vec<Chunk>;
}
```

---

### 6. Embedding Integration

#### 6.1 Embedding Provider Abstraction

```rust
#[async_trait]
pub trait EmbeddingProvider: Send + Sync {
    /// Embed a single text
    async fn embed(&self, text: &str) -> Result<Vector, EmbeddingError>;
    
    /// Batch embed multiple texts
    async fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Vector>, EmbeddingError>;
    
    /// Get model info
    fn model_info(&self) -> ModelInfo;
}

pub struct ModelInfo {
    pub name: String,
    pub dimensions: usize,
    pub max_tokens: usize,
    pub supports_batch: bool,
}
```

#### 6.2 Ollama Embeddings

kix-indexing uses Ollama exclusively for embeddings. Ollama is assumed to be available in your environment.

```rust
// Ollama with nomic-embed-text (the only supported provider)
let embeddings = OllamaEmbeddings::builder()
    .model("nomic-embed-text")
    .base_url("http://localhost:11434")
    .dimensions(768)
    .build();
```

**Why Ollama + nomic-embed-text:**
- Local inference with no API costs
- 768-dimensional embeddings
- 8192 token context window (excellent for long documents)
- Matryoshka support (can reduce dimensions without retraining)
- GPU auto-detection handled by Ollama
- Strong performance on retrieval benchmarks
- Already required for LLM features

#### 6.3 Embedding Pipeline

```rust
pub struct EmbeddingPipeline {
    provider: Box<dyn EmbeddingProvider>,
    cache: Option<EmbeddingCache>,
    batch_size: usize,
    retry_config: RetryConfig,
    rate_limiter: Option<RateLimiter>,
}

impl EmbeddingPipeline {
    pub async fn embed_chunks(&self, chunks: &[Chunk]) -> Result<Vec<EmbeddedChunk>> {
        // 1. Check cache for existing embeddings
        // 2. Batch remaining chunks
        // 3. Apply rate limiting
        // 4. Retry failed batches
        // 5. Update cache
        // 6. Return embedded chunks
    }
}

pub struct EmbeddingCache {
    backend: CacheBackend,
    ttl: Option<Duration>,
}

pub enum CacheBackend {
    InMemory(LruCache),
    Sqlite(SqliteConnection),  // Recommended: persists across restarts
    FileSystem(PathBuf),
}
```

---

### 7. Vector Store Integration

#### 7.1 Store Abstraction

```rust
#[async_trait]
pub trait VectorStore: Send + Sync {
    /// Upsert chunks with vectors
    async fn upsert(&self, chunks: &[EmbeddedChunk]) -> Result<UpsertResult>;
    
    /// Search by vector
    async fn search(&self, query: &Vector, limit: usize) -> Result<Vec<SearchResult>>;
    
    /// Search with filters
    async fn search_filtered(
        &self,
        query: &Vector,
        filter: &Filter,
        limit: usize,
    ) -> Result<Vec<SearchResult>>;
    
    /// Delete by IDs
    async fn delete(&self, ids: &[ChunkId]) -> Result<DeleteResult>;
    
    /// Get collection stats
    async fn stats(&self) -> Result<CollectionStats>;
}

pub struct Filter {
    pub conditions: Vec<FilterCondition>,
    pub operator: LogicalOperator,
}

pub enum FilterCondition {
    Equals { field: String, value: Value },
    Contains { field: String, value: String },
    Range { field: String, min: Option<Value>, max: Option<Value> },
    In { field: String, values: Vec<Value> },
}
```

#### 7.2 SQLite + sqlite-vec (Default)

kix-indexing uses SQLite with the sqlite-vec extension as the default vector store. The architecture separates concerns:

- **SeaORM** → Relational tables (chunks, documents, metadata) with type-safe entities, migrations, and transactions
- **Raw SQL** → sqlite-vec vector operations (via `sea_orm::DatabaseConnection::execute_unprepared()`)
- **Tantivy** → Full-text search (separate index files, BM25 ranking)

This separation exists because SeaORM doesn't have native bindings for sqlite-vec's virtual tables (`vec0`). SeaORM handles the relational data, and we drop down to raw SQL for vector similarity queries.

**Benefits:**
- Zero external dependencies (single database file + Tantivy index directory)
- Type-safe queries for relational data with SeaORM entities and migrations
- Excellent performance for small-to-medium datasets (<10M vectors)
- ACID transactions for reliable indexing
- Full SQL filtering capabilities
- Easy backup and portability

```rust
use kix_indexing::storage::SqliteVecStore;

// Default configuration
let store = SqliteVecStore::builder()
    .path("./kix-index.db")           // Or ":memory:" for in-memory
    .vector_dimensions(768)            // nomic-embed-text
    .distance_metric(Distance::Cosine)
    .build()
    .await?;

// With full configuration
let store = SqliteVecStore::builder()
    .path("./kix-index.db")
    .vector_dimensions(768)
    .distance_metric(Distance::Cosine)
    .table_prefix("kix")               // Table naming prefix
    .enable_wal(true)                  // Write-ahead logging for concurrency
    .cache_size_mb(64)                 // SQLite page cache
    .busy_timeout_ms(5000)             // Lock timeout
    .auto_vacuum(true)
    .run_migrations(true)              // Auto-run SeaORM migrations
    .build()
    .await?;
```

**SeaORM Entity Definitions:**

```rust
// entity/chunk.rs
use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "kix_chunks")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,
    pub document_id: String,
    #[sea_orm(column_type = "Text")]
    pub content: String,
    pub content_type: String,
    #[sea_orm(column_type = "JsonBinary")]
    pub metadata: serde_json::Value,
    pub created_at: DateTimeUtc,
    pub updated_at: DateTimeUtc,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_one = "super::vector::Entity")]
    Vector,
    #[sea_orm(belongs_to = "super::document::Entity", from = "Column::DocumentId", to = "super::document::Column::Id")]
    Document,
}

impl ActiveModelBehavior for ActiveModel {}

// entity/vector.rs
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "kix_vectors")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub chunk_id: String,
    #[sea_orm(column_type = "VarBinary(StringLen::None)")]
    pub embedding: Vec<u8>,  // Stored as blob, converted to/from Vec<f32>
    pub vector_type: String, // "content", "summary", "hyde"
}

// entity/document.rs
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "kix_documents")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,
    pub source_url: Option<String>,
    pub source_path: Option<String>,
    pub title: Option<String>,
    pub content_hash: String,
    pub chunk_count: i32,
    pub indexed_at: DateTimeUtc,
    pub updated_at: DateTimeUtc,
}
```

**SeaORM Migrations:**

```rust
// migration/m20240101_000001_create_tables.rs
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Create documents table
        manager.create_table(
            Table::create()
                .table(Documents::Table)
                .col(ColumnDef::new(Documents::Id).string().not_null().primary_key())
                .col(ColumnDef::new(Documents::SourceUrl).string())
                .col(ColumnDef::new(Documents::SourcePath).string())
                .col(ColumnDef::new(Documents::Title).string())
                .col(ColumnDef::new(Documents::ContentHash).string().not_null())
                .col(ColumnDef::new(Documents::ChunkCount).integer().not_null())
                .col(ColumnDef::new(Documents::IndexedAt).timestamp().not_null())
                .col(ColumnDef::new(Documents::UpdatedAt).timestamp().not_null())
                .to_owned(),
        ).await?;

        // Create chunks table
        manager.create_table(
            Table::create()
                .table(Chunks::Table)
                .col(ColumnDef::new(Chunks::Id).string().not_null().primary_key())
                .col(ColumnDef::new(Chunks::DocumentId).string().not_null())
                .col(ColumnDef::new(Chunks::Content).text().not_null())
                .col(ColumnDef::new(Chunks::ContentType).string().not_null())
                .col(ColumnDef::new(Chunks::Metadata).json_binary().not_null())
                .col(ColumnDef::new(Chunks::CreatedAt).timestamp().not_null())
                .col(ColumnDef::new(Chunks::UpdatedAt).timestamp().not_null())
                .foreign_key(
                    ForeignKey::create()
                        .from(Chunks::Table, Chunks::DocumentId)
                        .to(Documents::Table, Documents::Id)
                        .on_delete(ForeignKeyAction::Cascade),
                )
                .to_owned(),
        ).await?;

        // Create indexes
        manager.create_index(
            Index::create()
                .table(Chunks::Table)
                .name("idx_chunks_document")
                .col(Chunks::DocumentId)
                .to_owned(),
        ).await?;

        // Create sqlite-vec virtual table (raw SQL required)
        manager.get_connection().execute_unprepared(
            "CREATE VIRTUAL TABLE kix_vectors USING vec0(
                chunk_id TEXT PRIMARY KEY,
                embedding FLOAT[768],
                vector_type TEXT
            )"
        ).await?;

        // NOTE: Full-text search uses Tantivy (separate index files)
        // Tantivy index is initialized separately via kix-search crate
        // This provides better BM25 ranking, faceted search, and scalability

        Ok(())
    }
}
```

**SQLite-vec Operations with SeaORM:**

```rust
use sea_orm::{DatabaseConnection, EntityTrait, QueryFilter, ColumnTrait};
use crate::entity::{chunk, document, vector};

impl SqliteVecStore {
    /// Vector similarity search (hybrid SeaORM + raw SQL for vec0)
    pub async fn search(&self, query: &Vector, limit: usize) -> Result<Vec<SearchResult>> {
        // sqlite-vec requires raw SQL for KNN search
        let vector_results: Vec<VectorMatch> = self.db
            .query_all(Statement::from_sql_and_values(
                DbBackend::Sqlite,
                r#"
                    SELECT chunk_id, distance
                    FROM kix_vectors
                    WHERE embedding MATCH ?1 AND vector_type = 'content'
                    ORDER BY distance
                    LIMIT ?2
                "#,
                [query.as_bytes().into(), (limit as i64).into()],
            ))
            .await?;
        
        // Fetch full chunk data via SeaORM
        let chunk_ids: Vec<String> = vector_results.iter().map(|v| v.chunk_id.clone()).collect();
        
        let chunks = chunk::Entity::find()
            .filter(chunk::Column::Id.is_in(chunk_ids))
            .all(&self.db)
            .await?;
        
        // Combine results with scores
        Ok(self.merge_results(chunks, vector_results))
    }
    
    /// Filtered search combining SeaORM queries with vector search
    pub async fn search_filtered(
        &self,
        query: &Vector,
        filter: &Filter,
        limit: usize,
    ) -> Result<Vec<SearchResult>> {
        // Build SeaORM filter conditions
        let mut chunk_query = chunk::Entity::find();
        
        for condition in &filter.conditions {
            chunk_query = match condition {
                FilterCondition::Equals { field, value } => {
                    chunk_query.filter(Expr::col(Alias::new(field)).eq(value.clone()))
                }
                FilterCondition::Contains { field, value } => {
                    chunk_query.filter(Expr::col(Alias::new(field)).contains(value))
                }
                // ... other conditions
            };
        }
        
        // Get filtered chunk IDs
        let filtered_ids: Vec<String> = chunk_query
            .select_only()
            .column(chunk::Column::Id)
            .into_tuple()
            .all(&self.db)
            .await?;
        
        // Vector search within filtered set
        self.search_within_ids(query, &filtered_ids, limit).await
    }
    
    /// Batch upsert with SeaORM transactions
    pub async fn upsert(&self, chunks: &[EmbeddedChunk]) -> Result<UpsertResult> {
        let txn = self.db.begin().await?;
        
        for embedded in chunks {
            // Upsert chunk via SeaORM
            let chunk_model = chunk::ActiveModel {
                id: Set(embedded.chunk.id.to_string()),
                document_id: Set(embedded.chunk.document_id.to_string()),
                content: Set(embedded.chunk.content.clone()),
                content_type: Set(embedded.chunk.content_type.to_string()),
                metadata: Set(serde_json::to_value(&embedded.chunk.metadata)?),
                created_at: Set(embedded.chunk.created_at),
                updated_at: Set(Utc::now()),
            };
            
            chunk::Entity::insert(chunk_model)
                .on_conflict(
                    OnConflict::column(chunk::Column::Id)
                        .update_columns([
                            chunk::Column::Content,
                            chunk::Column::Metadata,
                            chunk::Column::UpdatedAt,
                        ])
                        .to_owned()
                )
                .exec(&txn)
                .await?;
            
            // Insert vector (raw SQL for sqlite-vec)
            txn.execute(Statement::from_sql_and_values(
                DbBackend::Sqlite,
                "INSERT OR REPLACE INTO kix_vectors (chunk_id, embedding, vector_type) VALUES (?1, ?2, ?3)",
                [
                    embedded.chunk.id.to_string().into(),
                    embedded.vectors.content_vector.as_bytes().into(),
                    "content".into(),
                ],
            )).await?;
            
            // Insert summary vector if present
            if let Some(ref summary_vec) = embedded.vectors.summary_vector {
                txn.execute(Statement::from_sql_and_values(
                    DbBackend::Sqlite,
                    "INSERT OR REPLACE INTO kix_vectors (chunk_id, embedding, vector_type) VALUES (?1, ?2, ?3)",
                    [
                        embedded.chunk.id.to_string().into(),
                        summary_vec.as_bytes().into(),
                        "summary".into(),
                    ],
                )).await?;
            }
        }
        
        txn.commit().await?;
        Ok(UpsertResult { upserted: chunks.len() })
    }
    
    /// Delete chunks by document ID
    pub async fn delete_by_document(&self, document_id: &str) -> Result<DeleteResult> {
        let result = chunk::Entity::delete_many()
            .filter(chunk::Column::DocumentId.eq(document_id))
            .exec(&self.db)
            .await?;
        
        Ok(DeleteResult { deleted: result.rows_affected as usize })
    }
}
```

**Performance Tuning:**

```rust
pub struct SqliteVecConfig {
    /// Enable WAL mode for better concurrency
    pub wal_mode: bool,
    
    /// Page cache size in MB (default: 64)
    pub cache_size_mb: usize,
    
    /// Memory-mapped I/O size (default: 1GB)
    pub mmap_size_mb: usize,
    
    /// Synchronous mode (NORMAL for performance, FULL for durability)
    pub synchronous: SynchronousMode,
    
    /// Journal size limit
    pub journal_size_limit_mb: usize,
    
    /// Auto-vacuum mode
    pub auto_vacuum: AutoVacuumMode,
    
    /// Run SeaORM migrations on startup
    pub run_migrations: bool,
}

impl Default for SqliteVecConfig {
    fn default() -> Self {
        Self {
            wal_mode: true,
            cache_size_mb: 64,
            mmap_size_mb: 1024,
            synchronous: SynchronousMode::Normal,
            journal_size_limit_mb: 64,
            auto_vacuum: AutoVacuumMode::Incremental,
            run_migrations: true,
        }
    }
}
```

**Backup & Maintenance:**

```rust
impl SqliteVecStore {
    /// Create backup of the database
    pub async fn backup(&self, path: &Path) -> Result<()>;
    
    /// Optimize database (VACUUM, reindex)
    pub async fn optimize(&self) -> Result<()>;
    
    /// Get database statistics via SeaORM
    pub async fn stats(&self) -> Result<DatabaseStats> {
        let chunk_count = chunk::Entity::find().count(&self.db).await?;
        let document_count = document::Entity::find().count(&self.db).await?;
        
        Ok(DatabaseStats {
            chunk_count: chunk_count as usize,
            document_count: document_count as usize,
            // ... other stats from raw SQL
        })
    }
    
    /// Rebuild vector index
    pub async fn rebuild_index(&self) -> Result<()>;
}

pub struct DatabaseStats {
    pub file_size_bytes: u64,
    pub chunk_count: usize,
    pub document_count: usize,
    pub vector_count: usize,
    pub index_size_bytes: u64,
    pub page_count: usize,
    pub freelist_count: usize,
}
```

#### 7.3 Alternative Store Implementations

The `VectorStore` trait allows plugging in alternative backends (via feature flags):

```rust
// Qdrant (for large-scale distributed deployments)
// Enable with: cargo build --features qdrant
let qdrant = QdrantStore::builder()
    .url("http://localhost:6333")
    .collection("documents")
    .vector_size(768)
    .distance(Distance::Cosine)
    .build();

// In-Memory (for testing)
let memory = InMemoryStore::new(768);
```

---

### 8. Command Line Interface (CLI)

kix-indexing provides a comprehensive CLI for common indexing operations without writing code.

#### 8.1 Installation

```bash
cargo install kix-indexing-cli

# Or build from source
cargo build --release --features cli
```

#### 8.2 Basic Commands

```bash
# Index a website
kix index web https://docs.example.com \
    --depth 3 \
    --output ./my-docs.db

# Index a git repository
kix index git https://github.com/org/repo \
    --branch main \
    --include "*.rs,*.md" \
    --output ./code-docs.db

# Index local files
kix index files ./docs \
    --recursive \
    --watch \
    --output ./local-docs.db

# Index from config file
kix index --config ./kix.toml
```

#### 8.3 Management Commands

```bash
# Check index health
kix health ./my-docs.db

# List indexed documents
kix list ./my-docs.db --limit 100

# Delete chunks by source
kix delete ./my-docs.db \
    --source "https://docs.example.com/*"

# Re-index stale documents
kix reindex ./my-docs.db \
    --older-than 7d

# Export chunks to JSON
kix export ./my-docs.db \
    --output ./chunks.jsonl

# Import chunks from JSON
kix import ./chunks.jsonl \
    --output ./my-docs.db

# Backup database
kix backup ./my-docs.db ./backups/my-docs-$(date +%Y%m%d).db

# Optimize/vacuum database
kix optimize ./my-docs.db
```

#### 8.4 Query & Debug Commands

```bash
# Test search query
kix search ./my-docs.db \
    "How do I configure authentication?" \
    --limit 5

# Hybrid search (sqlite-vec vectors + Tantivy BM25, fused via RRF)
kix search ./my-docs.db \
    "authentication config" \
    --hybrid \
    --vector-weight 0.7

# Analyze a document without indexing
kix analyze ./document.md --show-chunks --show-entities

# Validate configuration
kix validate --config ./kix.toml

# Show statistics
kix stats ./my-docs.db
```

#### 8.5 CLI Output Formats

```bash
# JSON output for scripting
kix list ... --format json | jq '.chunks[].id'

# Table output (default)
kix list ... --format table

# Minimal output
kix list ... --format minimal
```

---

### 9. Indexing Orchestrator

#### 9.1 Main Indexer Interface

```rust
use kix_indexing::Indexer;

let indexer = Indexer::builder()
    // Sources
    .add_source(web_source)
    .add_source(git_source)
    
    // Chunking
    .chunking_strategy(ChunkingStrategy::Auto)
    .target_chunk_tokens(512)
    .chunk_overlap_tokens(50)
    
    // Enrichment (all disabled by default for fast indexing)
    .enable_hyde(false)              // Default: disabled
    .enable_summaries(false)         // Default: disabled
    .enable_entity_extraction(false) // Default: disabled
    
    // Embedding (Ollama default)
    .embedding_provider(ollama_embeddings)
    .embedding_batch_size(32)
    
    // Storage (SQLite + SeaORM default)
    .vector_store(sqlite_store)
    
    // Processing
    .concurrency(10)
    .retry_attempts(3)
    
    // Callbacks
    .on_progress(|progress| println!("{:?}", progress))
    .on_error(|error| eprintln!("{:?}", error))
    
    .build();

// Run full indexing
let result = indexer.index_all().await?;

// Incremental update
let result = indexer.index_incremental().await?;
```

**With Enrichment Enabled:**

```rust
// Enable enrichment features when needed (adds latency + LLM costs)
let indexer = Indexer::builder()
    .add_source(source)
    .enable_summaries(true)          // Generate chunk summaries
    .enable_hyde(true)               // Generate hypothetical questions
    .enable_entity_extraction(true)  // Extract named entities
    .build();
```

#### 9.2 Progress Tracking

```rust
pub struct IndexingProgress {
    pub phase: IndexingPhase,
    pub total_documents: usize,
    pub processed_documents: usize,
    pub total_chunks: usize,
    pub embedded_chunks: usize,
    pub stored_chunks: usize,
    pub errors: usize,
    pub elapsed: Duration,
    pub estimated_remaining: Option<Duration>,
}

pub enum IndexingPhase {
    Acquiring,
    Analyzing,
    Chunking,
    Enriching,
    Embedding,
    Storing,
    Finalizing,
}
```

#### 8.3 Indexing Result

```rust
pub struct IndexingResult {
    pub success: bool,
    pub documents_processed: usize,
    pub chunks_created: usize,
    pub chunks_embedded: usize,
    pub chunks_stored: usize,
    pub duplicates_skipped: usize,
    pub errors: Vec<IndexingError>,
    pub duration: Duration,
    pub statistics: IndexStatistics,
}

pub struct IndexStatistics {
    pub avg_chunk_tokens: f32,
    pub avg_chunks_per_document: f32,
    pub content_type_distribution: HashMap<ContentType, usize>,
    pub language_distribution: HashMap<Language, usize>,
    pub quality_histogram: Vec<(f32, usize)>,
}
```

---

### 9. Quality & Observability

#### 9.1 Chunk Quality Scoring

```rust
pub struct QualityScorer {
    weights: QualityWeights,
}

pub struct QualityWeights {
    pub information_density: f32,    // Ratio of meaningful content
    pub coherence: f32,              // Semantic completeness
    pub self_containment: f32,       // Can chunk stand alone?
    pub formatting_quality: f32,     // Proper structure preserved
    pub code_completeness: f32,      // For code: complete constructs?
}

impl QualityScorer {
    pub fn score(&self, chunk: &Chunk) -> ChunkQuality;
}

pub struct ChunkQuality {
    pub overall_score: f32,          // 0.0 - 1.0
    pub subscores: HashMap<String, f32>,
    pub issues: Vec<QualityIssue>,
    pub suggestions: Vec<String>,
}

pub enum QualityIssue {
    TooShort,
    TooLong,
    IncompleteCode,
    MissingContext,
    HighDuplication,
    LowInformationDensity,
    BrokenFormatting,
}
```

#### 9.2 Index Health Metrics

```rust
pub struct IndexHealth {
    pub total_chunks: usize,
    pub total_documents: usize,
    pub avg_quality_score: f32,
    pub coverage: CoverageMetrics,
    pub freshness: FreshnessMetrics,
    pub distribution: DistributionMetrics,
}

pub struct CoverageMetrics {
    pub sources_indexed: usize,
    pub sources_failed: usize,
    pub documents_skipped: usize,
    pub content_types_covered: Vec<ContentType>,
}

pub struct FreshnessMetrics {
    pub oldest_chunk: DateTime<Utc>,
    pub newest_chunk: DateTime<Utc>,
    pub avg_age: Duration,
    pub stale_chunks: usize,  // Older than threshold
}
```

#### 9.3 Telemetry & Logging

```rust
use tracing::{info, warn, error, instrument};

#[instrument(skip(self, document))]
async fn process_document(&self, document: &Document) -> Result<Vec<Chunk>> {
    info!(doc_id = %document.id, "Processing document");
    
    let chunks = self.chunker.chunk(document)?;
    
    info!(
        doc_id = %document.id,
        chunk_count = chunks.len(),
        "Document chunked successfully"
    );
    
    Ok(chunks)
}
```

OpenTelemetry integration for distributed tracing:

```rust
let indexer = Indexer::builder()
    .telemetry(TelemetryConfig {
        enabled: true,
        exporter: OtlpExporter::new("http://localhost:4317"),
        service_name: "kix-indexing",
        sample_rate: 1.0,
    })
    .build();
```

---

### 10. Configuration

#### 10.1 TOML Configuration File

```toml
[indexer]
name = "my-knowledge-base"
concurrency = 10
retry_attempts = 3

[sources.web]
urls = ["https://docs.example.com"]
max_depth = 3
respect_robots_txt = true
rate_limit_ms = 100

[sources.git]
repos = ["https://github.com/org/repo"]
branches = ["main"]
include_patterns = [".*\\.rs$", ".*\\.md$"]

[chunking]
strategy = "auto"
target_tokens = 512
overlap_tokens = 50
code_granularity = "function"

[enrichment]
enable_hyde = false           # Disabled by default
hyde_questions = 3
enable_summaries = false      # Disabled by default (enable for long-form content)
summary_levels = ["chunk"]    # Only used when enable_summaries = true
enable_entities = false       # Disabled by default

[embedding]
# Ollama is the only supported embedding provider
model = "nomic-embed-text"
base_url = "http://localhost:11434"
dimensions = 768
batch_size = 32

[storage]
provider = "sqlite-vec"
path = "./kix-index.db"
vector_dimensions = 768
enable_wal = true
cache_size_mb = 64

[quality]
min_chunk_tokens = 50
max_chunk_tokens = 2000
min_quality_score = 0.5
enable_deduplication = true
similarity_threshold = 0.95
```

#### 10.2 Environment Variables

```bash
# Ollama (required - embeddings)
OLLAMA_BASE_URL=http://localhost:11434
OLLAMA_MODEL=nomic-embed-text

# SQLite (usually set via CLI or config)
KIX_DATABASE_PATH=./kix-index.db

# Optional: Alternative vector stores
QDRANT_URL=http://localhost:6333  # If using qdrant feature

# Configuration
KIX_CONFIG_PATH=/etc/kix/config.toml
KIX_LOG_LEVEL=info
KIX_CACHE_DIR=/var/cache/kix
```

---

### 11. Distributed Crawling

Scale crawling across multiple workers for large-scale indexing.

#### 11.1 Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                     Distributed Crawler                          │
├─────────────────────────────────────────────────────────────────┤
│                                                                  │
│  ┌──────────┐     ┌─────────────┐     ┌──────────────────────┐  │
│  │ Scheduler│────▶│ URL Queue   │────▶│ Worker Pool          │  │
│  │          │     │ (Redis)     │     │ ┌────┐┌────┐┌────┐   │  │
│  └──────────┘     └─────────────┘     │ │ W1 ││ W2 ││ W3 │   │  │
│       │                               │ └────┘└────┘└────┘   │  │
│       │           ┌─────────────┐     └──────────────────────┘  │
│       └──────────▶│ Result Store│◀───────────────┘              │
│                   │ (Redis/S3)  │                                │
│                   └─────────────┘                                │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘
```

#### 11.2 Coordinator Configuration

```rust
use kix_indexing::distributed::{Coordinator, WorkerConfig};

let coordinator = Coordinator::builder()
    .redis_url("redis://localhost:6379")
    .queue_name("kix-crawl-queue")
    .result_store(ResultStore::Redis)
    .max_workers(10)
    .heartbeat_interval(Duration::from_secs(30))
    .task_timeout(Duration::from_secs(300))
    .build();

// Submit crawl job
let job_id = coordinator.submit_job(CrawlJob {
    source: WebSource::builder()
        .url("https://docs.example.com")
        .max_depth(5)
        .build(),
    priority: JobPriority::Normal,
    callback_url: Some("https://api.example.com/webhook"),
}).await?;

// Monitor progress
let status = coordinator.job_status(job_id).await?;
```

#### 11.3 Worker Configuration

```rust
use kix_indexing::distributed::Worker;

let worker = Worker::builder()
    .coordinator_url("redis://localhost:6379")
    .queue_name("kix-crawl-queue")
    .worker_id("worker-1")
    .concurrency(5)           // URLs per worker
    .memory_limit(1_000_000_000)  // 1GB
    .cpu_limit(2.0)           // 2 cores
    .build();

// Run worker (blocking)
worker.run().await?;
```

#### 11.4 URL Queue Management

```rust
pub struct UrlQueue {
    redis: RedisClient,
    queue_name: String,
}

impl UrlQueue {
    /// Add URLs to crawl queue with priority
    pub async fn enqueue(&self, urls: &[CrawlUrl]) -> Result<()>;
    
    /// Dequeue next URL for processing
    pub async fn dequeue(&self) -> Result<Option<CrawlUrl>>;
    
    /// Mark URL as completed
    pub async fn complete(&self, url: &str, result: CrawlResult) -> Result<()>;
    
    /// Mark URL as failed (will retry)
    pub async fn fail(&self, url: &str, error: &str) -> Result<()>;
    
    /// Get queue statistics
    pub async fn stats(&self) -> Result<QueueStats>;
}

pub struct CrawlUrl {
    pub url: String,
    pub depth: usize,
    pub priority: i32,
    pub parent_url: Option<String>,
    pub retry_count: usize,
}

pub struct QueueStats {
    pub pending: usize,
    pub processing: usize,
    pub completed: usize,
    pub failed: usize,
    pub total_urls_discovered: usize,
}
```

#### 11.5 Distributed Deduplication

```rust
pub struct DistributedDeduplicator {
    /// Bloom filter for fast URL deduplication across workers
    bloom_filter: RedisBloomFilter,
    
    /// Content hash store for content deduplication
    content_hashes: RedisSet,
}

impl DistributedDeduplicator {
    /// Check if URL was already crawled (probabilistic)
    pub async fn url_seen(&self, url: &str) -> Result<bool>;
    
    /// Check if content hash exists
    pub async fn content_exists(&self, hash: &ContentHash) -> Result<bool>;
    
    /// Mark URL as seen
    pub async fn mark_url_seen(&self, url: &str) -> Result<()>;
    
    /// Store content hash
    pub async fn store_content_hash(&self, hash: &ContentHash) -> Result<()>;
}
```

---

### 12. Incremental Indexing

Efficiently update indexes without full re-indexing.

#### 12.1 Change Detection

```rust
use kix_indexing::incremental::{ChangeDetector, ChangeSet};

let detector = ChangeDetector::builder()
    .vector_store(sqlite_store.clone())
    .change_tracking(ChangeTracking::ContentHash)
    .build();

// Detect changes since last index
let changes = detector.detect_changes(&source).await?;

println!("New: {}, Modified: {}, Deleted: {}", 
    changes.added.len(),
    changes.modified.len(),
    changes.deleted.len()
);
```

**Change Detection Strategies:**

```rust
pub enum ChangeTracking {
    /// Compare content hashes
    ContentHash,
    
    /// Use HTTP ETag/Last-Modified headers
    HttpHeaders,
    
    /// Git commit comparison
    GitCommit,
    
    /// File system modification times
    FileModTime,
    
    /// Custom change detection
    Custom(Box<dyn ChangeDetectorFn>),
}

pub struct ChangeSet {
    /// New documents to add
    pub added: Vec<DocumentRef>,
    
    /// Modified documents to update
    pub modified: Vec<DocumentRef>,
    
    /// Deleted documents to remove
    pub deleted: Vec<DocumentRef>,
    
    /// Unchanged documents (skip)
    pub unchanged: usize,
}

pub struct DocumentRef {
    pub id: DocumentId,
    pub url: String,
    pub old_hash: Option<ContentHash>,
    pub new_hash: Option<ContentHash>,
    pub change_type: ChangeType,
}
```

#### 12.2 Incremental Update Pipeline

```rust
use kix_indexing::incremental::IncrementalIndexer;

let incremental = IncrementalIndexer::builder()
    .indexer(indexer)
    .change_detector(detector)
    .update_strategy(UpdateStrategy::InPlace)
    .batch_size(100)
    .build();

// Run incremental update
let result = incremental.update().await?;

println!("Added: {}, Updated: {}, Deleted: {}", 
    result.chunks_added,
    result.chunks_updated,
    result.chunks_deleted
);
```

**Update Strategies:**

```rust
pub enum UpdateStrategy {
    /// Update chunks in place (default)
    InPlace,
    
    /// Delete old, insert new (safer but slower)
    DeleteInsert,
    
    /// Create new version, swap atomically
    Versioned,
    
    /// Soft delete old, insert new
    SoftDelete,
}
```

#### 12.3 State Management

```rust
pub struct IndexState {
    /// Last successful index timestamp
    pub last_indexed: DateTime<Utc>,
    
    /// Content hashes for all indexed documents
    pub document_hashes: HashMap<DocumentId, ContentHash>,
    
    /// Source-specific cursors (e.g., git commit, API cursor)
    pub cursors: HashMap<String, String>,
    
    /// Index version
    pub version: u64,
}

impl IndexState {
    /// Save state to persistent storage
    pub async fn save(&self, store: &dyn StateStore) -> Result<()>;
    
    /// Load state from persistent storage
    pub async fn load(store: &dyn StateStore) -> Result<Self>;
}

pub trait StateStore: Send + Sync {
    async fn save(&self, key: &str, state: &IndexState) -> Result<()>;
    async fn load(&self, key: &str) -> Result<Option<IndexState>>;
}

// Built-in state stores
pub struct FileStateStore { path: PathBuf }
pub struct RedisStateStore { client: RedisClient }
pub struct SqliteStateStore { conn: SqliteConnection }
```

#### 12.4 Watch Mode

```rust
use kix_indexing::incremental::Watcher;

let watcher = Watcher::builder()
    .indexer(incremental)
    .poll_interval(Duration::from_secs(60))  // For web sources
    .debounce(Duration::from_secs(5))        // For file sources
    .on_change(|event| {
        println!("Detected change: {:?}", event);
    })
    .on_indexed(|result| {
        println!("Indexed {} chunks", result.chunks_added);
    })
    .build();

// Run watcher (blocking)
watcher.watch().await?;
```

**Watch Sources:**

```rust
pub enum WatchSource {
    /// File system watcher (inotify/FSEvents)
    FileSystem {
        path: PathBuf,
        recursive: bool,
    },
    
    /// HTTP polling with ETag/Last-Modified
    Http {
        urls: Vec<String>,
        poll_interval: Duration,
    },
    
    /// Git repository polling
    Git {
        repo: String,
        branch: String,
        poll_interval: Duration,
    },
    
    /// Webhook receiver
    Webhook {
        listen_addr: SocketAddr,
        secret: Option<String>,
    },
}
```

---

### 13. Plugin System

Extend kix-indexing with custom functionality.

#### 13.1 Plugin Architecture

```rust
use kix_indexing::plugin::{Plugin, PluginContext, PluginResult};

/// Core plugin trait
#[async_trait]
pub trait Plugin: Send + Sync {
    /// Plugin identifier
    fn id(&self) -> &str;
    
    /// Plugin version
    fn version(&self) -> &str;
    
    /// Initialize plugin with context
    async fn init(&mut self, ctx: &PluginContext) -> PluginResult<()>;
    
    /// Cleanup on shutdown
    async fn shutdown(&mut self) -> PluginResult<()>;
}

pub struct PluginContext {
    pub config: PluginConfig,
    pub indexer: Arc<Indexer>,
    pub event_bus: Arc<EventBus>,
    pub storage: Arc<dyn PluginStorage>,
}
```

#### 13.2 Plugin Types

**Source Plugins:**

```rust
/// Add custom content sources
#[async_trait]
pub trait SourcePlugin: Plugin {
    /// Fetch documents from source
    async fn fetch(&self) -> PluginResult<Vec<Document>>;
    
    /// Check for updates (incremental)
    async fn check_updates(&self, since: DateTime<Utc>) -> PluginResult<Vec<DocumentRef>>;
}

// Example: Confluence source plugin
pub struct ConfluencePlugin {
    client: ConfluenceClient,
    space_keys: Vec<String>,
}

#[async_trait]
impl SourcePlugin for ConfluencePlugin {
    async fn fetch(&self) -> PluginResult<Vec<Document>> {
        let mut docs = vec![];
        for space in &self.space_keys {
            let pages = self.client.get_space_pages(space).await?;
            docs.extend(pages.into_iter().map(|p| p.into()));
        }
        Ok(docs)
    }
}
```

**Parser Plugins:**

```rust
/// Add custom document parsers
#[async_trait]
pub trait ParserPlugin: Plugin {
    /// Supported content types
    fn supported_types(&self) -> Vec<ContentType>;
    
    /// Parse document into structured content
    async fn parse(&self, content: &[u8], content_type: &ContentType) 
        -> PluginResult<ParsedDocument>;
}

// Example: Jupyter notebook parser
pub struct JupyterPlugin;

#[async_trait]
impl ParserPlugin for JupyterPlugin {
    fn supported_types(&self) -> Vec<ContentType> {
        vec![ContentType::Notebook(NotebookFormat::Jupyter)]
    }
    
    async fn parse(&self, content: &[u8], _: &ContentType) 
        -> PluginResult<ParsedDocument> {
        let notebook: JupyterNotebook = serde_json::from_slice(content)?;
        Ok(notebook.into())
    }
}
```

**Chunker Plugins:**

```rust
/// Add custom chunking strategies
#[async_trait]
pub trait ChunkerPlugin: Plugin {
    /// Chunk document with custom strategy
    async fn chunk(&self, document: &Document) -> PluginResult<Vec<Chunk>>;
}
```

**Enrichment Plugins:**

```rust
/// Add custom metadata enrichment
#[async_trait]
pub trait EnrichmentPlugin: Plugin {
    /// Enrich chunk with additional metadata
    async fn enrich(&self, chunk: &mut Chunk) -> PluginResult<()>;
}

// Example: Sentiment analysis plugin
pub struct SentimentPlugin {
    model: SentimentModel,
}

#[async_trait]
impl EnrichmentPlugin for SentimentPlugin {
    async fn enrich(&self, chunk: &mut Chunk) -> PluginResult<()> {
        let sentiment = self.model.analyze(&chunk.content)?;
        chunk.metadata.custom.insert(
            "sentiment".to_string(),
            serde_json::to_value(sentiment)?
        );
        Ok(())
    }
}
```

**Storage Plugins:**

```rust
/// Add custom vector stores
#[async_trait]
pub trait StoragePlugin: Plugin + VectorStore {
    /// Additional storage-specific configuration
    fn configure(&mut self, config: &StorageConfig) -> PluginResult<()>;
}
```

#### 13.3 Plugin Registry

```rust
use kix_indexing::plugin::{PluginRegistry, PluginLoader};

let mut registry = PluginRegistry::new();

// Register built-in plugins
registry.register(Box::new(ConfluencePlugin::new(config)));
registry.register(Box::new(JupyterPlugin::new()));
registry.register(Box::new(SentimentPlugin::new(model)));

// Load plugins from directory
let loader = PluginLoader::new("./plugins");
for plugin in loader.load_all()? {
    registry.register(plugin);
}

// Use with indexer
let indexer = Indexer::builder()
    .plugin_registry(registry)
    .build();
```

#### 13.4 Plugin Configuration

```toml
# kix.toml

[plugins]
enabled = ["confluence", "jupyter", "sentiment"]

[plugins.confluence]
base_url = "https://company.atlassian.net/wiki"
username = "${CONFLUENCE_USER}"
api_token = "${CONFLUENCE_TOKEN}"
space_keys = ["DEV", "DOCS", "KB"]

[plugins.jupyter]
# No additional config needed

[plugins.sentiment]
model_path = "./models/sentiment-bert"
threshold = 0.7
```

#### 13.5 Event Hooks

```rust
use kix_indexing::plugin::{EventBus, Event};

// Subscribe to indexing events
event_bus.subscribe(EventType::DocumentFetched, |event| async {
    println!("Fetched: {}", event.document_id);
});

event_bus.subscribe(EventType::ChunkCreated, |event| async {
    // Custom processing for each chunk
});

event_bus.subscribe(EventType::IndexingComplete, |event| async {
    // Send notification, update dashboard, etc.
});

pub enum EventType {
    // Acquisition
    SourceStarted,
    DocumentFetched,
    DocumentFailed,
    SourceComplete,
    
    // Processing
    DocumentAnalyzed,
    ChunkCreated,
    ChunkEnriched,
    ChunkEmbedded,
    ChunkStored,
    
    // Lifecycle
    IndexingStarted,
    IndexingProgress,
    IndexingComplete,
    IndexingFailed,
    
    // Incremental
    ChangeDetected,
    IncrementalUpdate,
}
```

#### 13.6 Plugin Development Kit

```rust
// plugins/my-plugin/src/lib.rs
use kix_indexing::plugin::prelude::*;

#[derive(Default)]
pub struct MyPlugin {
    config: MyPluginConfig,
}

#[async_trait]
impl Plugin for MyPlugin {
    fn id(&self) -> &str { "my-plugin" }
    fn version(&self) -> &str { env!("CARGO_PKG_VERSION") }
    
    async fn init(&mut self, ctx: &PluginContext) -> PluginResult<()> {
        self.config = ctx.config.get::<MyPluginConfig>("my-plugin")?;
        Ok(())
    }
    
    async fn shutdown(&mut self) -> PluginResult<()> {
        Ok(())
    }
}

#[async_trait]
impl EnrichmentPlugin for MyPlugin {
    async fn enrich(&self, chunk: &mut Chunk) -> PluginResult<()> {
        // Custom enrichment logic
        Ok(())
    }
}

// Export plugin
kix_plugin!(MyPlugin);
```

---

### Core Types

```rust
/// Unique identifier for chunks
pub type ChunkId = uuid::Uuid;

/// Unique identifier for documents
pub type DocumentId = uuid::Uuid;

/// Embedding vector
pub type Vector = Vec<f32>;

/// Primary chunk structure
pub struct Chunk {
    pub id: ChunkId,
    pub document_id: DocumentId,
    pub content: String,
    pub content_type: ContentType,
    pub metadata: ChunkMetadata,
    pub vector: Option<Vector>,
}

/// Embedded chunk ready for storage
pub struct EmbeddedChunk {
    pub chunk: Chunk,
    pub vectors: MultiVectorSet,
}

/// Search result from vector store
pub struct SearchResult {
    pub chunk: Chunk,
    pub score: f32,
    pub highlights: Vec<String>,
}
```

### Builder Pattern

All major components use the builder pattern for configuration:

```rust
// Source builder
WebSource::builder()
    .url("...")
    .max_depth(3)
    .build();

// Chunker builder
SemanticChunker::builder()
    .target_tokens(512)
    .build();

// Indexer builder
Indexer::builder()
    .add_source(source)
    .chunking_strategy(strategy)
    .embedding_provider(provider)
    .vector_store(store)
    .build();
```

---

### 14. Web Dashboard UI Requirements

The indexing dashboard requires updates to support spider + CodeExtractor visibility.

#### 14.1 SSE Event Updates

New events for code extraction progress:

```rust
/// Enhanced SSE events for code extraction
pub enum IndexingEvent {
    // Existing events...
    JobStarted { job_id: String, source: String, total_items: usize },
    Progress { processed: usize, total: usize, rate: f32, eta_seconds: Option<u64> },
    ItemProcessed { url: String, chunks: usize, duration_ms: u64 },

    // NEW: Code extraction events
    CodeExtracted {
        url: String,
        code_blocks: usize,
        languages: HashMap<Language, usize>,  // e.g., {"Rust": 5, "Python": 3}
        pattern: CodePattern,                  // e.g., Docusaurus, MkDocs
        validation_passed: usize,
        validation_filtered: usize,
    },

    // NEW: Job-level code summary
    CodeExtractionSummary {
        total_code_blocks: usize,
        language_breakdown: HashMap<Language, usize>,
        pattern_breakdown: HashMap<CodePattern, usize>,
        validation_stats: ValidationStats,
    },
}

pub struct ValidationStats {
    pub passed: usize,
    pub filtered_prose_ratio: usize,
    pub filtered_too_short: usize,
    pub filtered_no_structure: usize,
}
```

#### 14.2 Enhanced JobMetrics Component

Display code extraction metrics alongside existing stats:

```
┌─────────────────────────────────────────────────────────────┐
│ Items     │ Chunks    │ Embeddings │ Errors  │ Rate       │
│ 42/50     │ 1,203     │ 1,203      │ 0       │ 3.2/sec    │
├─────────────────────────────────────────────────────────────┤
│ Code Blocks: 127     │ Languages: 5      │ Pattern: Docusaurus │
│ └─ Validation: 124 passed, 3 filtered                       │
└─────────────────────────────────────────────────────────────┘
```

**New metrics:**
- `code_blocks` - Total code blocks extracted
- `languages` - Count of unique programming languages
- `dominant_pattern` - Most common extraction pattern (with %)
- `validation_passed` / `validation_filtered` - Code validation stats

#### 14.3 Enhanced PageStatusRow Component

Show per-page code extraction results:

```
┌───────────────────────────────────────────────────────────────────┐
│ ✓ /docs/patterns/messaging │ 234ms │ 12 chunks                   │
│   </> 8 code blocks (Rust, Python) • Docusaurus                  │
├───────────────────────────────────────────────────────────────────┤
│ ⚠ /docs/intro              │ 156ms │ 3 chunks                    │
│   </> 0 code blocks • No patterns matched                        │
└───────────────────────────────────────────────────────────────────┘
```

**Visual indicators:**
- `</>` icon for code-related information
- Show top 2 languages per page
- Display extraction pattern name
- Alert state (⚠) when expected code not found

#### 14.4 CodeExtractionPanel Component (NEW)

Detailed code extraction analytics panel (expandable):

```
┌─ CODE EXTRACTION DETAILS ─────────────────────────────────────────┐
│                                                                    │
│ Language Breakdown                                                 │
│ ┌────────────────────────────────────────────────────────────┐    │
│ │ Rust       █████████████████████  87 blocks                │    │
│ │ Python     ████████                32 blocks                │    │
│ │ JavaScript ████                    15 blocks                │    │
│ │ TypeScript ██                       8 blocks                │    │
│ │ YAML       █                        3 blocks                │    │
│ └────────────────────────────────────────────────────────────┘    │
│                                                                    │
│ Extraction Patterns                                                │
│ ┌────────────────────────────────────────────────────────────┐    │
│ │ Docusaurus    38 pages (90%)                               │    │
│ │ GitHub Blocks  3 pages (7%)                                │    │
│ │ Generic        1 page  (3%)                                │    │
│ └────────────────────────────────────────────────────────────┘    │
│                                                                    │
│ Validation Stats                                                   │
│ ┌────────────────────────────────────────────────────────────┐    │
│ │ ✓ Passed:       124 blocks (98%)                           │    │
│ │ ✗ Filtered:       3 blocks (2%)                            │    │
│ │   - High prose ratio: 2                                    │    │
│ │   - Too short: 1                                           │    │
│ └────────────────────────────────────────────────────────────┘    │
└────────────────────────────────────────────────────────────────────┘
```

#### 14.5 Updated URL Indexing Form

Replace legacy crawl settings with spider configuration:

```
┌─ CRAWL STRATEGY ─────────────────────────────────────────────────┐
│                                                                   │
│ Mode                                                              │
│ ○ HTTP Only  ● Smart (Recommended)  ○ JS Required                │
│ └─ Smart: Try HTTP first, fallback to JS rendering if needed     │
│                                                                   │
│ ☑ Enable HTTP caching (ETag/Last-Modified)                       │
│ ☑ Respect robots.txt                                             │
│                                                                   │
│ ▸ Advanced                                                        │
│   Crawl Budget: [____500___] pages per domain path               │
│   Render Timeout: [___30____] seconds (if JS required)           │
│                                                                   │
└───────────────────────────────────────────────────────────────────┘

┌─ CODE EXTRACTION ────────────────────────────────────────────────┐
│                                                                   │
│ ☑ Enable framework-aware code extraction                          │
│ ☑ Validate code blocks (filter non-code content)                 │
│                                                                   │
│ Minimum code length: [___10____] characters                       │
│ Max prose ratio:     [___0.6___] (0.0-1.0)                       │
│                                                                   │
└───────────────────────────────────────────────────────────────────┘
```

#### 14.6 API Response Updates

Enhanced job responses include code extraction data:

```typescript
interface JobProgress {
  // Existing fields
  processed: number;
  total: number;
  percentage: number;
  rate: number;
  current_item?: string;

  // NEW: Code extraction fields
  code_extraction: {
    total_blocks: number;
    languages: Record<string, number>;  // {"Rust": 5, "Python": 3}
    patterns: Record<string, number>;   // {"Docusaurus": 38, "GitHub": 3}
    validation: {
      passed: number;
      filtered: number;
      filter_reasons: Record<string, number>;
    };
  };
}

interface PageStatus {
  // Existing fields
  url: string;
  status: 'pending' | 'running' | 'completed' | 'error';
  chunks_created?: number;
  duration_ms?: number;

  // NEW: Per-page code extraction
  code_blocks?: number;
  code_languages?: string[];  // Top 2: ["Rust", "Python"]
  extraction_pattern?: string;  // "Docusaurus"
}
```

#### 14.7 Implementation Priority

| Phase | Component | Effort | Priority |
|-------|-----------|--------|----------|
| 1 | JobMetrics code stats | 0.5 day | **HIGH** |
| 1 | PageStatusRow code indicators | 0.5 day | **HIGH** |
| 1 | SSE event updates | 0.5 day | **HIGH** |
| 2 | CodeExtractionPanel | 1-2 days | **MEDIUM** |
| 2 | URL form smart crawling | 1 day | **MEDIUM** |
| 3 | Job history code column | 0.5 day | **LOW** |
| 3 | Code block preview | 1 day | **LOW** |

---

## Crate Structure

```
kix-indexing/
├── Cargo.toml
├── src/
│   ├── lib.rs                 # Public API exports
│   ├── indexer.rs             # Main orchestrator
│   ├── sources/
│   │   ├── mod.rs
│   │   ├── web.rs             # Spider integration
│   │   ├── git.rs             # Git repository indexing
│   │   ├── filesystem.rs      # Local file scanning
│   │   └── api.rs             # REST API ingestion
│   ├── analysis/
│   │   ├── mod.rs
│   │   ├── detection.rs       # Content type detection
│   │   ├── structure.rs       # Document structure extraction
│   │   └── entities.rs        # Entity extraction
│   ├── parsers/
│   │   ├── mod.rs
│   │   ├── pdf.rs             # PDF parsing & extraction
│   │   ├── docx.rs            # DOCX parsing & extraction
│   │   ├── html.rs            # HTML parsing
│   │   ├── markdown.rs        # Markdown parsing
│   │   └── table.rs           # Table detection & extraction
│   ├── extraction/
│   │   ├── mod.rs             # Public exports
│   │   ├── code_extractor.rs  # 30+ framework-aware patterns
│   │   ├── patterns.rs        # CodePattern enum + CSS selectors
│   │   ├── language.rs        # Language detection + normalization
│   │   └── validation.rs      # Code structure + prose ratio validation
│   ├── chunking/
│   │   ├── mod.rs
│   │   ├── semantic.rs        # Semantic chunking
│   │   ├── code.rs            # AST-aware code chunking
│   │   ├── hierarchical.rs    # Document hierarchy chunking
│   │   ├── sliding.rs         # Sliding window chunking
│   │   ├── late.rs            # Late chunking (contextual)
│   │   ├── table.rs           # Table-aware chunking
│   │   ├── pdf.rs             # PDF-aware chunking
│   │   └── docx.rs            # DOCX-aware chunking
│   ├── enrichment/
│   │   ├── mod.rs
│   │   ├── metadata.rs        # Metadata extraction
│   │   ├── hyde.rs            # Hypothetical document embeddings
│   │   ├── summary.rs         # Summary generation
│   │   └── relationships.rs   # Cross-reference extraction
│   ├── embedding/
│   │   ├── mod.rs
│   │   ├── provider.rs        # Provider trait
│   │   ├── ollama.rs          # Ollama (only provider - uses nomic-embed-text)
│   │   ├── cache.rs           # Embedding cache (SQLite-backed)
│   │   └── pipeline.rs        # Batch processing pipeline
│   ├── storage/
│   │   ├── mod.rs
│   │   ├── store.rs           # Store trait
│   │   ├── sqlite_vec.rs      # SQLite + sqlite-vec (SeaORM + raw SQL)
│   │   ├── entity/            # SeaORM entities (relational tables)
│   │   │   ├── mod.rs
│   │   │   ├── chunk.rs
│   │   │   ├── document.rs
│   │   │   └── vector.rs
│   │   ├── migration/         # SeaORM migrations
│   │   │   ├── mod.rs
│   │   │   └── m20240101_000001_create_tables.rs
│   │   ├── tantivy.rs         # Tantivy full-text search (BM25)
│   │   ├── qdrant.rs          # Qdrant (optional, feature: qdrant)
│   │   └── memory.rs          # In-memory (testing)
│   ├── quality/
│   │   ├── mod.rs
│   │   ├── scoring.rs
│   │   ├── deduplication.rs
│   │   └── health.rs
│   ├── distributed/
│   │   ├── mod.rs
│   │   ├── coordinator.rs     # Job coordination
│   │   ├── worker.rs          # Crawl workers
│   │   ├── queue.rs           # URL queue (Redis)
│   │   └── dedup.rs           # Distributed deduplication
│   ├── incremental/
│   │   ├── mod.rs
│   │   ├── detector.rs        # Change detection
│   │   ├── state.rs           # Index state management
│   │   ├── updater.rs         # Incremental update pipeline
│   │   └── watcher.rs         # File/source watching
│   ├── plugin/
│   │   ├── mod.rs
│   │   ├── traits.rs          # Plugin traits
│   │   ├── registry.rs        # Plugin registry
│   │   ├── loader.rs          # Dynamic plugin loading
│   │   ├── events.rs          # Event bus
│   │   └── prelude.rs         # Plugin development kit
│   ├── cli/
│   │   ├── mod.rs
│   │   ├── commands/
│   │   │   ├── index.rs       # Index commands
│   │   │   ├── search.rs      # Search/query commands
│   │   │   ├── manage.rs      # Management commands
│   │   │   └── stats.rs       # Statistics commands
│   │   └── output.rs          # Output formatting
│   └── config/
│       ├── mod.rs
│       └── toml.rs
├── kix-cli/                   # Separate CLI binary crate
│   ├── Cargo.toml
│   └── src/
│       └── main.rs
├── examples/
│   ├── basic_indexing.rs
│   ├── code_repository.rs
│   ├── documentation_site.rs
│   ├── incremental_updates.rs
│   ├── distributed_crawl.rs
│   └── custom_plugin.rs
├── plugins/                   # Example plugins
│   ├── confluence/
│   ├── notion/
│   └── jupyter/
└── tests/
    ├── integration/
    └── fixtures/
```

---

## Dependencies

```toml
[package]
name = "kix-indexing"
version = "0.1.0"
edition = "2021"
license = "MIT OR Apache-2.0"
description = "AutoRAG indexing engine for Rust"
repository = "https://github.com/helmsai/kix-indexing"
keywords = ["rag", "embeddings", "indexing", "search", "ai"]
categories = ["text-processing", "database"]

[dependencies]
# Async runtime
tokio = { version = "1", features = ["full"] }
async-trait = "0.1"
futures = "0.3"

# Web crawling (spider for fetching, CodeExtractor for code blocks)
spider = { version = "2", features = ["sync", "smart", "cache"] }
spider_transformations = "2"   # HTML → Markdown conversion
reqwest = { version = "0.12", features = ["json", "gzip"] }
scraper = "0.19"               # CSS selector-based code extraction
url = "2"

# Code parsing
tree-sitter = "0.22"
tree-sitter-rust = "0.21"
tree-sitter-python = "0.21"
tree-sitter-javascript = "0.21"
tree-sitter-typescript = "0.21"
tree-sitter-go = "0.21"
tree-sitter-java = "0.21"

# Text processing
text-splitter = { version = "0.13", features = ["tiktoken-rs", "tokenizers"] }
tiktoken-rs = "0.5"
tokenizers = "0.19"
pulldown-cmark = "0.11"
unicode-segmentation = "1"

# Document parsing
pdf-extract = "0.7"
docx-rs = "0.4"
calamine = "0.24"          # Excel/CSV parsing
lopdf = "0.31"             # Low-level PDF manipulation

# Serialization
serde = { version = "1", features = ["derive"] }
serde_json = "1"
toml = "0.8"

# Database/Storage (SeaORM + SQLite + sqlite-vec)
sea-orm = { version = "1.0", features = ["sqlx-sqlite", "runtime-tokio-native-tls", "macros"] }
sea-orm-migration = "1.0"
sqlite-vec = "0.1"         # Vector similarity search extension (raw SQL for vec0 virtual tables)
redis = { version = "0.25", features = ["tokio-comp"], optional = true }

# Full-text search (Tantivy)
tantivy = "0.22"           # BM25 ranking, schema-based indexing, faceted search

# Ollama Integration
ollama-rs = { version = "0.2", features = ["stream"] }

# Vector stores (optional alternatives)
qdrant-client = { version = "1", optional = true }

# CLI
clap = { version = "4", features = ["derive", "env"] }
indicatif = "0.17"         # Progress bars
console = "0.15"           # Terminal styling

# File watching (incremental indexing)
notify = "6"
notify-debouncer-mini = "0.4"

# Distributed crawling
deadpool-redis = "0.15"    # Redis connection pooling

# Plugin system
libloading = "0.8"         # Dynamic library loading

# Utilities
uuid = { version = "1", features = ["v4", "serde"] }
chrono = { version = "0.4", features = ["serde"] }
thiserror = "1"
anyhow = "1"
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
opentelemetry = { version = "0.22", optional = true }
opentelemetry-otlp = { version = "0.15", optional = true }

# Hashing/Deduplication
xxhash-rust = { version = "0.8", features = ["xxh3"] }
simhash = "0.2"
bloomfilter = "1"          # Distributed URL dedup

# Configuration
config = "0.14"
dotenvy = "0.15"

[dev-dependencies]
tokio-test = "0.4"
wiremock = "0.6"
tempfile = "3"
criterion = "0.5"

[features]
default = ["sqlite-vec", "cli"]
cli = ["clap", "indicatif", "console"]
sqlite-vec = []  # SeaORM for relational tables + raw SQL for sqlite-vec vectors + Tantivy for FTS
# Note: Ollama is always required (no feature flag) - embeddings use ollama-rs crate
qdrant = ["qdrant-client"]  # Optional: alternative vector store
distributed = ["redis", "deadpool-redis", "bloomfilter"]  # Optional: distributed crawling
telemetry = ["opentelemetry", "opentelemetry-otlp"]  # Optional: observability
plugins = ["libloading"]  # Optional: plugin system
full = ["cli", "sqlite-vec", "qdrant", "distributed", "telemetry", "plugins"]
```

---

## Usage Examples

### Basic Documentation Indexing

```rust
use kix_indexing::{Indexer, sources::WebSource, embedding::OllamaEmbeddings, storage::SqliteVecStore};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Configure source
    let source = WebSource::builder()
        .url("https://docs.rs/tokio/latest/tokio/")
        .max_depth(2)
        .build();
    
    // Configure embedding (Ollama with nomic-embed-text)
    let embeddings = OllamaEmbeddings::builder()
        .model("nomic-embed-text")
        .base_url("http://localhost:11434")
        .build();
    
    // Configure storage (SQLite + sqlite-vec)
    let store = SqliteVecStore::builder()
        .path("./tokio-docs.db")
        .vector_dimensions(768)
        .enable_wal(true)
        .build()
        .await?;
    
    // Build and run indexer
    let indexer = Indexer::builder()
        .add_source(source)
        .embedding_provider(embeddings)
        .vector_store(store)
        .build();
    
    let result = indexer.index_all().await?;
    
    println!("Indexed {} chunks from {} documents", 
        result.chunks_stored, 
        result.documents_processed
    );
    
    Ok(())
}
```

### Code Repository Indexing

```rust
use kix_indexing::{
    Indexer, 
    sources::GitSource, 
    chunking::{ChunkingStrategy, CodeGranularity},
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let source = GitSource::builder()
        .repo("https://github.com/tokio-rs/tokio")
        .branch("master")
        .include_patterns(vec![r".*\.rs$"])
        .exclude_patterns(vec![r"tests/.*", r"benches/.*"])
        .build();
    
    let indexer = Indexer::builder()
        .add_source(source)
        .chunking_strategy(ChunkingStrategy::CodeAst {
            granularity: CodeGranularity::Function,
            include_context: true,
        })
        .embedding_provider(embeddings)
        .vector_store(store)
        .build();
    
    let result = indexer.index_all().await?;
    
    // Print code-specific stats
    for (lang, count) in &result.statistics.language_distribution {
        println!("{:?}: {} chunks", lang, count);
    }
    
    Ok(())
}
```

### Incremental Updates with File Watching

```rust
use kix_indexing::{Indexer, sources::FileSystemSource};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let source = FileSystemSource::builder()
        .path("./docs")
        .watch(true)
        .build();
    
    let indexer = Indexer::builder()
        .add_source(source)
        .embedding_provider(embeddings)
        .vector_store(store)
        .on_change(|event| async {
            println!("File changed: {:?}", event.path);
        })
        .build();
    
    // Initial index
    indexer.index_all().await?;
    
    // Watch for changes (blocking)
    indexer.watch().await?;
    
    Ok(())
}
```

---

## Performance Considerations

### Concurrency

- Crawling: Configurable concurrent requests (default: 10)
- Embedding: Batch processing with configurable batch size
- Storage: Async upserts with connection pooling

### Memory Management

- Streaming document processing (no full corpus in memory)
- Chunked file reading for large documents
- LRU cache for embeddings with configurable size

### Optimization Targets

| Metric | Target |
|--------|--------|
| Documents/second (crawling) | 50+ |
| Chunks/second (embedding) | 100+ |
| Chunks/second (storage) | 500+ |
| Memory usage (1M chunks) | <2GB |

---

## Roadmap

### Phase 1: Core (v0.1.0)
- [x] Web crawling with spider
- [x] Semantic chunking
- [x] Code chunking with tree-sitter
- [x] Ollama embeddings (nomic-embed-text)
- [x] Qdrant storage
- [x] Basic CLI

### Phase 2: Advanced Chunking (v0.2.0)
- [x] Hierarchical document chunking
- [x] Late chunking implementation
- [x] Table-aware chunking
- [x] PDF/DOCX parsing

### Phase 3: Enrichment (v0.3.0)
- [x] HyDE generation
- [x] Summary generation
- [x] Entity extraction
- [x] Relationship extraction

### Phase 4: Scale (v0.4.0)
- [x] Distributed crawling
- [x] Incremental indexing
- [x] Deduplication pipeline
- [x] Multi-vector storage

### Phase 5: Ecosystem (v0.5.0)
- [x] Additional embedding providers
- [x] Additional vector stores
- [x] Plugin system
- [ ] Web UI dashboard

---

## Success Metrics

| Metric | Definition | Target |
|--------|------------|--------|
| Retrieval Accuracy | % relevant chunks in top-5 | >85% |
| Indexing Throughput | Documents per minute | >1000 |
| Chunk Quality Score | Average quality metric | >0.8 |
| Deduplication Rate | % duplicates detected | >95% |
| API Latency (p99) | Search response time | <100ms |

---

## Appendix A: Supported Languages (Tree-sitter)

| Language | Crate | Status |
|----------|-------|--------|
| Rust | tree-sitter-rust | ✅ |
| Python | tree-sitter-python | ✅ |
| JavaScript | tree-sitter-javascript | ✅ |
| TypeScript | tree-sitter-typescript | ✅ |
| Go | tree-sitter-go | ✅ |
| Java | tree-sitter-java | ✅ |
| C# | tree-sitter-c-sharp | ✅ |
| C/C++ | tree-sitter-c / tree-sitter-cpp | ✅ |
| Ruby | tree-sitter-ruby | ✅ |
| PHP | tree-sitter-php | ✅ |
| Swift | tree-sitter-swift | ✅ |
| Kotlin | tree-sitter-kotlin | ✅ |
| SQL | tree-sitter-sql | ✅ |
| Shell/Bash | tree-sitter-bash | ✅ |
| YAML | tree-sitter-yaml | ✅ |
| JSON | tree-sitter-json | ✅ |
| TOML | tree-sitter-toml | ✅ |
| Markdown | tree-sitter-md | ✅ |

---

## Appendix B: Vector Store Options

| Store | Self-Hosted | Filtering | Hybrid Search | Recommended For |
|-------|-------------|-----------|---------------|-----------------|
| **SQLite-vec + SeaORM + Tantivy** | ✅ | ✅ | ✅ | **Default**, single-node, <10M vectors |
| Qdrant (feature: `qdrant`) | ✅ | ✅ | ✅ | Large-scale distributed |
| In-Memory | ✅ | ✅ | ❌ | Testing only |

**Why SQLite-vec + SeaORM + Tantivy is the default:**
- Zero external server dependencies (single `.db` file + Tantivy index directory)
- No server process to manage
- Type-safe queries for relational data with SeaORM entities
- Raw SQL for sqlite-vec vector operations (SeaORM doesn't bind to `vec0` virtual tables)
- Automatic SeaORM migrations on startup
- ACID transactions for reliable indexing
- Full SQL filtering with vector search
- Easy backup (copy the `.db` file + Tantivy index)
- Excellent for development and production up to ~10M vectors
- Hybrid search via Tantivy (BM25 ranking) + sqlite-vec (vector similarity) with Reciprocal Rank Fusion

---

## Appendix C: Chunking Strategy Decision Matrix

| Content Type | Recommended Strategy | Granularity | Overlap |
|--------------|---------------------|-------------|---------|
| Technical docs | Hierarchical | Section | 10% |
| Blog posts | Semantic | Paragraph | 15% |
| API reference | Hierarchical | Endpoint | 5% |
| Source code | CodeAST | Function | Context |
| Legal documents | Semantic | Clause | 20% |
| Research papers | Hierarchical | Section | 10% |
| Chat logs | Sliding | Message | 50% |
| Structured data | Tabular | Row group | Headers |

---

## Appendix D: Embedding Model

kix-indexing uses **nomic-embed-text** via Ollama exclusively.

| Property | Value |
|----------|-------|
| Model | nomic-embed-text |
| Provider | Ollama (local) |
| Dimensions | 768 |
| Max Context | 8192 tokens |
| Matryoshka Support | Yes (768, 512, 256, 128, 64) |
| Cost | Free (local inference) |

**Why nomic-embed-text:**
- Best balance of quality, context length (8192 tokens), and local inference
- Matryoshka representations allow dimension reduction without retraining
- Competitive with cloud embeddings on MTEB benchmarks
- No API costs or rate limits
- Full data privacy (never leaves your machine)
- GPU acceleration handled automatically by Ollama

---

## Appendix E: Framework-Aware Code Extraction Patterns

The `CodeExtractor` module supports 30+ patterns for extracting code blocks from HTML documentation. Patterns are applied in priority order (most specific first).

### Documentation Frameworks

| Pattern | CSS Selector | Description |
|---------|-------------|-------------|
| `DocusaurusCodeBlock` | `.prism-code, [class*='codeBlockContent']` | Docusaurus v2+ code blocks |
| `DocusaurusTabCodeBlock` | `.tabs-container pre code` | Docusaurus tabbed code panels |
| `MkDocsCodeBlock` | `.highlight pre, .codehilite pre` | MkDocs Material theme |
| `SphinxCodeBlock` | `.highlight-python pre, .highlight-default pre` | Sphinx documentation |
| `ReadTheDocsCode` | `.rst-content pre` | ReadTheDocs hosted docs |
| `JekyllHighlight` | `.highlighter-rouge pre, .highlight pre.highlight` | Jekyll static sites |
| `HugoHighlight` | `.highlight pre, .chroma pre` | Hugo static sites |
| `VuePressCode` | `div[class*='language-'] pre` | VuePress documentation |
| `GatsbyCode` | `.gatsby-highlight pre` | Gatsby sites |
| `NextjsRehype` | `[data-rehype-pretty-code] code` | Next.js with rehype |
| `AstroCode` | `.astro-code pre` | Astro framework |

### Syntax Highlighters

| Pattern | CSS Selector | Description |
|---------|-------------|-------------|
| `PrismJs` | `[class*='language-'] code, pre[class*='language-']` | Prism.js syntax highlighting |
| `HighlightJs` | `.hljs, pre code.hljs` | Highlight.js library |
| `SyntaxHighlighter` | `.syntaxhighlighter` | SyntaxHighlighter library |
| `RougeSyntax` | `.rouge pre, .rouge-code` | Rouge (Ruby) highlighter |
| `Shiki` | `.shiki code, pre.shiki` | Shiki (VS Code themes) |

### Platform-Specific

| Pattern | CSS Selector | Description |
|---------|-------------|-------------|
| `GitHubCode` | `.blob-code-content, .highlight pre, .js-file-line` | GitHub code views |
| `GitLabCode` | `.blob-content pre, .code pre` | GitLab code views |
| `BitbucketCode` | `.code-container pre` | Bitbucket code views |
| `StackOverflowCode` | `.s-prose pre, .s-code-block, .post-text pre` | Stack Overflow Q&A |

### Editor Components

| Pattern | CSS Selector | Description |
|---------|-------------|-------------|
| `MonacoEditor` | `.monaco-editor .view-lines` | Monaco (VS Code) editor |
| `CodeMirror` | `.CodeMirror-code, .cm-content` | CodeMirror editor |
| `AceEditor` | `.ace_editor .ace_content` | Ace editor |

### Terminal/Shell

| Pattern | CSS Selector | Description |
|---------|-------------|-------------|
| `TerminalOutput` | `.terminal pre, .console pre, .shell pre` | Terminal output blocks |
| `AsciinemaPlayer` | `.asciinema-player pre` | Asciinema recordings |

### Generic Patterns (Fallback)

| Pattern | CSS Selector | Description |
|---------|-------------|-------------|
| `DataLanguageAttr` | `[data-language] code, [data-lang] code` | data-* attribute hints |
| `ClassPrefixCode` | `[class*='code-'] pre, [class*='snippet'] pre` | Class prefix patterns |
| `DataCodeAttr` | `[data-code]` | data-code attribute |
| `PreCode` | `pre code` | Standard pre > code |
| `PreOnly` | `pre:not(:has(code))` | Pre without code tag |
| `CodeOnly` | `code:not(pre code)` | Standalone code element |

### Language Detection Priority

When detecting the programming language, the extractor checks in this order:

1. **Class attribute**: `class="language-rust"`, `class="lang-rust"`, `class="rust"`
2. **Data attributes**: `data-language="rust"`, `data-lang="rust"`, `data-code-language="rust"`
3. **Parent element**: Check parent's class and data attributes
4. **Known languages**: Match against known language names in class list
5. **Tree-sitter validation**: Optionally validate by attempting to parse

### Language Normalization

Common aliases are normalized to canonical names:

| Alias | Normalized |
|-------|------------|
| `js` | `JavaScript` |
| `ts` | `TypeScript` |
| `rs` | `Rust` |
| `py` | `Python` |
| `rb` | `Ruby` |
| `cs`, `c#` | `CSharp` |
| `c++`, `cxx` | `Cpp` |
| `sh`, `zsh` | `Shell` |
| `yml` | `Yaml` |
| `md` | `Markdown` |

---

*End of PRD*
