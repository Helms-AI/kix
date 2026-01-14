# Entry Embedding Cache for Related Entries

## Overview

Cache embedding vectors for indexed entries to avoid regenerating them when viewing entry details. Currently, viewing an entry triggers an Ollama embedding request to find "Related Entries" via semantic similarity search. This is slow (~100-500ms) and unnecessary since entries already have embeddings stored during indexing.

## Problem Statement

When viewing an entry detail page, the current flow:

1. `GET /api/entries/:id` - Get entry metadata
2. `GET /api/entries-related/:id` - **Generates new embedding via Ollama**, then searches
3. `GET /api/entries-chunks/:id` - Get entry chunks

The problem with step 2:
- Calls Ollama to generate an embedding for the entry's content
- Takes 30-500ms depending on content length and model load state
- **The embedding already exists** - it was generated during indexing
- Regenerating it is wasteful and slows down the UI

## Design Goals

1. **Zero redundant embedding calls**: Reuse existing embeddings from indexing
2. **Fast related entries**: Sub-10ms response time for related entries
3. **Backward compatible**: Graceful fallback if cached embedding not found
4. **No schema changes**: Use existing data where possible

## Current Architecture

### Indexing Flow (embeddings ARE stored)
```
Content → Chunking → Embed each chunk → Store in SQLite + sqlite-vec
                                         ↓
                              chunk_vectors table (chunk_id, embedding)
                              chunk_metadata table (chunk_id, entry_id, chunk_index, ...)
```

### Entry View Flow (embeddings regenerated unnecessarily)
```
Entry ID → Get entry content → Generate NEW embedding (Ollama) → Vector search
                               ↑
                        UNNECESSARY - already exists!
```

## Proposed Solution

### Option A: Use Chunk Embeddings (Recommended)

Reuse the embeddings already stored for the entry's chunks:

```rust
// Current: Generate new embedding
let embedding = embedder.embed_query(&format!("{} {}", title, description))?;

// Proposed: Get first chunk's embedding from vector store
let embedding = store.get_entry_embedding(&entry_id)?;
```

**Implementation:**
1. Add `get_entry_embedding(entry_id)` to `VectorStore`
2. Returns the embedding of chunk_index=0 (or averaged across all chunks)
3. Falls back to generating if not found (for entries without chunks)

### Option B: Store Entry-Level Embedding

Add a dedicated entry embedding during indexing:

```sql
-- New table
CREATE TABLE entry_embeddings (
    entry_id TEXT PRIMARY KEY,
    embedding BLOB NOT NULL,  -- serialized f32 vector
    created_at TEXT NOT NULL
);
```

**Pros:** Clean separation, explicit cache
**Cons:** Schema change, migration needed, storage overhead

### Option C: In-Memory LRU Cache

Cache recently generated embeddings in memory:

```rust
struct EmbeddingCache {
    cache: LruCache<String, Vec<f32>>,
    max_size: usize,
}
```

**Pros:** Simple, no DB changes
**Cons:** Lost on restart, memory overhead, still generates on first view

## Recommended Approach: Option A

Use chunk embeddings with intelligent aggregation:

### 1. New VectorStore Method

```rust
impl VectorStore {
    /// Get the representative embedding for an entry.
    /// Returns the first chunk's embedding, or None if no chunks exist.
    pub fn get_entry_embedding(&self, entry_id: &str) -> Result<Option<Vec<f32>>> {
        let conn = self.conn.lock().unwrap();

        // Get first chunk's embedding (chunk_index = 0 is usually the title/intro)
        // Join chunk_vectors with chunk_metadata to get embedding for first chunk
        let result = conn.query_row(
            "SELECT v.embedding FROM chunk_vectors v
             JOIN chunk_metadata m ON v.chunk_id = m.chunk_id
             WHERE m.entry_id = ?
             ORDER BY m.chunk_index ASC
             LIMIT 1",
            [entry_id],
            |row| {
                let blob: Vec<u8> = row.get(0)?;
                Ok(deserialize_embedding(&blob))
            }
        ).optional()?;

        Ok(result)
    }
}
```

### 2. Updated Related Entries Handler

```rust
async fn get_related_entries(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<EntryListResponse>, StatusCode> {
    // Try to get cached embedding first
    let embedding = {
        let store = state.store.read().await;
        store.get_entry_embedding(&id)
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    };

    let embedding = match embedding {
        Some(emb) => emb,
        None => {
            // Fallback: Generate embedding (for entries without chunks)
            let store = state.store.read().await;
            let entry = store.get_pattern_by_id(&id).await?
                .ok_or(StatusCode::NOT_FOUND)?;
            drop(store);

            let query = format!("{} {}", entry.title, entry.description);
            let mut embedder = state.embedder.write().await;
            embedder.embed_query(&query)?
        }
    };

    // Search for related entries
    let store = state.store.read().await;
    let results = store.vector_search(&embedding, 6, &filters)?;
    // ... rest of handler
}
```

### 3. Performance Impact

| Metric | Before | After |
|--------|--------|-------|
| Related entries latency | 100-500ms | <10ms |
| Ollama calls per view | 1 | 0 (usually) |
| Database queries | 1 vector search | 1 embedding lookup + 1 vector search |

## Implementation Plan

### Phase 1: Core Implementation
- [ ] Add `get_entry_embedding()` to `VectorStore` (kix-vectors/src/lib.rs)
- [ ] Add async `get_entry_embedding()` wrapper to `KixStore` (kix-store/src/store.rs)
- [ ] Update `find_related()` in kix-services to use cached embedding first
- [ ] Update `get_related_entries()` in kix-api/routes.rs to use shared service

### Phase 2: Fallback Handling
- [ ] Handle entries with no chunks (newly created, failed indexing)
- [ ] Add logging for cache hits/misses
- [ ] Add metric for embedding generation fallbacks

### Phase 3: Testing
- [ ] Unit test for `get_entry_embedding()` in VectorStore
- [ ] Integration test for related entries with cached vs generated
- [ ] Performance benchmark comparing before/after

## API Changes

None - this is an internal optimization. The `/api/entries-related/:id` endpoint behavior remains identical.

## Database Changes

None required for Option A. Existing `chunk_vectors` and `chunk_metadata` tables already have all needed data.

## Configuration

Optional: Add config to control fallback behavior:

```toml
[related_entries]
# Use cached chunk embeddings instead of generating new ones
use_cached_embeddings = true

# Generate embedding if cache miss (vs returning empty)
fallback_to_generation = true
```

## Migration Path

1. Deploy new code with cached embedding lookup
2. Monitor for increased cache misses (logged)
3. If excessive misses, investigate entries without chunks

## Future Enhancements

1. **Average embedding**: Average all chunk embeddings for better representation
2. **Weighted average**: Weight by chunk position (title chunks = higher weight)
3. **Pre-compute on index**: Store explicit entry-level embedding during indexing
4. **Batch prefetch**: When listing entries, prefetch embeddings for likely views

## References

- Related entries handler: `server/crates/kix-api/src/routes.rs:484-565`
- Related entries service: `server/crates/kix-services/src/retrieval.rs:367-423`
- Vector store: `server/crates/kix-vectors/src/lib.rs`
- Unified store: `server/crates/kix-store/src/store.rs`
- Ollama embedding backend: `server/crates/kix-embeddings/src/backend/ollama.rs`
