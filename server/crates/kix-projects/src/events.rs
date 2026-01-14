//! Real-time events for project operations.
//!
//! This module provides a broadcast-based event bus for publishing project events
//! that can be consumed by SSE endpoints to keep the UI in sync.

use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::broadcast;
use chrono::{DateTime, Utc};

/// Maximum number of events to buffer in the channel.
const EVENT_CHANNEL_CAPACITY: usize = 1024;

/// Project event types.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProjectEventType {
    /// Project was created
    ProjectCreated,
    /// Project was updated
    ProjectUpdated,
    /// Project was deleted
    ProjectDeleted,
    /// Project was archived
    ProjectArchived,
    /// Project was unarchived
    ProjectUnarchived,
    /// Issue was created
    IssueCreated,
    /// Issue was updated
    IssueUpdated,
    /// Issue was deleted
    IssueDeleted,
    /// Issue was closed
    IssueClosed,
    /// Issue was reopened
    IssueReopened,
    /// Knowledge entry was linked
    EntryLinked,
    /// Knowledge entry was unlinked
    EntryUnlinked,
    /// GitHub sync started
    GitHubSyncStarted,
    /// GitHub sync completed successfully
    GitHubSyncCompleted,
    /// GitHub sync failed
    GitHubSyncFailed,
    /// GitHub project created
    GitHubProjectCreated,
    /// Issue added to GitHub project
    IssueAddedToProject,
}

impl std::fmt::Display for ProjectEventType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ProjectCreated => write!(f, "project.created"),
            Self::ProjectUpdated => write!(f, "project.updated"),
            Self::ProjectDeleted => write!(f, "project.deleted"),
            Self::ProjectArchived => write!(f, "project.archived"),
            Self::ProjectUnarchived => write!(f, "project.unarchived"),
            Self::IssueCreated => write!(f, "issue.created"),
            Self::IssueUpdated => write!(f, "issue.updated"),
            Self::IssueDeleted => write!(f, "issue.deleted"),
            Self::IssueClosed => write!(f, "issue.closed"),
            Self::IssueReopened => write!(f, "issue.reopened"),
            Self::EntryLinked => write!(f, "entry.linked"),
            Self::EntryUnlinked => write!(f, "entry.unlinked"),
            Self::GitHubSyncStarted => write!(f, "github.sync.started"),
            Self::GitHubSyncCompleted => write!(f, "github.sync.completed"),
            Self::GitHubSyncFailed => write!(f, "github.sync.failed"),
            Self::GitHubProjectCreated => write!(f, "github.project.created"),
            Self::IssueAddedToProject => write!(f, "issue.added_to_project"),
        }
    }
}

/// A project event with metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectEvent {
    /// Event ID (UUID)
    pub id: String,

    /// Event type
    pub event_type: ProjectEventType,

    /// Project ID this event relates to
    pub project_id: String,

    /// Optional resource ID (issue ID, entry ID, etc.)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resource_id: Option<String>,

    /// Additional event data
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,

    /// Event timestamp
    pub timestamp: DateTime<Utc>,
}

impl ProjectEvent {
    /// Create a new project event.
    pub fn new(event_type: ProjectEventType, project_id: impl Into<String>) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            event_type,
            project_id: project_id.into(),
            resource_id: None,
            data: None,
            timestamp: Utc::now(),
        }
    }

    /// Add a resource ID to the event.
    pub fn with_resource(mut self, resource_id: impl Into<String>) -> Self {
        self.resource_id = Some(resource_id.into());
        self
    }

    /// Add custom data to the event.
    pub fn with_data(mut self, data: serde_json::Value) -> Self {
        self.data = Some(data);
        self
    }

    /// Format the event for SSE.
    pub fn to_sse(&self) -> String {
        let data = serde_json::to_string(self).unwrap_or_else(|_| "{}".to_string());
        format!("event: {}\ndata: {}\nid: {}\n\n", self.event_type, data, self.id)
    }
}

/// Event bus for publishing and subscribing to project events.
#[derive(Clone)]
pub struct ProjectEventBus {
    sender: broadcast::Sender<ProjectEvent>,
}

impl Default for ProjectEventBus {
    fn default() -> Self {
        Self::new()
    }
}

impl ProjectEventBus {
    /// Create a new event bus.
    pub fn new() -> Self {
        let (sender, _) = broadcast::channel(EVENT_CHANNEL_CAPACITY);
        Self { sender }
    }

    /// Publish an event to all subscribers.
    pub fn publish(&self, event: ProjectEvent) {
        // Ignore send errors (no subscribers)
        let _ = self.sender.send(event);
    }

    /// Publish a simple event without additional data.
    pub fn emit(&self, event_type: ProjectEventType, project_id: &str) {
        self.publish(ProjectEvent::new(event_type, project_id));
    }

    /// Publish an event with a resource ID.
    pub fn emit_with_resource(
        &self,
        event_type: ProjectEventType,
        project_id: &str,
        resource_id: &str,
    ) {
        self.publish(
            ProjectEvent::new(event_type, project_id).with_resource(resource_id),
        );
    }

    /// Subscribe to events.
    pub fn subscribe(&self) -> broadcast::Receiver<ProjectEvent> {
        self.sender.subscribe()
    }

    /// Get the number of active subscribers.
    pub fn subscriber_count(&self) -> usize {
        self.sender.receiver_count()
    }
}

/// Thread-safe shared event bus.
pub type SharedEventBus = Arc<ProjectEventBus>;

/// Create a new shared event bus.
pub fn create_event_bus() -> SharedEventBus {
    Arc::new(ProjectEventBus::new())
}

// ============================================================================
// Convenience Event Builders
// ============================================================================

impl ProjectEventBus {
    /// Emit project created event.
    pub fn project_created(&self, project_id: &str, name: &str) {
        self.publish(
            ProjectEvent::new(ProjectEventType::ProjectCreated, project_id)
                .with_data(serde_json::json!({ "name": name })),
        );
    }

    /// Emit project updated event.
    pub fn project_updated(&self, project_id: &str) {
        self.emit(ProjectEventType::ProjectUpdated, project_id);
    }

    /// Emit project deleted event.
    pub fn project_deleted(&self, project_id: &str) {
        self.emit(ProjectEventType::ProjectDeleted, project_id);
    }

    /// Emit project archived event.
    pub fn project_archived(&self, project_id: &str) {
        self.emit(ProjectEventType::ProjectArchived, project_id);
    }

    /// Emit project unarchived event.
    pub fn project_unarchived(&self, project_id: &str) {
        self.emit(ProjectEventType::ProjectUnarchived, project_id);
    }

    /// Emit issue created event.
    pub fn issue_created(&self, project_id: &str, issue_id: &str, title: &str) {
        self.publish(
            ProjectEvent::new(ProjectEventType::IssueCreated, project_id)
                .with_resource(issue_id)
                .with_data(serde_json::json!({ "title": title })),
        );
    }

    /// Emit issue updated event.
    pub fn issue_updated(&self, project_id: &str, issue_id: &str) {
        self.emit_with_resource(ProjectEventType::IssueUpdated, project_id, issue_id);
    }

    /// Emit issue deleted event.
    pub fn issue_deleted(&self, project_id: &str, issue_id: &str) {
        self.emit_with_resource(ProjectEventType::IssueDeleted, project_id, issue_id);
    }

    /// Emit issue closed event.
    pub fn issue_closed(&self, project_id: &str, issue_id: &str) {
        self.emit_with_resource(ProjectEventType::IssueClosed, project_id, issue_id);
    }

    /// Emit issue reopened event.
    pub fn issue_reopened(&self, project_id: &str, issue_id: &str) {
        self.emit_with_resource(ProjectEventType::IssueReopened, project_id, issue_id);
    }

    /// Emit entry linked event.
    pub fn entry_linked(&self, project_id: &str, entry_id: &str, link_id: &str) {
        self.publish(
            ProjectEvent::new(ProjectEventType::EntryLinked, project_id)
                .with_resource(link_id)
                .with_data(serde_json::json!({ "entry_id": entry_id })),
        );
    }

    /// Emit entry unlinked event.
    pub fn entry_unlinked(&self, project_id: &str, entry_id: &str) {
        self.emit_with_resource(ProjectEventType::EntryUnlinked, project_id, entry_id);
    }

    /// Emit GitHub sync started event.
    pub fn github_sync_started(&self, project_id: &str) {
        self.emit(ProjectEventType::GitHubSyncStarted, project_id);
    }

    /// Emit GitHub sync completed event.
    pub fn github_sync_completed(&self, project_id: &str, created: usize, updated: usize) {
        self.publish(
            ProjectEvent::new(ProjectEventType::GitHubSyncCompleted, project_id)
                .with_data(serde_json::json!({
                    "created": created,
                    "updated": updated,
                })),
        );
    }

    /// Emit GitHub sync failed event.
    pub fn github_sync_failed(&self, project_id: &str, error: &str) {
        self.publish(
            ProjectEvent::new(ProjectEventType::GitHubSyncFailed, project_id)
                .with_data(serde_json::json!({ "error": error })),
        );
    }

    /// Emit GitHub project created event.
    pub fn github_project_created(&self, project_id: &str, gh_project_id: &str, title: &str) {
        self.publish(
            ProjectEvent::new(ProjectEventType::GitHubProjectCreated, project_id)
                .with_resource(gh_project_id)
                .with_data(serde_json::json!({ "title": title })),
        );
    }

    /// Emit issue added to GitHub project event.
    pub fn issue_added_to_project(&self, project_id: &str, issue_id: &str, gh_project_id: &str) {
        self.publish(
            ProjectEvent::new(ProjectEventType::IssueAddedToProject, project_id)
                .with_resource(issue_id)
                .with_data(serde_json::json!({ "github_project_id": gh_project_id })),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_type_display() {
        assert_eq!(ProjectEventType::ProjectCreated.to_string(), "project.created");
        assert_eq!(ProjectEventType::IssueUpdated.to_string(), "issue.updated");
        assert_eq!(ProjectEventType::GitHubSyncCompleted.to_string(), "github.sync.completed");
    }

    #[test]
    fn test_event_creation() {
        let event = ProjectEvent::new(ProjectEventType::ProjectCreated, "proj-123")
            .with_resource("res-456")
            .with_data(serde_json::json!({ "name": "Test" }));

        assert!(!event.id.is_empty());
        assert_eq!(event.event_type, ProjectEventType::ProjectCreated);
        assert_eq!(event.project_id, "proj-123");
        assert_eq!(event.resource_id, Some("res-456".to_string()));
        assert!(event.data.is_some());
    }

    #[test]
    fn test_event_sse_format() {
        let event = ProjectEvent::new(ProjectEventType::IssueCreated, "proj-123");
        let sse = event.to_sse();

        assert!(sse.starts_with("event: issue.created"));
        assert!(sse.contains("data: "));
        assert!(sse.contains(&format!("id: {}", event.id)));
        assert!(sse.ends_with("\n\n"));
    }

    #[tokio::test]
    async fn test_event_bus_publish_subscribe() {
        let bus = ProjectEventBus::new();
        let mut receiver = bus.subscribe();

        bus.project_created("proj-123", "Test Project");

        let event = receiver.recv().await.unwrap();
        assert_eq!(event.event_type, ProjectEventType::ProjectCreated);
        assert_eq!(event.project_id, "proj-123");
    }

    #[tokio::test]
    async fn test_event_bus_multiple_subscribers() {
        let bus = ProjectEventBus::new();
        let mut rx1 = bus.subscribe();
        let mut rx2 = bus.subscribe();

        assert_eq!(bus.subscriber_count(), 2);

        bus.issue_created("proj-123", "issue-1", "Test Issue");

        let e1 = rx1.recv().await.unwrap();
        let e2 = rx2.recv().await.unwrap();

        assert_eq!(e1.id, e2.id);
        assert_eq!(e1.event_type, ProjectEventType::IssueCreated);
    }

    #[test]
    fn test_shared_event_bus() {
        let bus = create_event_bus();
        let bus_clone = Arc::clone(&bus);

        bus.emit(ProjectEventType::ProjectUpdated, "proj-123");
        bus_clone.emit(ProjectEventType::ProjectDeleted, "proj-456");

        // Both references work
        assert_eq!(bus.subscriber_count(), 0);
    }
}
