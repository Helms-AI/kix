---
name: kix-link
description: |
  Link knowledge base entries to projects for context-aware planning.

  Usage: /kix-link <project> <entry-id|--search "query"|--list|--unlink>
  Examples:
    /kix-link my-project abc123-def456
    /kix-link my-project --search "authentication docs"
    /kix-link my-project --list
    /kix-link my-project --unlink abc123-def456
argument-hint: <project> <entry-id|--search "query"|--list|--unlink>
---

# KIX Link Command

Link knowledge base entries to projects for AI-assisted planning and context.

## Parse Arguments

Extract from `$ARGUMENTS`:
- **project**: Project ID or slug (required)
- **entry-id**: Direct entry ID to link
- **--search**: Search query to find entries to link
- **--list**: List currently linked entries
- **--unlink**: Entry ID to unlink
- **--relevance**: Relevance score 0.0-1.0 (default: 0.8)
- **--notes**: Notes about why this entry is linked

## Action: List Linked Entries

If `--list` is present:

```
mcp__kix__list_project_entries with:
- project: <project>
- limit: 50
```

**Output:**
```markdown
## Linked Knowledge for {project}

| Entry | Type | Relevance | Linked |
|-------|------|-----------|--------|
| {title} | {entry_type} | {relevance} | {linked_at} |
...

**Total**: {total} linked entries

**Actions:**
- Link more: `/kix-link {project} <entry-id>`
- Search to link: `/kix-link {project} --search "<query>"`
- Unlink: `/kix-link {project} --unlink <entry-id>`
```

## Action: Search and Link

If `--search` is present:

1. **Search for entries:**
   ```
   mcp__kix__search with:
   - query: <search-query>
   - limit: 10
   ```

2. **Present results for selection:**
   ```markdown
   ## Search Results to Link

   Found {total} entries matching "{query}"

   | # | Entry | Type | Score |
   |---|-------|------|-------|
   | 1 | {title} | {entry_type} | {score} |
   ...

   **To link an entry**, use:
   `/kix-link {project} <entry-id>`

   **Entry IDs:**
   - {title}: `{entry_id}`
   ...
   ```

## Action: Link Entry

```
mcp__kix__link_entry_to_project with:
- project: <project>
- entry_id: <entry-id>
- relevance: <relevance or 0.8>
- notes: <notes if provided>
```

**Output:**
```markdown
## Entry Linked

- **Entry**: {entry_title}
- **Project**: {project}
- **Link ID**: {link_id}
- **Relevance**: {relevance}
{notes ? "- **Notes**: " + notes : ""}

This entry will now be included in AI planning context for this project.

**Related actions:**
- Get project context: use `mcp__kix__search_project`
- Plan project: use `kix:project-planner` agent
```

## Action: Unlink Entry

If `--unlink` is present:

```
mcp__kix__unlink_entry_from_project with:
- project: <project>
- entry_id: <entry-id>
```

**Output:**
```markdown
## Entry Unlinked

Entry {entry_id} has been unlinked from project {project}.

This entry will no longer be included in AI planning context.
```

## Finding Related Entries

After linking, suggest finding related content:
```markdown
## Finding Related Entries

Based on the linked entry, you might also want to link:

{Search with linked entry's title as query}

| Entry | Score | Command |
|-------|-------|---------|
| {title} | {score} | `/kix-link {project} {entry_id}` |
...
```

## Error Handling

- **Entry not found**: Suggest searching with `/kix-link --search`
- **Already linked**: Note existing link, offer to update relevance
- **Project not found**: List available projects with `/kix-project`
- **Invalid relevance**: Must be between 0.0 and 1.0
