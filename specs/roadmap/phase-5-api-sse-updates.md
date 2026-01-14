# Phase 5: API & SSE Updates

**Duration**: 2-3 days
**Dependencies**: Phase 2
**Status**: Not Started

---

## Objective

Update REST API endpoints and SSE events to expose code extraction metrics and enhance indexing visibility.

---

## Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                    API & SSE Architecture                        │
├─────────────────────────────────────────────────────────────────┤
│                                                                  │
│  Client (Dashboard)                                              │
│         │                                                        │
│         ├──────────────────┬──────────────────┐                 │
│         ▼                  ▼                  ▼                  │
│  ┌─────────────┐   ┌─────────────┐   ┌─────────────┐           │
│  │ REST API    │   │ SSE Stream  │   │ MCP Tools   │           │
│  │ /api/*      │   │ /events     │   │ JSON-RPC    │           │
│  └─────────────┘   └─────────────┘   └─────────────┘           │
│         │                  │                  │                  │
│         └──────────────────┴──────────────────┘                 │
│                            │                                     │
│                            ▼                                     │
│  ┌─────────────────────────────────────────────────────────┐    │
│  │  kix-services (Shared Business Logic)                    │    │
│  │  ├─ indexing.rs   → Indexing operations                 │    │
│  │  ├─ retrieval.rs  → Search and document access          │    │
│  │  └─ events.rs     → SSE event emission                  │    │
│  └─────────────────────────────────────────────────────────┘    │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘
```

---

## Tasks

### 5.1 Define New SSE Event Types

**File**: `server/crates/kix-sse/src/events.rs` (MODIFY)

Add new event types for code extraction:

```rust
use serde::{Deserialize, Serialize};

/// SSE event types for indexing
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum IndexingEvent {
    /// Job started
    JobStarted {
        job_id: String,
        url: String,
        timestamp: String,
    },

    /// Discovery phase update
    Discovery {
        job_id: String,
        urls_found: usize,
        method: String, // "sitemap", "robots", "crawl"
    },

    /// Page crawled
    PageCrawled {
        job_id: String,
        url: String,
        status: u16,
        content_length: usize,
    },

    /// Code extraction results (NEW)
    CodeExtraction {
        job_id: String,
        url: String,
        blocks_found: usize,
        patterns_matched: Vec<String>,
        languages: Vec<LanguageCount>,
        validation_stats: ValidationStats,
    },

    /// Processing progress
    Progress {
        job_id: String,
        stage: String,
        progress: f32,
        message: Option<String>,
    },

    /// Chunk created
    ChunkCreated {
        job_id: String,
        chunk_index: usize,
        content_preview: String,
        has_code: bool,
    },

    /// Embedding generated
    EmbeddingGenerated {
        job_id: String,
        chunks_embedded: usize,
        total_chunks: usize,
    },

    /// Page stored
    PageStored {
        job_id: String,
        url: String,
        page_id: i64,
        chunk_count: usize,
    },

    /// Job completed
    JobCompleted {
        job_id: String,
        summary: JobSummary,
    },

    /// Job failed
    JobFailed {
        job_id: String,
        error: String,
        stage: String,
    },
}

/// Language count for code extraction
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LanguageCount {
    pub language: String,
    pub count: usize,
}

/// Validation statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationStats {
    pub total_extracted: usize,
    pub passed_validation: usize,
    pub rejected_too_short: usize,
    pub rejected_prose: usize,
    pub rejected_duplicates: usize,
}

/// Job completion summary
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobSummary {
    pub pages_crawled: usize,
    pub pages_stored: usize,
    pub chunks_created: usize,
    pub code_blocks_extracted: usize,
    pub duration_ms: u64,
    pub languages_detected: Vec<LanguageCount>,
    pub top_patterns: Vec<PatternCount>,
}

/// Pattern match count
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatternCount {
    pub pattern: String,
    pub count: usize,
}
```

---

### 5.2 Update SSE Broadcaster

**File**: `server/crates/kix-sse/src/broadcaster.rs` (MODIFY)

```rust
use crate::events::IndexingEvent;
use tokio::sync::broadcast;

/// SSE event broadcaster
pub struct EventBroadcaster {
    sender: broadcast::Sender<IndexingEvent>,
}

impl EventBroadcaster {
    pub fn new(capacity: usize) -> Self {
        let (sender, _) = broadcast::channel(capacity);
        Self { sender }
    }

    /// Broadcast an indexing event
    pub fn broadcast(&self, event: IndexingEvent) {
        // Ignore send errors (no subscribers)
        let _ = self.sender.send(event);
    }

    /// Subscribe to events
    pub fn subscribe(&self) -> broadcast::Receiver<IndexingEvent> {
        self.sender.subscribe()
    }

    /// Convenience methods for common events
    pub fn emit_code_extraction(
        &self,
        job_id: &str,
        url: &str,
        result: &CodeExtractionResult,
    ) {
        let languages: Vec<LanguageCount> = result
            .language_counts()
            .into_iter()
            .map(|(lang, count)| LanguageCount {
                language: lang.to_string(),
                count,
            })
            .collect();

        let patterns_matched: Vec<String> = result
            .patterns_used()
            .into_iter()
            .map(|p| p.to_string())
            .collect();

        self.broadcast(IndexingEvent::CodeExtraction {
            job_id: job_id.to_string(),
            url: url.to_string(),
            blocks_found: result.blocks.len(),
            patterns_matched,
            languages,
            validation_stats: ValidationStats {
                total_extracted: result.stats.total_extracted,
                passed_validation: result.stats.passed_validation,
                rejected_too_short: result.stats.rejected_too_short,
                rejected_prose: result.stats.rejected_prose,
                rejected_duplicates: result.stats.rejected_duplicates,
            },
        });
    }

    pub fn emit_progress(
        &self,
        job_id: &str,
        stage: &str,
        progress: f32,
        message: Option<String>,
    ) {
        self.broadcast(IndexingEvent::Progress {
            job_id: job_id.to_string(),
            stage: stage.to_string(),
            progress,
            message,
        });
    }
}
```

---

### 5.3 Create New API Endpoints

**File**: `server/crates/kix-api/src/indexing_routes.rs` (MODIFY)

Add new endpoints for code extraction information:

```rust
use axum::{
    extract::{Path, Query, State},
    response::Json,
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};

/// Routes for indexing API
pub fn indexing_routes() -> Router<AppState> {
    Router::new()
        // Existing routes
        .route("/jobs", get(list_jobs).post(create_job))
        .route("/jobs/:id", get(get_job).delete(cancel_job))
        .route("/jobs/:id/status", get(get_job_status))

        // New code extraction routes
        .route("/jobs/:id/code-stats", get(get_code_stats))
        .route("/jobs/:id/code-blocks", get(list_code_blocks))
        .route("/patterns", get(list_patterns))
        .route("/languages", get(list_languages))

        // SSE endpoint
        .route("/events", get(sse_handler))
}

/// Code extraction statistics for a job
#[derive(Debug, Serialize)]
pub struct CodeExtractionStats {
    pub job_id: String,
    pub total_pages: usize,
    pub pages_with_code: usize,
    pub total_code_blocks: usize,
    pub languages: Vec<LanguageStats>,
    pub patterns: Vec<PatternStats>,
    pub validation: ValidationSummary,
}

#[derive(Debug, Serialize)]
pub struct LanguageStats {
    pub language: String,
    pub block_count: usize,
    pub total_lines: usize,
    pub percentage: f32,
}

#[derive(Debug, Serialize)]
pub struct PatternStats {
    pub pattern: String,
    pub match_count: usize,
    pub percentage: f32,
}

#[derive(Debug, Serialize)]
pub struct ValidationSummary {
    pub total_extracted: usize,
    pub passed: usize,
    pub pass_rate: f32,
    pub rejection_reasons: Vec<RejectionReason>,
}

#[derive(Debug, Serialize)]
pub struct RejectionReason {
    pub reason: String,
    pub count: usize,
}

/// Get code extraction stats for a job
async fn get_code_stats(
    State(state): State<AppState>,
    Path(job_id): Path<String>,
) -> Result<Json<CodeExtractionStats>, ApiError> {
    let stats = kix_services::indexing::get_code_extraction_stats(
        &state.store,
        &job_id,
    ).await?;

    Ok(Json(stats))
}

/// Query parameters for code blocks
#[derive(Debug, Deserialize)]
pub struct CodeBlocksQuery {
    /// Filter by language
    pub language: Option<String>,
    /// Filter by pattern
    pub pattern: Option<String>,
    /// Pagination offset
    pub offset: Option<usize>,
    /// Pagination limit
    pub limit: Option<usize>,
}

/// Code block response
#[derive(Debug, Serialize)]
pub struct CodeBlockResponse {
    pub id: String,
    pub content: String,
    pub language: String,
    pub pattern: String,
    pub line_count: usize,
    pub source_url: String,
    pub validated: bool,
}

/// List code blocks for a job
async fn list_code_blocks(
    State(state): State<AppState>,
    Path(job_id): Path<String>,
    Query(query): Query<CodeBlocksQuery>,
) -> Result<Json<Vec<CodeBlockResponse>>, ApiError> {
    let blocks = kix_services::indexing::list_code_blocks(
        &state.store,
        &job_id,
        query.language.as_deref(),
        query.pattern.as_deref(),
        query.offset.unwrap_or(0),
        query.limit.unwrap_or(50),
    ).await?;

    Ok(Json(blocks))
}

/// Pattern information
#[derive(Debug, Serialize)]
pub struct PatternInfo {
    pub name: String,
    pub css_selector: String,
    pub description: String,
    pub example_sites: Vec<String>,
}

/// List all supported code extraction patterns
async fn list_patterns() -> Json<Vec<PatternInfo>> {
    let patterns = kix_services::indexing::list_code_patterns();
    Json(patterns)
}

/// Language information
#[derive(Debug, Serialize)]
pub struct LanguageInfo {
    pub name: String,
    pub aliases: Vec<String>,
    pub extensions: Vec<String>,
    pub tree_sitter_support: bool,
}

/// List all supported languages
async fn list_languages() -> Json<Vec<LanguageInfo>> {
    let languages = kix_services::indexing::list_supported_languages();
    Json(languages)
}
```

---

### 5.4 Create SSE Handler

**File**: `server/crates/kix-api/src/sse.rs` (NEW or MODIFY)

```rust
use axum::{
    extract::{Query, State},
    response::sse::{Event, KeepAlive, Sse},
};
use futures::stream::{Stream, StreamExt};
use serde::Deserialize;
use std::convert::Infallible;
use tokio_stream::wrappers::BroadcastStream;

use crate::state::AppState;
use kix_sse::events::IndexingEvent;

/// SSE query parameters
#[derive(Debug, Deserialize)]
pub struct SseQuery {
    /// Filter by job ID (optional)
    pub job_id: Option<String>,
}

/// SSE event handler
pub async fn sse_handler(
    State(state): State<AppState>,
    Query(query): Query<SseQuery>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let receiver = state.event_broadcaster.subscribe();

    let stream = BroadcastStream::new(receiver)
        .filter_map(move |result| {
            let job_filter = query.job_id.clone();

            async move {
                match result {
                    Ok(event) => {
                        // Filter by job_id if specified
                        if let Some(ref filter) = job_filter {
                            if !event_matches_job(&event, filter) {
                                return None;
                            }
                        }

                        // Serialize event
                        let data = serde_json::to_string(&event).ok()?;
                        let event_type = event_type_name(&event);

                        Some(Ok(Event::default()
                            .event(event_type)
                            .data(data)))
                    }
                    Err(_) => None,
                }
            }
        });

    Sse::new(stream).keep_alive(KeepAlive::default())
}

/// Check if event matches job filter
fn event_matches_job(event: &IndexingEvent, job_id: &str) -> bool {
    match event {
        IndexingEvent::JobStarted { job_id: id, .. } => id == job_id,
        IndexingEvent::Discovery { job_id: id, .. } => id == job_id,
        IndexingEvent::PageCrawled { job_id: id, .. } => id == job_id,
        IndexingEvent::CodeExtraction { job_id: id, .. } => id == job_id,
        IndexingEvent::Progress { job_id: id, .. } => id == job_id,
        IndexingEvent::ChunkCreated { job_id: id, .. } => id == job_id,
        IndexingEvent::EmbeddingGenerated { job_id: id, .. } => id == job_id,
        IndexingEvent::PageStored { job_id: id, .. } => id == job_id,
        IndexingEvent::JobCompleted { job_id: id, .. } => id == job_id,
        IndexingEvent::JobFailed { job_id: id, .. } => id == job_id,
    }
}

/// Get event type name for SSE
fn event_type_name(event: &IndexingEvent) -> &'static str {
    match event {
        IndexingEvent::JobStarted { .. } => "job_started",
        IndexingEvent::Discovery { .. } => "discovery",
        IndexingEvent::PageCrawled { .. } => "page_crawled",
        IndexingEvent::CodeExtraction { .. } => "code_extraction",
        IndexingEvent::Progress { .. } => "progress",
        IndexingEvent::ChunkCreated { .. } => "chunk_created",
        IndexingEvent::EmbeddingGenerated { .. } => "embedding_generated",
        IndexingEvent::PageStored { .. } => "page_stored",
        IndexingEvent::JobCompleted { .. } => "job_completed",
        IndexingEvent::JobFailed { .. } => "job_failed",
    }
}
```

---

### 5.5 Update MCP Tools

**File**: `server/crates/kix-mcp/src/server.rs` (MODIFY)

Add code extraction information to MCP tools:

```rust
use rmcp::{tool, McpServer};

impl McpServer for KixMcpServer {
    /// Get code extraction stats for an indexing job
    #[tool(
        name = "get_code_stats",
        description = "Get code extraction statistics for an indexing job"
    )]
    async fn get_code_stats(
        &self,
        #[arg(description = "The indexing job ID")] job_id: String,
    ) -> Result<CallToolResult, McpError> {
        let stats = kix_services::indexing::get_code_extraction_stats(
            &self.store,
            &job_id,
        ).await
            .map_err(|e| McpError::internal(e.to_string()))?;

        Ok(CallToolResult::from_json(&stats)?)
    }

    /// List supported code extraction patterns
    #[tool(
        name = "list_code_patterns",
        description = "List all supported code extraction patterns with their CSS selectors"
    )]
    async fn list_code_patterns(&self) -> Result<CallToolResult, McpError> {
        let patterns = kix_services::indexing::list_code_patterns();
        Ok(CallToolResult::from_json(&patterns)?)
    }

    /// List supported programming languages
    #[tool(
        name = "list_languages",
        description = "List all supported programming languages for code extraction"
    )]
    async fn list_languages(&self) -> Result<CallToolResult, McpError> {
        let languages = kix_services::indexing::list_supported_languages();
        Ok(CallToolResult::from_json(&languages)?)
    }
}
```

---

### 5.6 Update kix-services

**File**: `server/crates/kix-services/src/indexing.rs` (MODIFY)

Add service functions for code extraction:

```rust
use crate::error::ServiceError;

/// Get code extraction statistics for a job
pub async fn get_code_extraction_stats(
    store: &KixStore,
    job_id: &str,
) -> Result<CodeExtractionStats, ServiceError> {
    let job = store.get_job(job_id).await?
        .ok_or(ServiceError::NotFound("Job not found".to_string()))?;

    let pages = store.get_pages_for_job(job_id).await?;

    let mut total_code_blocks = 0;
    let mut pages_with_code = 0;
    let mut language_counts: HashMap<String, (usize, usize)> = HashMap::new();
    let mut pattern_counts: HashMap<String, usize> = HashMap::new();
    let mut validation_totals = ValidationSummary::default();

    for page in &pages {
        if let Some(code_stats) = &page.code_extraction_stats {
            pages_with_code += 1;
            total_code_blocks += code_stats.block_count;

            for (lang, count) in &code_stats.languages {
                let entry = language_counts.entry(lang.clone()).or_insert((0, 0));
                entry.0 += count;
                entry.1 += code_stats.lines_per_language.get(lang).unwrap_or(&0);
            }

            for (pattern, count) in &code_stats.patterns {
                *pattern_counts.entry(pattern.clone()).or_insert(0) += count;
            }

            validation_totals.total_extracted += code_stats.validation.total_extracted;
            validation_totals.passed += code_stats.validation.passed;
        }
    }

    let total_blocks = total_code_blocks.max(1) as f32;

    let languages: Vec<LanguageStats> = language_counts
        .into_iter()
        .map(|(language, (count, lines))| LanguageStats {
            language,
            block_count: count,
            total_lines: lines,
            percentage: (count as f32 / total_blocks) * 100.0,
        })
        .collect();

    let patterns: Vec<PatternStats> = pattern_counts
        .into_iter()
        .map(|(pattern, count)| PatternStats {
            pattern,
            match_count: count,
            percentage: (count as f32 / total_blocks) * 100.0,
        })
        .collect();

    validation_totals.pass_rate = if validation_totals.total_extracted > 0 {
        (validation_totals.passed as f32 / validation_totals.total_extracted as f32) * 100.0
    } else {
        100.0
    };

    Ok(CodeExtractionStats {
        job_id: job_id.to_string(),
        total_pages: pages.len(),
        pages_with_code,
        total_code_blocks,
        languages,
        patterns,
        validation: validation_totals,
    })
}

/// List code blocks for a job with filtering
pub async fn list_code_blocks(
    store: &KixStore,
    job_id: &str,
    language: Option<&str>,
    pattern: Option<&str>,
    offset: usize,
    limit: usize,
) -> Result<Vec<CodeBlockResponse>, ServiceError> {
    let blocks = store.query_code_blocks(job_id, language, pattern, offset, limit).await?;

    Ok(blocks.into_iter().map(|b| CodeBlockResponse {
        id: b.id,
        content: b.content,
        language: b.language,
        pattern: b.pattern,
        line_count: b.line_count,
        source_url: b.source_url,
        validated: b.validated,
    }).collect())
}

/// List all supported code extraction patterns
pub fn list_code_patterns() -> Vec<PatternInfo> {
    use kix_crawler::extraction::CodePattern;

    CodePattern::all()
        .iter()
        .map(|p| PatternInfo {
            name: p.to_string(),
            css_selector: p.css_selector().to_string(),
            description: p.description().to_string(),
            example_sites: p.example_sites().iter().map(|s| s.to_string()).collect(),
        })
        .collect()
}

/// List all supported programming languages
pub fn list_supported_languages() -> Vec<LanguageInfo> {
    use kix_crawler::extraction::Language;
    use kix_parser::treesitter::SourceLanguage;

    Language::all()
        .iter()
        .map(|lang| {
            let tree_sitter = SourceLanguage::from_extension(
                lang.file_extension()
            ).is_some();

            LanguageInfo {
                name: lang.display_name().to_string(),
                aliases: lang.aliases().iter().map(|s| s.to_string()).collect(),
                extensions: lang.file_extensions().iter().map(|s| s.to_string()).collect(),
                tree_sitter_support: tree_sitter,
            }
        })
        .collect()
}
```

---

### 5.7 Write Tests

**File**: `server/crates/kix-api/src/indexing_routes_tests.rs` (NEW)

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::StatusCode;
    use axum_test::TestServer;

    async fn create_test_server() -> TestServer {
        let state = AppState::test_state().await;
        let app = indexing_routes().with_state(state);
        TestServer::new(app).unwrap()
    }

    #[tokio::test]
    async fn test_list_patterns() {
        let server = create_test_server().await;
        let response = server.get("/patterns").await;

        assert_eq!(response.status_code(), StatusCode::OK);

        let patterns: Vec<PatternInfo> = response.json();
        assert!(!patterns.is_empty());

        // Should include common patterns
        let pattern_names: Vec<_> = patterns.iter().map(|p| &p.name).collect();
        assert!(pattern_names.contains(&&"DocusaurusCodeBlock".to_string()));
        assert!(pattern_names.contains(&&"MkDocsCodeBlock".to_string()));
    }

    #[tokio::test]
    async fn test_list_languages() {
        let server = create_test_server().await;
        let response = server.get("/languages").await;

        assert_eq!(response.status_code(), StatusCode::OK);

        let languages: Vec<LanguageInfo> = response.json();
        assert!(!languages.is_empty());

        // Should include common languages
        let lang_names: Vec<_> = languages.iter().map(|l| &l.name).collect();
        assert!(lang_names.contains(&&"Rust".to_string()));
        assert!(lang_names.contains(&&"Python".to_string()));
        assert!(lang_names.contains(&&"JavaScript".to_string()));
    }

    #[tokio::test]
    async fn test_sse_connection() {
        let server = create_test_server().await;

        // SSE endpoint should accept connections
        let response = server
            .get("/events")
            .add_header("Accept", "text/event-stream")
            .await;

        assert_eq!(response.status_code(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_code_stats_not_found() {
        let server = create_test_server().await;
        let response = server.get("/jobs/nonexistent/code-stats").await;

        assert_eq!(response.status_code(), StatusCode::NOT_FOUND);
    }
}
```

---

## Deliverables

| Deliverable | File | Description |
|-------------|------|-------------|
| SSE events | `kix-sse/src/events.rs` | New event types |
| Event broadcaster | `kix-sse/src/broadcaster.rs` | Broadcasting logic |
| API routes | `kix-api/src/indexing_routes.rs` | New endpoints |
| SSE handler | `kix-api/src/sse.rs` | SSE stream handler |
| MCP tools | `kix-mcp/src/server.rs` | Code extraction tools |
| Service layer | `kix-services/src/indexing.rs` | Shared business logic |
| Tests | Various | API and integration tests |

---

## API Endpoints Summary

| Method | Path | Description |
|--------|------|-------------|
| GET | `/api/indexing/jobs` | List indexing jobs |
| POST | `/api/indexing/jobs` | Create new job |
| GET | `/api/indexing/jobs/:id` | Get job details |
| DELETE | `/api/indexing/jobs/:id` | Cancel job |
| GET | `/api/indexing/jobs/:id/status` | Get job status |
| GET | `/api/indexing/jobs/:id/code-stats` | **NEW** Code extraction stats |
| GET | `/api/indexing/jobs/:id/code-blocks` | **NEW** List code blocks |
| GET | `/api/indexing/patterns` | **NEW** List patterns |
| GET | `/api/indexing/languages` | **NEW** List languages |
| GET | `/api/indexing/events` | SSE event stream |

---

## SSE Event Types

| Event | Description |
|-------|-------------|
| `job_started` | Job initialization |
| `discovery` | URL discovery update |
| `page_crawled` | Page fetch complete |
| `code_extraction` | **NEW** Code extracted from page |
| `progress` | Stage progress update |
| `chunk_created` | Chunk created |
| `embedding_generated` | Embeddings complete |
| `page_stored` | Page saved to database |
| `job_completed` | Job finished successfully |
| `job_failed` | Job failed with error |

---

## Exit Criteria

- [ ] `cargo check -p kix-api` passes
- [ ] `cargo check -p kix-sse` passes
- [ ] `cargo check -p kix-mcp` passes
- [ ] `/patterns` endpoint returns 30+ patterns
- [ ] `/languages` endpoint returns all languages
- [ ] `/jobs/:id/code-stats` returns stats
- [ ] SSE events stream correctly
- [ ] MCP tools work via JSON-RPC
- [ ] All existing tests still pass

---

## Testing Commands

```bash
# Run API tests
cargo test -p kix-api --release

# Run SSE tests
cargo test -p kix-sse --release

# Manual endpoint testing
curl http://localhost:3001/api/indexing/patterns | jq
curl http://localhost:3001/api/indexing/languages | jq

# Test SSE stream
curl -N http://localhost:3001/api/indexing/events

# Test MCP tools
echo '{"jsonrpc":"2.0","method":"list_code_patterns","params":{},"id":1}' | \
  curl -X POST http://localhost:3002/mcp -d @-
```

---

## Next Phase

Upon completion, proceed to [Phase 6: UI Updates](./phase-6-ui-updates.md).
