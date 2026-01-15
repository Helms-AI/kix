//! Board templates for creating pre-configured project boards.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Available project templates for Kanban boards.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProjectTemplate {
    /// Simple Kanban board: Todo -> In Progress -> Done
    Kanban,

    /// Bug tracking with Priority and Severity fields
    BugTracking,

    /// Sprint planning with Sprint and Story Points fields
    SprintPlanning,

    /// Feature roadmap with Quarter and Team fields
    FeatureRoadmap,
}

impl ProjectTemplate {
    /// Get the template configuration.
    pub fn get_config(&self) -> TemplateConfig {
        match self {
            ProjectTemplate::Kanban => TemplateConfig {
                title_suffix: "Board",
                status_options: vec![
                    "Todo".to_string(),
                    "In Progress".to_string(),
                    "Done".to_string(),
                ],
                custom_fields: vec![],
                default_view: ProjectView::Board,
            },

            ProjectTemplate::BugTracking => TemplateConfig {
                title_suffix: "Bug Tracker",
                status_options: vec![
                    "Triage".to_string(),
                    "Todo".to_string(),
                    "In Progress".to_string(),
                    "Done".to_string(),
                ],
                custom_fields: vec![
                    CustomFieldDefinition {
                        name: "Priority".to_string(),
                        field_type: CustomFieldType::SingleSelect,
                        options: Some(vec![
                            "P0 - Critical".to_string(),
                            "P1 - High".to_string(),
                            "P2 - Medium".to_string(),
                            "P3 - Low".to_string(),
                        ]),
                    },
                    CustomFieldDefinition {
                        name: "Severity".to_string(),
                        field_type: CustomFieldType::SingleSelect,
                        options: Some(vec![
                            "Blocker".to_string(),
                            "Major".to_string(),
                            "Minor".to_string(),
                            "Trivial".to_string(),
                        ]),
                    },
                ],
                default_view: ProjectView::Board,
            },

            ProjectTemplate::SprintPlanning => TemplateConfig {
                title_suffix: "Sprint Board",
                status_options: vec![
                    "Backlog".to_string(),
                    "Sprint".to_string(),
                    "In Progress".to_string(),
                    "Review".to_string(),
                    "Done".to_string(),
                ],
                custom_fields: vec![
                    CustomFieldDefinition {
                        name: "Story Points".to_string(),
                        field_type: CustomFieldType::Number,
                        options: None,
                    },
                    // Note: Iteration fields require special handling
                ],
                default_view: ProjectView::Board,
            },

            ProjectTemplate::FeatureRoadmap => TemplateConfig {
                title_suffix: "Roadmap",
                status_options: vec![
                    "Planning".to_string(),
                    "In Progress".to_string(),
                    "Shipped".to_string(),
                ],
                custom_fields: vec![
                    CustomFieldDefinition {
                        name: "Quarter".to_string(),
                        field_type: CustomFieldType::SingleSelect,
                        options: Some(vec![
                            "Q1".to_string(),
                            "Q2".to_string(),
                            "Q3".to_string(),
                            "Q4".to_string(),
                        ]),
                    },
                    CustomFieldDefinition {
                        name: "Team".to_string(),
                        field_type: CustomFieldType::SingleSelect,
                        options: Some(vec![
                            "Frontend".to_string(),
                            "Backend".to_string(),
                            "Platform".to_string(),
                            "Design".to_string(),
                        ]),
                    },
                ],
                default_view: ProjectView::Roadmap,
            },
        }
    }

    /// Get the default project title based on template.
    pub fn default_title(&self, project_name: &str) -> String {
        let config = self.get_config();
        format!("{} {}", project_name, config.title_suffix)
    }

    /// Parse a template from a string.
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "kanban" => Some(ProjectTemplate::Kanban),
            "bug_tracking" | "bugtracking" | "bugs" => Some(ProjectTemplate::BugTracking),
            "sprint_planning" | "sprintplanning" | "sprint" | "agile" => {
                Some(ProjectTemplate::SprintPlanning)
            }
            "feature_roadmap" | "featureroadmap" | "roadmap" => {
                Some(ProjectTemplate::FeatureRoadmap)
            }
            _ => None,
        }
    }
}

impl std::fmt::Display for ProjectTemplate {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProjectTemplate::Kanban => write!(f, "kanban"),
            ProjectTemplate::BugTracking => write!(f, "bug_tracking"),
            ProjectTemplate::SprintPlanning => write!(f, "sprint_planning"),
            ProjectTemplate::FeatureRoadmap => write!(f, "feature_roadmap"),
        }
    }
}

/// Configuration for a project template.
#[derive(Debug, Clone)]
pub struct TemplateConfig {
    /// Suffix to add to project title
    pub title_suffix: &'static str,

    /// Status options for the Status field
    pub status_options: Vec<String>,

    /// Custom fields to create
    pub custom_fields: Vec<CustomFieldDefinition>,

    /// Default view type
    pub default_view: ProjectView,
}

/// Definition for a custom field to create.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CustomFieldDefinition {
    /// Field name
    pub name: String,

    /// Field type
    pub field_type: CustomFieldType,

    /// Options for single-select fields
    #[serde(skip_serializing_if = "Option::is_none")]
    pub options: Option<Vec<String>>,
}

/// Custom field types for board configuration.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CustomFieldType {
    Text,
    Number,
    Date,
    SingleSelect,
    Iteration,
}

impl CustomFieldType {
    /// Convert to string representation.
    pub fn as_str(&self) -> &'static str {
        match self {
            CustomFieldType::Text => "text",
            CustomFieldType::Number => "number",
            CustomFieldType::Date => "date",
            CustomFieldType::SingleSelect => "single_select",
            CustomFieldType::Iteration => "iteration",
        }
    }
}

/// Project view types.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ProjectView {
    Board,
    Table,
    Roadmap,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_kanban_template() {
        let config = ProjectTemplate::Kanban.get_config();
        assert_eq!(config.status_options.len(), 3);
        assert!(config.status_options.contains(&"Todo".to_string()));
        assert!(config.custom_fields.is_empty());
        assert_eq!(config.default_view, ProjectView::Board);
    }

    #[test]
    fn test_bug_tracking_template() {
        let config = ProjectTemplate::BugTracking.get_config();
        assert_eq!(config.status_options.len(), 4);
        assert_eq!(config.custom_fields.len(), 2);

        let priority = config.custom_fields.iter().find(|f| f.name == "Priority");
        assert!(priority.is_some());
        assert_eq!(priority.unwrap().field_type, CustomFieldType::SingleSelect);
    }

    #[test]
    fn test_sprint_planning_template() {
        let config = ProjectTemplate::SprintPlanning.get_config();
        assert_eq!(config.status_options.len(), 5);

        let story_points = config.custom_fields.iter().find(|f| f.name == "Story Points");
        assert!(story_points.is_some());
        assert_eq!(story_points.unwrap().field_type, CustomFieldType::Number);
    }

    #[test]
    fn test_roadmap_template() {
        let config = ProjectTemplate::FeatureRoadmap.get_config();
        assert_eq!(config.default_view, ProjectView::Roadmap);
        assert_eq!(config.custom_fields.len(), 2);
    }

    #[test]
    fn test_default_title() {
        assert_eq!(
            ProjectTemplate::Kanban.default_title("My Project"),
            "My Project Board"
        );
        assert_eq!(
            ProjectTemplate::BugTracking.default_title("App"),
            "App Bug Tracker"
        );
    }

    #[test]
    fn test_template_from_str() {
        assert_eq!(ProjectTemplate::from_str("kanban"), Some(ProjectTemplate::Kanban));
        assert_eq!(ProjectTemplate::from_str("KANBAN"), Some(ProjectTemplate::Kanban));
        assert_eq!(ProjectTemplate::from_str("sprint"), Some(ProjectTemplate::SprintPlanning));
        assert_eq!(ProjectTemplate::from_str("roadmap"), Some(ProjectTemplate::FeatureRoadmap));
        assert_eq!(ProjectTemplate::from_str("unknown"), None);
    }
}
