//! AI-assisted project planning data structures and services.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::knowledge::KnowledgeReference;
use crate::templates::ProjectTemplate;

/// Parameters for the `plan_project` MCP tool.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct PlanProjectParams {
    /// Project ID or name
    pub project: String,

    /// Description of what you want to build
    pub description: String,

    /// Use knowledge base for context (default: true)
    #[serde(default = "default_true")]
    pub use_knowledge: bool,

    /// Template for task structure (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub template: Option<PlanTemplate>,

    /// Maximum number of knowledge references to include
    #[serde(default = "default_knowledge_limit")]
    pub knowledge_limit: usize,
}

fn default_true() -> bool {
    true
}

fn default_knowledge_limit() -> usize {
    10
}

/// Template for structuring planned tasks.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PlanTemplate {
    /// Feature development: Design -> Implement -> Test -> Document
    Feature,

    /// Bug fix: Investigate -> Fix -> Test -> Verify
    BugFix,

    /// Research: Explore -> Prototype -> Evaluate -> Document
    Research,

    /// Refactoring: Analyze -> Plan -> Execute -> Verify
    Refactor,
}

impl PlanTemplate {
    /// Get the phases for this template.
    pub fn get_phases(&self) -> Vec<&'static str> {
        match self {
            PlanTemplate::Feature => vec!["Design", "Implement", "Test", "Document"],
            PlanTemplate::BugFix => vec!["Investigate", "Fix", "Test", "Verify"],
            PlanTemplate::Research => vec!["Explore", "Prototype", "Evaluate", "Document"],
            PlanTemplate::Refactor => vec!["Analyze", "Plan", "Execute", "Verify"],
        }
    }

    /// Suggest a template based on description keywords.
    pub fn suggest(description: &str) -> Option<Self> {
        let desc_lower = description.to_lowercase();

        if desc_lower.contains("bug") || desc_lower.contains("fix") || desc_lower.contains("broken") {
            Some(PlanTemplate::BugFix)
        } else if desc_lower.contains("research") || desc_lower.contains("explore") || desc_lower.contains("investigate") {
            Some(PlanTemplate::Research)
        } else if desc_lower.contains("refactor") || desc_lower.contains("reorganize") || desc_lower.contains("restructure") {
            Some(PlanTemplate::Refactor)
        } else {
            Some(PlanTemplate::Feature)
        }
    }
}

/// Result of the `plan_project` tool.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PlanResult {
    /// Project ID
    pub project_id: String,

    /// Project name
    pub project_name: String,

    /// Knowledge sources found for context
    pub knowledge_sources: Vec<KnowledgeReference>,

    /// Suggested GitHub Project template
    pub suggested_template: ProjectTemplate,

    /// Suggested plan template
    pub suggested_plan_template: PlanTemplate,

    /// Hint for AI about the phases
    pub phases: Vec<String>,

    /// Summary of what was found in the knowledge base
    #[serde(skip_serializing_if = "Option::is_none")]
    pub knowledge_summary: Option<String>,
}

/// A planned task to be created as an issue.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PlannedTask {
    /// Task title
    pub title: String,

    /// Task body (markdown)
    pub body: String,

    /// Labels to apply
    #[serde(default)]
    pub labels: Vec<String>,

    /// Priority (1-5, 1=highest)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub priority: Option<u8>,

    /// Other task titles this depends on
    #[serde(default)]
    pub depends_on: Vec<String>,

    /// Phase this task belongs to
    #[serde(skip_serializing_if = "Option::is_none")]
    pub phase: Option<String>,
}

/// Parameters for the `suggest_tasks` MCP tool.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct SuggestTasksParams {
    /// Project ID or name
    pub project: String,

    /// Additional context for suggestions
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context: Option<String>,

    /// Maximum number of suggestions
    #[serde(default = "default_suggestion_limit")]
    pub limit: usize,
}

fn default_suggestion_limit() -> usize {
    5
}

/// Result of the `suggest_tasks` tool.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct TaskSuggestions {
    /// Suggested tasks
    pub suggestions: Vec<TaskSuggestion>,

    /// Related knowledge entries
    pub related_knowledge: Vec<KnowledgeReference>,
}

/// A single task suggestion.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct TaskSuggestion {
    /// Suggested title
    pub title: String,

    /// Why this task is suggested
    pub rationale: String,

    /// Suggested labels
    #[serde(default)]
    pub labels: Vec<String>,

    /// Suggested priority
    #[serde(skip_serializing_if = "Option::is_none")]
    pub priority: Option<u8>,
}

/// Parameters for the `get_project_context` MCP tool.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct GetProjectContextParams {
    /// Project ID or name
    pub project: String,

    /// Optional query to filter knowledge
    #[serde(skip_serializing_if = "Option::is_none")]
    pub query: Option<String>,

    /// Maximum number of entries
    #[serde(default = "default_knowledge_limit")]
    pub limit: usize,

    /// Include existing issues in context
    #[serde(default = "default_true")]
    pub include_issues: bool,
}

/// Result of the `get_project_context` tool.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ProjectContext {
    /// Project ID
    pub project_id: String,

    /// Project name
    pub project_name: String,

    /// Project description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// GitHub repository
    pub github_repo: String,

    /// Linked knowledge entries
    pub knowledge: Vec<KnowledgeReference>,

    /// Recent issues (if requested)
    #[serde(default)]
    pub recent_issues: Vec<IssueContext>,

    /// Project statistics
    pub stats: ProjectContextStats,
}

/// Simplified issue info for context.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct IssueContext {
    /// Issue number
    pub number: u32,

    /// Issue title
    pub title: String,

    /// Issue state
    pub state: String,

    /// Labels
    #[serde(default)]
    pub labels: Vec<String>,
}

/// Statistics for project context.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct ProjectContextStats {
    /// Total open issues
    pub open_issues: u32,

    /// Total closed issues
    pub closed_issues: u32,

    /// Total linked knowledge entries
    pub knowledge_entries: u32,
}

/// Parameters for the `breakdown_task` MCP tool.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct BreakdownTaskParams {
    /// Project ID or name
    pub project: String,

    /// Task description to break down
    pub task_description: String,

    /// Use knowledge base for context
    #[serde(default = "default_true")]
    pub use_knowledge: bool,
}

/// Result of the `breakdown_task` tool.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct TaskBreakdown {
    /// Original task description
    pub original: String,

    /// Suggested subtasks
    pub subtasks: Vec<PlannedTask>,

    /// Related knowledge for context
    pub related_knowledge: Vec<KnowledgeReference>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plan_template_phases() {
        assert_eq!(
            PlanTemplate::Feature.get_phases(),
            vec!["Design", "Implement", "Test", "Document"]
        );
        assert_eq!(
            PlanTemplate::BugFix.get_phases(),
            vec!["Investigate", "Fix", "Test", "Verify"]
        );
    }

    #[test]
    fn test_plan_template_suggest() {
        assert_eq!(
            PlanTemplate::suggest("Fix the broken login"),
            Some(PlanTemplate::BugFix)
        );
        assert_eq!(
            PlanTemplate::suggest("Research caching strategies"),
            Some(PlanTemplate::Research)
        );
        assert_eq!(
            PlanTemplate::suggest("Refactor the auth module"),
            Some(PlanTemplate::Refactor)
        );
        assert_eq!(
            PlanTemplate::suggest("Add dark mode support"),
            Some(PlanTemplate::Feature)
        );
    }
}
