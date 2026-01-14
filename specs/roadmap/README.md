# kix-indexing Implementation Roadmap

## Overview

This roadmap breaks down the [kix-indexing PRD](../new-indexing-crate-implementation.md) into actionable implementation phases. Each phase is designed to be completed independently while building toward the complete system.

**Target**: Migrate from current kix-crawler to spider-based architecture with framework-aware code extraction, Ollama embeddings, and enhanced UI visibility.

---

## Current State Assessment

### What Exists (kix crates)
| Crate | Status | Notes |
|-------|--------|-------|
| kix-sqlite | ✅ Keep | SeaORM entities, migrations |
| kix-search | ✅ Keep | Tantivy full-text search |
| kix-store | ✅ Keep | Storage layer |
| kix-embeddings | ⚠️ Migrate | FastEmbed → Ollama |
| kix-crawler | ❌ Replace | → spider + CodeExtractor |
| kix-parser | ✅ Keep | Chunking, validation |
| kix-api | ⚠️ Update | Add code extraction endpoints |
| kix-mcp | ⚠️ Update | Add code extraction tools |
| kix-jobs | ⚠️ Update | Integrate spider |
| client | ⚠️ Update | UI enhancements |

### What's New (from PRD)
- spider integration for crawling
- spider_transformations for HTML→Markdown
- CodeExtractor module (30+ patterns)
- Tree-sitter for source file parsing
- Ollama-only embeddings (nomic-embed-text)
- Enhanced SSE events for code extraction
- UI code extraction visibility

---

## Implementation Phases

| Phase | Name | Duration | Dependencies | Status |
|-------|------|----------|--------------|--------|
| [0](./phase-0-preparation.md) | Preparation & Planning | 1-2 days | None | Not Started |
| [1](./phase-1-spider-integration.md) | Spider Integration | 3-4 days | Phase 0 | Not Started |
| [2](./phase-2-code-extractor.md) | CodeExtractor Module | 2-3 days | Phase 1 | Not Started |
| [3](./phase-3-embedding-migration.md) | Embedding Migration | 1-2 days | Phase 0 | Not Started |
| [4](./phase-4-tree-sitter.md) | Tree-sitter Integration | 3-4 days | Phase 2 | Not Started |
| [5](./phase-5-api-sse-updates.md) | API & SSE Updates | 2-3 days | Phase 2 | Not Started |
| [6](./phase-6-ui-updates.md) | UI Updates | 3-4 days | Phase 5 | Not Started |
| [7](./phase-7-testing-docs.md) | Testing & Documentation | 2-3 days | All | Not Started |

**Total Estimated Duration**: 17-25 days

---

## Dependency Graph

```
Phase 0 (Preparation)
    │
    ├──────────────────────────┐
    ▼                          ▼
Phase 1 (Spider)          Phase 3 (Embeddings)
    │                          │
    ▼                          │
Phase 2 (CodeExtractor)        │
    │                          │
    ├──────────────────────────┤
    │                          │
    ▼                          │
Phase 4 (Tree-sitter)          │
    │                          │
    ▼                          │
Phase 5 (API/SSE)◄─────────────┘
    │
    ▼
Phase 6 (UI)
    │
    ▼
Phase 7 (Testing/Docs)
```

**Parallel Tracks**:
- Phase 1 + Phase 3 can run in parallel
- Phase 4 can start after Phase 2

---

## Quick Reference

### Key Files to Create
```
server/crates/kix-indexing/
├── src/
│   ├── crawler/
│   │   ├── spider_adapter.rs    # Phase 1
│   │   └── config.rs            # Phase 1
│   ├── extraction/
│   │   ├── code_extractor.rs    # Phase 2
│   │   ├── patterns.rs          # Phase 2
│   │   ├── language.rs          # Phase 2
│   │   └── validation.rs        # Phase 2
│   ├── chunking/
│   │   └── treesitter.rs        # Phase 4
│   └── embedding/
│       └── ollama.rs            # Phase 3
```

### Key Files to Modify
```
server/crates/kix-jobs/src/processor.rs     # Phase 1, 2
server/crates/kix-api/src/indexing_routes.rs # Phase 5
server/crates/kix-sse/src/events.rs          # Phase 5
client/src/pages/IndexingDashboard.tsx       # Phase 6
client/src/components/indexing/*             # Phase 6
```

### Dependencies to Add
```toml
# Cargo.toml additions
spider = { version = "2", features = ["sync", "smart", "cache"] }
spider_transformations = "2"
tree-sitter = "0.22"
tree-sitter-rust = "0.21"
tree-sitter-python = "0.21"
# ... more tree-sitter languages
ollama-rs = { version = "0.2", features = ["stream"] }
```

---

## Success Criteria

### Phase Completion Checklist
- [ ] All tests pass
- [ ] No regression in existing functionality
- [ ] Documentation updated
- [ ] Code reviewed

### Final Acceptance Criteria
- [ ] Spider crawls documentation sites successfully
- [ ] Code blocks extracted with 30+ patterns
- [ ] Languages detected and normalized correctly
- [ ] Ollama embeddings working with nomic-embed-text
- [ ] UI shows code extraction metrics
- [ ] All existing MCP tools still functional
- [ ] Performance equal or better than current system

---

## Risk Mitigation

| Risk | Mitigation |
|------|------------|
| spider API changes | Pin specific version, add integration tests |
| Code extraction regression | Keep code.rs tests, run against known sites |
| Embedding quality change | Benchmark against current FastEmbed output |
| UI complexity | Phase incrementally, get user feedback |

---

## Getting Started

1. **Read Phase 0** first to understand preparation steps
2. **Set up development environment** with Ollama running
3. **Create feature branch** for each phase
4. **Follow the spec** for each phase in order
5. **Run tests** before merging each phase

```bash
# Start with Phase 0
cat specs/roadmap/phase-0-preparation.md
```

---

*Last Updated: January 2025*
