# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

KIX (Knowledge Indexer) - A high-performance Rust-based semantic search and knowledge management system. Originally built for Enterprise Integration Patterns, now a general-purpose knowledge indexing system. It provides:
- Vector storage with LanceDB for semantic search
- Multi-format document parsing (HTML, PDF, DOCX, Excel, CSV, Markdown)
- MCP (Model Context Protocol) server for AI assistant integration
- REST API for a React dashboard
- Real-time indexing with SSE progress updates

## Common Commands

### Development
```bash
./run.sh                                                        # Start all services (auto-builds with SIMD)
./build.sh                                                      # Build everything (Rust + client)
./build-performance.sh                                          # Build with maximum optimizations
cargo test --manifest-path server/Cargo.toml                    # Run all tests
cargo test --manifest-path server/Cargo.toml -p kix-parser      # Run tests for specific crate
```

### CLI Usage
```bash
./server/target/release/kix api --port 3001                     # Start REST API
./server/target/release/kix serve                               # Start MCP server (stdio)
./server/target/release/kix serve-http --port 3002              # Start MCP server (HTTP)
./server/target/release/kix search "query" --limit 5            # CLI search
./server/target/release/kix stats                               # Show index statistics
```

### Client (Web Dashboard)
```bash
cd client && npm ci && npm run dev    # Development server on port 3000
cd client && npm run build            # Production build
cd client && npm run lint             # Run ESLint
```

### Docker
```bash
./build.sh                            # Build all artifacts
docker compose build                  # Build Docker images (with SIMD optimizations)
docker compose up -d                  # Start services
docker compose run --rm kix-tools     # Run utility commands
```

### Performance Builds
```bash
# Default (CPU with SIMD)
./build-performance.sh

# With GPU support (NVIDIA CUDA)
cargo build --release --features onnx-cuda --manifest-path server/Cargo.toml

# With GPU support (Apple Metal)
cargo build --release --features onnx-coreml --manifest-path server/Cargo.toml
```

## Architecture

### Rust Workspace (9 crates)

```
server/crates/
├── kix-cli/         # Main CLI binary - orchestrates all other crates
├── kix-parser/      # Document parsing, smart chunking, code validation
├── kix-embeddings/  # Embedding generation (fastembed) with contextual support
├── kix-store/       # LanceDB two-layer storage (pages + chunks)
├── kix-mcp-server/  # MCP server with search/indexing tools (rmcp crate)
├── kix-api/         # Axum REST API for dashboard
├── kix-crawler/     # URL discovery, crawling strategies, code extraction
├── kix-jobs/        # Job queue, executor, and content processor
└── kix-sse/         # Server-Sent Events for real-time progress updates
```

### kix-crawler Submodules

```
kix-crawler/
├── discovery.rs      # URL discovery (llms.txt → sitemap → robots priority)
├── ssrf.rs           # SSRF protection and URL validation
├── progress.rs       # 9-stage progress tracking with monotonicity
├── cancellation.rs   # Global cancellation registry for job control
├── extractor.rs      # Markdown generation from HTML with code preservation
├── code.rs           # 30+ code extraction patterns (Docusaurus, MkDocs, etc.)
├── service.rs        # CrawlerService orchestration
└── strategies/
    ├── single_page.rs  # Single page with retry and framework detection
    ├── batch.rs        # Parallel batch crawling with semaphore
    ├── recursive.rs    # Depth-first recursive crawling
    └── sitemap.rs      # XML sitemap parsing and crawling
```

### kix-parser Submodules

```
kix-parser/
├── chunker.rs        # Smart chunking (code → paragraphs → sentences)
├── validator.rs      # Multi-stage code validation (length, structure, prose ratio)
├── html.rs           # HTML parsing with readability extraction
├── document.rs       # Entry and EntryChunk types with page_id FK
└── ...               # PDF, DOCX, Excel, CSV, Markdown parsers
```

### kix-store Two-Layer Storage

```
kix-store/
├── store.rs          # KixStore with init_tables(), store_page_with_chunks()
├── pages.rs          # PageStore for full page content (RAG context)
├── search.rs         # Hybrid search (vector + full-text)
└── schema.rs         # LanceDB schemas for entries, chunks, pages
```

### Key Dependencies
- **fastembed**: Local embedding model (no external API calls)
- **lancedb**: Vector database with hybrid search support
- **rmcp**: Model Context Protocol server implementation
- **axum**: Web framework for REST API
- **tokio**: Async runtime

### Data Flow

```
┌─────────────────────────────────────────────────────────────────┐
│                    KIX Processing Pipeline                       │
├─────────────────────────────────────────────────────────────────┤
│  1. Discovery      → URL discovery (llms.txt → sitemap → robots)│
│  2. Crawling       → Playwright browser with strategy selection │
│  3. Processing     → Readability extraction → Markdown          │
│  4. Code Extract   → 30+ patterns with multi-stage validation   │
│  5. Smart Chunking → Code blocks → paragraphs → sentences       │
│  6. Embeddings     → fastembed with optional page context       │
│  7. Two-Layer Store→ Pages (full) + Chunks (searchable) in Lance│
│  8. Search         → Hybrid (vector + FTS) with RAG context     │
└─────────────────────────────────────────────────────────────────┘
```

**Detailed Flow:**
1. `kix-crawler/discovery` discovers URLs via llms.txt → sitemap → robots priority
2. `kix-crawler/strategies` crawls using single_page, batch, recursive, or sitemap strategy
3. `kix-parser/html` extracts content using Mozilla Readability → Markdown conversion
4. `kix-crawler/code` extracts code blocks using 30+ patterns (Docusaurus, MkDocs, etc.)
5. `kix-parser/chunker` uses smart algorithm: code blocks (never split) → paragraphs → sentences
6. `kix-embeddings` generates vector embeddings with worker pool (auto-scales CPU/GPU)
7. `kix-store` stores pages (full content for RAG) + chunks (with page_id FK for context)
8. Search returns chunks; `get_page_context()` retrieves full page for RAG enrichment

### Core Types

- `Entry` (kix-parser): Parsed document with title, content, source_type, entry_type
- `EntryChunk` (kix-parser): Text chunks with chunk_index, page_id FK, and metadata
- `PageRecord` (kix-store): Full page content for RAG context retrieval
- `KixStore` (kix-store): Main store with `hybrid_search`, `store_page_with_chunks`, `get_page_for_chunk`
- `ContentProcessor` (kix-jobs): Orchestrates parsing, chunking, embedding, storage
- `CrawlerService` (kix-crawler): 9-stage crawl pipeline with progress and cancellation
- `SmartChunker` (kix-parser): Smart chunking with consolidation
- `CodeValidator` (kix-parser): Multi-stage validation (length, structure, prose ratio)

## Smart Chunking

KIX uses intelligent chunking optimized for documentation and code content:

### Chunking Algorithm (kix-parser/src/chunker.rs)
- Priority: Code blocks → Paragraphs → Sentences → Hard cut
- Break threshold: 30% minimum position (`SMART_BREAK_THRESHOLD`)
- Consolidation: Chunks < 200 chars merged (`SMART_CONSOLIDATION_THRESHOLD`)

### Content Extraction (kix-crawler/src/extractor.rs)
- Content source: Raw HTML (NOT cleaned) - preserves code blocks
- Code preservation: Priority 1
- Markdown generation: htmd library

### Progress Tracking (9 Stages)

| Stage | Range | Description |
|-------|-------|-------------|
| starting | 0-5% | Initialization |
| discovery | 5-15% | URL discovery |
| analyzing | 15-25% | Content analysis |
| crawling | 25-60% | Page fetching |
| processing | 60-75% | HTML → Markdown |
| source_creation | 75-80% | Entry creation |
| document_storage | 80-90% | LanceDB storage |
| code_extraction | 90-95% | Code block extraction |
| finalization | 95-100% | Cleanup and completion |

### MCP Tools (exposed to AI assistants)
Search: `search_patterns`, `get_pattern`, `list_patterns`, `find_related`, `search_by_problem`, `search_by_technology`
Analysis: `explain_pattern`, `compare_patterns`, `get_category_overview`, `suggest_architecture`, `pattern_sequence`
Indexing: `index_document`, `index_batch`, `delete_document`, `get_index_status`

## Client Frontend

React + TypeScript + Vite + TailwindCSS

```
client/src/
├── pages/           # Route components (Dashboard, SearchPage, IndexingDashboard, etc.)
├── api/             # API client hooks
├── hooks/           # Custom React hooks
├── components/      # Shared components
└── types/           # TypeScript types
```

Vite proxy configuration:
- `/api` → `http://127.0.0.1:3001` (REST API)
- `/api/indexing/sse` → SSE-specific proxy settings
- `/mcp` → `http://127.0.0.1:3002` (MCP HTTP server)

## Port Allocation

| Service | Port |
|---------|------|
| Web UI  | 3000 |
| REST API | 3001 |
| MCP HTTP | 3002 |

## Testing

Tests are colocated with source files using `#[cfg(test)]` modules. **95+ tests across core crates.**

### Unit Tests
```bash
cargo test --release -p kix-crawler    # 45 tests (discovery, code, strategies, etc.)
cargo test --release -p kix-parser     # 40 tests (chunker, validator, parsers)
cargo test --release -p kix-store      # 10 tests (pages, search, schema)
```

### Integration Tests
```bash
cargo test --release -p kix-jobs --test pipeline_integration  # Full pipeline tests
```

### Key Test Modules
- `kix-crawler/src/discovery.rs` - URL discovery (llms.txt, sitemap, robots)
- `kix-crawler/src/code.rs` - 30+ code extraction patterns
- `kix-parser/src/chunker.rs` - Smart chunking algorithm
- `kix-parser/src/validator.rs` - Multi-stage code validation
- `kix-store/src/pages.rs` - Two-layer storage
- `kix-store/src/search.rs` - Hybrid search functionality
- `kix-jobs/tests/pipeline_integration.rs` - End-to-end pipeline tests
