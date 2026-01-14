# Project Management System for KIX

## Overview

Add an **AI-powered Project management system** to Kix that enables users to create bounded containers for organizing knowledge and issues, with deep **GitHub integration** including **GitHub Projects V2** for Kanban boards, issue tracking, and sprint planning.

**Key Capability**: Claude Code (via MCP) acts as an AI project manager, using knowledge from the Kix knowledge base to help plan projects, create GitHub Projects with templates, and break down work into trackable issues.

## Problem Statement

Currently, Kix indexes knowledge globally without organizational boundaries. Users need:
- **Bounded contexts** for different projects/domains
- **Issue tracking** integrated with their knowledge base
- **GitHub Projects integration** for visual task management (Kanban, sprints)
- **AI-assisted planning** that leverages indexed knowledge
- **Project-scoped search** to find relevant information within a specific context

## Design Goals

1. **AI Project Manager**: Claude Code helps plan and manage projects using knowledge context
2. **GitHub Projects V2**: Full Kanban/board support with templates
3. **Unified Access**: All operations available via MCP tools AND REST API/UI
4. **Flexible Authentication**: Global token with per-project override
5. **Real-time Sync**: MCP → UI events for live updates
6. **Consistent UX**: Follow existing Kix patterns for UI and API

## Design Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Data Model | New Project Entity | Full flexibility for project-specific features |
| GitHub Auth | Global + Per-Project | Both options with per-project override |
| GitHub Scope | **Issues + Projects V2** | Full project management capabilities |
| GitHub Repo | **Required** | Must connect to repo for issues/projects |
| API Type | **GraphQL + REST** | GraphQL for Projects V2, REST for Issues |
| Sync Strategy | Manual + Scheduled | Works everywhere, no public URL required |

---

## Data Models

### Project

```rust
/// A project is a user-created workspace for organizing knowledge and issues.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    /// Unique identifier (UUID)
    pub id: String,
    /// Human-readable name (unique, used for reference)
    pub name: String,
    /// URL-safe slug (auto-generated from name)
    pub slug: String,
    /// Short description
    pub description: Option<String>,
    /// GitHub repository integration (optional)
    pub github: Option<GitHubConfig>,
    /// Project color for UI (hex without #)
    pub color: Option<String>,
    /// Whether project is archived
    pub archived: bool,
    /// Created timestamp
    pub created_at: DateTime<Utc>,
    /// Updated timestamp
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitHubConfig {
    /// Repository owner (e.g., "anthropics")
    pub owner: String,
    /// Repository name (e.g., "claude-code")
    pub repo: String,
    /// Sync settings
    pub sync: GitHubSyncConfig,
    /// Last successful sync timestamp
    pub last_synced_at: Option<DateTime<Utc>>,
    /// Last sync error (if any)
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitHubSyncConfig {
    /// Enable automatic sync
    pub auto_sync: bool,
    /// Sync interval in minutes (0 = manual only)
    pub interval_minutes: u32,
    /// Issue labels to sync (empty = all)
    pub labels: Vec<String>,
    /// Issue states to sync: open, closed, all
    pub states: Vec<IssueState>,
}
```

### Issue

```rust
/// A project issue (local or synced from GitHub).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Issue {
    /// Unique identifier (UUID)
    pub id: String,
    /// Parent project ID
    pub project_id: String,
    /// Issue number within project (auto-incremented)
    pub number: u32,
    /// Issue title
    pub title: String,
    /// Issue body (markdown)
    pub body: Option<String>,
    /// Issue state
    pub state: IssueState,
    /// Labels
    pub labels: Vec<String>,
    /// Priority (1-5, 1=highest)
    pub priority: Option<u8>,
    /// Assignees
    pub assignees: Vec<String>,
    /// GitHub issue number (if synced)
    pub github_number: Option<u32>,
    /// GitHub issue URL (if synced)
    pub github_url: Option<String>,
    /// Source of the issue
    pub source: IssueSource,
    /// Created timestamp
    pub created_at: DateTime<Utc>,
    /// Updated timestamp
    pub updated_at: DateTime<Utc>,
    /// Closed timestamp (if closed)
    pub closed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum IssueState { Open, Closed }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum IssueSource { Local, GitHub, Mcp }
```

### ProjectEntry (Knowledge Link)

```rust
/// Links an entry to a project with optional notes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectEntry {
    /// Link ID
    pub id: String,
    /// Project ID
    pub project_id: String,
    /// Entry ID
    pub entry_id: String,
    /// Optional notes about this entry in project context
    pub notes: Option<String>,
    /// Custom tags for this entry within the project
    pub project_tags: Vec<String>,
    /// Added timestamp
    pub added_at: DateTime<Utc>,
}
```

---

## LanceDB Schema

### Projects Table

```rust
pub fn project_schema() -> Arc<Schema> {
    Arc::new(Schema::new(vec![
        Field::new("id", DataType::Utf8, false),
        Field::new("name", DataType::Utf8, false),
        Field::new("slug", DataType::Utf8, false),
        Field::new("description", DataType::Utf8, true),
        Field::new("github_config", DataType::Utf8, true),  // JSON
        Field::new("color", DataType::Utf8, true),
        Field::new("archived", DataType::Boolean, false),
        Field::new("created_at", DataType::Utf8, false),
        Field::new("updated_at", DataType::Utf8, false),
    ]))
}
```

### Issues Table

```rust
pub fn issue_schema() -> Arc<Schema> {
    Arc::new(Schema::new(vec![
        Field::new("id", DataType::Utf8, false),
        Field::new("project_id", DataType::Utf8, false),
        Field::new("number", DataType::UInt32, false),
        Field::new("title", DataType::Utf8, false),
        Field::new("body", DataType::Utf8, true),
        Field::new("state", DataType::Utf8, false),
        Field::new("labels", DataType::Utf8, true),        // JSON array
        Field::new("priority", DataType::UInt8, true),
        Field::new("assignees", DataType::Utf8, true),     // JSON array
        Field::new("github_number", DataType::UInt32, true),
        Field::new("github_url", DataType::Utf8, true),
        Field::new("source", DataType::Utf8, false),
        Field::new("created_at", DataType::Utf8, false),
        Field::new("updated_at", DataType::Utf8, false),
        Field::new("closed_at", DataType::Utf8, true),
        // Vector for semantic search on title+body
        Field::new("vector", DataType::FixedSizeList(
            Arc::new(Field::new("item", DataType::Float32, true)),
            384  // or 768 depending on model
        ), true),
    ]))
}
```

### Project Entries Table

```rust
pub fn project_entry_schema() -> Arc<Schema> {
    Arc::new(Schema::new(vec![
        Field::new("id", DataType::Utf8, false),
        Field::new("project_id", DataType::Utf8, false),
        Field::new("entry_id", DataType::Utf8, false),
        Field::new("notes", DataType::Utf8, true),
        Field::new("project_tags", DataType::Utf8, true),  // JSON array
        Field::new("added_at", DataType::Utf8, false),
    ]))
}
```

---

## Crate Structure: `kix-projects`

```
server/crates/kix-projects/
├── Cargo.toml
├── src/
│   ├── lib.rs              # Public exports
│   ├── project.rs          # ProjectStore - CRUD operations
│   ├── issue.rs            # IssueStore - CRUD operations
│   ├── knowledge.rs        # ProjectEntryStore - knowledge linking
│   ├── search.rs           # Project-scoped search service
│   ├── github/
│   │   ├── mod.rs          # Module exports
│   │   ├── client.rs       # GitHub REST API client
│   │   ├── sync.rs         # GitHubSyncService
│   │   └── models.rs       # GitHub API response types
│   └── error.rs            # ProjectError type
```

### Dependencies

```toml
[dependencies]
kix-store = { path = "../kix-store" }
kix-parser = { path = "../kix-parser" }
kix-auth = { path = "../kix-auth" }
kix-embeddings = { path = "../kix-embeddings" }
reqwest = { workspace = true, features = ["json"] }
serde = { workspace = true }
serde_json = { workspace = true }
tokio = { workspace = true }
chrono = { workspace = true }
uuid = { workspace = true }
thiserror = { workspace = true }
tracing = { workspace = true }
```

---

## MCP Tools Specification (25+ Tools)

### Project Management Tools

| Tool | Description | Parameters |
|------|-------------|------------|
| `create_project` | Create Kix project (GitHub repo required) | name, github_repo (owner/repo), description?, color? |
| `list_projects` | List all projects | include_archived? |
| `get_project` | Get project by ID or name | project (id/name) |
| `update_project` | Update project details | project, name?, description?, archived? |
| `delete_project` | Delete a project | project, delete_issues? |

### Issue Management Tools

| Tool | Description | Parameters |
|------|-------------|------------|
| `create_issue` | Create issue in project | project, title, body?, labels?, priority? |
| `list_issues` | List project issues | project, state?, labels?, limit? |
| `get_issue` | Get issue details | project, issue (number/id) |
| `update_issue` | Update an issue | project, issue, title?, body?, state?, labels?, priority? |
| `delete_issue` | Delete an issue | project, issue |

### GitHub Projects V2 Tools

| Tool | Description | Parameters |
|------|-------------|------------|
| `create_github_project` | Create GitHub Project board | project, title, template (kanban/bug_tracking/sprint/roadmap) |
| `get_github_project` | Get project board state | project, project_number |
| `add_issue_to_project` | Add issue to project board | project, issue, status? |
| `update_project_item` | Update item status/fields | project, item_id, status?, priority?, sprint? |
| `sync_github_project` | Sync project board state | project |
| `create_draft_issue` | Create draft issue on board | project, project_number, title, body? |

### AI Planning Tools (Knowledge-Powered)

| Tool | Description | Parameters |
|------|-------------|------------|
| `plan_project` | AI generates tasks from description | project, description, use_knowledge? (default: true) |
| `suggest_tasks` | Get task suggestions for project | project, context?, limit? |
| `get_project_context` | Retrieve relevant knowledge | project, query?, limit? |
| `breakdown_task` | Break down a task into subtasks | project, task_description |

### GitHub Token Management

| Tool | Description | Parameters |
|------|-------------|------------|
| `set_github_token` | Set GitHub token | project? (global if omitted), token |
| `delete_github_token` | Remove stored token | project? (global if omitted) |
| `sync_github_issues` | Sync issues from GitHub | project, states?, labels? |

### Knowledge & Search Tools

| Tool | Description | Parameters |
|------|-------------|------------|
| `add_entry_to_project` | Link entry to project | project, entry_id, notes?, tags? |
| `remove_entry_from_project` | Unlink entry | project, entry_id |
| `list_project_entries` | List linked entries | project, limit? |
| `search_project` | Search within project | project, query, include_issues?, include_entries?, limit? |

---

## AI Planning System

The AI Planning tools enable Claude Code (via MCP) to act as an intelligent project manager.

### Planning Workflow

```
┌─────────────────────────────────────────────────────────────────────┐
│                     AI Project Planning Flow                         │
├─────────────────────────────────────────────────────────────────────┤
│  1. User describes what they want to build                          │
│  2. Claude Code calls `get_project_context` to retrieve relevant    │
│     knowledge from the Kix knowledge base                           │
│  3. Claude Code calls `plan_project` with the description           │
│  4. System searches knowledge base for related patterns/docs        │
│  5. AI generates a structured task breakdown                        │
│  6. Tasks are created as GitHub issues via `create_issue`           │
│  7. Issues are added to GitHub Project board via `add_issue_to_project` │
│  8. User can iterate with `suggest_tasks` for additional ideas      │
└─────────────────────────────────────────────────────────────────────┘
```

### `plan_project` Tool Implementation

```rust
// kix-projects/src/planning.rs

#[derive(Debug, Deserialize, JsonSchema)]
pub struct PlanProjectParams {
    #[schemars(description = "Project ID or name")]
    pub project: String,

    #[schemars(description = "Description of what you want to build")]
    pub description: String,

    #[schemars(description = "Use knowledge base for context (default: true)")]
    pub use_knowledge: Option<bool>,

    #[schemars(description = "Template for task structure")]
    pub template: Option<PlanTemplate>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub enum PlanTemplate {
    /// Feature development: Design → Implement → Test → Document
    Feature,
    /// Bug fix: Investigate → Fix → Test → Verify
    BugFix,
    /// Research: Explore → Prototype → Evaluate → Document
    Research,
    /// Refactoring: Analyze → Plan → Execute → Verify
    Refactor,
}

#[derive(Debug, Serialize)]
pub struct PlanResult {
    pub project_id: String,
    pub tasks: Vec<PlannedTask>,
    pub knowledge_sources: Vec<KnowledgeReference>,
    pub suggested_template: ProjectTemplate,
}

#[derive(Debug, Serialize)]
pub struct PlannedTask {
    pub title: String,
    pub body: String,
    pub labels: Vec<String>,
    pub priority: Option<u8>,
    pub depends_on: Vec<String>,  // Task titles this depends on
}

#[derive(Debug, Serialize)]
pub struct KnowledgeReference {
    pub entry_id: String,
    pub title: String,
    pub relevance_score: f32,
    pub snippet: String,
}
```

### `plan_project` Response Example

When Claude Code calls `plan_project`, it receives:

```json
{
  "project_id": "proj_abc123",
  "tasks": [
    {
      "title": "Design authentication flow",
      "body": "Based on the OAuth 2.1 patterns in the knowledge base...\n\n## Acceptance Criteria\n- [ ] Define token storage strategy\n- [ ] Choose session management approach",
      "labels": ["design", "auth"],
      "priority": 1,
      "depends_on": []
    },
    {
      "title": "Implement JWT token validation",
      "body": "Reference: Similar implementation in kix-auth crate...",
      "labels": ["implementation", "auth"],
      "priority": 2,
      "depends_on": ["Design authentication flow"]
    }
  ],
  "knowledge_sources": [
    {
      "entry_id": "entry_oauth",
      "title": "OAuth 2.1 Best Practices",
      "relevance_score": 0.92,
      "snippet": "For secure token storage, use encrypted..."
    }
  ],
  "suggested_template": "Kanban"
}
```

### Knowledge Integration

```rust
impl PlanningService {
    pub async fn plan_project(
        &self,
        project_id: &str,
        description: &str,
        use_knowledge: bool,
    ) -> Result<PlanResult, PlanningError> {
        let mut knowledge_sources = Vec::new();

        // 1. Search knowledge base for relevant context
        if use_knowledge {
            let search_results = self.search_service
                .search_project(project_id, description, 10)
                .await?;

            knowledge_sources = search_results.entries
                .into_iter()
                .map(|e| KnowledgeReference {
                    entry_id: e.entry_id,
                    title: e.title,
                    relevance_score: e.score,
                    snippet: e.text.chars().take(200).collect(),
                })
                .collect();
        }

        // 2. Return structured data for AI to process
        // The actual task generation happens in Claude Code using this context
        Ok(PlanResult {
            project_id: project_id.to_string(),
            tasks: vec![],  // AI fills this in
            knowledge_sources,
            suggested_template: self.suggest_template(description),
        })
    }

    fn suggest_template(&self, description: &str) -> ProjectTemplate {
        let desc_lower = description.to_lowercase();
        if desc_lower.contains("bug") || desc_lower.contains("fix") {
            ProjectTemplate::BugTracking
        } else if desc_lower.contains("sprint") || desc_lower.contains("iteration") {
            ProjectTemplate::SprintPlanning
        } else if desc_lower.contains("roadmap") || desc_lower.contains("quarter") {
            ProjectTemplate::FeatureRoadmap
        } else {
            ProjectTemplate::Kanban
        }
    }
}
```

---

## REST API Endpoints

### Projects

```
GET    /api/projects                 # List projects
POST   /api/projects                 # Create project
GET    /api/projects/:id             # Get project
PUT    /api/projects/:id             # Update project
DELETE /api/projects/:id             # Delete project
```

### Issues

```
GET    /api/projects/:id/issues              # List issues
POST   /api/projects/:id/issues              # Create issue
GET    /api/projects/:id/issues/:number      # Get issue
PUT    /api/projects/:id/issues/:number      # Update issue
DELETE /api/projects/:id/issues/:number      # Delete issue
```

### Knowledge

```
GET    /api/projects/:id/entries             # List linked entries
POST   /api/projects/:id/entries             # Link entry
DELETE /api/projects/:id/entries/:entry_id   # Unlink entry
```

### GitHub

```
POST   /api/projects/:id/github/token        # Set project token
DELETE /api/projects/:id/github/token        # Delete project token
POST   /api/projects/:id/github/sync         # Trigger sync
GET    /api/projects/:id/github/status       # Sync status
POST   /api/github/token                     # Set global token
DELETE /api/github/token                     # Delete global token
```

### Search

```
GET    /api/projects/:id/search?q=...        # Project-scoped search
```

### Real-time Events (SSE)

```
GET    /api/projects/events                  # SSE stream for all project events
GET    /api/projects/:id/events              # SSE stream for specific project
```

---

## Real-time MCP → UI Events

When MCP tools modify data, the UI should automatically update. This is achieved via Server-Sent Events (SSE).

### Event Types

| Event | Trigger | Payload |
|-------|---------|---------|
| `project.created` | `create_project` MCP/API | `{ project_id, name }` |
| `project.updated` | `update_project` MCP/API | `{ project_id, changes }` |
| `project.deleted` | `delete_project` MCP/API | `{ project_id }` |
| `issue.created` | `create_issue` MCP/API | `{ project_id, issue_id, number }` |
| `issue.updated` | `update_issue` MCP/API | `{ project_id, issue_id, changes }` |
| `issue.deleted` | `delete_issue` MCP/API | `{ project_id, issue_id }` |
| `entry.linked` | `add_entry_to_project` MCP/API | `{ project_id, entry_id }` |
| `entry.unlinked` | `remove_entry_from_project` MCP/API | `{ project_id, entry_id }` |
| `github.sync.started` | `sync_github_issues` MCP/API | `{ project_id }` |
| `github.sync.completed` | Sync finishes | `{ project_id, created, updated, errors }` |
| `github.sync.failed` | Sync fails | `{ project_id, error }` |

### Architecture

```rust
// In kix-projects/src/events.rs
use tokio::sync::broadcast;

pub struct ProjectEventBus {
    sender: broadcast::Sender<ProjectEvent>,
}

impl ProjectEventBus {
    pub fn new() -> Self {
        let (sender, _) = broadcast::channel(100);
        Self { sender }
    }

    pub fn emit(&self, event: ProjectEvent) {
        let _ = self.sender.send(event);
    }

    pub fn subscribe(&self) -> broadcast::Receiver<ProjectEvent> {
        self.sender.subscribe()
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct ProjectEvent {
    pub event_type: String,
    pub project_id: Option<String>,
    pub payload: serde_json::Value,
    pub timestamp: DateTime<Utc>,
}
```

### SSE Endpoint

```rust
// In kix-api/src/project_routes.rs
async fn project_events_stream(
    State(state): State<ProjectState>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let rx = state.event_bus.subscribe();

    let stream = BroadcastStream::new(rx)
        .filter_map(|result| async move {
            match result {
                Ok(event) => {
                    let json = serde_json::to_string(&event).ok()?;
                    Some(Ok(Event::default().data(json)))
                }
                Err(_) => None,
            }
        });

    Sse::new(stream).keep_alive(
        axum::response::sse::KeepAlive::new()
            .interval(Duration::from_secs(30))
    )
}
```

### React Integration

```typescript
// client/src/hooks/useProjectEvents.ts
export function useProjectEvents(projectId?: string) {
  const queryClient = useQueryClient();

  useEffect(() => {
    const url = projectId
      ? `/api/projects/${projectId}/events`
      : '/api/projects/events';

    const eventSource = new EventSource(url);

    eventSource.onmessage = (event) => {
      const data = JSON.parse(event.data);

      // Invalidate relevant queries based on event type
      switch (data.event_type) {
        case 'project.created':
        case 'project.updated':
        case 'project.deleted':
          queryClient.invalidateQueries({ queryKey: ['projects'] });
          break;
        case 'issue.created':
        case 'issue.updated':
        case 'issue.deleted':
          queryClient.invalidateQueries({
            queryKey: ['projects', data.project_id, 'issues']
          });
          break;
        case 'github.sync.completed':
          queryClient.invalidateQueries({
            queryKey: ['projects', data.project_id]
          });
          break;
      }
    };

    return () => eventSource.close();
  }, [projectId, queryClient]);
}
```

### Usage in Pages

```typescript
// client/src/pages/projects/ProjectList.tsx
function ProjectList() {
  useProjectEvents(); // Subscribe to all project events
  const { data: projects } = useQuery({ queryKey: ['projects'], ... });
  // UI auto-updates when events arrive
}

// client/src/pages/projects/IssueList.tsx
function IssueList({ projectId }: { projectId: string }) {
  useProjectEvents(projectId); // Subscribe to this project's events
  const { data: issues } = useQuery({
    queryKey: ['projects', projectId, 'issues'], ...
  });
  // UI auto-updates when MCP creates/updates issues
}
```

---

## React UI Components

### New Pages

| Page | Route | Description |
|------|-------|-------------|
| ProjectList | `/projects` | Grid of project cards with create button |
| ProjectDetail | `/projects/:id` | Tabbed view: Overview, Issues, Knowledge, Settings |
| IssueDetail | `/projects/:id/issues/:number` | Full issue view |

### Navigation Updates

- Dynamic project list in sidebar under "Projects" section
- Quick-add project button
- Project selector for context switching

### Component Hierarchy

```
pages/projects/
├── ProjectList.tsx           # Main project list page
├── ProjectDetail.tsx         # Project detail with tabs
├── ProjectDashboard.tsx      # Dashboard tab with metrics
├── ProjectSettings.tsx       # GitHub config, danger zone
├── IssueList.tsx             # Issue list with filters
├── IssueDetail.tsx           # Single issue view
├── IssueForm.tsx             # Create/edit issue modal
├── KnowledgeList.tsx         # Linked entries grid
└── components/
    ├── ProjectCard.tsx       # Card for list view
    ├── IssueCard.tsx         # Issue card component
    ├── GitHubSyncStatus.tsx  # Sync status indicator
    ├── ProjectSidebar.tsx    # Project-specific sidebar
    ├── DashboardStats.tsx    # Stats cards (open/closed issues, entries)
    └── ActivityFeed.tsx      # Recent activity list
```

### Project Dashboard Features

The Dashboard tab displays:
- **Quick Stats**: Open issues, closed issues, knowledge entries linked
- **Issue Breakdown**: Pie/donut chart by state and labels
- **Recent Activity**: Last 10 actions (issue created, synced, entry linked)
- **GitHub Sync Status**: Last sync time, next scheduled sync, error state
- **Quick Actions**: Sync now, create issue, add knowledge

---

## GitHub Integration Details

### Token Priority

1. Per-project token (if set)
2. Global token (fallback)
3. Error (no token available)

### Secure Token Storage

GitHub PATs are stored **encrypted at rest** in SQLite using AES-256-GCM symmetric encryption.

#### Security Architecture

```
┌───────────────────────────────────────────────────────────────────┐
│                    Token Encryption Flow                          │
├───────────────────────────────────────────────────────────────────┤
│  1. Server loads encryption key from KIX_ENCRYPTION_KEY env var   │
│  2. On first run, if not set, generates and logs a new key        │
│  3. Token encrypted with AES-256-GCM before SQLite write          │
│  4. Unique nonce per encryption (stored alongside ciphertext)     │
│  5. On retrieval, decrypt with same key + stored nonce            │
└───────────────────────────────────────────────────────────────────┘
```

#### Implementation

```rust
// kix-auth/src/encryption.rs
use aes_gcm::{
    aead::{Aead, KeyInit, OsRng},
    Aes256Gcm, Nonce,
};
use rand::RngCore;

const NONCE_SIZE: usize = 12;  // 96 bits for GCM

pub struct TokenEncryptor {
    cipher: Aes256Gcm,
}

impl TokenEncryptor {
    /// Create from 32-byte (256-bit) key
    pub fn new(key: &[u8; 32]) -> Self {
        let cipher = Aes256Gcm::new_from_slice(key).expect("valid key size");
        Self { cipher }
    }

    /// Load key from environment or generate new one
    pub fn from_env() -> Result<Self, EncryptionError> {
        if let Ok(key_hex) = std::env::var("KIX_ENCRYPTION_KEY") {
            let key = hex::decode(&key_hex)?;
            if key.len() != 32 {
                return Err(EncryptionError::InvalidKeyLength);
            }
            Ok(Self::new(key.as_slice().try_into().unwrap()))
        } else {
            // Generate new key and warn user to persist it
            let mut key = [0u8; 32];
            OsRng.fill_bytes(&mut key);
            let key_hex = hex::encode(&key);
            tracing::warn!(
                "No KIX_ENCRYPTION_KEY set. Generated new key. \
                 Add this to your environment to persist encrypted data: \
                 KIX_ENCRYPTION_KEY={}",
                key_hex
            );
            Ok(Self::new(&key))
        }
    }

    /// Encrypt token, returns nonce + ciphertext
    pub fn encrypt(&self, plaintext: &str) -> Result<Vec<u8>, EncryptionError> {
        let mut nonce_bytes = [0u8; NONCE_SIZE];
        OsRng.fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);

        let ciphertext = self.cipher
            .encrypt(nonce, plaintext.as_bytes())
            .map_err(|_| EncryptionError::EncryptionFailed)?;

        // Prepend nonce to ciphertext for storage
        let mut result = Vec::with_capacity(NONCE_SIZE + ciphertext.len());
        result.extend_from_slice(&nonce_bytes);
        result.extend_from_slice(&ciphertext);
        Ok(result)
    }

    /// Decrypt token from nonce + ciphertext
    pub fn decrypt(&self, data: &[u8]) -> Result<String, EncryptionError> {
        if data.len() < NONCE_SIZE {
            return Err(EncryptionError::InvalidData);
        }

        let nonce = Nonce::from_slice(&data[..NONCE_SIZE]);
        let ciphertext = &data[NONCE_SIZE..];

        let plaintext = self.cipher
            .decrypt(nonce, ciphertext)
            .map_err(|_| EncryptionError::DecryptionFailed)?;

        String::from_utf8(plaintext)
            .map_err(|_| EncryptionError::InvalidUtf8)
    }
}
```

#### Token Store

```rust
// kix-auth/src/github_tokens.rs

pub struct GitHubTokenStore {
    db: SqlitePool,
    encryptor: TokenEncryptor,
}

impl GitHubTokenStore {
    pub async fn init_schema(&self) -> Result<(), AuthError> {
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS github_tokens (
                scope TEXT PRIMARY KEY,  -- 'global' or 'project:{uuid}'
                encrypted_token BLOB NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            )
            "#,
        )
        .execute(&self.db)
        .await?;
        Ok(())
    }

    pub async fn store_token(
        &self,
        scope: TokenScope,
        token: &str,
    ) -> Result<(), AuthError> {
        let scope_key = scope.to_key();
        let encrypted = self.encryptor.encrypt(token)?;
        let now = Utc::now().to_rfc3339();

        sqlx::query(
            r#"
            INSERT INTO github_tokens (scope, encrypted_token, created_at, updated_at)
            VALUES (?, ?, ?, ?)
            ON CONFLICT(scope) DO UPDATE SET
                encrypted_token = excluded.encrypted_token,
                updated_at = excluded.updated_at
            "#,
        )
        .bind(&scope_key)
        .bind(&encrypted)
        .bind(&now)
        .bind(&now)
        .execute(&self.db)
        .await?;

        Ok(())
    }

    pub async fn get_token(&self, scope: TokenScope) -> Result<Option<String>, AuthError> {
        let scope_key = scope.to_key();

        let row = sqlx::query_as::<_, (Vec<u8>,)>(
            "SELECT encrypted_token FROM github_tokens WHERE scope = ?"
        )
        .bind(&scope_key)
        .fetch_optional(&self.db)
        .await?;

        match row {
            Some((encrypted,)) => {
                let token = self.encryptor.decrypt(&encrypted)?;
                Ok(Some(token))
            }
            None => Ok(None),
        }
    }

    /// Get token with priority: project-specific → global
    pub async fn resolve_token(&self, project_id: &str) -> Result<Option<String>, AuthError> {
        // Try project-specific first
        if let Some(token) = self.get_token(TokenScope::Project(project_id.to_string())).await? {
            return Ok(Some(token));
        }
        // Fall back to global
        self.get_token(TokenScope::Global).await
    }

    pub async fn delete_token(&self, scope: TokenScope) -> Result<(), AuthError> {
        let scope_key = scope.to_key();
        sqlx::query("DELETE FROM github_tokens WHERE scope = ?")
            .bind(&scope_key)
            .execute(&self.db)
            .await?;
        Ok(())
    }
}

pub enum TokenScope {
    Global,
    Project(String),
}

impl TokenScope {
    fn to_key(&self) -> String {
        match self {
            TokenScope::Global => "global".to_string(),
            TokenScope::Project(id) => format!("project:{}", id),
        }
    }
}
```

#### Security Guarantees

| Aspect | Implementation |
|--------|---------------|
| **Algorithm** | AES-256-GCM (authenticated encryption) |
| **Key Size** | 256 bits (32 bytes) |
| **Nonce** | 96-bit random, unique per encryption |
| **Key Storage** | Environment variable (`KIX_ENCRYPTION_KEY`) |
| **At Rest** | Encrypted in SQLite BLOB field |
| **In Memory** | Decrypted only when needed for API calls |
| **Key Rotation** | Change env var, re-encrypt stored tokens |

#### Environment Setup

```bash
# Generate a secure encryption key (one-time)
openssl rand -hex 32

# Add to your environment
export KIX_ENCRYPTION_KEY="<64-character-hex-string>"

# Or in .env file (ensure .env is in .gitignore!)
KIX_ENCRYPTION_KEY=abc123...
```

#### Dependencies to Add

```toml
# kix-auth/Cargo.toml
[dependencies]
aes-gcm = "0.10"
hex = "0.4"
```

---

## GitHub Projects V2 Integration

GitHub Projects V2 uses **GraphQL API** (not REST) for full board management capabilities.

### Project Templates

Kix supports creating GitHub Projects with pre-configured templates:

| Template | Status Options | Custom Fields | Default View |
|----------|---------------|---------------|--------------|
| **Kanban** | Todo, In Progress, Done | - | Board |
| **Bug Tracking** | Triage, Todo, In Progress, Done | Priority (P0-P3), Severity | Board + Table |
| **Sprint Planning** | Backlog, Sprint, In Progress, Review, Done | Sprint (Iteration), Story Points | Board |
| **Feature Roadmap** | Planning, In Progress, Shipped | Quarter, Team | Roadmap |

### GraphQL Operations

| Operation | Mutation | Purpose |
|-----------|----------|---------|
| Create Project | `createProjectV2` | Create new project board |
| Link to Repo | `linkProjectV2ToRepository` | Associate project with repository |
| Add Field | `createProjectV2Field` | Add Priority, Sprint, etc. |
| Add Issue | `addProjectV2ItemById` | Add issue to project board |
| Add Draft | `addProjectV2DraftIssue` | Create draft issue on board |
| Update Status | `updateProjectV2ItemFieldValue` | Move item between columns |
| Get Project | Query `projectV2` | Retrieve board state |

### GraphQL Client Implementation

```rust
// kix-projects/src/github/graphql_client.rs

const GITHUB_GRAPHQL_ENDPOINT: &str = "https://api.github.com/graphql";

pub struct GitHubGraphQLClient {
    client: reqwest::Client,
}

impl GitHubGraphQLClient {
    async fn execute<T: for<'de> Deserialize<'de>>(
        &self,
        token: &str,
        query: &str,
        variables: serde_json::Value,
    ) -> Result<T, GitHubError> {
        let response = self.client
            .post(GITHUB_GRAPHQL_ENDPOINT)
            .header("Authorization", format!("Bearer {}", token))
            .header("User-Agent", "kix-projects")
            .json(&serde_json::json!({
                "query": query,
                "variables": variables
            }))
            .send()
            .await?;

        // Parse GraphQL response with error handling
        let result: GraphQLResponse<T> = response.json().await?;
        if let Some(errors) = result.errors {
            return Err(GitHubError::GraphQL(errors));
        }
        result.data.ok_or(GitHubError::NoData)
    }
}
```

### Create Project from Template

```rust
// kix-projects/src/github/projects.rs

impl GitHubGraphQLClient {
    /// Create a fully configured project from a template
    pub async fn create_project_from_template(
        &self,
        token: &str,
        owner_id: &str,
        repository_id: &str,
        title: &str,
        template: &ProjectTemplate,
    ) -> Result<GitHubProjectV2, GitHubError> {
        // 1. Create the project
        let project = self.create_project(token, owner_id, title).await?;

        // 2. Link to repository
        self.link_to_repository(token, &project.id, repository_id).await?;

        // 3. Create custom fields from template
        let config = template.get_config();
        for field_def in &config.custom_fields {
            self.create_field(token, &project.id, field_def).await?;
        }

        Ok(project)
    }

    /// Add issue to project and set initial status
    pub async fn add_issue_to_project(
        &self,
        token: &str,
        project_id: &str,
        issue_node_id: &str,
        status_field_id: &str,
        status_option_id: &str,
    ) -> Result<String, GitHubError> {
        // Add issue to project
        let item_id = self.add_item(token, project_id, issue_node_id).await?;

        // Set initial status
        self.update_item_field(
            token,
            project_id,
            &item_id,
            status_field_id,
            ProjectFieldValue::SingleSelect(status_option_id.to_string()),
        ).await?;

        Ok(item_id)
    }
}
```

### Token Scopes Required

```bash
# Personal Access Token needs these scopes:
project              # Read/write access to projects
repo                 # For creating issues and linking
read:org             # For organization projects
```

---

### Sync Algorithm

```
1. Get project GitHub config
2. Resolve token (project → global → error)
3. Fetch issues from GitHub API (with pagination)
4. For each issue:
   a. Check if exists (by github_number)
   b. If exists: update fields, preserve local fields (priority, etc.)
   c. If new: create with github_number set
5. Update last_synced_at timestamp
6. Return sync summary (created, updated, errors)
```

### GitHub API Client

```rust
pub struct GitHubClient {
    http: reqwest::Client,
}

impl GitHubClient {
    pub async fn list_issues(
        &self,
        token: &str,
        owner: &str,
        repo: &str,
        state: Option<IssueState>,
        labels: &[String],
        page: u32,
    ) -> Result<Vec<GitHubIssue>, GitHubError>;
}
```

---

## Files to Create

| File | Purpose |
|------|---------|
| `server/crates/kix-projects/Cargo.toml` | Crate manifest |
| `server/crates/kix-projects/src/lib.rs` | Public exports |
| `server/crates/kix-projects/src/project.rs` | ProjectStore implementation |
| `server/crates/kix-projects/src/issue.rs` | IssueStore implementation |
| `server/crates/kix-projects/src/knowledge.rs` | ProjectEntryStore implementation |
| `server/crates/kix-projects/src/search.rs` | ProjectSearchService |
| `server/crates/kix-projects/src/error.rs` | ProjectError type |
| `server/crates/kix-projects/src/events.rs` | ProjectEventBus for real-time updates |
| `server/crates/kix-projects/src/github/mod.rs` | GitHub module |
| `server/crates/kix-projects/src/github/client.rs` | GitHub API client |
| `server/crates/kix-projects/src/github/sync.rs` | Sync service |
| `server/crates/kix-projects/src/github/models.rs` | GitHub API types |
| `server/crates/kix-api/src/project_routes.rs` | REST API routes |
| `client/src/pages/projects/*.tsx` | React pages |
| `client/src/api/projectsClient.ts` | API client |

## Files to Modify

| File | Changes |
|------|---------|
| `server/Cargo.toml` | Add kix-projects to workspace |
| `server/crates/kix-store/src/schema.rs` | Add project/issue schemas |
| `server/crates/kix-store/src/store.rs` | Initialize new tables |
| `server/crates/kix-mcp/Cargo.toml` | Add kix-projects dependency |
| `server/crates/kix-mcp/src/server.rs` | Add 15 new MCP tools |
| `server/crates/kix-api/Cargo.toml` | Add kix-projects dependency |
| `server/crates/kix-api/src/lib.rs` | Export project routes |
| `server/crates/kix-cli/Cargo.toml` | Add kix-projects dependency |
| `server/crates/kix-cli/src/main.rs` | Wire project routes |
| `server/crates/kix-auth/src/lib.rs` | Add token storage |
| `client/src/App.tsx` | Add project routes, dynamic nav |
| `client/src/api/client.ts` | Export projects API |

---

## Testing Plan

### Unit Tests

```rust
// project.rs
#[test] fn test_create_project() { ... }
#[test] fn test_get_project_by_name() { ... }
#[test] fn test_get_project_by_id() { ... }
#[test] fn test_update_project() { ... }
#[test] fn test_delete_project() { ... }
#[test] fn test_slug_generation() { ... }
#[test] fn test_unique_name_constraint() { ... }

// issue.rs
#[test] fn test_create_issue_auto_number() { ... }
#[test] fn test_update_issue_state() { ... }
#[test] fn test_filter_issues_by_state() { ... }
#[test] fn test_filter_issues_by_labels() { ... }

// github/sync.rs
#[test] fn test_issue_upsert_new() { ... }
#[test] fn test_issue_upsert_update() { ... }
#[test] fn test_preserve_local_fields() { ... }
```

### Integration Tests

```bash
# Test project lifecycle via MCP
claude mcp call kix create_project '{"name": "Test Project"}'
claude mcp call kix create_issue '{"project": "Test Project", "title": "First issue"}'
claude mcp call kix search_project '{"project": "Test Project", "query": "first"}'

# Test via REST API
curl -X POST http://localhost:3001/api/projects -d '{"name": "API Test"}'
curl http://localhost:3001/api/projects
curl http://localhost:3001/api/projects/api-test/issues
```

---

## Implementation Checklist

### Phase 1: Crate Setup & Data Models
- [ ] Create `server/crates/kix-projects/` directory structure
- [ ] Create `Cargo.toml` with dependencies
- [ ] Add `kix-projects` to workspace in `server/Cargo.toml`
- [ ] Define `Project` struct in `project.rs`
- [ ] Define `Issue` struct in `issue.rs`
- [ ] Define `ProjectEntry` struct in `knowledge.rs`
- [ ] Define `GitHubConfig` and related types
- [ ] Define `IssueState`, `IssueSource` enums
- [ ] Create `ProjectError` type in `error.rs`
- [ ] Create `lib.rs` with public exports

### Phase 2: LanceDB Schema & Storage
- [ ] Add `project_schema()` to `kix-store/src/schema.rs`
- [ ] Add `issue_schema()` to `kix-store/src/schema.rs`
- [ ] Add `project_entry_schema()` to `kix-store/src/schema.rs`
- [ ] Update `KixStore::init_tables()` to create new tables
- [ ] Implement `ProjectStore` with CRUD operations
  - [ ] `create_project()` with slug generation
  - [ ] `get_project()` (by id or name)
  - [ ] `list_projects()` with archive filter
  - [ ] `update_project()`
  - [ ] `delete_project()`
- [ ] Implement `IssueStore` with CRUD operations
  - [ ] `create_issue()` with auto-numbering
  - [ ] `get_issue()` (by number or id)
  - [ ] `list_issues()` with filters (state, labels)
  - [ ] `update_issue()`
  - [ ] `delete_issue()`
  - [ ] `get_next_issue_number()`
- [ ] Implement `ProjectEntryStore`
  - [ ] `link_entry()`
  - [ ] `unlink_entry()`
  - [ ] `list_entries()`
- [ ] Add unit tests for all storage operations

### Phase 3: GitHub Integration (REST + GraphQL)
- [ ] Create `github/models.rs` with GitHub API types
  - [ ] `GitHubIssue` response type
  - [ ] `GitHubLabel` type
  - [ ] `GitHubUser` type (for assignees)
  - [ ] `GitHubProjectV2` response type
  - [ ] `GitHubProjectItem` type
- [ ] Create `github/rest_client.rs` with REST API client
  - [ ] `GitHubRestClient::new()`
  - [ ] `list_issues()` with pagination
  - [ ] `create_issue()`, `update_issue()`
  - [ ] `get_repository()` - get repo ID for linking
  - [ ] Handle rate limiting
  - [ ] Parse API errors
- [ ] Create `github/graphql_client.rs` with GraphQL client
  - [ ] `GitHubGraphQLClient::new()`
  - [ ] `execute()` - generic GraphQL query execution
  - [ ] Error handling for GraphQL responses
- [ ] Create `github/projects.rs` with Projects V2 operations
  - [ ] `create_project()` - createProjectV2 mutation
  - [ ] `link_to_repository()` - linkProjectV2ToRepository mutation
  - [ ] `create_field()` - createProjectV2Field mutation
  - [ ] `add_item()` - addProjectV2ItemById mutation
  - [ ] `add_draft_issue()` - addProjectV2DraftIssue mutation
  - [ ] `update_item_field()` - updateProjectV2ItemFieldValue mutation
  - [ ] `get_project()` - query project with fields and items
  - [ ] `create_project_from_template()` - orchestrates full setup
- [ ] Create `templates.rs` with project templates
  - [ ] `ProjectTemplate` enum (Kanban, BugTracking, Sprint, Roadmap)
  - [ ] `TemplateConfig` with status options and custom fields
  - [ ] `get_config()` implementation for each template
- [ ] **Secure Token Storage** in `kix-auth`:
  - [ ] Add `aes-gcm` and `hex` dependencies to `kix-auth/Cargo.toml`
  - [ ] Create `kix-auth/src/encryption.rs`:
    - [ ] `TokenEncryptor` struct with AES-256-GCM cipher
    - [ ] `from_env()` - load key from `KIX_ENCRYPTION_KEY` or generate
    - [ ] `encrypt()` - returns nonce + ciphertext
    - [ ] `decrypt()` - validates and decrypts
  - [ ] Create `kix-auth/src/github_tokens.rs`:
    - [ ] `GitHubTokenStore` with encryptor and SQLite pool
    - [ ] `init_schema()` - create `github_tokens` table
    - [ ] `store_token()` - encrypt and store
    - [ ] `get_token()` - retrieve and decrypt
    - [ ] `resolve_token()` - project → global priority
    - [ ] `delete_token()` - secure deletion
  - [ ] `TokenScope` enum (Global, Project)
  - [ ] Export from `kix-auth/src/lib.rs`
  - [ ] Unit tests for encryption roundtrip
  - [ ] Unit tests for token store operations
- [ ] Create `github/sync.rs` with sync service
  - [ ] `GitHubSyncService::sync_project()`
  - [ ] Issue upsert logic (match by github_number)
  - [ ] Preserve local-only fields during update
  - [ ] Update `last_synced_at` on success
  - [ ] Error handling and reporting
- [ ] Add unit tests for sync logic
- [ ] Add integration test with mock GitHub API

### Phase 4: Project-Scoped Search
- [ ] Create `search.rs` with `ProjectSearchService`
  - [ ] `search()` method for project scope
  - [ ] Filter by project entries
  - [ ] Include issues in search (FTS on title+body)
  - [ ] Combine and rank results
- [ ] Add issue embedding during create/update
- [ ] Add unit tests for search

### Phase 5: MCP Tools (25+ tools)
- [ ] Add dependency on `kix-projects` in `kix-mcp/Cargo.toml`
- [ ] Add `ProjectStore` to `KixMcpServer` state
- [ ] **Project Management tools**:
  - [ ] `create_project` (requires github_repo parameter)
  - [ ] `list_projects`
  - [ ] `get_project`
  - [ ] `update_project`
  - [ ] `delete_project`
- [ ] **Issue Management tools**:
  - [ ] `create_issue`
  - [ ] `list_issues`
  - [ ] `get_issue`
  - [ ] `update_issue`
  - [ ] `delete_issue`
- [ ] **GitHub Projects V2 tools**:
  - [ ] `create_github_project` (with template parameter)
  - [ ] `get_github_project`
  - [ ] `add_issue_to_project`
  - [ ] `update_project_item`
  - [ ] `sync_github_project`
  - [ ] `create_draft_issue`
- [ ] **AI Planning tools**:
  - [ ] `plan_project` - returns knowledge context for AI planning
  - [ ] `suggest_tasks` - get task suggestions
  - [ ] `get_project_context` - retrieve relevant knowledge
  - [ ] `breakdown_task` - break task into subtasks
- [ ] **GitHub Token tools**:
  - [ ] `set_github_token`
  - [ ] `delete_github_token`
  - [ ] `sync_github_issues`
- [ ] **Knowledge tools**:
  - [ ] `add_entry_to_project`
  - [ ] `remove_entry_from_project`
  - [ ] `list_project_entries`
- [ ] **Search tool**:
  - [ ] `search_project`
- [ ] Test all tools via Claude Code

### Phase 6: Real-time Events (MCP → UI)
- [ ] Create `kix-projects/src/events.rs`
  - [ ] Define `ProjectEvent` struct
  - [ ] Define event type constants
  - [ ] Implement `ProjectEventBus` with broadcast channel
  - [ ] `emit()` method for publishing events
  - [ ] `subscribe()` method for consumers
- [ ] Add `event_bus: Arc<ProjectEventBus>` to shared state
- [ ] Emit events from MCP tools after successful operations:
  - [ ] `create_project` → `project.created`
  - [ ] `update_project` → `project.updated`
  - [ ] `delete_project` → `project.deleted`
  - [ ] `create_issue` → `issue.created`
  - [ ] `update_issue` → `issue.updated`
  - [ ] `delete_issue` → `issue.deleted`
  - [ ] `add_entry_to_project` → `entry.linked`
  - [ ] `remove_entry_from_project` → `entry.unlinked`
  - [ ] `sync_github_issues` → `github.sync.started`, `github.sync.completed`/`failed`
- [ ] Add SSE endpoint `/api/projects/events`
- [ ] Add SSE endpoint `/api/projects/:id/events` (filtered)
- [ ] Test events are emitted and received

### Phase 7: REST API
- [ ] Add dependency on `kix-projects` in `kix-api/Cargo.toml`
- [ ] Create `project_routes.rs`
- [ ] Implement project endpoints:
  - [ ] `GET /api/projects`
  - [ ] `POST /api/projects`
  - [ ] `GET /api/projects/:id`
  - [ ] `PUT /api/projects/:id`
  - [ ] `DELETE /api/projects/:id`
- [ ] Implement issue endpoints:
  - [ ] `GET /api/projects/:id/issues`
  - [ ] `POST /api/projects/:id/issues`
  - [ ] `GET /api/projects/:id/issues/:number`
  - [ ] `PUT /api/projects/:id/issues/:number`
  - [ ] `DELETE /api/projects/:id/issues/:number`
- [ ] Implement knowledge endpoints:
  - [ ] `GET /api/projects/:id/entries`
  - [ ] `POST /api/projects/:id/entries`
  - [ ] `DELETE /api/projects/:id/entries/:entry_id`
- [ ] Implement GitHub endpoints:
  - [ ] `POST /api/projects/:id/github/token`
  - [ ] `DELETE /api/projects/:id/github/token`
  - [ ] `POST /api/projects/:id/github/sync`
  - [ ] `GET /api/projects/:id/github/status`
  - [ ] `POST /api/github/token` (global)
- [ ] Implement search endpoint:
  - [ ] `GET /api/projects/:id/search`
- [ ] Wire routes in `kix-cli/src/main.rs`
- [ ] Test all endpoints with curl

### Phase 8: React UI (use frontend-design agent)
- [ ] Create `client/src/api/projectsClient.ts`
- [ ] Update `client/src/App.tsx`:
  - [ ] Add routes for project pages
  - [ ] Make Projects nav section dynamic
- [ ] Create project pages:
  - [ ] `ProjectList.tsx` - project grid with create modal
  - [ ] `ProjectDetail.tsx` - tabbed view
  - [ ] `ProjectSettings.tsx` - GitHub config, delete
- [ ] Create issue pages:
  - [ ] `IssueList.tsx` - filterable issue list
  - [ ] `IssueDetail.tsx` - full issue view
  - [ ] `IssueForm.tsx` - create/edit modal
- [ ] Create knowledge page:
  - [ ] `KnowledgeList.tsx` - linked entries
- [ ] Create components:
  - [ ] `ProjectCard.tsx`
  - [ ] `IssueCard.tsx`
  - [ ] `GitHubSyncStatus.tsx`
- [ ] Create real-time hooks:
  - [ ] `useProjectEvents.ts` - SSE subscription hook
  - [ ] Integrate with React Query for auto-invalidation
- [ ] Test real-time updates:
  - [ ] Create issue via MCP, verify UI updates
  - [ ] Sync GitHub via MCP, verify UI updates
- [ ] Test all UI flows manually

### Phase 9: Documentation & Polish
- [ ] Update README with project management features
- [ ] Add project management to MCP docs page
- [ ] Add example usage for Claude Code
- [ ] Add example usage for Claude Desktop
- [ ] End-to-end testing:
  - [ ] Create project via UI
  - [ ] Set GitHub token
  - [ ] Sync issues from real repo
  - [ ] Create local issue
  - [ ] Link knowledge entry
  - [ ] Search within project
  - [ ] Test MCP tools via Claude

---

## Future Enhancements

1. **Project Templates** - Pre-configured setups for common use cases
2. **Project Dashboard** - Metrics, activity feed, quick stats
3. **Issue-to-Knowledge Links** - Connect issues to relevant entries
4. **Knowledge Graph per Project** - Visualize relationships
5. **Activity Feed** - Track changes and sync history
6. **Export/Import** - Backup and share projects
7. **Pull Requests** - Extend GitHub sync to PRs
8. **Comments** - Sync and display issue comments
9. **Multi-user** - Project members and permissions
10. **Webhooks** - Real-time GitHub sync (requires public URL)
11. **GitLab/Bitbucket** - Additional Git provider integrations
12. **Jira Integration** - Enterprise issue tracking
