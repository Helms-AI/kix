---
name: kix-index
description: |
  Quick indexing of URLs, files, or text content into KIX.

  Usage: /kix-index <source> [options]
  Examples:
    /kix-index https://docs.example.com --depth 1
    /kix-index /path/to/document.pdf --tag manual
    /kix-index --text "Content to index" --title "My Note"
argument-hint: <url|file|--text "content"> [--depth N] [--tag TAG] [--title TITLE]
---

# KIX Index Command

Index URLs, files, or raw text into the knowledge base.

## Parse Arguments

Extract from `$ARGUMENTS`:
- **source**: URL (starts with http/https), file path (starts with /), or --text flag
- **--depth**: Crawl depth for URLs (0=single page, default: 1)
- **--max-pages**: Maximum pages for crawls (default: unlimited)
- **--tag**: Tag(s) to apply (can repeat)
- **--title**: Custom title (auto-extracted if omitted)
- **--id**: Custom document ID (auto-generated if omitted)
- **--async**: Force async indexing (auto for URLs with depth > 0)

## Determine Indexing Strategy

1. **Single URL (depth=0) or File or Text:**
   - Use synchronous `mcp__kix__index`
   - Immediate result with chunk count

2. **URL Crawl (depth > 0) or Multiple Files:**
   - Use async `mcp__kix__index_async`
   - Return job ID and monitor progress

## Execute Indexing

### For Synchronous Indexing:

```
mcp__kix__index with:
- content: {
    url: <url if URL>,
    file: <path if file>,
    text: <content if --text>
  }
- title: <title if provided>
- id: <id if provided>
- tags: [<tags>]
- replace: false (unless --replace specified)
```

**Report Result:**
```markdown
## Indexing Complete

- **Document**: {title}
- **ID**: {document_id}
- **Chunks created**: {chunks_created}
- **Status**: {success ? "Success" : "Failed - " + error}
```

### For Async Indexing:

```
mcp__kix__index_async with:
- source: {
    url: {
      url: <url>,
      depth: <depth>,
      max_pages: <max_pages>,
      render_js: true
    }
  }
- tags: [<tags>]
```

**Report Job Started:**
```markdown
## Indexing Job Started

- **Job ID**: {job_id}
- **Status**: {status}
- **Source**: {source_type}
- **Estimated items**: {estimated_items or "Unknown (discovery mode)"}

Monitoring progress...
```

## Monitor Async Jobs

If async job started, poll `mcp__kix__job_status` periodically:

```
Job {job_id}: {status}
Progress: {processed}/{total} ({percentage}%)
Current: {current_item}
ETA: {eta_seconds}s
```

When complete:
```markdown
## Indexing Complete

- **Job ID**: {job_id}
- **Documents created**: {documents_created}
- **Chunks created**: {chunks_created}
- **Errors**: {errors.length > 0 ? errors.join("\n") : "None"}
```

## Error Handling

- **Invalid URL**: Suggest checking URL format
- **File not found**: Verify path exists
- **Job failed**: Show error details and suggest retry
- **Timeout**: Note job continues in background, provide job ID for status check

## Tips After Success

- "Search your new content: `/kix-search <query>`"
- "Check index status: `/kix-status`"
- "Link to project: `/kix-link <project> <entry-id>`"
