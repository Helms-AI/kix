---
name: kix-board
description: |
  KIX Board Command - View and manage Kanban boards.

  Usage: /kix-board <project> [action] [options]
  Examples:
    /kix-board my-project                  # View board
    /kix-board my-project --type story     # View stories only
    /kix-board move my-project 5 in_progress
    /kix-board columns my-project          # Column counts
argument-hint: <project> [move|columns] [options]
---

# KIX Board Command

View and manage Kanban boards for project workflow visualization.

## Parse Arguments

Parse `$ARGUMENTS` to determine action:

| Pattern | Action |
|---------|--------|
| project (only) | View board |
| "move" + project + item + column | Move card |
| "columns" + project | Show column counts |

### Options
- `--type` or `-t`: Filter by item type (epic, story, task, subtask, bug)
- `--swimlane` or `-s`: Show specific swimlane only

## Action: View Board

```
mcp__kix__get_board {
  project: <project-slug>,
  item_type: <--type value>
}
```

**Format output as:**
```markdown
## Kanban Board: {project}

### Column Summary
| Column | Count |
|--------|-------|
| Backlog | {column_counts.backlog or 0} |
| To Do | {column_counts.todo or 0} |
| In Progress | {column_counts.in_progress or 0} |
| In Review | {column_counts.in_review or 0} |
| Testing | {column_counts.testing or 0} |
| Done | {column_counts.done or 0} |
| **Total** | {total_items} |

---

{for each swimlane with items:}

### {swimlane.label} ({swimlane.total_items})

#### Backlog
{swimlane.columns.backlog.map(i => "- [ ] #" + i.number + " " + i.title)}

#### To Do
{swimlane.columns.todo.map(i => "- [ ] #" + i.number + " " + i.title + " @" + i.assignees.join(", "))}

#### In Progress
{swimlane.columns.in_progress.map(i => "- [~] #" + i.number + " " + i.title + " @" + i.assignees.join(", "))}

#### In Review
{swimlane.columns.in_review.map(i => "- [?] #" + i.number + " " + i.title)}

#### Testing
{swimlane.columns.testing.map(i => "- [T] #" + i.number + " " + i.title)}

#### Done
{swimlane.columns.done.map(i => "- [x] #" + i.number + " " + i.title)}

{/for}

---

### Quick Actions
- Move card: `/kix-board move {project} <number> <column>`
- View item: `/kix-work {project} <number>`
- Create item: `/kix-work create {project} "Title"`
```

## Action: Move Card

```
mcp__kix__move_card {
  project: <project-slug>,
  item: <item-number>,
  to_column: <target-column>,
  to_position: <position or 0>
}
```

Valid columns:
- `backlog` - Not yet planned
- `todo` - Ready to start
- `in_progress` - Currently working
- `in_review` - Awaiting review
- `testing` - In QA testing
- `done` - Completed

**Format output as:**
```markdown
## Card Moved

**#{item}** moved to **{to_column}**

| From | To |
|------|-----|
| {from_column} | {to_column} |

View board: `/kix-board {project}`
```

## Action: Column Counts

Quick view of column distribution:

```
mcp__kix__get_board {
  project: <project-slug>
}
```

**Format output as (compact):**
```markdown
## Board: {project}

| Backlog | To Do | In Progress | In Review | Testing | Done |
|---------|-------|-------------|-----------|---------|------|
| {n} | {n} | {n} | {n} | {n} | {n} |

**Total**: {total_items} items

### Work In Progress
{in_progress items listed}
```

## Board Visualization (ASCII)

For terminal-friendly visualization:

```
┌───────────┬───────────┬───────────┬───────────┬───────────┬───────────┐
│  BACKLOG  │   TODO    │IN PROGRESS│ IN REVIEW │  TESTING  │   DONE    │
│    (5)    │    (3)    │    (2)    │    (1)    │    (0)    │   (10)    │
├───────────┼───────────┼───────────┼───────────┼───────────┼───────────┤
│ #12 Auth  │ #15 API   │ #8 Login  │ #7 Tests  │           │ #1 Setup  │
│ #14 Docs  │ #16 Cache │ #9 UI     │           │           │ #2 Config │
│ #18 Tests │ #17 Log   │           │           │           │ #3 DB     │
│ #19 API   │           │           │           │           │ ...       │
│ #20 Cache │           │           │           │           │           │
└───────────┴───────────┴───────────┴───────────┴───────────┴───────────┘
```

## Workflow Commands

### Start Work
```bash
/kix-board move {project} {item} in_progress
```

### Submit for Review
```bash
/kix-board move {project} {item} in_review
```

### Send to Testing
```bash
/kix-board move {project} {item} testing
```

### Complete
```bash
/kix-board move {project} {item} done
/kix-work update {project} {item} --state closed
```

## Swimlane Types

| Swimlane | Items | Color |
|----------|-------|-------|
| Epics | Large initiatives | Purple |
| Stories | User features | Blue |
| Tasks | Implementation | Green |
| Subtasks | Granular work | Gray |
| Bugs | Defects | Red |

## Error Handling

| Error | Response |
|-------|----------|
| Project not found | List projects with `/kix-project list` |
| Item not found | List items with `/kix-work list {project}` |
| Invalid column | Valid: backlog, todo, in_progress, in_review, testing, done |
| Invalid item type | Valid: epic, story, task, subtask, bug |
