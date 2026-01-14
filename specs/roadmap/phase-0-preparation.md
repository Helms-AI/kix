# Phase 0: Preparation & Planning

**Duration**: 1-2 days
**Dependencies**: None
**Status**: Not Started

---

## Objective

Prepare the codebase and development environment for the migration to spider-based architecture with Ollama embeddings.

---

## Tasks

### 0.1 Environment Setup

#### Ollama Configuration
```bash
# Verify Ollama is running
curl http://localhost:11434/api/tags

# Pull nomic-embed-text model
ollama pull nomic-embed-text

# Test embedding generation
curl http://localhost:11434/api/embeddings -d '{
  "model": "nomic-embed-text",
  "prompt": "Test embedding"
}'
```

**Acceptance Criteria**:
- [ ] Ollama running on localhost:11434
- [ ] nomic-embed-text model downloaded
- [ ] Embedding generation returns 768-dimensional vector

#### Development Dependencies
```bash
# Add spider crates to workspace
cd server
cargo add spider --features "sync,smart,cache" -p kix-jobs
cargo add spider_transformations -p kix-jobs

# Verify compilation
cargo check --workspace
```

---

### 0.2 Codebase Audit

#### Current kix-crawler Analysis

**Files to Review**:
| File | Lines | Purpose | Action |
|------|-------|---------|--------|
| `kix-crawler/src/code.rs` | ~500 | Code extraction | Extract to standalone module |
| `kix-crawler/src/extractor.rs` | ~300 | Content extraction | Replace with spider_transformations |
| `kix-crawler/src/browser.rs` | ~300 | Playwright | Replace with spider's chrome |
| `kix-crawler/src/strategies/*.rs` | ~1200 | Crawl strategies | Replace with spider |
| `kix-crawler/src/discovery.rs` | ~400 | URL discovery | Replace with spider |
| `kix-crawler/src/service.rs` | ~600 | Orchestration | Rewrite for spider |
| `kix-crawler/src/progress.rs` | ~400 | Progress tracking | Keep and adapt |
| `kix-crawler/src/ssrf.rs` | ~200 | Security | Keep |
| `kix-crawler/src/cancellation.rs` | ~150 | Job control | Keep |

**Command to analyze**:
```bash
# Count lines in kix-crawler
find server/crates/kix-crawler/src -name "*.rs" -exec wc -l {} + | sort -n

# List all public functions in code.rs
grep -n "pub fn\|pub async fn" server/crates/kix-crawler/src/code.rs
```

#### Document Current Test Coverage
```bash
# Run existing tests
cargo test --release -p kix-crawler --no-fail-fast 2>&1 | tee crawler-tests.log

# Count tests
grep -c "test result" crawler-tests.log
```

**Acceptance Criteria**:
- [ ] All current tests documented
- [ ] Test baseline established (X tests passing)
- [ ] Critical code paths identified

---

### 0.3 Create Feature Branches

```bash
# Create main feature branch
git checkout -b feature/spider-migration

# Create phase-specific branches (optional)
git checkout -b feature/phase-1-spider-integration
git checkout -b feature/phase-2-code-extractor
# etc.
```

---

### 0.4 Define Test Sites

Create a test matrix of documentation sites with known code extraction patterns:

| Site | Framework | Expected Patterns | Notes |
|------|-----------|-------------------|-------|
| https://docs.rs/tokio | Docusaurus | DocusaurusCodeBlock | Rust code |
| https://docs.python.org | Sphinx | SphinxCodeBlock | Python code |
| https://react.dev | Custom | PrismJs | JavaScript/JSX |
| https://go.dev/doc | Hugo | HugoHighlight | Go code |
| https://docs.github.com | Custom | GitHubCode | Mixed |

**Create test fixture file**:
```bash
# Create test sites config
cat > server/crates/kix-crawler/tests/fixtures/test_sites.json << 'EOF'
{
  "sites": [
    {
      "url": "https://docs.rs/tokio/latest/tokio/",
      "expected_pattern": "DocusaurusCodeBlock",
      "expected_languages": ["rust"],
      "min_code_blocks": 5
    },
    {
      "url": "https://docs.python.org/3/tutorial/",
      "expected_pattern": "SphinxCodeBlock",
      "expected_languages": ["python"],
      "min_code_blocks": 10
    }
  ]
}
EOF
```

---

### 0.5 Backup Current State

```bash
# Tag current working state
git tag -a v0.x-pre-spider-migration -m "Before spider migration"

# Export current test results
cargo test --release --workspace 2>&1 | tee pre-migration-tests.log

# Document current embedding model
echo "Current: FastEmbed with model X" > migration-notes.md
```

---

### 0.6 Create Migration Checklist

Create a checklist file to track progress:

```markdown
# Migration Checklist

## Pre-Migration
- [ ] Ollama running with nomic-embed-text
- [ ] All current tests passing
- [ ] Feature branches created
- [ ] Test sites identified
- [ ] Current state tagged

## Phase 1: Spider Integration
- [ ] spider_adapter.rs created
- [ ] Basic crawling works
- [ ] Smart mode tested
- [ ] HTTP caching verified

## Phase 2: CodeExtractor
- [ ] Module extracted from code.rs
- [ ] All 30+ patterns working
- [ ] Language detection working
- [ ] Validation working

... (continue for all phases)
```

---

## Deliverables

| Deliverable | Location | Description |
|-------------|----------|-------------|
| Environment verified | N/A | Ollama + nomic-embed-text working |
| Codebase audit | `migration-notes.md` | Current state documented |
| Test baseline | `pre-migration-tests.log` | Current test results |
| Feature branch | `feature/spider-migration` | Ready for development |
| Test sites | `test_sites.json` | Sites for validation |
| Migration checklist | `MIGRATION.md` | Progress tracking |

---

## Exit Criteria

- [ ] Ollama running with nomic-embed-text
- [ ] All current kix-crawler tests pass
- [ ] Codebase audit complete
- [ ] Feature branch created
- [ ] Test sites documented
- [ ] Team aligned on approach

---

## Next Phase

Upon completion, proceed to [Phase 1: Spider Integration](./phase-1-spider-integration.md).

**Parallel Option**: Phase 3 (Embedding Migration) can start alongside Phase 1 since they're independent.
