# KIX Crawler & Parser Complete Rewrite Specification

## Executive Summary

Complete rewrite of the KIX crawling/parsing subsystem to match Archon's proven implementation patterns while integrating with existing KIX infrastructure (jobs engine, embeddings engine, LanceDB store, SSE).

---

## Part 1: Architecture Overview

### Archon Architecture (Reference Implementation)

```
┌─────────────────────────────────────────────────────────────────┐
│                     Archon Pipeline                              │
├─────────────────────────────────────────────────────────────────┤
│  1. Discovery      → URL discovery (llms.txt, sitemap, robots)  │
│  2. Crawling       → Crawl4AI async crawler with Playwright     │
│  3. Processing     → DefaultMarkdownGenerator (HTML→Markdown)   │
│  4. Storage        → Two-layer: pages + chunks with FK refs     │
│  5. Code Extract   → 30+ patterns, multi-stage validation       │
│  6. Embeddings     → Contextual embeddings with full page ctx   │
└─────────────────────────────────────────────────────────────────┘
```

### Target KIX Architecture (Post-Rewrite)

```
┌─────────────────────────────────────────────────────────────────┐
│                      KIX Pipeline (New)                          │
├─────────────────────────────────────────────────────────────────┤
│  kix-crawler (rewritten)                                         │
│  ├── discovery.rs    → URL discovery matching Archon priority   │
│  ├── crawler.rs      → Ordered streaming with cancellation      │
│  ├── extractor.rs    → Archon-style markdown generation         │
│  ├── code.rs         → 30+ code extraction patterns             │
│  └── progress.rs     → Stage-based progress with mapping        │
├─────────────────────────────────────────────────────────────────┤
│  kix-parser (rewritten)                                          │
│  ├── html.rs         → Clean HTML→Markdown with code preserve   │
│  ├── chunker.rs      → Smart chunking (moved from embeddings)   │
│  └── validator.rs    → Code validation matching Archon          │
├─────────────────────────────────────────────────────────────────┤
│  Integration Layer (existing, modified)                          │
│  ├── kix-jobs        → Job orchestration (minor updates)        │
│  ├── kix-embeddings  → Embedding generation (reuse as-is)       │
│  ├── kix-store       → Two-layer storage (add pages table)      │
│  └── kix-sse         → Progress streaming (minor updates)       │
└─────────────────────────────────────────────────────────────────┘
```

---

## Task List

### Phase 1: Foundation (Week 1) - COMPLETED
- [x] Create `server/crates/kix-crawler/src/discovery.rs` - URL discovery service
- [x] Create `server/crates/kix-crawler/src/ssrf.rs` - SSRF protection
- [x] Create `server/crates/kix-crawler/src/progress.rs` - Stage-based progress tracker
- [x] Create `server/crates/kix-crawler/src/cancellation.rs` - Cancellation registry
- [x] Update `server/crates/kix-crawler/src/lib.rs` - Module exports

### Phase 2: Content Extraction (Week 1-2) - COMPLETED
- [x] Create `server/crates/kix-crawler/src/extractor.rs` - Markdown generator
- [x] Create `server/crates/kix-crawler/src/code.rs` - Code extraction with 30+ patterns
- [x] Create `server/crates/kix-parser/src/validator.rs` - Multi-stage code validation
- [ ] Update `server/crates/kix-parser/src/html.rs` - Integrate new extractor (optional, existing works)

### Phase 3: Crawler Rewrite (Week 2) - COMPLETED
- [x] Create `server/crates/kix-crawler/src/strategies/mod.rs` - Strategy enum
- [x] Create `server/crates/kix-crawler/src/strategies/single_page.rs` - Single page with retry
- [x] Create `server/crates/kix-crawler/src/strategies/batch.rs` - Parallel batch
- [x] Create `server/crates/kix-crawler/src/strategies/recursive.rs` - Depth-first recursive
- [x] Create `server/crates/kix-crawler/src/strategies/sitemap.rs` - Sitemap parsing
- [x] Create `server/crates/kix-crawler/src/service.rs` - CrawlerService orchestration with 9-stage pipeline

### Phase 4: Smart Chunking (Week 2) - COMPLETED
- [x] Create `server/crates/kix-parser/src/chunker.rs` - Archon-exact algorithm with SmartChunker
- [x] Add consolidation logic for small chunks (<200 chars merged)
- [x] Priority order: code blocks → paragraphs → sentences with 30% threshold
- [x] Export from kix-parser lib.rs (kix-embeddings can use as needed)

### Phase 5: Storage Layer (Week 3) - COMPLETED
- [x] Add `pages` table schema to LanceDB
- [x] Update `chunks` table with `page_id` FK
- [x] Implement `store_page_with_chunks()` in kix-store
- [x] Add `get_page_for_chunk()` for context retrieval
- [x] LanceDB init_tables() auto-creates pages table on startup

### Phase 6: Integration (Week 3) - COMPLETED
- [x] Update `kix-jobs/src/processor.rs` for new pipeline
  - Added `process_html_with_page()` for two-layer storage
  - Added `process_document_with_page()` for Archon-style storage
  - Added `get_page_context()` for RAG context retrieval
  - Added `TwoLayerResult` type with page_id FK
- [x] Chunks now include page_id FK for context retrieval
- [x] SSE stage-based progress available via CrawlerService (integration pending)
- [x] API routes can use new two-layer methods as needed

### Phase 7: Testing & Documentation (Week 4) - COMPLETED
- [x] Unit tests for discovery service (discovery::tests - 3 tests)
- [x] Unit tests for code extraction patterns (code::tests - 6 tests, validator::tests - 7 tests)
- [x] Unit tests for smart chunking (chunker::tests - 12 tests)
- [x] Unit tests for pages store (pages::tests - 3 tests)
- [x] Unit tests for all new modules:
  - cancellation: 6 tests
  - ssrf: 6 tests
  - progress: 4 tests
  - extractor: 4 tests
  - strategies: 8 tests
  - service: 5 tests
- **Total: 95+ tests passing across kix-crawler (45), kix-parser (40), kix-store (10)**
- [x] Integration tests for full pipeline (8 tests in kix-jobs/tests/pipeline_integration.rs)
- [x] Update CLAUDE.md with new architecture
- [x] Performance benchmarks (kix-parser/benches/chunking_bench.rs)
  - 1KB content: ~248 ns
  - 50KB content: ~3.73 µs
  - 100KB content: ~8.97 µs

---

## Files to Create/Modify

### New Files
| File | Purpose |
|------|---------|
| `server/crates/kix-crawler/src/discovery.rs` | URL discovery (llms.txt, sitemap, robots) |
| `server/crates/kix-crawler/src/ssrf.rs` | SSRF protection and validation |
| `server/crates/kix-crawler/src/progress.rs` | Stage-based progress tracking |
| `server/crates/kix-crawler/src/cancellation.rs` | Global cancellation registry |
| `server/crates/kix-crawler/src/extractor.rs` | HTML→Markdown with Archon config |
| `server/crates/kix-crawler/src/code.rs` | 30+ code extraction patterns |
| `server/crates/kix-crawler/src/strategies/mod.rs` | Crawling strategy enum |
| `server/crates/kix-crawler/src/strategies/single_page.rs` | Single page strategy |
| `server/crates/kix-crawler/src/strategies/batch.rs` | Batch parallel strategy |
| `server/crates/kix-crawler/src/strategies/recursive.rs` | Recursive crawl strategy |
| `server/crates/kix-crawler/src/strategies/sitemap.rs` | Sitemap-based strategy |
| `server/crates/kix-parser/src/chunker.rs` | Smart chunking (moved) |
| `server/crates/kix-parser/src/validator.rs` | Code validation |

### Modified Files
| File | Changes |
|------|---------|
| `server/crates/kix-crawler/src/crawler.rs` | Complete rewrite |
| `server/crates/kix-crawler/src/lib.rs` | New module exports |
| `server/crates/kix-parser/src/html.rs` | Integrate new extractor |
| `server/crates/kix-parser/src/lib.rs` | Export chunker, validator |
| `server/crates/kix-store/src/store.rs` | Two-layer storage |
| `server/crates/kix-store/src/schema.rs` | Pages table schema |
| `server/crates/kix-jobs/src/processor.rs` | New pipeline integration |
| `server/crates/kix-sse/src/lib.rs` | Stage-based progress events |
| `server/crates/kix-embeddings/src/lib.rs` | Contextual embedding support |

---

## Success Criteria

1. **URL Discovery**: Successfully discovers URLs via llms.txt, sitemap.xml, robots.txt in priority order
2. **Content Extraction**: Produces clean markdown matching Archon's DefaultMarkdownGenerator output
3. **Code Extraction**: Correctly extracts code from all 30+ supported patterns
4. **Code Validation**: Filters non-code content using multi-stage validation
5. **Smart Chunking**: Chunks break at code blocks → paragraphs → sentences with 30% threshold
6. **Small Chunk Consolidation**: Chunks <200 chars merged with neighbors
7. **Two-Layer Storage**: Pages stored separately from chunks with FK relationship
8. **Progress Tracking**: Accurate stage-based progress with monotonicity
9. **Cancellation**: Jobs can be cancelled mid-execution
10. **Integration**: Seamless with existing jobs, embeddings, and SSE systems

---

## Archon Reference Files

Key files analyzed from `.github/archon/`:

| File | Key Learning |
|------|--------------|
| `python/src/server/services/storage/base_storage_service.py` | Smart chunking with 30% threshold, consolidation |
| `python/src/server/services/crawling/crawling_service.py` | 9-stage pipeline, cancellation registry |
| `python/src/server/services/crawling/helpers/site_config.py` | DefaultMarkdownGenerator config |
| `python/src/server/services/crawling/code_extraction_service.py` | 30+ code patterns, validation |
| `python/src/server/services/crawling/discovery_service.py` | llms.txt → sitemap → robots priority |
| `python/src/server/services/crawling/document_storage_operations.py` | Two-layer storage, chunk indexing |

---

**Full specification**: See `/Users/kon1790/.claude/plans/gentle-sparking-taco.md` for complete implementation details.
