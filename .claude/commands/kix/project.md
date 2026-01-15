---
name: kix-project
description: |
  KIX Project Command - Create, list, view, and manage projects.

  Usage: /kix-project [action] [options]
  Examples:
    /kix-project                     # List all projects
    /kix-project my-project          # Show project details
    /kix-project create "New App"    # Create new project
    /kix-project archive my-project  # Archive a project
    /kix-project delete my-project   # Delete a project
argument-hint: [list|create|<project-slug>|archive|delete] [options]
---

# KIX Project Command

Manage KIX projects for work item tracking and knowledge organization.

## Parse Arguments

Parse `$ARGUMENTS` to determine action:

| Pattern | Action |
|---------|--------|
| (empty) or "list" | List all projects |
| "create" + name | Create new project |
| "archive" + slug | Archive project |
| "delete" + slug | Delete project |
| Any other string | Show project details |

### Options
- `--description` or `-d`: Project description (for create)
- `--color` or `-c`: Project color in hex (for create)
- `--archived` or `-a`: Include archived projects (for list)

## Action: List Projects

```
mcp__kix__list_projects {
  include_archived: <--archived flag present>,
  limit: 50
}
```

**Format output as:**
```markdown
## KIX Projects

| Project | Slug | Open | Closed | Status |
|---------|------|------|--------|--------|
| {name} | {slug} | {stats.open_items} | {stats.closed_items} | {archived ? "Archived" : "Active"} |

**Total**: {total} projects

### Quick Actions
- View details: `/kix-project <slug>`
- Create project: `/kix-project create "Name"`
- View board: `/kix-board <slug>`
```

## Action: Show Project Details

```
mcp__kix__get_project {
  project: <project-slug>,
  include_stats: true
}
```

**Format output as:**
```markdown
## Project: {name}

| Property | Value |
|----------|-------|
| **Slug** | {slug} |
| **Description** | {description or "No description"} |
| **Color** | {color or "Default"} |
| **Status** | {archived ? "Archived" : "Active"} |
| **Created** | {created_at} |
| **Updated** | {updated_at} |

### Statistics
| Metric | Count |
|--------|-------|
| Open Items | {stats.open_items} |
| Closed Items | {stats.closed_items} |
| Total Items | {stats.total_items} |
| Linked Knowledge | {stats.linked_entries} |

### Quick Actions
- View board: `/kix-board {slug}`
- List items: `/kix-work list {slug}`
- Create item: `/kix-work create {slug} "Title"`
- Link knowledge: `/kix-link {slug} <entry-id>`
- Search: `/kix-search "query" --project {slug}`
```

## Action: Create Project

```
mcp__kix__create_project {
  name: <project-name>,
  description: <--description value>,
  color: <--color value>
}
```

**Format output as:**
```markdown
## Project Created

| Property | Value |
|----------|-------|
| **Name** | {name} |
| **Slug** | {slug} |
| **ID** | {project_id} |

### Next Steps
1. Create work items: `/kix-work create {slug} "Title"`
2. View the board: `/kix-board {slug}`
3. Link knowledge: `/kix-link {slug} <entry-id>`
```

## Action: Archive Project

```
mcp__kix__update_project {
  project: <project-slug>,
  archived: true
}
```

**Format output as:**
```markdown
## Project Archived

**{project}** has been archived. It will no longer appear in the default project list.

To view archived projects: `/kix-project list --archived`
To restore: `/kix-project restore {slug}`
```

## Action: Restore Project (unarchive)

```
mcp__kix__update_project {
  project: <project-slug>,
  archived: false
}
```

## Action: Delete Project

**IMPORTANT: Confirm with user before deleting!**

First, get project details to show what will be deleted:
```
mcp__kix__get_project { project: <slug>, include_stats: true }
```

Then ask for confirmation before calling:
```
mcp__kix__delete_project {
  project: <project-slug>,
  delete_items: true
}
```

**Format output as:**
```markdown
## Project Deleted

**{project}** has been deleted.

| Removed | Count |
|---------|-------|
| Work Items | {items_deleted} |
| Knowledge Links | {entries_unlinked} |

This action cannot be undone.
```

## Error Handling

| Error | Response |
|-------|----------|
| Project not found | List available projects with `/kix-project list` |
| Invalid color format | Use hex format like "#3B82F6" or "3B82F6" |
| Delete without confirmation | Do not delete, ask for explicit confirmation |
