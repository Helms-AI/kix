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
├── kix-parser/      # Document parsing (HTML, PDF, DOCX, Excel, CSV, Markdown, text)
├── kix-embeddings/  # Embedding generation (fastembed) and document chunking
├── kix-store/       # LanceDB vector storage, indexing, and search operations
├── kix-mcp-server/  # MCP server with search/indexing tools (rmcp crate)
├── kix-api/         # Axum REST API for dashboard
├── kix-crawler/     # URL crawling, file upload handling, rate limiting
├── kix-jobs/        # Job queue and executor for async indexing tasks
└── kix-sse/         # Server-Sent Events for real-time progress updates
```

### Key Dependencies
- **fastembed**: Local embedding model (no external API calls)
- **lancedb**: Vector database with hybrid search support
- **rmcp**: Model Context Protocol server implementation
- **axum**: Web framework for REST API
- **tokio**: Async runtime

### Data Flow
1. Content is parsed by `kix-parser` into `Document` structs
2. `kix-embeddings` chunks documents and generates vector embeddings
3. `kix-store` stores documents and chunks in LanceDB tables
4. Search queries use hybrid search (vector + full-text) via `kix-store`
5. `kix-mcp-server` exposes tools for AI assistants to search/index
6. `kix-api` provides REST endpoints for the client dashboard
7. `kix-jobs` + `kix-crawler` handle async indexing from URLs/files
8. `kix-sse` streams progress updates to the dashboard

### Core Types

- `Document` (kix-parser): Parsed document with title, content, categories, pattern_type
- `DocumentChunk` (kix-parser): Text chunks with metadata for embedding
- `KixStore` (kix-store): Main store interface with `hybrid_search`, `vector_search`, `text_search`
- `EmbeddingGenerator` (kix-embeddings): Generates embeddings for queries and documents
- `KixMcpServer` (kix-mcp-server): MCP server with 14 tools for search/indexing

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

Tests are colocated with source files using `#[cfg(test)]` modules. Key test files:
- `server/crates/kix-parser/src/*.rs` - Parser tests for each format
- `server/crates/kix-store/src/search.rs` - Search functionality tests
- `server/crates/kix-embeddings/src/chunker.rs` - Chunking logic tests
- `server/crates/kix-api/src/routes.rs` - API endpoint tests
