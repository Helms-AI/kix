# KIX - Knowledge Indexer

A high-performance Rust-based semantic search and knowledge management system. KIX provides intelligent document indexing with hybrid search (vector + full-text), AI assistant integration via MCP, and a modern React dashboard.

## Key Capabilities

### Semantic Search
- **Hybrid Search Engine**: Combines vector similarity search with Tantivy BM25 full-text search for optimal results
- **Local Embeddings via Ollama**: Uses nomic-embed-text model (768 dimensions, 8192 max tokens) with GPU auto-detection
- **Smart Chunking**: AST-aware code chunking with tree-sitter (14 languages), intelligent paragraph/sentence splitting
- **Two-Layer Storage**: Full pages for RAG context + searchable chunks with page references

### Document Processing
- **Multi-Format Parsing**: HTML, PDF, DOCX, Excel, CSV, Markdown with automatic format detection
- **Code Extraction**: 30+ extraction patterns for major frameworks (Docusaurus, MkDocs, Sphinx, Hugo, GitHub, GitLab)
- **Language Detection**: 20+ programming languages with automatic syntax detection
- **Multi-Stage Validation**: Length, structure, prose ratio, and placeholder detection filters

### AI Assistant Integration
- **MCP Server**: Full Model Context Protocol support with HTTP and stdio transports
- **25+ Tools**: Search, indexing, project management, GitHub integration, AI planning
- **Real-Time Updates**: Server-Sent Events for live progress tracking during indexing

### Web Crawling & Indexing
- **Smart Discovery**: Priority-based URL discovery (llms.txt → sitemap → robots.txt)
- **Multiple Strategies**: Single page, batch parallel, recursive depth-first, sitemap-based
- **Code Preservation**: Maintains code blocks through HTML → Markdown conversion
- **9-Stage Pipeline**: Tracked progress from discovery through finalization

### Project Management
- **Local Issue Tracking**: Create and manage issues with Kanban-style boards
- **GitHub Integration**: Bidirectional sync with GitHub Issues and Projects V2
- **Knowledge Linking**: Connect knowledge base entries to project context
- **AI Planning**: Use indexed knowledge to assist with project planning

### Performance
- **GPU Acceleration**: CUDA (NVIDIA) and Metal/CoreML (Apple Silicon) support
- **SIMD Optimizations**: Native CPU optimizations with target-cpu=native
- **jemalloc Allocator**: Improved memory performance and reduced fragmentation
- **Parallel Processing**: 8 concurrent jobs, optimized batch sizes (256-4096 depending on hardware)

---

## Quick Start

### Prerequisites
- **Rust** (1.75+): https://rustup.rs
- **Node.js** (18+): https://nodejs.org
- **Ollama**: https://ollama.ai (for local embeddings)

### One-Command Start

```bash
./run.sh
```

This will:
1. Install Ollama (macOS) or use Docker (Linux/Windows)
2. Pull the `nomic-embed-text` embedding model
3. Build the Rust binary with SIMD optimizations
4. Install frontend dependencies
5. Start all services:
   - **Web UI**: http://localhost:3000
   - **REST API**: http://localhost:3001
   - **MCP HTTP**: http://localhost:3002/mcp
   - **MCP stdio**: Enabled for IDE integration

### Manual Build

```bash
# Build everything
./build.sh

# Or build with GPU support
./build-performance.sh

# For Apple Silicon GPU:
cargo build --release --features onnx-coreml --manifest-path server/Cargo.toml

# For NVIDIA GPU:
cargo build --release --features onnx-cuda --manifest-path server/Cargo.toml
```

### CLI Usage

```bash
# Start unified server (API + MCP)
./server/target/release/kix run

# Start with stdio transport for IDE integration
./server/target/release/kix run --stdio

# Search from command line
./server/target/release/kix search "authentication patterns" --limit 5

# Show index statistics
./server/target/release/kix stats
```

---

## MCP Integration

KIX exposes a full MCP server for AI assistant integration. Configure in your MCP client:

### HTTP Transport
```json
{
  "mcpServers": {
    "kix": {
      "url": "http://localhost:3002/mcp"
    }
  }
}
```

### stdio Transport
```json
{
  "mcpServers": {
    "kix": {
      "command": "/path/to/kix",
      "args": ["serve"]
    }
  }
}
```

### Available MCP Tools

| Category | Tools |
|----------|-------|
| **Search** | `search_patterns`, `get_pattern`, `list_patterns`, `find_related`, `search_by_problem`, `search_by_technology` |
| **Analysis** | `explain_pattern`, `compare_patterns`, `get_category_overview`, `suggest_architecture`, `pattern_sequence` |
| **Indexing** | `index_document`, `index_batch`, `delete_document`, `get_index_status` |
| **Projects** | `create_project`, `list_projects`, `get_project`, `update_project`, `delete_project` |
| **Issues** | `create_issue`, `list_issues`, `get_issue`, `update_issue`, `delete_issue` |
| **GitHub** | `set_github_token`, `sync_github_issues`, `create_github_project`, `add_issue_to_project` |
| **Knowledge** | `link_entry`, `unlink_entry`, `list_project_entries` |
| **AI Planning** | `plan_project`, `suggest_tasks`, `breakdown_task`, `get_project_context` |

---

## REST API

### Core Endpoints

| Endpoint | Description |
|----------|-------------|
| `GET /api/search` | Hybrid search with query parameters |
| `GET /api/entries` | List all indexed entries |
| `GET /api/entries/:id` | Get entry details |
| `POST /api/indexing/url` | Index a URL |
| `POST /api/indexing/batch` | Batch index multiple URLs |
| `GET /api/indexing/jobs` | List indexing jobs |
| `GET /api/indexing/sse` | SSE stream for progress updates |
| `GET /api/projects` | List projects |
| `POST /api/projects` | Create project |
| `GET /api/projects/:id/issues` | List project issues |
| `GET /api/projects/events` | SSE stream for project events |

---

## Docker

```bash
# Build Docker images
docker compose build

# Start all services
docker compose up -d

# View logs
docker compose logs -f

# Stop services
docker compose down
```

Docker automatically handles Ollama setup and model downloading.

---

## Developer Guide

### Architecture

KIX is built as a Rust workspace with 15 specialized crates:

```
server/crates/
├── kix-cli/         # Main CLI binary - orchestrates all crates
├── kix-parser/      # Document parsing, smart chunking, code validation
├── kix-embeddings/  # Embedding generation via Ollama (GPU auto-detection)
├── kix-sqlite/      # SQLite + SeaORM persistence (entities + migrations)
├── kix-search/      # Tantivy full-text search (BM25 ranking)
├── kix-store/       # Two-layer storage (pages + chunks + projects)
├── kix-services/    # Shared service layer for API and MCP
├── kix-mcp/         # MCP server with rmcp (HTTP + stdio transports)
├── kix-api/         # Axum REST API for dashboard
├── kix-crawler/     # URL discovery, crawling strategies, code extraction
├── kix-jobs/        # Job queue, executor, content processor
├── kix-sse/         # Server-Sent Events for real-time updates
├── kix-projects/    # Project management with GitHub integration
├── kix-vectors/     # Vector storage abstraction
└── kix-auth/        # OAuth 2.1 authentication
```

### Data Flow

```
1. Discovery      → URL discovery (llms.txt → sitemap → robots)
2. Crawling       → Playwright browser with strategy selection
3. Processing     → Readability extraction → Markdown conversion
4. Code Extract   → 30+ patterns with multi-stage validation
5. Smart Chunking → Code blocks → paragraphs → sentences
6. Embeddings     → Ollama with GPU auto-detection
7. Two-Layer Store→ Pages (full) + Chunks (searchable) in SQLite
8. Tantivy Index  → Full-text search with BM25 ranking
9. Search         → Hybrid (vector + Tantivy) with RAG context
```

### Key Technologies

| Layer | Technology |
|-------|------------|
| Runtime | Tokio async runtime |
| Database | SQLite + SeaORM |
| Vectors | sqlite-vec (in-memory) |
| Full-Text | Tantivy (BM25) |
| Embeddings | Ollama (nomic-embed-text) |
| Web Framework | Axum |
| MCP | rmcp crate |
| Frontend | React + TypeScript + Vite + TailwindCSS |
| Browser | Playwright |
| Code AST | Tree-sitter (14 languages) |

### Smart Chunking Algorithm

The chunker prioritizes preserving code blocks and semantic boundaries:

1. **Code blocks** - Never split (preserve complete code examples)
2. **Paragraphs** - Split on double newlines
3. **Sentences** - Split on sentence boundaries
4. **Hard cut** - Last resort at chunk size limit

Configuration:
- Break threshold: 30% minimum position (`SMART_BREAK_THRESHOLD`)
- Consolidation: Chunks < 200 chars merged (`SMART_CONSOLIDATION_THRESHOLD`)

### Code Extraction Patterns

Framework-aware code extraction from documentation sites:

| Framework | Patterns |
|-----------|----------|
| Docusaurus | theme-code-block, prism-code-block |
| MkDocs | codehilite, md-code |
| Sphinx | highlight-python, highlight-rust |
| Hugo | highlight.js, chroma |
| GitHub | highlight-source-*, blob-code |
| GitLab | blob-content |
| General | pre>code, pre.language-*, fenced code blocks |

### Tree-sitter Integration

AST-aware chunking for source code files (14 languages):

- **Languages**: Rust, Python, JavaScript, TypeScript, Go, Java, C, C++, C#, Ruby, Bash, JSON, HTML, CSS
- **Features**: Symbol extraction, function/class boundaries, configurable chunk sizes

### Testing

350+ tests across core crates:

```bash
# Run all tests
cargo test --manifest-path server/Cargo.toml

# Run specific crate tests
cargo test --release -p kix-crawler    # 59 tests
cargo test --release -p kix-parser     # 57 tests
cargo test --release -p kix-sqlite     # 33 tests
cargo test --release -p kix-search     # 21 tests
cargo test --release -p kix-projects   # 59 tests
cargo test --release -p kix-jobs       # 23 tests

# Integration tests
cargo test --release -p kix-jobs --test pipeline_integration
```

### Frontend Development

```bash
cd client
npm ci
npm run dev      # Development server (port 3000)
npm run build    # Production build
npm run lint     # Run ESLint
```

Stack: React 18, TypeScript, Vite, TailwindCSS, React Query, React Router, Recharts, Lucide Icons

### Port Allocation

| Service | Port | Description |
|---------|------|-------------|
| Web UI | 3000 | Vite dev server |
| REST API | 3001 | Axum REST endpoints |
| MCP HTTP | 3002 | Streamable HTTP transport |
| Ollama | 11434 | Embedding model server |

### Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `KIX_DATA_DIR` | `./data` | Data directory |
| `KIX_SQLITE_PATH` | `./data/sqlite/kix.db` | SQLite database path |
| `OLLAMA_HOST` | `http://localhost:11434` | Ollama server URL |
| `OLLAMA_MODEL` | `nomic-embed-text` | Embedding model |
| `KIX_EMBEDDING_DIM` | `768` | Embedding dimensions |
| `KIX_ENCRYPTION_KEY` | (dev key) | AES-256 key for token encryption |
| `RUST_LOG` | `kix=info,warn` | Log level |

### API/MCP Unification

KIX uses a shared service layer ensuring consistency between REST API and MCP:

```
User ──────► REST API ──────┐
             (kix-api)       │
                             ├──► kix-services ◄── Store Layer
MCP Client ► MCP Server ────┘    (shared logic)
             (kix-mcp)
```

Both the API and MCP call the same functions in `kix-services`, emitting events through a shared `EventBus`.

### Performance Optimizations

Built-in optimizations:
- **jemalloc** allocator for better memory performance
- **SIMD** via `target-cpu=native` compiler flag
- **Parallel jobs** (8 concurrent) with optimized batch sizes
- **GPU acceleration** via ONNX Runtime (CUDA/CoreML features)

Performance builds:
```bash
# Maximum optimizations
RUSTFLAGS="-C target-cpu=native -C lto=fat -C codegen-units=1" \
    cargo build --manifest-path server/Cargo.toml --release
```

---

## License

MIT

---

## Contributing

1. Fork the repository
2. Create a feature branch
3. Run tests: `cargo test --manifest-path server/Cargo.toml`
4. Submit a pull request

For major changes, please open an issue first to discuss the approach.
