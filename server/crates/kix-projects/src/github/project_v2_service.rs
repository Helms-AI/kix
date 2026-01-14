//! GitHub Projects V2 orchestration service.
//!
//! This service provides high-level operations for creating and managing
//! GitHub Projects V2 boards with template support.

use tracing::{info, warn};

use crate::error::ProjectError;
use crate::github::graphql_client::GitHubGraphQLClient;
use crate::project::{CustomFieldConfig, GitHubProjectV2Config, ProjectFieldType, StatusOption};
use crate::templates::{CustomFieldType, ProjectTemplate};

/// Service for orchestrating GitHub Projects V2 operations.
pub struct ProjectV2Service {
    graphql_client: GitHubGraphQLClient,
}

impl ProjectV2Service {
    /// Create a new ProjectV2Service with a GitHub token.
    pub fn new(token: &str) -> Result<Self, ProjectError> {
        let graphql_client = GitHubGraphQLClient::new(token)?;
        Ok(Self { graphql_client })
    }

    /// Create a new ProjectV2Service from an existing GraphQL client.
    pub fn from_client(graphql_client: GitHubGraphQLClient) -> Self {
        Self { graphql_client }
    }

    /// Get a reference to the underlying GraphQL client.
    pub fn graphql(&self) -> &GitHubGraphQLClient {
        &self.graphql_client
    }

    /// Create a GitHub Project V2 with a template applied.
    ///
    /// This method:
    /// 1. Creates the Project V2 on GitHub
    /// 2. Links it to the specified repository
    /// 3. Updates the Status field with template-defined options
    /// 4. Creates any custom fields defined in the template
    /// 5. Fetches and returns all field IDs for later use
    pub async fn create_project_with_template(
        &self,
        owner: &str,
        repo: &str,
        project_name: &str,
        template: ProjectTemplate,
    ) -> Result<GitHubProjectV2Config, ProjectError> {
        let template_config = template.get_config();
        let title = template.default_title(project_name);

        info!(
            "Creating GitHub Project V2 '{}' with {} template for {}/{}",
            title, template, owner, repo
        );

        // Step 1: Create the Project V2
        let project = self.graphql_client.create_project(owner, &title).await?;
        info!(
            "Created Project V2: {} (#{}) at {}",
            project.title, project.number, project.url
        );

        // Step 2: Link to repository
        if let Err(e) = self
            .graphql_client
            .link_project_to_repository(&project.id, owner, repo)
            .await
        {
            warn!(
                "Failed to link project to repository (non-fatal): {}",
                e
            );
            // Continue - linking is nice to have but not required
        }

        // Step 3: Update Status field options
        let status_field = match self
            .graphql_client
            .update_status_field_options(&project.id, &template_config.status_options)
            .await
        {
            Ok(field) => field,
            Err(e) => {
                warn!("Failed to update Status options: {}. Using defaults.", e);
                // Fetch current status field instead
                let fields = self.graphql_client.get_project_fields(&project.id).await?;
                fields
                    .into_iter()
                    .find(|f| f.name == "Status")
                    .ok_or_else(|| {
                        ProjectError::GitHub("Status field not found".to_string())
                    })?
            }
        };

        let status_options: Vec<StatusOption> = status_field
            .options
            .iter()
            .map(|o| StatusOption {
                name: o.name.clone(),
                option_id: o.id.clone(),
            })
            .collect();

        // Step 4: Create custom fields from template
        let mut custom_fields: Vec<CustomFieldConfig> = Vec::new();

        for field_def in &template_config.custom_fields {
            match field_def.field_type {
                CustomFieldType::SingleSelect => {
                    if let Some(options) = &field_def.options {
                        match self
                            .graphql_client
                            .create_single_select_field(&project.id, &field_def.name, options)
                            .await
                        {
                            Ok(created_field) => {
                                let options: Vec<StatusOption> = created_field
                                    .options
                                    .iter()
                                    .map(|o| StatusOption {
                                        name: o.name.clone(),
                                        option_id: o.id.clone(),
                                    })
                                    .collect();

                                custom_fields.push(CustomFieldConfig {
                                    name: created_field.name,
                                    field_id: created_field.id,
                                    field_type: ProjectFieldType::SingleSelect,
                                    options,
                                });

                                info!("Created custom field: {}", field_def.name);
                            }
                            Err(e) => {
                                warn!(
                                    "Failed to create custom field '{}': {}",
                                    field_def.name, e
                                );
                                // Continue - custom fields are nice to have
                            }
                        }
                    }
                }
                CustomFieldType::Number => {
                    // Number fields don't need options, but GitHub GraphQL
                    // doesn't support creating number fields via mutation yet
                    info!(
                        "Skipping number field '{}' (not supported via API)",
                        field_def.name
                    );
                }
                _ => {
                    info!(
                        "Skipping field '{}' with type {:?} (not implemented)",
                        field_def.name, field_def.field_type
                    );
                }
            }
        }

        let config = GitHubProjectV2Config {
            project_id: project.id,
            project_number: project.number,
            url: project.url,
            status_field_id: Some(status_field.id),
            status_options,
            custom_fields,
        };

        info!(
            "GitHub Project V2 created successfully with {} status options and {} custom fields",
            config.status_options.len(),
            config.custom_fields.len()
        );

        Ok(config)
    }

    /// Add an issue to a GitHub Project V2.
    ///
    /// Returns the project item ID which can be used to update field values.
    pub async fn add_issue_to_project(
        &self,
        project_id: &str,
        issue_node_id: &str,
    ) -> Result<String, ProjectError> {
        self.graphql_client
            .add_issue_to_project(project_id, issue_node_id)
            .await
    }

    /// Set the status of a project item.
    pub async fn set_item_status(
        &self,
        config: &GitHubProjectV2Config,
        item_id: &str,
        status_name: &str,
    ) -> Result<(), ProjectError> {
        let status_field_id = config.status_field_id.as_ref().ok_or_else(|| {
            ProjectError::GitHub("Status field ID not configured".to_string())
        })?;

        // Find the option ID for the given status name
        let option = config
            .status_options
            .iter()
            .find(|o| o.name.eq_ignore_ascii_case(status_name))
            .ok_or_else(|| {
                ProjectError::GitHub(format!(
                    "Status '{}' not found. Available: {:?}",
                    status_name,
                    config.status_options.iter().map(|o| &o.name).collect::<Vec<_>>()
                ))
            })?;

        self.graphql_client
            .update_item_field(
                &config.project_id,
                item_id,
                status_field_id,
                &option.option_id,
            )
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_service_creation() {
        let service = ProjectV2Service::new("test_token");
        assert!(service.is_ok());
    }
}
