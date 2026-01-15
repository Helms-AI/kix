//! Data Explorer API Routes
//!
//! Provides REST API endpoints for the admin Data Explorer interface,
//! enabling database discovery, schema inspection, and query execution.

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{error, info};

use kix_services::{
    discover_databases, execute_sqlite_query, get_query_templates, get_sqlite_schema,
    get_table_data, DatabaseInfo, DatabaseType, QueryResult, QueryTemplate, TableSchema,
};

/// State for explorer routes
#[derive(Clone)]
pub struct ExplorerState {
    /// Base data directory for database discovery
    pub data_dir: PathBuf,
    /// Cache of discovered databases
    pub database_cache: Arc<RwLock<Vec<DatabaseInfo>>>,
}

impl ExplorerState {
    pub fn new(data_dir: PathBuf) -> Self {
        Self {
            data_dir,
            database_cache: Arc::new(RwLock::new(Vec::new())),
        }
    }
}

// ============================================================================
// Request/Response Types
// ============================================================================

#[derive(Debug, Deserialize)]
pub struct QueryParams {
    pub query: String,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
    #[serde(default)]
    pub safe_mode: bool,
}

#[derive(Debug, Deserialize)]
pub struct TableDataParams {
    pub limit: Option<u32>,
    pub offset: Option<u32>,
    pub order_by: Option<String>,
    pub order_dir: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct DatabaseListResponse {
    pub databases: Vec<DatabaseInfo>,
    pub total: usize,
}

#[derive(Debug, Serialize)]
pub struct SchemaResponse {
    pub database_id: String,
    pub database_name: String,
    pub tables: Vec<TableSchema>,
}

#[derive(Debug, Serialize)]
pub struct QueryResponse {
    pub success: bool,
    pub result: Option<QueryResult>,
    pub error: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct TemplatesResponse {
    pub templates: Vec<QueryTemplate>,
}

#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub error: String,
    pub message: String,
}

// ============================================================================
// Route Handlers
// ============================================================================

/// List all discovered databases
async fn list_databases(
    State(state): State<ExplorerState>,
) -> Result<Json<DatabaseListResponse>, (StatusCode, Json<ErrorResponse>)> {
    info!("Discovering databases in {:?}", state.data_dir);

    match discover_databases(&state.data_dir).await {
        Ok(databases) => {
            let total = databases.len();

            // Update cache
            {
                let mut cache = state.database_cache.write().await;
                *cache = databases.clone();
            }

            Ok(Json(DatabaseListResponse { databases, total }))
        }
        Err(e) => {
            error!("Failed to discover databases: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "discovery_failed".to_string(),
                    message: e.to_string(),
                }),
            ))
        }
    }
}

/// Refresh database list (force re-scan)
async fn refresh_databases(
    State(state): State<ExplorerState>,
) -> Result<Json<DatabaseListResponse>, (StatusCode, Json<ErrorResponse>)> {
    info!("Force refreshing database list");
    list_databases(State(state)).await
}

/// Get schema for a specific database
async fn get_database_schema(
    State(state): State<ExplorerState>,
    Path(database_id): Path<String>,
) -> Result<Json<SchemaResponse>, (StatusCode, Json<ErrorResponse>)> {
    // Find database in cache
    let cache = state.database_cache.read().await;
    let database = cache
        .iter()
        .find(|db| db.id == database_id)
        .cloned();
    drop(cache);

    let database = match database {
        Some(db) => db,
        None => {
            // Try to refresh cache and find again
            drop(discover_databases(&state.data_dir).await);
            let cache = state.database_cache.read().await;
            match cache.iter().find(|db| db.id == database_id).cloned() {
                Some(db) => db,
                None => {
                    return Err((
                        StatusCode::NOT_FOUND,
                        Json(ErrorResponse {
                            error: "database_not_found".to_string(),
                            message: format!("Database with id '{}' not found", database_id),
                        }),
                    ));
                }
            }
        }
    };

    // Only SQLite databases have schema inspection
    if database.db_type != DatabaseType::Sqlite {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "unsupported_database_type".to_string(),
                message: "Schema inspection is only available for SQLite databases".to_string(),
            }),
        ));
    }

    match get_sqlite_schema(&database.path).await {
        Ok(tables) => Ok(Json(SchemaResponse {
            database_id: database.id,
            database_name: database.name,
            tables,
        })),
        Err(e) => {
            error!("Failed to get schema for {}: {}", database_id, e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "schema_fetch_failed".to_string(),
                    message: e.to_string(),
                }),
            ))
        }
    }
}

/// Execute a SQL query against a database
async fn execute_query(
    State(state): State<ExplorerState>,
    Path(database_id): Path<String>,
    Json(params): Json<QueryParams>,
) -> Result<Json<QueryResponse>, (StatusCode, Json<ErrorResponse>)> {
    // Find database
    let cache = state.database_cache.read().await;
    let database = cache.iter().find(|db| db.id == database_id).cloned();
    drop(cache);

    let database = match database {
        Some(db) => db,
        None => {
            return Err((
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: "database_not_found".to_string(),
                    message: format!("Database with id '{}' not found", database_id),
                }),
            ));
        }
    };

    // Only SQLite databases support SQL queries
    if database.db_type != DatabaseType::Sqlite {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "unsupported_database_type".to_string(),
                message: "SQL queries are only available for SQLite databases".to_string(),
            }),
        ));
    }

    info!(
        "Executing query on {}: {}",
        database_id,
        &params.query[..params.query.len().min(100)]
    );

    match execute_sqlite_query(
        &database.path,
        &params.query,
        params.limit.or(Some(1000)), // Default limit
        params.offset,
        params.safe_mode,
    )
    .await
    {
        Ok(result) => Ok(Json(QueryResponse {
            success: true,
            result: Some(result),
            error: None,
        })),
        Err(e) => {
            error!("Query execution failed: {}", e);
            Ok(Json(QueryResponse {
                success: false,
                result: None,
                error: Some(e.to_string()),
            }))
        }
    }
}

/// Get table data with pagination
async fn get_table(
    State(state): State<ExplorerState>,
    Path((database_id, table_name)): Path<(String, String)>,
    Query(params): Query<TableDataParams>,
) -> Result<Json<QueryResponse>, (StatusCode, Json<ErrorResponse>)> {
    // Find database
    let cache = state.database_cache.read().await;
    let database = cache.iter().find(|db| db.id == database_id).cloned();
    drop(cache);

    let database = match database {
        Some(db) => db,
        None => {
            return Err((
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: "database_not_found".to_string(),
                    message: format!("Database with id '{}' not found", database_id),
                }),
            ));
        }
    };

    if database.db_type != DatabaseType::Sqlite {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "unsupported_database_type".to_string(),
                message: "Table data is only available for SQLite databases".to_string(),
            }),
        ));
    }

    let limit = params.limit.unwrap_or(50);
    let offset = params.offset.unwrap_or(0);

    match get_table_data(
        &database.path,
        &table_name,
        limit,
        offset,
        params.order_by.as_deref(),
        params.order_dir.as_deref(),
    )
    .await
    {
        Ok(result) => Ok(Json(QueryResponse {
            success: true,
            result: Some(result),
            error: None,
        })),
        Err(e) => {
            error!("Failed to get table data: {}", e);
            Ok(Json(QueryResponse {
                success: false,
                result: None,
                error: Some(e.to_string()),
            }))
        }
    }
}

/// Get available query templates
async fn get_templates() -> Json<TemplatesResponse> {
    Json(TemplatesResponse {
        templates: get_query_templates(),
    })
}

/// Get a specific database by ID
async fn get_database(
    State(state): State<ExplorerState>,
    Path(database_id): Path<String>,
) -> Result<Json<DatabaseInfo>, (StatusCode, Json<ErrorResponse>)> {
    let cache = state.database_cache.read().await;
    let database = cache.iter().find(|db| db.id == database_id).cloned();
    drop(cache);

    match database {
        Some(db) => Ok(Json(db)),
        None => Err((
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: "database_not_found".to_string(),
                message: format!("Database with id '{}' not found", database_id),
            }),
        )),
    }
}

// ============================================================================
// Router
// ============================================================================

/// Create the explorer router
pub fn create_explorer_router(state: ExplorerState) -> Router {
    Router::new()
        // Database discovery
        .route("/api/explorer/databases", get(list_databases))
        .route("/api/explorer/databases/refresh", post(refresh_databases))
        .route("/api/explorer/databases/:database_id", get(get_database))
        // Schema inspection
        .route(
            "/api/explorer/databases/:database_id/schema",
            get(get_database_schema),
        )
        // Query execution
        .route(
            "/api/explorer/databases/:database_id/query",
            post(execute_query),
        )
        // Table data
        .route(
            "/api/explorer/databases/:database_id/tables/:table_name",
            get(get_table),
        )
        // Templates
        .route("/api/explorer/templates", get(get_templates))
        .with_state(state)
}
