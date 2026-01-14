# Rust ORM Migration: SeaORM + Tantivy Hybrid Architecture

## Executive Summary

This specification documents the migration of KIX's data access layer from raw sqlx to a hybrid architecture using:
- **SeaORM** for all CRUD operations (entries, pages, projects, issues, etc.)
- **Tantivy** for full-text search (replacing SQLite FTS5)
- **rusqlite + sqlite-vec** for vector search (unchanged)

**Key Finding:** No Rust ORM has native FTS5 support due to:
- FTS5 virtual tables have no PRIMARY KEY (ORMs require PKs)
- FTS5 uses special `MATCH` syntax not supported by ORM query builders
- FTS5 functions (`bm25()`, `snippet()`) are SQLite-specific

---

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────────────┐
│                         KIX Architecture                             │
├─────────────────────────────────────────────────────────────────────┤
│                                                                      │
│  ┌──────────────┐    ┌──────────────┐    ┌──────────────┐          │
│  │   SeaORM     │    │   Tantivy    │    │  sqlite-vec  │          │
│  │  (SQLite)    │    │   (Index)    │    │  (Vectors)   │          │
│  │              │    │              │    │              │          │
│  │  - entries   │    │  - entries   │    │  - chunks    │          │
│  │  - pages     │    │  - pages     │    │  - embeddings│          │
│  │  - projects  │    │  - issues    │    │              │          │
│  │  - issues    │    │              │    │              │          │
│  │  - jobs      │    │              │    │              │          │
│  └──────┬───────┘    └──────┬───────┘    └──────┬───────┘          │
│         │                   │                   │                   │
│         └───────────────────┼───────────────────┘                   │
│                             │                                       │
│                      ┌──────▼───────┐                               │
│                      │ Hybrid Search │                              │
│                      │  RRF Fusion   │                              │
│                      └──────────────┘                               │
└─────────────────────────────────────────────────────────────────────┘
```

---

## ORM Comparison: Why SeaORM?

| Criteria | Diesel | SeaORM | Winner |
|----------|--------|--------|--------|
| **SQLite Support** | Good (8/10) | Good (8/10) | Tie |
| **Async Support** | Wrapper required (6/10) | Native (10/10) | **SeaORM** |
| **Migration System** | Excellent (9/10) | Excellent (9/10) | Tie |
| **Query Building** | Good with `into_boxed()` (8/10) | Good with `Condition` (8/10) | Tie |
| **FTS5 Support** | Poor - raw SQL (3/10) | Poor - raw SQL (3/10) | Tie (Both Fail) |
| **Type Safety** | Excellent (compile-time) | Very Good (compile-time) | **Diesel** (slight) |
| **Learning Curve** | Steep | Moderate-Steep | **SeaORM** (slight) |
| **Community** | Excellent (13.7K stars) | Excellent (9.2K stars) | **Diesel** (slight) |
| **Boilerplate** | Moderate | Lower | **SeaORM** |

**Primary Reasons for SeaORM:**
1. **Native async** - No wrapper overhead, same paradigm as current architecture
2. **sqlx foundation** - Built on the same driver KIX currently uses
3. **Cleaner syntax** - Less boilerplate than Diesel

---

## Tantivy vs FTS5 Comparison

| Feature | SQLite FTS5 | Tantivy |
|---------|-------------|---------|
| Ranking | BM25 | BM25 + custom scorers |
| Fuzzy Search | Limited | Full support |
| Faceted Search | No | Yes |
| Highlighting | snippet() | Built-in highlighter |
| Boolean Queries | Yes | Yes + more operators |
| Phrase Search | Yes | Yes + slop tolerance |
| Index Updates | Triggers | Manual sync required |
| Performance | Good | Excellent |

---

## Dependencies

### New Dependencies

```toml
# server/crates/kix-sqlite/Cargo.toml
[dependencies]
sea-orm = { version = "1.1", features = ["sqlx-sqlite", "runtime-tokio-native-tls", "macros"] }
sea-orm-migration = { version = "1.1" }

# server/crates/kix-search/Cargo.toml (NEW)
[dependencies]
tantivy = "0.22"
```

---

## Directory Structure

```
server/crates/
├── kix-sqlite/              # SeaORM integration
│   ├── src/
│   │   ├── lib.rs
│   │   ├── entities/        # SeaORM entities
│   │   │   ├── mod.rs
│   │   │   ├── entry.rs
│   │   │   ├── page.rs
│   │   │   ├── project.rs
│   │   │   ├── issue.rs
│   │   │   └── ...
│   │   └── migrations/      # SeaORM migrations
│
├── kix-search/              # NEW: Tantivy search crate
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs           # SearchEngine struct
│       ├── schema.rs        # Tantivy field definitions
│       ├── indexer.rs       # Index writer operations
│       ├── searcher.rs      # Search query execution
│       └── sync.rs          # DB → Index synchronization
│
├── kix-vectors/             # UNCHANGED
│   └── src/lib.rs           # rusqlite + sqlite-vec
│
└── kix-store/               # Updated to use all three
    └── src/store.rs         # KixStore orchestration
```

---

## Implementation Details

### Tantivy Schema Definition

```rust
// kix-search/src/schema.rs
use tantivy::schema::*;

pub fn build_entry_schema() -> Schema {
    let mut schema_builder = Schema::builder();

    // Stored + Indexed fields
    schema_builder.add_text_field("id", STRING | STORED);
    schema_builder.add_text_field("title", TEXT | STORED);
    schema_builder.add_text_field("description", TEXT | STORED);
    schema_builder.add_text_field("content", TEXT);  // Indexed but not stored
    schema_builder.add_text_field("entry_type", STRING | STORED);
    schema_builder.add_text_field("source_domain", STRING | STORED);
    schema_builder.add_text_field("tags", TEXT | STORED);

    // Faceted fields for filtering
    schema_builder.add_facet_field("category", FacetOptions::default());

    // Date field for sorting
    schema_builder.add_date_field("created_at", INDEXED | STORED);

    schema_builder.build()
}
```

### Search Engine Implementation

```rust
// kix-search/src/lib.rs
use tantivy::{Index, IndexReader, IndexWriter, Document, TantivyError};
use tantivy::query::QueryParser;
use tantivy::collector::TopDocs;
use std::path::Path;

pub struct SearchEngine {
    index: Index,
    reader: IndexReader,
    schema: Schema,
}

impl SearchEngine {
    pub fn new(index_path: &Path) -> Result<Self, TantivyError> {
        let schema = build_entry_schema();
        let index = Index::create_in_dir(index_path, schema.clone())?;
        let reader = index.reader()?;

        Ok(Self { index, reader, schema })
    }

    pub fn search(
        &self,
        query_text: &str,
        limit: usize,
    ) -> Result<Vec<SearchResult>, TantivyError> {
        let searcher = self.reader.searcher();
        let query_parser = QueryParser::for_index(
            &self.index,
            vec![
                self.schema.get_field("title").unwrap(),
                self.schema.get_field("description").unwrap(),
                self.schema.get_field("content").unwrap(),
            ],
        );

        let query = query_parser.parse_query(query_text)?;
        let top_docs = searcher.search(&query, &TopDocs::with_limit(limit))?;

        let mut results = Vec::new();
        for (score, doc_address) in top_docs {
            let doc = searcher.doc(doc_address)?;
            results.push(SearchResult {
                id: get_field_value(&doc, "id"),
                title: get_field_value(&doc, "title"),
                snippet: generate_snippet(&doc, query_text),
                score,
            });
        }

        Ok(results)
    }

    pub fn index_entry(&self, entry: &EntryRecord) -> Result<(), TantivyError> {
        let mut index_writer = self.index.writer(50_000_000)?;

        let mut doc = Document::new();
        doc.add_text(self.schema.get_field("id").unwrap(), &entry.id);
        doc.add_text(self.schema.get_field("title").unwrap(), &entry.title);
        // ... add other fields

        index_writer.add_document(doc)?;
        index_writer.commit()?;

        Ok(())
    }

    pub fn delete_entry(&self, entry_id: &str) -> Result<(), TantivyError> {
        let mut index_writer = self.index.writer(50_000_000)?;
        let id_field = self.schema.get_field("id").unwrap();
        let term = Term::from_field_text(id_field, entry_id);
        index_writer.delete_term(term);
        index_writer.commit()?;
        Ok(())
    }
}
```

### Synchronization Strategy

```rust
// kix-search/src/sync.rs
use sea_orm::DatabaseConnection;
use crate::SearchEngine;

pub struct IndexSynchronizer {
    db: DatabaseConnection,
    search: SearchEngine,
}

impl IndexSynchronizer {
    /// Full reindex from database
    pub async fn full_reindex(&self) -> Result<(), Error> {
        // 1. Clear existing index
        self.search.clear_all()?;

        // 2. Stream all entries from SeaORM
        let entries = Entry::find()
            .stream(&self.db)
            .await?;

        // 3. Batch index
        let mut batch = Vec::new();
        while let Some(entry) = entries.try_next().await? {
            batch.push(entry);
            if batch.len() >= 1000 {
                self.search.index_batch(&batch)?;
                batch.clear();
            }
        }

        // Final batch
        if !batch.is_empty() {
            self.search.index_batch(&batch)?;
        }

        Ok(())
    }

    /// Incremental sync on entry changes
    pub async fn sync_entry(&self, entry_id: &str) -> Result<(), Error> {
        if let Some(entry) = Entry::find_by_id(entry_id).one(&self.db).await? {
            self.search.index_entry(&entry)?;
        } else {
            self.search.delete_entry(entry_id)?;
        }
        Ok(())
    }
}
```

### Hybrid Search Integration

```rust
// kix-store/src/store.rs
impl KixStore {
    pub async fn hybrid_search(
        &self,
        query: &str,
        embedding: &[f32],
        limit: usize,
    ) -> Result<Vec<SearchResult>, StoreError> {
        // 1. Tantivy full-text search
        let text_results = self.search_engine.search(query, limit * 2)?;

        // 2. Vector search (existing sqlite-vec)
        let vector_results = self.vector_search(embedding, limit * 2, &SearchFilters::default()).await?;

        // 3. Reciprocal Rank Fusion (existing logic)
        let fused = reciprocal_rank_fusion(text_results, vector_results, limit);

        Ok(fused)
    }
}
```

---

## SeaORM Entity Definitions

### Entry Entity

```rust
// entities/entry.rs
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "entries")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,
    pub title: String,
    pub description: Option<String>,
    #[sea_orm(column_type = "Text")]
    pub content: Option<String>,
    pub tags: Option<String>,
    pub collection_ids: Option<String>,
    pub entry_type: String,
    pub source_type: String,
    pub source_path: String,
    pub source_domain: Option<String>,
    pub slug: String,
    pub source_hash: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::page::Entity")]
    Pages,
    #[sea_orm(has_many = "super::project_entry::Entity")]
    ProjectEntries,
}

impl Related<super::page::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Pages.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

impl Model {
    pub fn tags_vec(&self) -> Vec<String> {
        self.tags.as_ref()
            .and_then(|t| serde_json::from_str(t).ok())
            .unwrap_or_default()
    }
}
```

### Project Entity

```rust
// entities/project.rs
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "projects")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,
    #[sea_orm(unique)]
    pub name: String,
    #[sea_orm(unique)]
    pub slug: String,
    pub description: Option<String>,
    pub color: Option<String>,
    pub github_config: String,
    pub archived: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::issue::Entity")]
    Issues,
    #[sea_orm(has_many = "super::project_entry::Entity")]
    ProjectEntries,
}
```

### Issue Entity

```rust
// entities/issue.rs
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "issues")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,
    pub project_id: String,
    pub number: i64,
    pub title: String,
    #[sea_orm(column_type = "Text")]
    pub body: Option<String>,
    pub state: String,
    pub labels: Option<String>,
    pub assignees: Option<String>,
    pub priority: Option<i64>,
    pub github_number: Option<i64>,
    pub github_node_id: Option<String>,
    pub github_url: Option<String>,
    pub github_project_item_id: Option<String>,
    pub source: String,
    pub created_at: String,
    pub updated_at: String,
    pub closed_at: Option<String>,
    pub synced_at: Option<String>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::project::Entity",
        from = "Column::ProjectId",
        to = "super::project::Column::Id",
        on_delete = "Cascade"
    )]
    Project,
}
```

---

## Migration from FTS5

| Component | Before (FTS5) | After (Tantivy) |
|-----------|---------------|-----------------|
| Tables | entries_fts, pages_fts, issues_fts | Remove virtual tables |
| Triggers | 12 auto-sync triggers | Remove (use IndexSynchronizer) |
| Schema | In migrations/001_initial.sql | In kix-search/src/schema.rs |
| Queries | `WHERE entries_fts MATCH ?` | `search_engine.search(query)` |
| Ranking | `bm25(entries_fts)` | Tantivy BM25 scorer |
| Snippets | `snippet(entries_fts, ...)` | Tantivy Highlighter |

---

## CLAUDE.md Updates

### Key Dependencies (replace FTS5 references)

```markdown
### Key Dependencies
- **sea-orm**: Async ORM for SQLite database operations
- **tantivy**: Full-text search engine (replaced FTS5)
- **fastembed**: Local embedding model (no external API calls)
- **rusqlite + sqlite-vec**: Vector database for semantic search
- **rmcp**: Model Context Protocol server implementation
- **axum**: Web framework for REST API
- **tokio**: Async runtime
```

### Data Flow (update search pipeline)

```markdown
### Data Flow
1. Discovery      → URL discovery (llms.txt → sitemap → robots)
2. Crawling       → Playwright browser with strategy selection
3. Processing     → Readability extraction → Markdown
4. Code Extract   → 30+ patterns with multi-stage validation
5. Smart Chunking → Code blocks → paragraphs → sentences
6. Embeddings     → fastembed with optional page context
7. Two-Layer Store→ Pages (full) + Chunks (searchable) in SQLite
8. **Tantivy Index** → Full-text search indexing (BM25)
9. Search         → Hybrid (Tantivy + vector) with RRF fusion
```

### Architecture (add kix-search crate)

```markdown
### Rust Workspace (12 crates)
server/crates/
├── kix-cli/         # Main CLI binary
├── kix-parser/      # Document parsing, smart chunking
├── kix-embeddings/  # Embedding generation (fastembed)
├── kix-store/       # Unified storage layer (SeaORM + Tantivy + vectors)
├── kix-sqlite/      # SeaORM entities and migrations
├── kix-search/      # NEW: Tantivy full-text search engine
├── kix-mcp/         # MCP server with search/indexing tools
├── kix-api/         # Axum REST API for dashboard
├── kix-crawler/     # URL discovery, crawling strategies
├── kix-jobs/        # Job queue, executor, content processor
├── kix-sse/         # Server-Sent Events for progress updates
├── kix-projects/    # Project management with GitHub integration
└── kix-auth/        # OAuth 2.1 authentication
```

### Remove FTS5 references from:
- "FTS5 full-text search with BM25 ranking" → "Tantivy full-text search with BM25 ranking"
- "3 FTS5 virtual tables" → Remove
- "FTS5 MATCH queries" → "Tantivy queries"
- Search sections referencing `entries_fts`, `pages_fts`, `issues_fts`

---

## Implementation Phases

### Phase 1: Add Tantivy Crate (Week 1)
- Create `kix-search` crate
- Define Tantivy schema for entries, pages, issues
- Implement basic search and indexing

### Phase 2: SeaORM Migration (Week 2)
- Add SeaORM entities for all 11 tables
- Replace sqlx CRUD operations
- Keep existing tests passing

### Phase 3: Index Synchronization (Week 3)
- Implement full reindex from SeaORM
- Add incremental sync on CRUD operations
- Build migration tool for existing data

### Phase 4: Remove FTS5 (Week 3-4)
- Remove FTS5 virtual tables from migrations
- Remove FTS5 triggers
- Update hybrid search to use Tantivy
- Integration testing

### Phase 5: Documentation (Week 4)
- Update CLAUDE.md
- Update README
- Performance benchmarks

---

## Implementation Checklist

> **Instructions:** Mark tasks with `[x]` when completed. Update this checklist as work progresses.

### Phase 1: Tantivy Search Crate
- [x] Create `kix-search` crate directory structure
- [x] Add `Cargo.toml` with Tantivy dependency
- [x] Implement `schema.rs` - Define Tantivy schema for entries
- [x] Implement `schema.rs` - Define Tantivy schema for pages
- [x] Implement `schema.rs` - Define Tantivy schema for issues
- [x] Implement `lib.rs` - SearchEngine struct and initialization
- [x] Implement `indexer.rs` - Single document indexing
- [x] Implement `indexer.rs` - Batch document indexing
- [x] Implement `indexer.rs` - Document deletion
- [x] Implement `searcher.rs` - Basic text search
- [x] Implement `searcher.rs` - BM25 ranking
- [x] Implement `searcher.rs` - Snippet/highlight generation
- [x] Add unit tests for kix-search
- [x] Verify Phase 1 builds: `cargo build -p kix-search`

### Phase 2: SeaORM Migration
- [ ] Add SeaORM dependencies to `kix-sqlite/Cargo.toml`
- [ ] Create `entities/mod.rs`
- [ ] Create `entities/entry.rs` entity
- [ ] Create `entities/page.rs` entity
- [ ] Create `entities/project.rs` entity
- [ ] Create `entities/issue.rs` entity
- [ ] Create `entities/project_entry.rs` entity
- [ ] Create `entities/github_token.rs` entity
- [ ] Create `entities/job.rs` entity
- [ ] Create `entities/job_item.rs` entity
- [ ] Create `entities/sync_state.rs` entity
- [ ] Create `entities/sync_history.rs` entity
- [ ] Create `entities/sync_conflict.rs` entity
- [ ] Create SeaORM migration files
- [ ] Replace `entries.rs` CRUD with SeaORM
- [ ] Replace `pages.rs` CRUD with SeaORM
- [ ] Replace `projects.rs` CRUD with SeaORM
- [ ] Replace `issues.rs` CRUD with SeaORM
- [ ] Replace `links.rs` CRUD with SeaORM
- [ ] Replace `tokens.rs` CRUD with SeaORM
- [ ] Replace `jobs.rs` CRUD with SeaORM
- [ ] Replace `sync_state.rs` CRUD with SeaORM
- [ ] Update `kix-store` to use SeaORM types
- [ ] Update `kix-services` to use SeaORM types
- [ ] Update `kix-api` routes to use SeaORM
- [ ] Update `kix-mcp` handlers to use SeaORM
- [ ] Run existing tests: `cargo test -p kix-sqlite`

### Phase 3: Index Synchronization
- [ ] Implement `sync.rs` - IndexSynchronizer struct
- [ ] Implement full reindex from SeaORM
- [ ] Implement incremental sync on entry create
- [ ] Implement incremental sync on entry update
- [ ] Implement incremental sync on entry delete
- [ ] Implement incremental sync for pages
- [ ] Implement incremental sync for issues
- [ ] Add sync hooks to SeaORM CRUD operations
- [ ] Create CLI command for full reindex
- [ ] Add migration tool for existing data
- [ ] Test sync consistency

### Phase 4: FTS5 Removal & Cleanup
- [ ] Remove `entries_fts` virtual table from migrations
- [ ] Remove `pages_fts` virtual table from migrations
- [ ] Remove `issues_fts` virtual table from migrations
- [ ] Remove 12 FTS5 sync triggers from migrations
- [ ] Remove `search.rs` FTS5 queries
- [ ] Update `kix-store` hybrid_search to use Tantivy
- [ ] Update `kix-services` search functions
- [ ] Update `kix-api` search endpoints
- [ ] Update `kix-mcp` search tools
- [ ] Remove sqlx dependency (if no longer needed)
- [ ] Run all tests: `cargo test`
- [ ] Integration testing with real data

### Phase 5: Documentation & Final Steps
- [ ] Update `CLAUDE.md` - Key Dependencies section
- [ ] Update `CLAUDE.md` - Data Flow section
- [ ] Update `CLAUDE.md` - Architecture section
- [ ] Update `CLAUDE.md` - Remove FTS5 references
- [ ] Performance benchmarks: FTS5 vs Tantivy
- [ ] Update README with new architecture
- [ ] Manual end-to-end testing
- [ ] Tag release version

### Verification Checklist
- [ ] All unit tests pass: `cargo test`
- [ ] Server starts without errors: `./run.sh`
- [ ] Create entry via API works
- [ ] Search entries returns results
- [ ] Hybrid search (text + vector) works
- [ ] Project/issue CRUD works
- [ ] GitHub sync works
- [ ] No regression in existing features

---

## Estimated Timeline

| Phase | Duration | Deliverable |
|-------|----------|-------------|
| Phase 1: Tantivy | 5 days | kix-search crate working |
| Phase 2: SeaORM | 5 days | All CRUD migrated |
| Phase 3: Sync | 3 days | Index synchronization |
| Phase 4: Cleanup | 2 days | FTS5 removed, tests passing |
| Phase 5: Docs | 1 day | Documentation updated |
| **Total** | **~3 weeks** | Full hybrid architecture |

---

## Files to Create/Modify

| File | Action | Description |
|------|--------|-------------|
| `kix-search/Cargo.toml` | Create | New crate with Tantivy |
| `kix-search/src/lib.rs` | Create | SearchEngine struct |
| `kix-search/src/schema.rs` | Create | Tantivy field definitions |
| `kix-search/src/indexer.rs` | Create | Index write operations |
| `kix-search/src/searcher.rs` | Create | Search execution |
| `kix-search/src/sync.rs` | Create | DB→Index synchronization |
| `kix-sqlite/src/entities/*.rs` | Create | 11 SeaORM entity files |
| `kix-store/src/store.rs` | Modify | Integrate Tantivy |
| `kix-sqlite/migrations/001_initial.sql` | Modify | Remove FTS5 tables/triggers |
| `CLAUDE.md` | Modify | Update technology stack documentation |

---

## Verification Plan

1. **Unit Tests:**
   ```bash
   cargo test -p kix-search
   cargo test -p kix-sqlite
   ```

2. **Integration Tests:**
   - Index 1000 entries
   - Search with various queries
   - Verify hybrid search quality

3. **Manual Testing:**
   - Start server: `./run.sh`
   - Create entries via API
   - Search and verify results
   - Check ranking quality vs FTS5

4. **Performance Comparison:**
   ```bash
   # Benchmark FTS5 vs Tantivy
   cargo bench -p kix-search
   ```
