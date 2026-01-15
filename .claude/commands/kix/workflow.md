---
name: kix-workflow
description: |
  Orchestrate complex KIX workflows that coordinate multiple agents and skills.

  Usage: /kix-workflow <workflow-type> [options]

  Workflows:
    setup-project  - Set up a new project with research, indexing, and planning
    expand-kb      - Analyze gaps and expand the knowledge base
    research-plan  - Research a topic and create an actionable project plan
    maintain       - Run maintenance: quality checks, deduplication, re-indexing

  Examples:
    /kix-workflow setup-project "OAuth 2.0 implementation" --repo myorg/auth-service
    /kix-workflow expand-kb --focus "React hooks"
    /kix-workflow research-plan "microservices communication patterns"
    /kix-workflow maintain --auto-fix
argument-hint: <setup-project|expand-kb|research-plan|maintain> [topic] [--options]
---

# KIX Workflow Command

Orchestrate complex, multi-step workflows that coordinate KIX agents and skills.

## Parse Arguments

Extract from `$ARGUMENTS`:

**Workflow Types**:
- **setup-project**: New project setup with research, indexing, planning
- **expand-kb**: Knowledge base expansion and gap filling
- **research-plan**: Research topic and create project plan
- **maintain**: Maintenance, quality checks, deduplication

**Common Options**:
- **topic/question**: Primary subject (positional or --topic)
- **--repo**: GitHub repository (owner/repo format)
- **--project**: Existing project ID or slug
- **--focus**: Focus area for analysis
- **--depth**: Crawl depth for indexing (default: 2)
- **--auto-fix**: Automatically fix issues (for maintain)
- **--dry-run**: Preview workflow without executing
- **--verbose**: Show detailed progress

## Workflow: setup-project

**Purpose**: Complete end-to-end project setup

**Arguments**:
- `<topic>` (required): What the project is about
- `--repo <owner/repo>`: GitHub repository for integration
- `--template <template>`: Project template (kanban, bug_tracking, sprint_planning, feature_roadmap)
- `--depth <n>`: Documentation crawl depth (default: 2)

**Execution**:

1. **Present Plan**:
```markdown
## Workflow: Setup Project

**Topic**: {topic}
**GitHub**: {repo or "None - local only"}
**Template**: {template}

### Phases
| # | Phase | Agent/Skill | Est. Time |
|---|-------|-------------|-----------|
| 1 | Research Topic | research-synthesizer | 2-3 min |
| 2 | Index Documentation | indexing-strategist | 3-5 min |
| 3 | Create Project | /kix-project | <1 min |
| 4 | Link Knowledge | /kix-link (batch) | <1 min |
| 5 | Create Plan & Work Items | project-planner | 2-3 min |

**Total Estimated**: 7-12 minutes

Proceed? [Yes / Modify / Cancel]
```

2. **Execute Phases** (if approved):

   **Phase 1: Research**
   ```
   Task tool:
     subagent_type: "kix:research-synthesizer"
     prompt: |
       Research "{topic}" with focus on:
       - Core concepts and implementation patterns
       - Best practices and common pitfalls
       - Related technologies and alternatives
       - Recommended documentation sources

       Provide a comprehensive synthesis for project planning.
   ```

   **Phase 2: Index Documentation**
   ```
   Task tool:
     subagent_type: "kix:indexing-strategist"
     prompt: |
       Based on the research findings, plan and execute indexing for
       the recommended documentation sources.

       Research recommended these sources: {recommended_sources}

       Use depth={depth}, prioritize official documentation.
       Monitor jobs until completion.
   ```

   **Phase 3: Create Project**
   ```
   mcp__kix__create_project with:
     name: "{topic}"
     github_owner: "{owner}" (if repo provided)
     github_repo: "{repo}" (if repo provided)
     template: "{template}"
   ```

   **Phase 4: Link Knowledge**
   ```
   # Search for relevant entries
   mcp__kix__search { query: "{topic}", limit: 20 }

   # Link top results
   For each high-relevance result:
     mcp__kix__link_entry_to_project {
       project: "{project_slug}",
       entry_id: "{entry_id}",
       relevance: {score}
     }
   ```

   **Phase 5: Plan & Issues**
   ```
   Task tool:
     subagent_type: "kix:project-planner"
     prompt: |
       Create a comprehensive project plan for "{topic}".
       Project: {project_slug}

       Use the linked knowledge entries for context.
       Generate detailed, actionable issues with:
       - Clear acceptance criteria
       - Implementation hints from documentation
       - Proper labels and priorities
   ```

3. **Report Completion**:
```markdown
## Workflow Complete: Setup Project

### Summary
- **Duration**: {total_time}
- **Project Created**: {project_name} ({project_slug})
- **GitHub**: {repo_url or "Local only"}

### Artifacts Created
| Type | Count | Details |
|------|-------|---------|
| Documents Indexed | {n} | From {source_count} sources |
| Knowledge Links | {n} | Connected to project |
| Work Items Created | {n} | Across {milestone_count} milestones |

### Project Plan Overview
{summary of milestones and key work items}

### Next Steps
- View project: `/kix-project {project_slug}`
- View board: `/kix-board {project_slug}`
- List work items: `/kix-work list {project_slug}`
- Add more knowledge: `/kix-link {project_slug} --search "<query>"`
```

## Workflow: expand-kb

**Purpose**: Analyze and expand knowledge base coverage

**Arguments**:
- `--focus <area>`: Specific domain to focus on
- `--max-jobs <n>`: Maximum concurrent crawl jobs (default: 5)
- `--domains <list>`: Specific domains to check

**Execution**:

1. **Present Plan**:
```markdown
## Workflow: Expand Knowledge Base

**Focus**: {focus or "All domains"}
**Max Crawl Jobs**: {max_jobs}

### Phases
| # | Phase | Agent/Skill | Est. Time |
|---|-------|-------------|-----------|
| 1 | Assess Coverage | knowledge-curator | 2-3 min |
| 2 | Plan Indexing | indexing-strategist | 1-2 min |
| 3 | Execute Crawls | /kix-index (parallel) | 5-15 min |
| 4 | Monitor Progress | /kix-status | continuous |
| 5 | Verify Quality | knowledge-curator | 2-3 min |
| 6 | Link to Projects | /kix-link | 1-2 min |

Proceed? [Yes / Modify / Cancel]
```

2. **Execute Phases** following the template in workflow-orchestrator agent

3. **Report Completion**:
```markdown
## Workflow Complete: Expand Knowledge Base

### Coverage Improvement
| Metric | Before | After | Change |
|--------|--------|-------|--------|
| Total Documents | {before} | {after} | +{delta} |
| Total Chunks | {before} | {after} | +{delta} |
| {focus} Coverage | {before}% | {after}% | +{delta}% |

### Indexing Summary
| Domain | Pages | Documents | Chunks | Quality |
|--------|-------|-----------|--------|---------|
| {domain} | {n} | {n} | {n} | {score}% |
...

### Quality Assessment
- High-quality sources: {list}
- Issues detected: {list or "None"}

### New Project Links
| Project | Entries Linked |
|---------|---------------|
| {project} | {n} |
...
```

## Workflow: research-plan

**Purpose**: Research topic and create actionable project plan

**Arguments**:
- `<question>` (required): Research question or topic
- `--project <slug>`: Add issues to existing project
- `--create-project`: Create new project for results

**Execution**:

1. **Present Plan**:
```markdown
## Workflow: Research to Plan

**Question**: {question}
**Target Project**: {project or "To be determined"}

### Phases
| # | Phase | Agent/Skill | Est. Time |
|---|-------|-------------|-----------|
| 1 | Deep Research | research-synthesizer | 3-5 min |
| 2 | Find Context | /kix-search, /kix-project | <1 min |
| 3 | Determine Project | (decision logic) | <1 min |
| 4 | Link Findings | /kix-link | <1 min |
| 5 | Create Plan | project-planner | 2-3 min |

Proceed? [Yes / Modify / Cancel]
```

2. **Execute Phases**

3. **Report Completion**:
```markdown
## Workflow Complete: Research to Plan

### Research Summary
{executive_summary from research}

### Project Plan
- **Project**: {project_name}
- **Issues Created**: {n}
- **Milestones**: {n}

### Issue Breakdown
| Milestone | Issues | Priority Distribution |
|-----------|--------|----------------------|
| {name} | {n} | {critical}/{high}/{med}/{low} |
...

### Knowledge Links
| Entry | Relevance | Why Linked |
|-------|-----------|------------|
| {title} | {score} | {reason} |
...
```

## Workflow: maintain

**Purpose**: Routine maintenance and health checks

**Arguments**:
- `--auto-fix`: Automatically fix issues (with confirmation for destructive actions)
- `--domains <list>`: Specific domains to check
- `--dry-run`: Preview what would be fixed

**Execution**:

1. **Present Plan**:
```markdown
## Workflow: Maintenance

**Mode**: {auto_fix ? "Auto-fix enabled" : "Report only"}
**Scope**: {domains or "All content"}

### Checks
| Check | Description |
|-------|-------------|
| Health | Index status and metrics |
| Duplicates | Content with >90% similarity |
| Outdated | Sources older than 6 months |
| Orphaned | Entries not linked to projects |
| Quality | Low-quality chunks and extraction issues |

Proceed? [Yes / Modify / Cancel]
```

2. **Execute Phases**

3. **Report Completion**:
```markdown
## Maintenance Report

### Health Status
| Metric | Value | Status |
|--------|-------|--------|
| Total Documents | {n} | {ok/warning} |
| Total Chunks | {n} | {ok/warning} |
| Active Jobs | {n} | {ok/warning} |
| Index Size | {size} | {ok/warning} |

### Issues Found
| Category | Count | Action Taken |
|----------|-------|--------------|
| Duplicates | {n} | {action or "Review needed"} |
| Outdated | {n} | {action or "Review needed"} |
| Orphaned | {n} | {action or "Review needed"} |
| Low Quality | {n} | {action or "Review needed"} |

### Recommendations
{if not auto_fix}
- To remove duplicates: `mcp__kix__delete { ids: [...] }`
- To re-index outdated: `/kix-index {url} --replace`
- To link orphaned: `/kix-link {project} --search "<query>"`
{/if}
```

## Error Handling

- **Phase failure**: Report error, offer to skip or abort
- **Agent timeout**: Retry with increased timeout
- **User cancellation**: Clean up partial state, report what was completed
- **Rate limits**: Wait and retry automatically

## Dry Run Mode

With `--dry-run`, show what would happen without executing:

```markdown
## Dry Run: {workflow}

### Would Execute
| Phase | Action | Estimated Impact |
|-------|--------|------------------|
| 1 | {action} | {impact} |
...

### Resources Required
- Estimated API calls: {n}
- Estimated duration: {time}
- Estimated new documents: {n}

To execute: remove --dry-run flag
```

## Integration with Agents

This skill coordinates multiple specialized agents for complex workflows:

```
Task tool:
  subagent_type: "kix:project-manager"
  prompt: |
    Execute the {workflow_type} workflow with:
    - Topic/Question: {topic}
    - Options: {options}

    Coordinate with other agents as needed:
    - kix:research-synthesizer for deep research
    - kix:project-planner for planning and work item creation
    - kix:indexing-strategist for documentation indexing
    - kix:knowledge-curator for quality assessment

    Report progress after each phase and provide a comprehensive completion report.
```

For simpler operations, skills and MCP tools are called directly to minimize latency.
