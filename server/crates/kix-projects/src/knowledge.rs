//! Project knowledge linking - connect entries from the knowledge base to projects.

use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Links an entry from the knowledge base to a project.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ProjectEntry {
    /// Link ID (UUID)
    pub id: String,

    /// Project ID
    pub project_id: String,

    /// Entry ID from the knowledge base
    pub entry_id: String,

    /// Optional notes about this entry in project context
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,

    /// Custom tags for this entry within the project
    #[serde(default)]
    pub project_tags: Vec<String>,

    /// Added timestamp
    pub added_at: DateTime<Utc>,
}

impl ProjectEntry {
    /// Create a new project entry link.
    pub fn new(project_id: String, entry_id: String) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            project_id,
            entry_id,
            notes: None,
            project_tags: Vec::new(),
            added_at: Utc::now(),
        }
    }

    /// Create with notes.
    pub fn with_notes(mut self, notes: String) -> Self {
        self.notes = Some(notes);
        self
    }

    /// Create with tags.
    pub fn with_tags(mut self, tags: Vec<String>) -> Self {
        self.project_tags = tags;
        self
    }
}

/// Parameters for linking an entry to a project.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct LinkEntryParams {
    /// Entry ID to link
    pub entry_id: String,

    /// Optional notes about the entry
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,

    /// Optional tags for this entry in the project
    #[serde(default)]
    pub tags: Vec<String>,
}

/// A knowledge reference returned from planning operations.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct KnowledgeReference {
    /// Entry ID
    pub entry_id: String,

    /// Entry title
    pub title: String,

    /// Relevance score (0-1)
    pub relevance_score: f32,

    /// Snippet from the entry content
    pub snippet: String,

    /// Entry source URL or path
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_project_entry_new() {
        let entry = ProjectEntry::new(
            "proj_123".to_string(),
            "entry_456".to_string(),
        );

        assert!(!entry.id.is_empty());
        assert_eq!(entry.project_id, "proj_123");
        assert_eq!(entry.entry_id, "entry_456");
        assert!(entry.notes.is_none());
        assert!(entry.project_tags.is_empty());
    }

    #[test]
    fn test_project_entry_with_notes() {
        let entry = ProjectEntry::new("proj".to_string(), "entry".to_string())
            .with_notes("Important reference".to_string());

        assert_eq!(entry.notes, Some("Important reference".to_string()));
    }

    #[test]
    fn test_project_entry_with_tags() {
        let entry = ProjectEntry::new("proj".to_string(), "entry".to_string())
            .with_tags(vec!["architecture".to_string(), "reference".to_string()]);

        assert_eq!(entry.project_tags.len(), 2);
        assert!(entry.project_tags.contains(&"architecture".to_string()));
    }
}
