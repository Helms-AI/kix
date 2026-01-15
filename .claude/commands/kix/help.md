---
name: kix-help
description: |
  Get help with KIX commands, agents, and workflows.

  Usage: /kix-help [topic]
  Examples:
    /kix-help                    # Overview of all KIX capabilities
    /kix-help workflows          # List available workflows
    /kix-help agents             # List available agents
    /kix-help search             # Help for /kix-search command
    /kix-help getting-started    # Quick start guide
argument-hint: [workflows|agents|commands|<command-name>|getting-started]
---

# KIX Help Command

Discover and learn about KIX capabilities, commands, agents, and workflows.

## Parse Arguments

Extract from `$ARGUMENTS`:
- **No args**: Show overview of all KIX capabilities
- **"workflows"**: List available workflow templates
- **"agents"**: List available specialized agents
- **"commands"**: List all KIX commands/skills
- **<command-name>**: Show help for specific command
- **"getting-started"**: Quick start guide
- **"examples"**: Common usage examples

## Topic: Overview (No Args)

**Output:**
```markdown
# KIX Knowledge Indexer

A powerful knowledge management system for AI-assisted development.

## Quick Start

```bash
# Check system status
/kix-status

# Search the knowledge base
/kix-search "your query"

# Index new documentation
/kix-index https://docs.example.com

# Run automated workflows
/kix-workflow setup-project "My Project"
```

## Capabilities

### Commands (Quick Actions)
| Command | Purpose |
|---------|---------|
| `/kix-search` | Search the knowledge base |
| `/kix-index` | Index URLs, files, or text |
| `/kix-project` | Manage projects |
| `/kix-work` | Manage work items |
| `/kix-board` | View Kanban boards |
| `/kix-link` | Link knowledge to projects |
| `/kix-status` | Check system health |
| `/kix-jobs` | Monitor indexing jobs |
| `/kix-workflow` | Run automated workflows |
| `/kix-help` | This help system |

### Agents (Specialized AI)
| Agent | Purpose |
|-------|---------|
| `kix:project-manager` | Complete project management |
| `kix:board-manager` | Kanban board operations |
| `kix:research-synthesizer` | Deep research with multi-source synthesis |
| `kix:project-planner` | Knowledge-driven project planning |
| `kix:knowledge-curator` | Quality analysis and gap detection |
| `kix:indexing-strategist` | Crawl planning and optimization |
| `kix:batch-processor` | Bulk operations handling |
| `kix:quality-verifier` | Content quality assurance |
| `kix:report-generator` | Report creation |

### Workflows (Automated Pipelines)
| Workflow | Purpose |
|----------|---------|
| `setup-project` | End-to-end project initialization |
| `expand-kb` | Knowledge base expansion |
| `research-plan` | Research to actionable plan |
| `maintain` | Quality maintenance |

## Get More Help

- `/kix-help workflows` - Detailed workflow guide
- `/kix-help agents` - Agent capabilities
- `/kix-help <command>` - Command-specific help
- `/kix-help getting-started` - Step-by-step tutorial
- `/kix-help examples` - Common usage patterns
```

## Topic: Workflows

**Output:**
```markdown
# KIX Workflows

Automated multi-step pipelines that coordinate agents and commands.

## Available Workflows

### setup-project
**Purpose**: Complete end-to-end project initialization

**What it does**:
1. Researches your topic thoroughly
2. Indexes relevant documentation
3. Creates a project with GitHub integration
4. Links knowledge entries to project
5. Generates detailed issues and milestones
6. Syncs everything to GitHub

**Usage**:
```bash
/kix-workflow setup-project "OAuth 2.0 implementation" --repo myorg/auth-service
/kix-workflow setup-project "React dashboard" --template sprint_planning
```

**Options**:
- `--repo <owner/repo>`: GitHub repository
- `--template <template>`: Project template (kanban, bug_tracking, sprint_planning, feature_roadmap)
- `--depth <n>`: Documentation crawl depth (default: 2)

---

### expand-kb
**Purpose**: Systematically grow your knowledge base

**What it does**:
1. Analyzes current coverage and gaps
2. Plans efficient crawl strategy
3. Executes indexing jobs
4. Monitors progress
5. Verifies content quality
6. Links new content to projects

**Usage**:
```bash
/kix-workflow expand-kb --focus "React hooks"
/kix-workflow expand-kb --max-jobs 5
```

**Options**:
- `--focus <area>`: Specific domain to focus on
- `--max-jobs <n>`: Maximum concurrent crawl jobs
- `--domains <list>`: Specific domains to check

---

### research-plan
**Purpose**: Research a topic and create an actionable project plan

**What it does**:
1. Conducts deep research with citations
2. Finds related existing content
3. Creates or identifies target project
4. Links research findings
5. Generates detailed project plan with issues

**Usage**:
```bash
/kix-workflow research-plan "microservices communication patterns"
/kix-workflow research-plan "caching strategies" --project backend-refactor
```

**Options**:
- `--project <slug>`: Add issues to existing project
- `--create-project`: Create new project for results

---

### maintain
**Purpose**: Routine maintenance and health checks

**What it does**:
1. Checks system health metrics
2. Analyzes content quality
3. Detects duplicates
4. Identifies outdated content
5. Updates project links
6. Generates maintenance report

**Usage**:
```bash
/kix-workflow maintain
/kix-workflow maintain --auto-fix
/kix-workflow maintain --dry-run
```

**Options**:
- `--auto-fix`: Automatically fix issues
- `--domains <list>`: Specific domains to check
- `--dry-run`: Preview without executing

---

## Workflow Options

All workflows support:
- `--dry-run`: Preview what would happen
- `--verbose`: Show detailed progress

## Starting a Workflow

Simply run the command and the orchestrator will:
1. Present a detailed plan
2. Ask for your approval
3. Execute each phase with progress updates
4. Generate a completion report
```

## Topic: Agents

**Output:**
```markdown
# KIX Agents

Specialized AI agents for complex tasks. Invoked via the Task tool or workflows.

## Research & Analysis Agents

### kix:research-synthesizer
**Purpose**: Deep, multi-faceted research with synthesis

**Capabilities**:
- Decomposes questions into sub-queries
- Searches across multiple dimensions
- Retrieves full context for high-relevance results
- Synthesizes findings with citations
- Identifies knowledge gaps

**When to use**: Complex research questions requiring multiple sources

**Example prompt**:
```
Research how authentication patterns compare for microservices,
including implementation considerations and security trade-offs.
```

---

### kix:knowledge-curator
**Purpose**: Knowledge base quality and organization

**Capabilities**:
- Analyzes coverage by topic
- Detects duplicate content
- Identifies low-quality sources
- Suggests indexing targets
- Recommends tagging strategies

**When to use**: Quality audits, gap analysis, curation tasks

---

## Planning Agents

### kix:project-planner
**Purpose**: Knowledge-driven project planning

**Capabilities**:
- Mines knowledge base for relevant context
- Structures goals into milestones
- Generates detailed issues
- Links knowledge entries
- Provides effort estimates

**When to use**: Starting new projects, planning features

---

### kix:indexing-strategist
**Purpose**: Optimal crawling strategies

**Capabilities**:
- Analyzes target site structure
- Plans crawl configurations
- Monitors job progress
- Handles failures with retries
- Assesses content quality

**When to use**: Indexing new documentation sources

---

## Orchestration Agents

### kix:workflow-orchestrator
**Purpose**: Multi-step workflow coordination

**Capabilities**:
- Parses high-level goals
- Coordinates agents and skills
- Manages state across phases
- Handles errors and recovery
- Generates completion reports

**When to use**: Complex multi-step tasks (via /kix-workflow)

---

### kix:batch-processor
**Purpose**: Efficient bulk operations

**Capabilities**:
- Batch linking/unlinking
- Batch tagging
- Batch search aggregation
- Progress tracking
- Rollback support

**When to use**: Operations on many items at once

---

## Quality Agents

### kix:quality-verifier
**Purpose**: Content quality assurance

**Capabilities**:
- Chunk quality assessment
- Code extraction validation
- Metadata accuracy checks
- Completeness verification
- Duplicate detection

**When to use**: Post-indexing QA, audits

---

### kix:report-generator
**Purpose**: Professional report creation

**Capabilities**:
- Workflow completion reports
- Status summaries
- Research reports
- Audit reports

**When to use**: Generating formatted outputs

---

## Invoking Agents

Agents are typically invoked via:
1. **Workflows**: Automatic coordination
2. **Task tool**: Direct invocation with prompt
3. **Other agents**: Delegation

Example direct invocation:
```
Task tool:
  subagent_type: "kix:research-synthesizer"
  prompt: "Research [topic]..."
```
```

## Topic: Getting Started

**Output:**
```markdown
# Getting Started with KIX

A step-by-step guide to using KIX for knowledge management.

## Step 1: Check System Status

First, verify KIX is running:

```bash
/kix-status
```

You should see document counts, chunk statistics, and health metrics.

## Step 2: Index Your First Content

Index a documentation site:

```bash
/kix-index https://docs.example.com --depth 1 --tag my-docs
```

Or index a local file:

```bash
/kix-index /path/to/document.pdf --tag manual
```

## Step 3: Search Your Knowledge Base

Find relevant content:

```bash
/kix-search "authentication patterns"
/kix-search "error handling" --type code
```

## Step 4: Create a Project

Set up a project for tracking work:

```bash
/kix-project create "My Feature" myorg/my-repo
```

## Step 5: Link Knowledge to Project

Connect relevant documentation:

```bash
/kix-link my-feature --search "authentication"
```

## Step 6: Run Your First Workflow

Automate a complete setup:

```bash
/kix-workflow setup-project "User Authentication" --repo myorg/auth-service
```

## Common Patterns

### Research Before Coding
```bash
/kix-search "how to implement [feature]"
# or for deep research:
/kix-workflow research-plan "implementing [feature]"
```

### Expand Documentation Coverage
```bash
/kix-workflow expand-kb --focus "React"
```

### Weekly Maintenance
```bash
/kix-workflow maintain
```

## Tips

1. **Use tags**: Tag content by domain, project, or type for easier filtering
2. **Link to projects**: Connect knowledge entries to projects for context
3. **Regular maintenance**: Run `/kix-workflow maintain` periodically
4. **Check quality**: After indexing, verify with `/kix-status`

## Getting Help

- `/kix-help` - This overview
- `/kix-help <command>` - Command-specific help
- `/kix-help workflows` - Available workflows
- `/kix-help agents` - Available agents
- `/kix-help examples` - Usage examples
```

## Topic: Examples

**Output:**
```markdown
# KIX Usage Examples

Common patterns and real-world usage scenarios.

## Search Examples

```bash
# Basic search
/kix-search "authentication patterns"

# Search for code examples
/kix-search "React hooks" --type code

# Filter by domain
/kix-search "API endpoints" --domain docs.example.com

# Full-text keyword search
/kix-search "error handling" --mode text

# Semantic similarity search
/kix-search "how to handle user sessions" --mode vector
```

## Indexing Examples

```bash
# Single page
/kix-index https://docs.example.com/getting-started

# Crawl with depth
/kix-index https://docs.example.com --depth 2 --tag official-docs

# Local file
/kix-index /path/to/document.pdf --title "Architecture Guide"

# Raw text
/kix-index --text "Important note about API changes" --title "API Notes"
```

## Project Examples

```bash
# Create project
/kix-project create "Auth Service" myorg/auth-service --template kanban

# List projects
/kix-project

# View project details
/kix-project auth-service

# Archive project
/kix-project archive old-project
```

## Linking Examples

```bash
# Link specific entry
/kix-link my-project abc123-def456 --relevance 0.9

# Search and link
/kix-link my-project --search "authentication patterns"

# List linked entries
/kix-link my-project --list

# Unlink entry
/kix-link my-project --unlink abc123-def456
```

## Workflow Examples

```bash
# New project setup
/kix-workflow setup-project "OAuth Implementation" --repo myorg/oauth

# Expand knowledge base
/kix-workflow expand-kb --focus "TypeScript"

# Research and plan
/kix-workflow research-plan "microservices communication"

# Maintenance
/kix-workflow maintain --auto-fix
```

## Job Monitoring Examples

```bash
# List active jobs
/kix-jobs

# Watch specific job
/kix-jobs watch abc-123 --poll 15

# View job history
/kix-jobs history

# Cancel job
/kix-jobs cancel abc-123
```

## Common Workflows

### Starting a New Feature
```bash
# 1. Research the topic
/kix-search "feature topic"

# 2. If more docs needed
/kix-index https://relevant-docs.com

# 3. Create/update project
/kix-project create "Feature Name" myorg/repo

# 4. Link relevant knowledge
/kix-link feature-name --search "feature topic"
```

### Weekly Knowledge Maintenance
```bash
# 1. Check status
/kix-status

# 2. Run maintenance
/kix-workflow maintain

# 3. Review recommendations
# 4. Index suggested sources
```
```

## Topic: Specific Command Help

If a specific command name is provided, show detailed help for that command.

**Execution:**
1. Read the corresponding command file from `.claude/commands/kix/{command}.md`
2. Extract key information: description, usage, arguments, examples
3. Format for quick reference

**Output:**
```markdown
# /kix-{command}

{description}

## Usage

```
/kix-{command} {argument-hint}
```

## Arguments

{parsed from command file}

## Examples

{examples from command file}

## Related

- {related commands}
- {related agents}
```

## Error Handling

- **Unknown topic**: Suggest similar topics
- **Command not found**: List available commands
- **Agent not found**: List available agents
