---
name: kix-status
description: |
  Show KIX system status, index statistics, and active jobs.

  Usage: /kix-status [options]
  Examples:
    /kix-status
    /kix-status --detailed
    /kix-status --jobs
argument-hint: [--detailed] [--jobs] [--projects]
---

# KIX Status Command

Display system health, index statistics, and active indexing jobs.

## Parse Arguments

- **--detailed**: Include breakdown by type and domain
- **--jobs**: Show active/recent indexing jobs
- **--projects**: Include project statistics

## Gather Status Information

1. **Call status tool:**
   ```
   mcp__kix__status with:
   - detailed: <true if --detailed>
   ```

2. **If --projects specified:**
   ```
   mcp__kix__list_projects with:
   - limit: 100
   - include_archived: false
   ```

## Format Output

```markdown
## KIX System Status

### Index Statistics
| Metric | Count |
|--------|-------|
| Documents | {total_documents} |
| Chunks | {total_chunks} |
| Pages | {total_pages} |

{if detailed:}
### Breakdown by Entry Type
| Type | Count |
|------|-------|
| document | {count} |
| pdf | {count} |
| article | {count} |
| code | {count} |

### Breakdown by Chunk Type
| Type | Count |
|------|-------|
| content | {count} |
| code | {count} |
| header | {count} |
| summary | {count} |

### Top Source Domains
| Domain | Documents |
|--------|-----------|
| {domain} | {count} |
...
{/if}

{if projects:}
### Projects Summary
| Project | Open Issues | Linked Entries |
|---------|-------------|----------------|
| {name} | {open_issues} | {linked_entries} |
...
{/if}

---
Last updated: {current_timestamp}
```

## Quick Actions

Offer follow-up commands based on status:
- If index empty: "Start indexing: `/kix-index <url>`"
- If projects exist: "View project: `/kix-project <slug>`"
- "Search content: `/kix-search <query>`"

## Error Handling

- **Status call fails**: Report connection error, suggest checking if KIX is running
- **Partial data**: Show what's available, note missing components
