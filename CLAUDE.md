# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

KIX (Knowledge Indexer) - A high-performance Rust-based semantic search and knowledge management system. Originally built for Enterprise Integration Patterns, now a general-purpose knowledge indexing system. It provides:
- SQLite storage with in-memory vectors for semantic search
- Full-text search via Tantivy (BM25 ranking)
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
./server/target/release/kix api --api-port 3001 --mcp-port 3002 # Start REST API
./server/target/release/kix serve                               # Start MCP server (HTTP)
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
nerdctl compose build                  # Build Docker images (with SIMD optimizations)
nerdctl compose up -d                  # Start services
nerdctl compose run --rm kix-tools     # Run utility commands
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

### Rust Workspace (14 crates)

```
server/crates/
├── kix-cli/         # Main CLI binary - orchestrates all other crates
├── kix-parser/      # Document parsing, smart chunking, code validation
├── kix-embeddings/  # Embedding generation via Ollama (GPU auto-detection)
├── kix-sqlite/      # SQLite + SeaORM persistence layer (entities + migrations)
├── kix-search/      # Tantivy full-text search engine (BM25 ranking)
├── kix-store/       # SQLite two-layer storage (pages + chunks + projects)
├── kix-services/    # Shared service layer for API and MCP (business logic)
├── kix-mcp/         # MCP server with search/indexing/project tools (rmcp crate)
├── kix-api/         # Axum REST API for dashboard
├── kix-crawler/     # URL discovery, crawling strategies, code extraction
├── kix-jobs/        # Job queue, executor, and content processor
├── kix-sse/         # Server-Sent Events for real-time progress updates
├── kix-projects/    # AI-powered project management with GitHub integration
└── kix-auth/        # OAuth 2.1 authentication for MCP server
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

### kix-sqlite (SeaORM Entities + Migrations)

```
kix-sqlite/
├── lib.rs            # SqliteStore with pool and SeaORM connection
├── entities/         # SeaORM entity definitions
│   ├── entry.rs      # Entry (document metadata)
│   ├── page.rs       # Page (full content for RAG)
│   ├── project.rs    # Project (with GitHub config)
│   ├── issue.rs      # Issue (local + GitHub sync)
│   ├── job.rs        # Job (indexing jobs)
│   └── ...           # Other entities (token, sync, etc.)
├── migrations/       # SQL migration files
├── entries.rs        # Entry CRUD operations
├── pages.rs          # Page CRUD operations
├── projects.rs       # Project CRUD operations
└── issues.rs         # Issue CRUD operations
```

### kix-search (Tantivy Full-Text Search)

```
kix-search/
├── lib.rs            # SearchEngine struct
├── schema.rs         # Tantivy field definitions (entries, pages, issues)
├── indexer.rs        # Index writer operations (batch, delete)
├── searcher.rs       # Search query execution (BM25 ranking)
└── sync.rs           # DB → Index synchronization
```

### kix-store Two-Layer Storage

```
kix-store/
├── store.rs          # KixStore with init_tables(), store_page_with_chunks()
├── pages.rs          # PageStore for full page content (RAG context)
├── projects.rs       # ProjectStore for project/issue/link storage
├── search.rs         # Hybrid search (vector + full-text)
└── schema.rs         # SQLite schemas for entries, chunks, pages, projects
```

### kix-projects Module

```
kix-projects/
├── project.rs        # Project data model and configuration
├── issue.rs          # Issue data model and CRUD
├── knowledge.rs      # Project-entry linking
├── templates.rs      # GitHub Project V2 templates (Kanban, Sprint, etc.)
├── planning.rs       # AI planning data structures
├── search.rs         # Project-scoped search (issues + knowledge)
├── events.rs         # Real-time event bus (MCP → UI)
└── github/
    ├── rest_client.rs    # GitHub REST API (issues)
    ├── graphql_client.rs # GitHub GraphQL API (Projects V2)
    ├── sync.rs           # Issue sync service
    └── tokens.rs         # Secure token storage (AES-256-GCM)
```

### kix-services Module (Shared Service Layer)

```
kix-services/
├── lib.rs            # Module exports and re-exports
├── error.rs          # ServiceError (maps to StatusCode + McpError)
├── retrieval.rs      # Search, documents, context, semantic similarity
├── projects.rs       # Project CRUD operations
├── issues.rs         # Issue CRUD with GitHub sync
├── github.rs         # Token management, user/org/repo queries
├── indexing.rs       # Indexing types and job management types
└── knowledge.rs      # Entry linking, project-scoped search
```

### Key Dependencies
- **Ollama**: Local embedding model server (GPU auto-detection)
- **sea-orm**: Async ORM for SQLite database operations
- **tantivy**: Full-text search engine with BM25 ranking
- **rusqlite + sqlite-vec**: SQLite database with vector search
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
│  6. Embeddings     → Ollama with GPU auto-detection             │
│  7. Two-Layer Store→ Pages (full) + Chunks (searchable) in SQLite│
│  8. Tantivy Index  → Full-text search with BM25 ranking         │
│  9. Search         → Hybrid (vector + Tantivy) with RAG context │
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

## API/MCP Unification Architecture

KIX uses a unified service layer to ensure consistency between the REST API and MCP server.

### Architecture Pattern

```
┌─────────────────────────────────────────────────────────────────┐
│                    Unified Service Architecture                  │
├─────────────────────────────────────────────────────────────────┤
│                                                                   │
│  User ──────► REST API ──────┐                                   │
│              (kix-api)        │                                   │
│                               ├──► kix-services ◄── Store Layer  │
│  MCP Client ► MCP Server ────┘    (shared logic)                 │
│              (kix-mcp)                                            │
│                                                                   │
└─────────────────────────────────────────────────────────────────┘
```

### Design Principles

1. **Every MCP tool MUST have a corresponding REST API endpoint**
   - MCP tools live in `kix-mcp`, API endpoints live in `kix-api`
   - Both call shared functions in `kix-services`

2. **API Path Conventions**
   - MCP-equivalent endpoints: `/api/mcp/*` (thin wrappers for MCP tools)
   - Domain-specific endpoints: `/api/{domain}/*` (e.g., `/api/projects/*`, `/api/indexing/*`)
   - API endpoints can have additional features beyond MCP (pagination, filtering, etc.)

3. **Shared Service Layer (`kix-services`)**
   - All business logic lives in `kix-services`
   - Services are stateless, receive stores/event bus as parameters
   - Services emit events through optional `SharedEventBus`

4. **Event Centralization**
   - Services receive optional `SharedEventBus`
   - Events emit from services, not handlers
   - Both API and MCP trigger the same events

### Example: Creating an Issue

```rust
// kix-services/src/issues.rs (shared logic)
pub async fn create_issue(
    store: &ProjectStore,
    event_bus: Option<&SharedEventBus>,
    github_client: Option<&GitHubRestClient>,
    project: &str,
    data: CreateIssueData,
    options: IssueOptions,
) -> Result<Issue, ServiceError> {
    // Business logic here (GitHub sync, validation, etc.)
    // ...
    if let Some(bus) = event_bus {
        bus.emit(ProjectEvent::IssueCreated { ... });
    }
    Ok(issue)
}

// kix-api/src/project_routes.rs (thin handler)
async fn create_issue_handler(...) -> impl IntoResponse {
    let issue = kix_services::issues::create_issue(
        &state.store, Some(&state.event_bus), ...
    ).await?;
    Json(IssueResponse::from(issue))
}

// kix-mcp/src/server.rs (thin tool)
#[tool]
async fn create_issue(&self, params: CreateIssueParams) -> Result<...> {
    let issue = kix_services::issues::create_issue(
        &self.store, Some(&self.event_bus), ...
    ).await?;
    Ok(CallToolResult::from(issue))
}
```

### kix-services Module Structure

```
server/crates/kix-services/
├── Cargo.toml
└── src/
    ├── lib.rs          # Module exports
    ├── error.rs        # ServiceError (maps to StatusCode + McpError)
    ├── retrieval.rs    # search, get_document, get_context, find_related
    ├── projects.rs     # Project CRUD operations
    ├── issues.rs       # Issue CRUD + GitHub sync
    ├── github.rs       # Token management, user/org/repo queries
    ├── indexing.rs     # URL/file indexing, job management
    └── knowledge.rs    # Entry linking, project-scoped search
```

### Adding New Functionality Checklist

When adding new functionality:

1. [ ] Create service function in `kix-services`
2. [ ] Add REST API endpoint in `kix-api` (at logical path)
3. [ ] Add MCP tool in `kix-mcp`
4. [ ] Ensure both call the shared service function
5. [ ] Events emit from service, not handlers
6. [ ] Update tests for service, API, and MCP

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
| document_storage | 80-90% | SQLite storage |
| code_extraction | 90-95% | Code block extraction |
| finalization | 95-100% | Cleanup and completion |

### MCP Tools (exposed to AI assistants)
**Search**: `search_patterns`, `get_pattern`, `list_patterns`, `find_related`, `search_by_problem`, `search_by_technology`
**Analysis**: `explain_pattern`, `compare_patterns`, `get_category_overview`, `suggest_architecture`, `pattern_sequence`
**Indexing**: `index_document`, `index_batch`, `delete_document`, `get_index_status`
**Projects**: `create_project`, `list_projects`, `get_project`, `update_project`, `delete_project`
**Issues**: `create_issue`, `list_issues`, `get_issue`, `update_issue`, `delete_issue`
**GitHub**: `set_github_token`, `sync_github_issues`, `create_github_project`, `add_issue_to_project`
**Knowledge Links**: `link_entry`, `unlink_entry`, `list_project_entries`
**AI Planning**: `plan_project`, `suggest_tasks`, `breakdown_task`, `get_project_context`

## Client Frontend

React + TypeScript + Vite + TailwindCSS

```
client/src/
├── pages/
│   ├── Dashboard.tsx          # Main dashboard
│   ├── EntryBrowser.tsx       # Entry listing and search
│   ├── EntryDetail.tsx        # Entry details
│   ├── IndexingDashboard.tsx  # Indexing status
│   └── projects/
│       ├── ProjectList.tsx    # Project listing
│       └── ProjectDetail.tsx  # Project detail with issues tab
├── api/
│   ├── client.ts              # Main API client
│   ├── projectClient.ts       # Project management API
│   └── indexingClient.ts      # Indexing API
├── hooks/
│   ├── useSSE.ts              # SSE connection hook
│   └── useProjectEvents.ts    # Project SSE events hook
├── components/                # Shared components
└── types/
    ├── index.ts               # General types
    └── project.ts             # Project/Issue types
```

Vite proxy configuration:
- `/api` → `http://127.0.0.1:3001` (REST API)
- `/api/indexing/sse` → SSE-specific proxy settings for indexing events
- `/api/projects/events` → SSE-specific proxy settings for project events
- `/mcp` → `http://127.0.0.1:3002` (MCP HTTP server)

## Port Allocation

| Service | Port |
|---------|------|
| Web UI  | 3000 |
| REST API | 3001 |
| MCP HTTP | 3002 |

## Testing

Tests are colocated with source files using `#[cfg(test)]` modules. **170+ tests across core crates.**

### Unit Tests
```bash
cargo test --release -p kix-crawler    # 59 tests (discovery, code, strategies, etc.)
cargo test --release -p kix-parser     # 40 tests (chunker, validator, parsers)
cargo test --release -p kix-sqlite     # 33 tests (entities, CRUD operations)
cargo test --release -p kix-search     # 21 tests (Tantivy indexing, search)
cargo test --release -p kix-store      # 3 tests (pages, hybrid search, projects)
cargo test --release -p kix-projects   # 47 tests (project, issue, github, events)
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
- `kix-projects/src/events.rs` - Event bus for real-time updates
- `kix-projects/src/github/` - GitHub integration (REST + GraphQL)

## Project Management System

KIX includes an AI-powered project management system with GitHub integration:

### Features
- **Projects**: Bounded containers connecting to GitHub repositories
- **Issues**: Local issue tracking with GitHub sync (bidirectional)
- **Knowledge Links**: Connect knowledge base entries to projects
- **GitHub Projects V2**: Create Kanban, Sprint Planning, Bug Tracking boards
- **AI Planning**: Use knowledge base context to help plan projects

### Templates (GitHub Projects V2)
| Template | Fields | Views |
|----------|--------|-------|
| **Kanban** | Status (Todo/In Progress/Done) | Board |
| **Bug Tracking** | Status, Priority, Severity | Board + Table |
| **Sprint Planning** | Status, Sprint, Story Points | Board + Table |
| **Feature Roadmap** | Status, Quarter, Team | Roadmap |

### Security
- GitHub tokens encrypted at rest with AES-256-GCM
- Per-project tokens with global fallback
- Encryption key from `KIX_ENCRYPTION_KEY` environment variable

### REST API Endpoints
- `GET/POST /api/projects` - List/create projects
- `GET/PUT/DELETE /api/projects/:id` - Project CRUD
- `GET/POST /api/projects/:id/issues` - Issue management
- `GET/POST /api/projects/:id/entries` - Knowledge links
- `POST /api/projects/:id/github/sync` - Sync with GitHub
- `GET /api/projects/events` - SSE for real-time updates
