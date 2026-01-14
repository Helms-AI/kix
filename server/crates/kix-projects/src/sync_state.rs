//! Issue sync state management for bidirectional GitHub synchronization

use crate::issue::Issue;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

/// Represents the synchronization state of an issue
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IssueSyncState {
    pub issue_id: String,
    pub content_hash: String,
    pub last_sync_hash: String,
    pub local_version: i64,
    pub github_version: Option<i64>,
    pub github_etag: Option<String>,
    pub sync_status: SyncStatus,
    pub last_sync_at: DateTime<Utc>,
    pub last_sync_direction: Option<SyncDirection>,
    pub last_sync_result: Option<SyncResult>,
    pub conflict_count: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Sync status for an issue
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SyncStatus {
    /// Issue is synchronized with GitHub
    Synced,
    /// Local version is ahead of GitHub
    LocalAhead,
    /// GitHub version is ahead of local
    GitHubAhead,
    /// Both local and GitHub have changes
    Conflict,
    /// New issue, not yet synced
    Pending,
}

/// Direction of last sync operation
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SyncDirection {
    Push,
    Pull,
    Merge,
}

/// Result of sync operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncResult {
    pub pushed_fields: Vec<String>,
    pub pulled_fields: Vec<String>,
    pub merged_fields: Vec<String>,
    pub strategy_used: String,
}

/// Type of conflict detected
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ConflictType {
    /// No conflict, issues are identical
    None,
    /// Only local has changes
    LocalOnly,
    /// Only GitHub has changes
    RemoteOnly,
    /// Both have changes
    BothChanged,
}

/// Merge strategy for conflict resolution
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MergeStrategy {
    /// Local version always wins
    LocalWins,
    /// Remote version always wins
    RemoteWins,
    /// Smart merge with local fallback
    SmartMerge,
    /// Require user intervention
    Interactive,
}

/// Result of a merge operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MergeResult {
    pub issue: Issue,
    pub auto_resolved_fields: Vec<String>,
    pub strategy_used: String,
    pub conflicts_resolved: usize,
}

/// Content hashing utilities for change detection
pub struct ContentHasher;

impl ContentHasher {
    /// Calculate a canonical hash for an issue
    /// This hash is used to detect changes between local and GitHub versions
    pub fn calculate_hash(issue: &Issue) -> String {
        // Create canonical representation
        // Sort fields deterministically to ensure consistent hashing
        let canonical = Self::canonical_representation(issue);

        // Calculate SHA-256 hash
        let mut hasher = Sha256::new();
        hasher.update(canonical.as_bytes());
        let result = hasher.finalize();

        // Return hex string
        hex::encode(result)
    }

    /// Create canonical representation of issue for hashing
    /// Excludes metadata fields that don't represent content changes
    fn canonical_representation(issue: &Issue) -> String {
        // Sort labels and assignees for consistent ordering
        let labels = Self::sorted_set(&issue.labels);
        let assignees = Self::sorted_set(&issue.assignees);

        // Normalize text fields
        let title = Self::normalize_text(&issue.title);
        let body = issue.body.as_deref().unwrap_or("").trim();
        let body_normalized = Self::normalize_text(body);

        // Create canonical string
        // Format: title|body|state|labels|assignees|priority
        let state_str = match issue.state {
            crate::project::IssueState::Open => "open",
            crate::project::IssueState::Closed => "closed",
        };
        format!(
            "{}|{}|{}|{}|{}|{}",
            title,
            body_normalized,
            state_str,
            labels.join(","),
            assignees.join(","),
            issue.priority.unwrap_or(0)
        )
    }

    /// Normalize text for consistent hashing
    fn normalize_text(text: &str) -> String {
        // Remove leading/trailing whitespace
        // Normalize line endings
        // Collapse multiple spaces
        text.trim()
            .lines()
            .map(|line| line.trim())
            .filter(|line| !line.is_empty())
            .collect::<Vec<_>>()
            .join("\n")
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// Sort a collection to ensure consistent ordering
    fn sorted_set(items: &[String]) -> Vec<String> {
        let set: BTreeSet<String> = items.iter().cloned().collect();
        set.into_iter().collect()
    }

    /// Check if two hashes represent the same content
    pub fn hashes_equal(hash1: &str, hash2: &str) -> bool {
        hash1 == hash2
    }
}

/// Conflict detection logic
pub struct ConflictDetector;

impl ConflictDetector {
    /// Detect type of conflict between local and GitHub versions
    pub fn detect_conflict(
        local: &Issue,
        github: &Issue,
        sync_state: &IssueSyncState,
    ) -> ConflictType {
        let local_hash = ContentHasher::calculate_hash(local);
        let github_hash = ContentHasher::calculate_hash(github);

        // No changes if hashes are identical
        if ContentHasher::hashes_equal(&local_hash, &github_hash) {
            return ConflictType::None;
        }

        // Check if only GitHub changed
        if ContentHasher::hashes_equal(&local_hash, &sync_state.last_sync_hash) {
            return ConflictType::RemoteOnly;
        }

        // Check if only local changed
        if ContentHasher::hashes_equal(&github_hash, &sync_state.last_sync_hash) {
            return ConflictType::LocalOnly;
        }

        // Both changed since last sync
        ConflictType::BothChanged
    }

    /// Determine which fields have changed
    pub fn detect_field_changes(
        old: &Issue,
        new: &Issue,
    ) -> Vec<String> {
        let mut changed_fields = Vec::new();

        if old.title != new.title {
            changed_fields.push("title".to_string());
        }
        if old.body != new.body {
            changed_fields.push("body".to_string());
        }
        if old.state != new.state {
            changed_fields.push("state".to_string());
        }
        if old.labels != new.labels {
            changed_fields.push("labels".to_string());
        }
        if old.assignees != new.assignees {
            changed_fields.push("assignees".to_string());
        }
        if old.priority != new.priority {
            changed_fields.push("priority".to_string());
        }

        changed_fields
    }
}

/// Smart merge utilities
pub struct SmartMerger;

impl SmartMerger {
    /// Merge two arrays (labels, assignees) using union strategy
    pub fn merge_arrays(local: &[String], remote: &[String]) -> Vec<String> {
        let mut merged = BTreeSet::new();

        // Add all local items (preserving local priority)
        for item in local {
            merged.insert(item.clone());
        }

        // Add remote items not in local
        for item in remote {
            merged.insert(item.clone());
        }

        merged.into_iter().collect()
    }

    /// Merge state field with special logic (closed wins)
    pub fn merge_state(local: &str, remote: &str) -> String {
        // If either is closed, the issue should be closed
        if local == "closed" || remote == "closed" {
            "closed".to_string()
        } else {
            // Otherwise prefer local state
            local.to_string()
        }
    }

    /// Attempt to merge text fields using three-way merge
    /// For now, returns local version (will be enhanced with proper three-way merge)
    pub fn merge_text(local: &str, _remote: &str, _base: &str) -> Result<String, String> {
        // TODO: Implement proper three-way merge using 'similar' crate
        // For now, prefer local version as per user preference
        Ok(local.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::issue::IssueSource;
    use crate::project::IssueState;
    use chrono::Utc;

    fn create_test_issue() -> Issue {
        Issue {
            id: "test-id".to_string(),
            project_id: "project-id".to_string(),
            number: 1,
            title: "Test Issue".to_string(),
            body: Some("Test body".to_string()),
            state: IssueState::Open,
            labels: vec!["bug".to_string(), "enhancement".to_string()],
            assignees: vec!["user1".to_string()],
            priority: Some(3),
            github_number: Some(42),
            github_node_id: None,
            github_url: None,
            github_project_item_id: None,
            source: IssueSource::Local,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            closed_at: None,
            synced_at: None,
            content_hash: None,
            local_version: 1,
        }
    }

    #[test]
    fn test_content_hash_consistency() {
        let issue1 = create_test_issue();
        let issue2 = create_test_issue();

        let hash1 = ContentHasher::calculate_hash(&issue1);
        let hash2 = ContentHasher::calculate_hash(&issue2);

        assert_eq!(hash1, hash2, "Identical issues should have same hash");
    }

    #[test]
    fn test_content_hash_changes() {
        let mut issue1 = create_test_issue();
        let mut issue2 = create_test_issue();
        issue2.title = "Different Title".to_string();

        let hash1 = ContentHasher::calculate_hash(&issue1);
        let hash2 = ContentHasher::calculate_hash(&issue2);

        assert_ne!(hash1, hash2, "Different titles should produce different hashes");

        // Test body changes
        issue1.body = Some("New body content".to_string());
        let hash3 = ContentHasher::calculate_hash(&issue1);
        assert_ne!(hash1, hash3, "Body changes should affect hash");
    }

    #[test]
    fn test_label_order_doesnt_affect_hash() {
        let mut issue1 = create_test_issue();
        issue1.labels = vec!["bug".to_string(), "enhancement".to_string()];

        let mut issue2 = create_test_issue();
        issue2.labels = vec!["enhancement".to_string(), "bug".to_string()];

        let hash1 = ContentHasher::calculate_hash(&issue1);
        let hash2 = ContentHasher::calculate_hash(&issue2);

        assert_eq!(hash1, hash2, "Label order should not affect hash");
    }

    #[test]
    fn test_merge_arrays() {
        let local = vec!["bug".to_string(), "local".to_string()];
        let remote = vec!["bug".to_string(), "remote".to_string()];

        let merged = SmartMerger::merge_arrays(&local, &remote);

        assert_eq!(merged.len(), 3);
        assert!(merged.contains(&"bug".to_string()));
        assert!(merged.contains(&"local".to_string()));
        assert!(merged.contains(&"remote".to_string()));
    }

    #[test]
    fn test_merge_state() {
        assert_eq!(SmartMerger::merge_state("open", "closed"), "closed");
        assert_eq!(SmartMerger::merge_state("closed", "open"), "closed");
        assert_eq!(SmartMerger::merge_state("open", "open"), "open");
    }
}