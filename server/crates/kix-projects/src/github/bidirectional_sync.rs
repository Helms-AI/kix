//! Enhanced bidirectional GitHub sync with conflict resolution

use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::{debug, info};

use crate::error::ProjectError;
use crate::github::models::GitHubIssue;
use crate::github::sync::{GitHubSyncService, IssueInfo, SyncConfig, SyncDirection};
use crate::issue::{Issue, IssueSource};
use crate::project::IssueState;
use crate::sync_state::{
    ConflictDetector, ConflictType, ContentHasher, IssueSyncState, MergeResult,
    MergeStrategy, SmartMerger,
};

/// Enhanced sync result with detailed tracking
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EnhancedSyncResult {
    /// Issues pulled from GitHub (new or updated)
    pub issues_pulled: u32,

    /// Issues pushed to GitHub (new or updated)
    pub issues_pushed: u32,

    /// Issues that were merged (conflicts resolved)
    pub issues_merged: u32,

    /// Number of auto-resolved conflicts
    pub conflicts_resolved: u32,

    /// Number of conflicts that couldn't be resolved
    pub conflicts_failed: u32,

    /// Issues that failed to sync
    pub issues_failed: u32,

    /// Detailed changes for UI notification
    pub change_details: Vec<ChangeDetail>,

    /// Error messages
    pub errors: Vec<String>,

    /// Timestamp of sync completion
    pub synced_at: chrono::DateTime<Utc>,
}

/// Details about a change for UI notification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangeDetail {
    pub issue_number: u32,
    pub title: String,
    pub action: ChangeAction,
    pub fields_affected: Vec<String>,
}

/// Type of change action
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChangeAction {
    Created,
    Updated,
    Merged,
    Pushed,
    Pulled,
    Conflicted,
}

/// Sync plan that determines what actions to take
#[derive(Debug, Default)]
struct SyncPlan {
    /// Issues to push to GitHub (new or updated)
    to_push: Vec<Issue>,

    /// Issues to pull from GitHub (new or updated)
    to_pull: Vec<GitHubIssue>,

    /// Conflicts that need resolution
    conflicts: Vec<ConflictItem>,

    /// Issues that are already in sync
    in_sync: Vec<String>,
}

/// A conflict between local and GitHub versions
#[derive(Debug)]
#[allow(dead_code)] // Fields used for future interactive conflict resolution UI
struct ConflictItem {
    local: Issue,
    github: GitHubIssue,
    sync_state: IssueSyncState,
    conflict_type: ConflictType,
}

/// Enhanced bidirectional sync service
pub struct BidirectionalSyncService {
    sync_service: GitHubSyncService,
    merge_strategy: MergeStrategy,
}

impl BidirectionalSyncService {
    /// Create a new bidirectional sync service
    pub fn new(
        sync_service: GitHubSyncService,
        merge_strategy: MergeStrategy,
    ) -> Self {
        Self {
            sync_service,
            merge_strategy,
        }
    }

    /// Perform full bidirectional sync
    pub async fn sync_bidirectional(
        &self,
        owner: &str,
        repo: &str,
        config: &SyncConfig,
        local_issues: Vec<Issue>,
        sync_states: HashMap<String, IssueSyncState>,
    ) -> Result<EnhancedSyncResult, ProjectError> {
        let mut result = EnhancedSyncResult::default();

        // Step 1: Fetch GitHub issues
        info!("Fetching issues from GitHub {}/{}", owner, repo);
        let github_issues = self.sync_service.pull_issues(owner, repo, config).await?;

        // Step 2: Build sync plan
        info!("Analyzing changes and conflicts");
        let sync_plan = self.analyze_changes(
            local_issues,
            github_issues,
            sync_states,
        )?;

        info!(
            "Sync plan: push={}, pull={}, conflicts={}, in_sync={}",
            sync_plan.to_push.len(),
            sync_plan.to_pull.len(),
            sync_plan.conflicts.len(),
            sync_plan.in_sync.len()
        );

        // Step 3: Execute sync plan

        // 3a. Push local changes to GitHub
        if config.direction != SyncDirection::Pull {
            for issue in &sync_plan.to_push {
                match self.push_issue_to_github(owner, repo, issue).await {
                    Ok(github_issue) => {
                        result.issues_pushed += 1;
                        result.change_details.push(ChangeDetail {
                            issue_number: github_issue.number,
                            title: issue.title.clone(),
                            action: if issue.github_number.is_some() {
                                ChangeAction::Updated
                            } else {
                                ChangeAction::Created
                            },
                            fields_affected: vec![],
                        });
                    }
                    Err(e) => {
                        result.issues_failed += 1;
                        result.errors.push(format!(
                            "Failed to push '{}': {}",
                            issue.title, e
                        ));
                    }
                }
            }
        }

        // 3b. Pull GitHub changes to local
        if config.direction != SyncDirection::Push {
            for github_issue in &sync_plan.to_pull {
                result.issues_pulled += 1;
                result.change_details.push(ChangeDetail {
                    issue_number: github_issue.number,
                    title: github_issue.title.clone(),
                    action: ChangeAction::Pulled,
                    fields_affected: vec![],
                });
            }
        }

        // 3c. Resolve conflicts with smart merge
        for conflict in sync_plan.conflicts {
            match self.resolve_conflict(conflict).await {
                Ok(merge_result) => {
                    // Push merged version to GitHub
                    if config.direction != SyncDirection::Pull {
                        match self.push_issue_to_github(owner, repo, &merge_result.issue).await {
                            Ok(_) => {
                                result.issues_merged += 1;
                                result.conflicts_resolved += 1;
                                result.change_details.push(ChangeDetail {
                                    issue_number: merge_result.issue.github_number.unwrap_or(0),
                                    title: merge_result.issue.title.clone(),
                                    action: ChangeAction::Merged,
                                    fields_affected: merge_result.auto_resolved_fields.clone(),
                                });
                            }
                            Err(e) => {
                                result.conflicts_failed += 1;
                                result.errors.push(format!(
                                    "Failed to push merged issue '{}': {}",
                                    merge_result.issue.title, e
                                ));
                            }
                        }
                    }
                }
                Err(e) => {
                    result.conflicts_failed += 1;
                    result.errors.push(format!("Failed to resolve conflict: {}", e));
                }
            }
        }

        result.synced_at = Utc::now();

        info!(
            "Bidirectional sync complete: pushed={}, pulled={}, merged={}, conflicts_resolved={}, failed={}",
            result.issues_pushed,
            result.issues_pulled,
            result.issues_merged,
            result.conflicts_resolved,
            result.issues_failed + result.conflicts_failed
        );

        Ok(result)
    }

    /// Analyze changes between local and GitHub
    fn analyze_changes(
        &self,
        local_issues: Vec<Issue>,
        github_issues: Vec<GitHubIssue>,
        sync_states: HashMap<String, IssueSyncState>,
    ) -> Result<SyncPlan, ProjectError> {
        let mut plan = SyncPlan::default();

        // Build maps for efficient lookup
        let mut local_by_github_num: HashMap<u32, Issue> = HashMap::new();
        let mut local_by_id: HashMap<String, Issue> = HashMap::new();

        for issue in local_issues {
            if let Some(gh_num) = issue.github_number {
                local_by_github_num.insert(gh_num, issue.clone());
            }
            local_by_id.insert(issue.id.clone(), issue);
        }

        let github_by_num: HashMap<u32, GitHubIssue> = github_issues
            .into_iter()
            .map(|issue| (issue.number, issue))
            .collect();

        // Check each local issue
        for (issue_id, local_issue) in &local_by_id {
            let sync_state = sync_states.get(issue_id);

            if let Some(github_num) = local_issue.github_number {
                // Issue exists on both sides
                if let Some(github_issue) = github_by_num.get(&github_num) {
                    // Check for conflicts
                    let local_for_comparison = self.github_issue_to_local(
                        github_issue,
                        &local_issue.project_id,
                    );

                    if let Some(state) = sync_state {
                        let conflict_type = ConflictDetector::detect_conflict(
                            local_issue,
                            &local_for_comparison,
                            state,
                        );

                        match conflict_type {
                            ConflictType::None => {
                                plan.in_sync.push(issue_id.clone());
                            }
                            ConflictType::LocalOnly => {
                                // Push local changes
                                plan.to_push.push(local_issue.clone());
                            }
                            ConflictType::RemoteOnly => {
                                // Pull GitHub changes
                                plan.to_pull.push(github_issue.clone());
                            }
                            ConflictType::BothChanged => {
                                // Conflict needs resolution
                                plan.conflicts.push(ConflictItem {
                                    local: local_issue.clone(),
                                    github: github_issue.clone(),
                                    sync_state: state.clone(),
                                    conflict_type,
                                });
                            }
                        }
                    } else {
                        // No sync state, compare directly
                        let local_hash = ContentHasher::calculate_hash(local_issue);
                        let github_hash = ContentHasher::calculate_hash(&local_for_comparison);

                        if local_hash != github_hash {
                            // Assume local is authoritative without sync state
                            plan.to_push.push(local_issue.clone());
                        } else {
                            plan.in_sync.push(issue_id.clone());
                        }
                    }
                } else {
                    // Local has GitHub number but GitHub issue doesn't exist
                    // This shouldn't normally happen, but push it
                    plan.to_push.push(local_issue.clone());
                }
            } else {
                // Local-only issue, push to GitHub
                if local_issue.source != IssueSource::GitHub {
                    plan.to_push.push(local_issue.clone());
                }
            }
        }

        // Check for GitHub-only issues
        for (github_num, github_issue) in github_by_num {
            if !local_by_github_num.contains_key(&github_num) {
                // GitHub issue doesn't exist locally, pull it
                plan.to_pull.push(github_issue);
            }
        }

        Ok(plan)
    }

    /// Resolve a conflict using smart merge
    async fn resolve_conflict(&self, conflict: ConflictItem) -> Result<MergeResult, ProjectError> {
        let mut merged = conflict.local.clone();

        // Smart merge based on strategy
        match self.merge_strategy {
            MergeStrategy::LocalWins => {
                // Local always wins, just track what we kept
                Ok(MergeResult {
                    issue: merged,
                    auto_resolved_fields: vec!["all".to_string()],
                    strategy_used: "local_wins".to_string(),
                    conflicts_resolved: 1,
                })
            }
            MergeStrategy::RemoteWins => {
                // GitHub always wins
                let github_as_local = self.github_issue_to_local(
                    &conflict.github,
                    &conflict.local.project_id,
                );
                Ok(MergeResult {
                    issue: github_as_local,
                    auto_resolved_fields: vec!["all".to_string()],
                    strategy_used: "remote_wins".to_string(),
                    conflicts_resolved: 1,
                })
            }
            MergeStrategy::SmartMerge => {
                // Intelligent field-by-field merge
                let mut resolved_fields = Vec::new();

                // Title: attempt three-way merge, fallback to local
                if conflict.local.title != conflict.github.title {
                    match SmartMerger::merge_text(
                        &conflict.local.title,
                        &conflict.github.title,
                        &conflict.local.title, // TODO: use actual last common ancestor
                    ) {
                        Ok(merged_title) => {
                            merged.title = merged_title;
                            resolved_fields.push("title".to_string());
                        }
                        Err(_) => {
                            // Keep local title
                            debug!("Title merge failed, keeping local");
                        }
                    }
                }

                // Body: attempt three-way merge, fallback to local
                let local_body = conflict.local.body.as_deref().unwrap_or("");
                let github_body = conflict.github.body.as_deref().unwrap_or("");
                if local_body != github_body {
                    match SmartMerger::merge_text(
                        local_body,
                        github_body,
                        local_body, // TODO: use actual last common ancestor
                    ) {
                        Ok(merged_body) => {
                            merged.body = Some(merged_body);
                            resolved_fields.push("body".to_string());
                        }
                        Err(_) => {
                            // Keep local body
                            debug!("Body merge failed, keeping local");
                        }
                    }
                }

                // Labels: union merge
                let github_labels: Vec<String> = conflict.github.labels
                    .iter()
                    .map(|l| l.name.clone())
                    .collect();
                merged.labels = SmartMerger::merge_arrays(&conflict.local.labels, &github_labels);
                if merged.labels != conflict.local.labels {
                    resolved_fields.push("labels".to_string());
                }

                // Assignees: union merge
                let github_assignees: Vec<String> = conflict.github.assignees
                    .iter()
                    .map(|u| u.login.clone())
                    .collect();
                merged.assignees = SmartMerger::merge_arrays(&conflict.local.assignees, &github_assignees);
                if merged.assignees != conflict.local.assignees {
                    resolved_fields.push("assignees".to_string());
                }

                // State: closed wins
                let github_state_str = &conflict.github.state;
                let local_state_str = match conflict.local.state {
                    IssueState::Open => "open",
                    IssueState::Closed => "closed",
                };
                let merged_state_str = SmartMerger::merge_state(local_state_str, github_state_str);
                merged.state = if merged_state_str == "closed" {
                    IssueState::Closed
                } else {
                    IssueState::Open
                };
                if merged.state != conflict.local.state {
                    resolved_fields.push("state".to_string());
                }

                // Update tracking fields
                merged.mark_modified();
                merged.synced_at = Some(Utc::now());

                Ok(MergeResult {
                    issue: merged,
                    auto_resolved_fields: resolved_fields,
                    strategy_used: "smart_merge".to_string(),
                    conflicts_resolved: 1,
                })
            }
            MergeStrategy::Interactive => {
                // For now, fall back to local wins
                // In future, this would show UI for user decision
                Ok(MergeResult {
                    issue: merged,
                    auto_resolved_fields: vec!["all".to_string()],
                    strategy_used: "interactive_fallback_local".to_string(),
                    conflicts_resolved: 1,
                })
            }
        }
    }

    /// Push issue to GitHub (create or update)
    async fn push_issue_to_github(
        &self,
        owner: &str,
        repo: &str,
        issue: &Issue,
    ) -> Result<GitHubIssue, ProjectError> {
        let issue_info = IssueInfo {
            id: issue.id.clone(),
            title: issue.title.clone(),
            body: issue.body.clone(),
            state: issue.state.clone(),
            labels: issue.labels.clone(),
            assignees: issue.assignees.clone(),
            github_number: issue.github_number,
            github_node_id: issue.github_node_id.clone(),
            source: issue.source.clone(),
        };

        self.sync_service.push_issue(owner, repo, &issue_info).await
    }

    /// Convert GitHub issue to local Issue format
    fn github_issue_to_local(&self, github: &GitHubIssue, project_id: &str) -> Issue {
        Issue {
            id: uuid::Uuid::new_v4().to_string(),
            project_id: project_id.to_string(),
            number: github.number,
            title: github.title.clone(),
            body: github.body.clone(),
            state: if github.state == "closed" {
                IssueState::Closed
            } else {
                IssueState::Open
            },
            labels: github.labels.iter().map(|l| l.name.clone()).collect(),
            assignees: github.assignees.iter().map(|u| u.login.clone()).collect(),
            priority: None,
            github_number: Some(github.number),
            github_node_id: Some(github.node_id.clone()),
            github_url: Some(github.html_url.clone()),
            github_project_item_id: None,
            source: IssueSource::GitHub,
            created_at: github.created_at,
            updated_at: github.updated_at,
            closed_at: github.closed_at,
            synced_at: Some(Utc::now()),
            content_hash: None,
            local_version: 1,
        }
    }
}