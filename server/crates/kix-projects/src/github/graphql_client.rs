//! GitHub GraphQL API client for Projects V2 operations.
//!
//! This module provides a client for interacting with GitHub's GraphQL API,
//! specifically for GitHub Projects V2 management.

use reqwest::header::{HeaderMap, HeaderValue, ACCEPT, AUTHORIZATION, USER_AGENT};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tracing::{debug, info};

use crate::error::ProjectError;
use crate::github::models::{
    GitHubFieldOption, GitHubGraphQLError, GitHubProjectField, GitHubProjectItem,
    GitHubProjectItemContent, GitHubProjectV2,
};

/// GitHub GraphQL API endpoint.
const GITHUB_GRAPHQL_URL: &str = "https://api.github.com/graphql";

/// User agent for API requests.
const USER_AGENT_VALUE: &str = "kix-projects/0.1.0";

/// GraphQL request body.
#[derive(Debug, Serialize)]
struct GraphQLRequest {
    query: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    variables: Option<Value>,
}

/// GraphQL response wrapper.
#[derive(Debug, Deserialize)]
struct GraphQLResponse {
    data: Option<Value>,
    errors: Option<Vec<GitHubGraphQLError>>,
}

/// GitHub GraphQL API client.
#[derive(Clone)]
pub struct GitHubGraphQLClient {
    client: Client,
    token: String,
}

impl GitHubGraphQLClient {
    /// Create a new GitHub GraphQL client with the given personal access token.
    pub fn new(token: impl Into<String>) -> Result<Self, ProjectError> {
        let token = token.into();

        let mut headers = HeaderMap::new();
        headers.insert(USER_AGENT, HeaderValue::from_static(USER_AGENT_VALUE));
        headers.insert(ACCEPT, HeaderValue::from_static("application/json"));

        let client = Client::builder()
            .default_headers(headers)
            .build()
            .map_err(|e| ProjectError::GitHub(format!("Failed to create HTTP client: {}", e)))?;

        Ok(Self { client, token })
    }

    /// Execute a GraphQL query.
    async fn execute(&self, query: &str, variables: Option<Value>) -> Result<Value, ProjectError> {
        let request = GraphQLRequest {
            query: query.to_string(),
            variables,
        };

        let response = self
            .client
            .post(GITHUB_GRAPHQL_URL)
            .header(AUTHORIZATION, format!("Bearer {}", self.token))
            .json(&request)
            .send()
            .await
            .map_err(|e| ProjectError::GitHub(format!("GraphQL request failed: {}", e)))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(ProjectError::GitHub(format!(
                "GraphQL request failed: {} - {}",
                status, body
            )));
        }

        let gql_response: GraphQLResponse = response
            .json()
            .await
            .map_err(|e| ProjectError::GitHub(format!("Failed to parse GraphQL response: {}", e)))?;

        if let Some(errors) = gql_response.errors {
            let error_msgs: Vec<String> = errors.iter().map(|e| e.message.clone()).collect();
            return Err(ProjectError::GitHub(format!(
                "GraphQL errors: {}",
                error_msgs.join(", ")
            )));
        }

        gql_response
            .data
            .ok_or_else(|| ProjectError::GitHub("No data in GraphQL response".to_string()))
    }

    // ========================================================================
    // User/Organization Operations
    // ========================================================================

    /// Get the node ID for a repository.
    pub async fn get_repository_id(&self, owner: &str, repo: &str) -> Result<String, ProjectError> {
        let query = r#"
            query($owner: String!, $repo: String!) {
                repository(owner: $owner, name: $repo) {
                    id
                }
            }
        "#;

        let variables = json!({
            "owner": owner,
            "repo": repo,
        });

        let data = self.execute(query, Some(variables)).await?;
        let repo_id = data["repository"]["id"]
            .as_str()
            .ok_or_else(|| ProjectError::GitHub("Repository not found".to_string()))?;

        Ok(repo_id.to_string())
    }

    /// Get the node ID for a user or organization.
    pub async fn get_owner_id(&self, owner: &str) -> Result<(String, bool), ProjectError> {
        // Try as organization first
        let org_query = r#"
            query($login: String!) {
                organization(login: $login) {
                    id
                }
            }
        "#;

        let variables = json!({ "login": owner });

        match self.execute(org_query, Some(variables.clone())).await {
            Ok(data) => {
                if let Some(org_id) = data["organization"]["id"].as_str() {
                    return Ok((org_id.to_string(), true));
                }
            }
            Err(_) => {
                debug!("Owner {} is not an organization, trying as user", owner);
            }
        }

        // Try as user
        let user_query = r#"
            query($login: String!) {
                user(login: $login) {
                    id
                }
            }
        "#;

        let data = self.execute(user_query, Some(variables)).await?;
        let user_id = data["user"]["id"]
            .as_str()
            .ok_or_else(|| ProjectError::GitHub(format!("Owner {} not found", owner)))?;

        Ok((user_id.to_string(), false))
    }

    // ========================================================================
    // Project V2 Operations
    // ========================================================================

    /// Create a new Project V2.
    pub async fn create_project(
        &self,
        owner: &str,
        title: &str,
    ) -> Result<GitHubProjectV2, ProjectError> {
        let (owner_id, is_org) = self.get_owner_id(owner).await?;

        let mutation = r#"
            mutation($input: CreateProjectV2Input!) {
                createProjectV2(input: $input) {
                    projectV2 {
                        id
                        number
                        title
                        url
                    }
                }
            }
        "#;

        let variables = json!({
            "input": {
                "ownerId": owner_id,
                "title": title,
            }
        });

        let data = self.execute(mutation, Some(variables)).await?;
        let project = &data["createProjectV2"]["projectV2"];

        let result = GitHubProjectV2 {
            id: project["id"]
                .as_str()
                .ok_or_else(|| ProjectError::GitHub("Missing project ID".to_string()))?
                .to_string(),
            number: project["number"]
                .as_u64()
                .ok_or_else(|| ProjectError::GitHub("Missing project number".to_string()))?
                as u32,
            title: project["title"]
                .as_str()
                .ok_or_else(|| ProjectError::GitHub("Missing project title".to_string()))?
                .to_string(),
            url: project["url"]
                .as_str()
                .ok_or_else(|| ProjectError::GitHub("Missing project URL".to_string()))?
                .to_string(),
        };

        info!(
            "Created Project V2: {} (#{}) for {} {}",
            result.title,
            result.number,
            if is_org { "org" } else { "user" },
            owner
        );

        Ok(result)
    }

    /// Get a Project V2 by number.
    pub async fn get_project(
        &self,
        owner: &str,
        project_number: u32,
    ) -> Result<GitHubProjectV2, ProjectError> {
        let (_, is_org) = self.get_owner_id(owner).await?;

        let query = if is_org {
            r#"
                query($owner: String!, $number: Int!) {
                    organization(login: $owner) {
                        projectV2(number: $number) {
                            id
                            number
                            title
                            url
                        }
                    }
                }
            "#
        } else {
            r#"
                query($owner: String!, $number: Int!) {
                    user(login: $owner) {
                        projectV2(number: $number) {
                            id
                            number
                            title
                            url
                        }
                    }
                }
            "#
        };

        let variables = json!({
            "owner": owner,
            "number": project_number as i64,
        });

        let data = self.execute(query, Some(variables)).await?;
        let project = if is_org {
            &data["organization"]["projectV2"]
        } else {
            &data["user"]["projectV2"]
        };

        Ok(GitHubProjectV2 {
            id: project["id"]
                .as_str()
                .ok_or_else(|| ProjectError::GitHub("Project not found".to_string()))?
                .to_string(),
            number: project["number"].as_u64().unwrap_or(0) as u32,
            title: project["title"].as_str().unwrap_or("").to_string(),
            url: project["url"].as_str().unwrap_or("").to_string(),
        })
    }

    /// List Projects V2 for an owner.
    pub async fn list_projects(
        &self,
        owner: &str,
        limit: Option<u32>,
    ) -> Result<Vec<GitHubProjectV2>, ProjectError> {
        let (_, is_org) = self.get_owner_id(owner).await?;
        let limit = limit.unwrap_or(20).min(100);

        let query = if is_org {
            r#"
                query($owner: String!, $limit: Int!) {
                    organization(login: $owner) {
                        projectsV2(first: $limit) {
                            nodes {
                                id
                                number
                                title
                                url
                            }
                        }
                    }
                }
            "#
        } else {
            r#"
                query($owner: String!, $limit: Int!) {
                    user(login: $owner) {
                        projectsV2(first: $limit) {
                            nodes {
                                id
                                number
                                title
                                url
                            }
                        }
                    }
                }
            "#
        };

        let variables = json!({
            "owner": owner,
            "limit": limit as i64,
        });

        let data = self.execute(query, Some(variables)).await?;
        let nodes = if is_org {
            &data["organization"]["projectsV2"]["nodes"]
        } else {
            &data["user"]["projectsV2"]["nodes"]
        };

        let projects: Vec<GitHubProjectV2> = nodes
            .as_array()
            .unwrap_or(&vec![])
            .iter()
            .filter_map(|p| {
                Some(GitHubProjectV2 {
                    id: p["id"].as_str()?.to_string(),
                    number: p["number"].as_u64()? as u32,
                    title: p["title"].as_str()?.to_string(),
                    url: p["url"].as_str()?.to_string(),
                })
            })
            .collect();

        Ok(projects)
    }

    /// Link a Project V2 to a repository.
    pub async fn link_project_to_repository(
        &self,
        project_id: &str,
        owner: &str,
        repo: &str,
    ) -> Result<(), ProjectError> {
        let repo_id = self.get_repository_id(owner, repo).await?;

        let mutation = r#"
            mutation($projectId: ID!, $repositoryId: ID!) {
                linkProjectV2ToRepository(input: {
                    projectId: $projectId,
                    repositoryId: $repositoryId
                }) {
                    repository {
                        id
                    }
                }
            }
        "#;

        let variables = json!({
            "projectId": project_id,
            "repositoryId": repo_id,
        });

        self.execute(mutation, Some(variables)).await?;
        info!(
            "Linked project {} to repository {}/{}",
            project_id, owner, repo
        );

        Ok(())
    }

    // ========================================================================
    // Field Operations
    // ========================================================================

    /// Get fields for a Project V2.
    pub async fn get_project_fields(
        &self,
        project_id: &str,
    ) -> Result<Vec<GitHubProjectField>, ProjectError> {
        let query = r#"
            query($projectId: ID!) {
                node(id: $projectId) {
                    ... on ProjectV2 {
                        fields(first: 50) {
                            nodes {
                                ... on ProjectV2Field {
                                    id
                                    name
                                    dataType
                                }
                                ... on ProjectV2SingleSelectField {
                                    id
                                    name
                                    dataType
                                    options {
                                        id
                                        name
                                    }
                                }
                                ... on ProjectV2IterationField {
                                    id
                                    name
                                    dataType
                                }
                            }
                        }
                    }
                }
            }
        "#;

        let variables = json!({ "projectId": project_id });
        let data = self.execute(query, Some(variables)).await?;

        let nodes = &data["node"]["fields"]["nodes"];
        let fields: Vec<GitHubProjectField> = nodes
            .as_array()
            .unwrap_or(&vec![])
            .iter()
            .filter_map(|f| {
                let id = f["id"].as_str()?.to_string();
                let name = f["name"].as_str()?.to_string();
                let field_type = f["dataType"].as_str().unwrap_or("").to_string();
                let options: Vec<GitHubFieldOption> = f["options"]
                    .as_array()
                    .unwrap_or(&vec![])
                    .iter()
                    .filter_map(|o| {
                        Some(GitHubFieldOption {
                            id: o["id"].as_str()?.to_string(),
                            name: o["name"].as_str()?.to_string(),
                        })
                    })
                    .collect();

                Some(GitHubProjectField {
                    id,
                    name,
                    field_type,
                    options,
                })
            })
            .collect();

        Ok(fields)
    }

    /// Create a single-select field.
    pub async fn create_single_select_field(
        &self,
        project_id: &str,
        name: &str,
        options: &[String],
    ) -> Result<GitHubProjectField, ProjectError> {
        let mutation = r#"
            mutation($projectId: ID!, $name: String!, $options: [ProjectV2SingleSelectFieldOptionInput!]!) {
                createProjectV2Field(input: {
                    projectId: $projectId,
                    dataType: SINGLE_SELECT,
                    name: $name,
                    singleSelectOptions: $options
                }) {
                    projectV2Field {
                        ... on ProjectV2SingleSelectField {
                            id
                            name
                            dataType
                            options {
                                id
                                name
                            }
                        }
                    }
                }
            }
        "#;

        let options_input: Vec<Value> = options
            .iter()
            .map(|o| json!({ "name": o, "color": "GRAY" }))
            .collect();

        let variables = json!({
            "projectId": project_id,
            "name": name,
            "options": options_input,
        });

        let data = self.execute(mutation, Some(variables)).await?;
        let field = &data["createProjectV2Field"]["projectV2Field"];

        let options: Vec<GitHubFieldOption> = field["options"]
            .as_array()
            .unwrap_or(&vec![])
            .iter()
            .filter_map(|o| {
                Some(GitHubFieldOption {
                    id: o["id"].as_str()?.to_string(),
                    name: o["name"].as_str()?.to_string(),
                })
            })
            .collect();

        info!(
            "Created single-select field '{}' with {} options",
            name,
            options.len()
        );

        Ok(GitHubProjectField {
            id: field["id"]
                .as_str()
                .ok_or_else(|| ProjectError::GitHub("Missing field ID".to_string()))?
                .to_string(),
            name: field["name"].as_str().unwrap_or("").to_string(),
            field_type: "SINGLE_SELECT".to_string(),
            options,
        })
    }

    /// Update the Status field options (Status is a built-in field).
    pub async fn update_status_field_options(
        &self,
        project_id: &str,
        options: &[String],
    ) -> Result<GitHubProjectField, ProjectError> {
        // First, get the Status field ID
        let fields = self.get_project_fields(project_id).await?;
        let status_field = fields
            .iter()
            .find(|f| f.name == "Status")
            .ok_or_else(|| ProjectError::GitHub("Status field not found".to_string()))?;

        // Update Status field options
        let mutation = r#"
            mutation($projectId: ID!, $fieldId: ID!, $options: [ProjectV2SingleSelectFieldOptionInput!]!) {
                updateProjectV2Field(input: {
                    projectId: $projectId,
                    fieldId: $fieldId,
                    singleSelectOptions: $options
                }) {
                    projectV2Field {
                        ... on ProjectV2SingleSelectField {
                            id
                            name
                            options {
                                id
                                name
                            }
                        }
                    }
                }
            }
        "#;

        let options_input: Vec<Value> = options
            .iter()
            .map(|o| json!({ "name": o, "color": "GRAY" }))
            .collect();

        let variables = json!({
            "projectId": project_id,
            "fieldId": status_field.id,
            "options": options_input,
        });

        let data = self.execute(mutation, Some(variables)).await?;
        let field = &data["updateProjectV2Field"]["projectV2Field"];

        let updated_options: Vec<GitHubFieldOption> = field["options"]
            .as_array()
            .unwrap_or(&vec![])
            .iter()
            .filter_map(|o| {
                Some(GitHubFieldOption {
                    id: o["id"].as_str()?.to_string(),
                    name: o["name"].as_str()?.to_string(),
                })
            })
            .collect();

        info!(
            "Updated Status field with {} options",
            updated_options.len()
        );

        Ok(GitHubProjectField {
            id: status_field.id.clone(),
            name: "Status".to_string(),
            field_type: "SINGLE_SELECT".to_string(),
            options: updated_options,
        })
    }

    // ========================================================================
    // Item Operations
    // ========================================================================

    /// Add an issue to a Project V2.
    pub async fn add_issue_to_project(
        &self,
        project_id: &str,
        issue_node_id: &str,
    ) -> Result<String, ProjectError> {
        let mutation = r#"
            mutation($projectId: ID!, $contentId: ID!) {
                addProjectV2ItemById(input: {
                    projectId: $projectId,
                    contentId: $contentId
                }) {
                    item {
                        id
                    }
                }
            }
        "#;

        let variables = json!({
            "projectId": project_id,
            "contentId": issue_node_id,
        });

        let data = self.execute(mutation, Some(variables)).await?;
        let item_id = data["addProjectV2ItemById"]["item"]["id"]
            .as_str()
            .ok_or_else(|| ProjectError::GitHub("Failed to add issue to project".to_string()))?;

        info!("Added issue {} to project {}", issue_node_id, project_id);
        Ok(item_id.to_string())
    }

    /// Create a draft issue in a Project V2.
    pub async fn create_draft_issue(
        &self,
        project_id: &str,
        title: &str,
        body: Option<&str>,
    ) -> Result<String, ProjectError> {
        let mutation = r#"
            mutation($projectId: ID!, $title: String!, $body: String) {
                addProjectV2DraftIssue(input: {
                    projectId: $projectId,
                    title: $title,
                    body: $body
                }) {
                    projectItem {
                        id
                    }
                }
            }
        "#;

        let variables = json!({
            "projectId": project_id,
            "title": title,
            "body": body,
        });

        let data = self.execute(mutation, Some(variables)).await?;
        let item_id = data["addProjectV2DraftIssue"]["projectItem"]["id"]
            .as_str()
            .ok_or_else(|| ProjectError::GitHub("Failed to create draft issue".to_string()))?;

        info!("Created draft issue '{}' in project {}", title, project_id);
        Ok(item_id.to_string())
    }

    /// Update a project item field value.
    pub async fn update_item_field(
        &self,
        project_id: &str,
        item_id: &str,
        field_id: &str,
        value: &str,
    ) -> Result<(), ProjectError> {
        let mutation = r#"
            mutation($projectId: ID!, $itemId: ID!, $fieldId: ID!, $value: ProjectV2FieldValue!) {
                updateProjectV2ItemFieldValue(input: {
                    projectId: $projectId,
                    itemId: $itemId,
                    fieldId: $fieldId,
                    value: $value
                }) {
                    projectV2Item {
                        id
                    }
                }
            }
        "#;

        let variables = json!({
            "projectId": project_id,
            "itemId": item_id,
            "fieldId": field_id,
            "value": {
                "singleSelectOptionId": value
            }
        });

        self.execute(mutation, Some(variables)).await?;
        debug!(
            "Updated field {} for item {} to {}",
            field_id, item_id, value
        );

        Ok(())
    }

    /// Get items in a Project V2.
    pub async fn get_project_items(
        &self,
        project_id: &str,
        limit: Option<u32>,
    ) -> Result<Vec<GitHubProjectItem>, ProjectError> {
        let limit = limit.unwrap_or(50).min(100);

        let query = r#"
            query($projectId: ID!, $limit: Int!) {
                node(id: $projectId) {
                    ... on ProjectV2 {
                        items(first: $limit) {
                            nodes {
                                id
                                fieldValues(first: 20) {
                                    nodes {
                                        ... on ProjectV2ItemFieldSingleSelectValue {
                                            name
                                            field {
                                                ... on ProjectV2SingleSelectField {
                                                    name
                                                }
                                            }
                                        }
                                        ... on ProjectV2ItemFieldTextValue {
                                            text
                                            field {
                                                ... on ProjectV2Field {
                                                    name
                                                }
                                            }
                                        }
                                    }
                                }
                                content {
                                    __typename
                                    ... on Issue {
                                        id
                                        number
                                        title
                                        state
                                    }
                                    ... on DraftIssue {
                                        title
                                        body
                                    }
                                    ... on PullRequest {
                                        id
                                        number
                                        title
                                    }
                                }
                            }
                        }
                    }
                }
            }
        "#;

        let variables = json!({
            "projectId": project_id,
            "limit": limit as i64,
        });

        let data = self.execute(query, Some(variables)).await?;
        let nodes = &data["node"]["items"]["nodes"];

        let items: Vec<GitHubProjectItem> = nodes
            .as_array()
            .unwrap_or(&vec![])
            .iter()
            .filter_map(|item| {
                let id = item["id"].as_str()?.to_string();

                // Parse field values
                let field_values = item["fieldValues"]["nodes"]
                    .as_array()
                    .unwrap_or(&vec![])
                    .iter()
                    .filter_map(|fv| {
                        let field_name = fv["field"]["name"].as_str()?.to_string();
                        let value = fv["name"]
                            .as_str()
                            .or_else(|| fv["text"].as_str())?
                            .to_string();
                        Some(crate::github::models::GitHubFieldValue { field_name, value })
                    })
                    .collect();

                // Parse content
                let content_raw = &item["content"];
                let content_type = content_raw["__typename"].as_str()?;
                let content = match content_type {
                    "Issue" => Some(GitHubProjectItemContent::Issue {
                        id: content_raw["id"].as_str()?.to_string(),
                        number: content_raw["number"].as_u64()? as u32,
                        title: content_raw["title"].as_str()?.to_string(),
                        state: content_raw["state"].as_str()?.to_string(),
                    }),
                    "DraftIssue" => Some(GitHubProjectItemContent::DraftIssue {
                        title: content_raw["title"].as_str()?.to_string(),
                        body: content_raw["body"].as_str().map(|s| s.to_string()),
                    }),
                    "PullRequest" => Some(GitHubProjectItemContent::PullRequest {
                        id: content_raw["id"].as_str()?.to_string(),
                        number: content_raw["number"].as_u64()? as u32,
                        title: content_raw["title"].as_str()?.to_string(),
                    }),
                    _ => None,
                };

                Some(GitHubProjectItem {
                    id,
                    field_values,
                    content,
                })
            })
            .collect();

        Ok(items)
    }

    /// Delete a project item.
    pub async fn delete_project_item(
        &self,
        project_id: &str,
        item_id: &str,
    ) -> Result<(), ProjectError> {
        let mutation = r#"
            mutation($projectId: ID!, $itemId: ID!) {
                deleteProjectV2Item(input: {
                    projectId: $projectId,
                    itemId: $itemId
                }) {
                    deletedItemId
                }
            }
        "#;

        let variables = json!({
            "projectId": project_id,
            "itemId": item_id,
        });

        self.execute(mutation, Some(variables)).await?;
        info!("Deleted item {} from project {}", item_id, project_id);

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_creation() {
        let client = GitHubGraphQLClient::new("test_token");
        assert!(client.is_ok());
    }

    #[test]
    fn test_graphql_request_serialization() {
        let request = GraphQLRequest {
            query: "query { viewer { login } }".to_string(),
            variables: Some(json!({ "foo": "bar" })),
        };

        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("viewer"));
        assert!(json.contains("foo"));
    }

    #[test]
    fn test_graphql_request_without_variables() {
        let request = GraphQLRequest {
            query: "query { viewer { login } }".to_string(),
            variables: None,
        };

        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("query"));
        assert!(!json.contains("variables"));
    }
}
