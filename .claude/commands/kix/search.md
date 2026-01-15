---
name: kix-search
description: |
  Quick semantic search across the KIX knowledge base.

  Usage: /kix-search "<query>" [options]
  Examples:
    /kix-search "authentication patterns"
    /kix-search "error handling" --type code --limit 5
    /kix-search "react hooks" --domain docs.react.dev
argument-hint: <query> [--type TYPE] [--limit N] [--mode MODE] [--domain DOMAIN]
---

# KIX Search Command

Perform semantic and keyword search across the indexed knowledge base.

## Parse Arguments

Extract from `$ARGUMENTS`:
- **query**: The search query (required, first quoted or unquoted string)
- **--type**: Filter by entry type (document, pdf, article, code)
- **--chunk-type**: Filter by chunk type (content, code, header, summary)
- **--limit**: Maximum results (default: 10, max: 100)
- **--mode**: Search mode - hybrid (default), vector, or text
- **--domain**: Filter by source domain
- **--tag**: Filter by tag

## Execute Search

1. **Call the MCP search tool:**
   ```
   mcp__kix__search with:
   - query: <extracted query>
   - limit: <limit or 10>
   - mode: <mode or "hybrid">
   - filters: {
       entry_type: <type if provided>,
       chunk_type: <chunk-type if provided>,
       source_domain: <domain if provided>,
       tag: <tag if provided>
     }
   ```

2. **Process results:**
   - For each result, note the `chunk_id`, `page_id`, `entry_id`
   - Track `score` for relevance ranking
   - Preserve `source_url` for citations

## Format Output

Present results in a clean, scannable format:

```markdown
## Search Results for "<query>"

Found {total_count} results {has_more ? "(showing top N)" : ""}

### 1. {entry_title} (score: {score})
{text snippet - first 200 chars}...
- Source: {source_url or "Local document"}
- Type: {entry_type}
- IDs: chunk={chunk_id}, page={page_id}

### 2. {entry_title} (score: {score})
...

---
{has_more ? "Use --limit N to see more results" : ""}
Tip: Use `mcp__kix__get_context` with page_id for full content.
```

## Follow-up Actions

After displaying results, offer these quick actions:
- **Get full context**: "To see full content, I can call `mcp__kix__get_context` with a page_id"
- **Find similar**: "To find similar content, I can search with that entry's title"
- **Show document details**: "To see all chunks, I can call `mcp__kix__get_document`"

## Error Handling

- **No results**: Suggest broader query or different search mode
- **Search fails**: Report error and suggest checking KIX status with `/kix-status`
- **Filters too restrictive**: Show count without filters for comparison
