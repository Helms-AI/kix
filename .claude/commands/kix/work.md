---
name: kix-work
description: |
  KIX Work Item Command - Create, list, view, and manage work items.

  Usage: /kix-work [action] <project> [options]
  Examples:
    /kix-work list my-project              # List work items
    /kix-work create my-project "Title"    # Create work item
    /kix-work my-project 5                 # View item #5
    /kix-work update my-project 5 --state closed
    /kix-work delete my-project 5
argument-hint: [list|create|update|delete|<item#>] <project> [options]
---

# KIX Work Item Command

Manage work items (epics, stories, tasks, subtasks, bugs) within projects.

## Parse Arguments

Parse `$ARGUMENTS` to determine action:

| Pattern | Action |
|---------|--------|
| "list" + project | List work items in project |
| "create" + project + title | Create new work item |
| "update" + project + item | Update work item |
| "delete" + project + item | Delete work item |
| project + number | Show work item details |

### Options for Create/Update
- `--type` or `-t`: Item type (epic, story, task, subtask, bug)
- `--body` or `-b`: Description/body text
- `--labels` or `-l`: Comma-separated labels
- `--assignees` or `-a`: Comma-separated assignees
- `--column` or `-c`: Board column (backlog, todo, in_progress, in_review, testing, done)
- `--parent` or `-p`: Parent item ID (for subtasks)
- `--points` or `-s`: Story points estimate
- `--epic-color`: Color for epics (hex format)

### Options for List
- `--state`: Filter by state (open, closed, all)
- `--type`: Filter by item type
- `--assignee`: Filter by assignee
- `--search` or `-q`: Search in title/body
- `--limit`: Maximum results (default: 50)

## Action: List Work Items

```
mcp__kix__list_work_items {
  project: <project-slug>,
  state: <--state value or "open">,
  labels: <--labels split by comma>,
  assignee: <--assignee value>,
  search: <--search value>,
  limit: <--limit or 50>,
  offset: 0
}
```

**Format output as:**
```markdown
## Work Items: {project}

| # | Title | Type | State | Column | Assignees |
|---|-------|------|-------|--------|-----------|
| {number} | {title} | {item_type} | {state} | {board_column} | {assignees.join(", ")} |

**Showing**: {items.length} of {total} items
{has_more ? "Use --limit to see more" : ""}

### Quick Actions
- View item: `/kix-work {project} <number>`
- Create item: `/kix-work create {project} "Title"`
- View board: `/kix-board {project}`
```

## Action: Show Work Item

```
mcp__kix__get_work_item {
  project: <project-slug>,
  item: <item-number-or-id>
}
```

If item has children, also fetch:
```
mcp__kix__get_child_work_items {
  project: <project-slug>,
  parent_id: <item-id>
}
```

**Format output as:**
```markdown
## #{number}: {title}

| Property | Value |
|----------|-------|
| **Type** | {item_type} |
| **State** | {state} |
| **Column** | {board_column} |
| **Story Points** | {story_points or "—"} |
| **Created** | {created_at} |
| **Updated** | {updated_at} |

### Description
{body or "_No description_"}

### Labels
{labels.length > 0 ? labels.join(", ") : "_No labels_"}

### Assignees
{assignees.length > 0 ? assignees.join(", ") : "_Unassigned_"}

{parent_id ? "### Parent\nPart of: #{parent_number}" : ""}

{children.length > 0 ? "### Children\n" + children.map(c => "- #" + c.number + " " + c.title + " [" + c.item_type + "]").join("\n") : ""}

### Quick Actions
- Edit: `/kix-work update {project} {number} --title "New Title"`
- Move: `/kix-board move {project} {number} in_progress`
- Close: `/kix-work update {project} {number} --state closed`
```

## Action: Create Work Item

```
mcp__kix__create_work_item {
  project: <project-slug>,
  title: <title>,
  body: <--body value>,
  item_type: <--type value or "task">,
  labels: <--labels split by comma>,
  assignees: <--assignees split by comma>,
  board_column: <--column value or "backlog">,
  parent_id: <--parent value>,
  story_points: <--points value>,
  epic_color: <--epic-color value>
}
```

**Format output as:**
```markdown
## Work Item Created

| Property | Value |
|----------|-------|
| **Number** | #{number} |
| **Title** | {title} |
| **Type** | {item_type} |
| **Column** | {board_column} |

### Quick Actions
- View: `/kix-work {project} {number}`
- Edit: `/kix-work update {project} {number} --body "Description"`
- Move: `/kix-board move {project} {number} todo`
```

## Action: Update Work Item

```
mcp__kix__update_work_item {
  project: <project-slug>,
  item: <item-number>,
  title: <--title value>,
  body: <--body value>,
  state: <--state value>,
  labels: <--labels split by comma>,
  assignees: <--assignees split by comma>,
  item_type: <--type value>,
  parent_id: <--parent value>,
  board_column: <--column value>,
  story_points: <--points value>,
  epic_color: <--epic-color value>
}
```

**Format output as:**
```markdown
## Work Item Updated

**#{item}** in **{project}** has been updated.

### Changes Applied
{list changes that were made}

View: `/kix-work {project} {item}`
```

## Action: Delete Work Item

**IMPORTANT: Confirm with user before deleting!**

```
mcp__kix__delete_work_item {
  project: <project-slug>,
  item: <item-number>
}
```

**Format output as:**
```markdown
## Work Item Deleted

**#{item}** has been deleted from **{project}**.

This action cannot be undone.
```

## Item Types Reference

| Type | Use For | Parent |
|------|---------|--------|
| `epic` | Large features/initiatives | None |
| `story` | User-facing features | Epic |
| `task` | Implementation work | Story or Epic |
| `subtask` | Granular work items | Task or Story |
| `bug` | Defects | Any |

## Board Columns Reference

| Column | Description |
|--------|-------------|
| `backlog` | Not yet planned |
| `todo` | Ready to start |
| `in_progress` | Currently working |
| `in_review` | Awaiting review |
| `testing` | In QA |
| `done` | Completed |

## Error Handling

| Error | Response |
|-------|----------|
| Project not found | List projects with `/kix-project list` |
| Work item not found | List items with `/kix-work list {project}` |
| Invalid item type | Valid: epic, story, task, subtask, bug |
| Invalid column | Valid: backlog, todo, in_progress, in_review, testing, done |
