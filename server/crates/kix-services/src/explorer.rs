//! Data Explorer Service
//!
//! Provides database discovery, schema inspection, and query execution
//! for the admin Data Explorer interface.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use tokio::fs;
use tracing::{debug, info, warn};

use crate::error::{ServiceError, ServiceResult};

/// Database type enumeration
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum DatabaseType {
    Sqlite,
    Tantivy,
}

/// Information about a discovered database
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseInfo {
    pub id: String,
    pub name: String,
    pub db_type: DatabaseType,
    pub path: String,
    pub size_bytes: u64,
    pub size_display: String,
    pub modified: Option<String>,
    pub status: DatabaseStatus,
    pub tables: Option<Vec<String>>,
}

/// Database availability status
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum DatabaseStatus {
    Available,
    Locked,
    Scanning,
    Error,
}

/// Table schema information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableSchema {
    pub name: String,
    pub columns: Vec<ColumnInfo>,
    pub row_count: Option<u64>,
    pub indexes: Vec<IndexInfo>,
    pub foreign_keys: Vec<ForeignKeyInfo>,
}

/// Column information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColumnInfo {
    pub name: String,
    pub data_type: String,
    pub nullable: bool,
    pub primary_key: bool,
    pub default_value: Option<String>,
}

/// Index information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexInfo {
    pub name: String,
    pub columns: Vec<String>,
    pub unique: bool,
}

/// Foreign key information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForeignKeyInfo {
    pub column: String,
    pub references_table: String,
    pub references_column: String,
    pub on_delete: Option<String>,
    pub on_update: Option<String>,
}

/// Query execution request
#[derive(Debug, Clone, Deserialize)]
pub struct QueryRequest {
    pub database_id: String,
    pub query: String,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
    pub safe_mode: Option<bool>,
}

/// Query execution result
#[derive(Debug, Clone, Serialize)]
pub struct QueryResult {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<serde_json::Value>>,
    pub row_count: usize,
    pub execution_time_ms: u64,
    pub affected_rows: Option<u64>,
    pub query_type: QueryType,
    pub warnings: Vec<String>,
}

/// Type of SQL query
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum QueryType {
    Select,
    Insert,
    Update,
    Delete,
    Create,
    Drop,
    Alter,
    Other,
}

/// Data modification request (for CRUD)
#[derive(Debug, Clone, Deserialize)]
pub struct DataModifyRequest {
    pub database_id: String,
    pub table: String,
    pub operation: DataOperation,
    pub data: Option<HashMap<String, serde_json::Value>>,
    pub where_clause: Option<String>,
    pub preview_only: bool,
}

/// Data operation type
#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum DataOperation {
    Insert,
    Update,
    Delete,
}

/// Result of a data modification
#[derive(Debug, Clone, Serialize)]
pub struct DataModifyResult {
    pub success: bool,
    pub affected_rows: u64,
    pub preview_sql: Option<String>,
    pub execution_time_ms: u64,
    pub message: String,
}

/// Query history entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryHistoryEntry {
    pub id: String,
    pub database_id: String,
    pub query: String,
    pub executed_at: String,
    pub execution_time_ms: u64,
    pub row_count: Option<usize>,
    pub success: bool,
    pub error: Option<String>,
}

/// Query template
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryTemplate {
    pub id: String,
    pub name: String,
    pub description: String,
    pub query: String,
    pub database_type: DatabaseType,
    pub category: String,
    pub parameters: Vec<TemplateParameter>,
}

/// Template parameter
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateParameter {
    pub name: String,
    pub description: String,
    pub data_type: String,
    pub default_value: Option<String>,
}

/// Format file size for display
fn format_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if bytes >= GB {
        format!("{:.1} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} bytes", bytes)
    }
}

/// Generate a unique ID for a database based on its path
fn generate_db_id(path: &Path) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut hasher = DefaultHasher::new();
    path.to_string_lossy().hash(&mut hasher);
    format!("db_{:x}", hasher.finish())
}

/// Discover all databases in a directory
pub async fn discover_databases(data_dir: &Path) -> ServiceResult<Vec<DatabaseInfo>> {
    let mut databases = Vec::new();

    if !data_dir.exists() {
        return Ok(databases);
    }

    // Recursively scan for database files
    scan_directory(data_dir, &mut databases).await?;

    // Sort by name
    databases.sort_by(|a, b| a.name.cmp(&b.name));

    info!(
        "Discovered {} databases in {:?}",
        databases.len(),
        data_dir
    );

    Ok(databases)
}

/// Recursively scan a directory for database files
async fn scan_directory(dir: &Path, databases: &mut Vec<DatabaseInfo>) -> ServiceResult<()> {
    let mut entries = match fs::read_dir(dir).await {
        Ok(entries) => entries,
        Err(e) => {
            warn!("Failed to read directory {:?}: {}", dir, e);
            return Ok(());
        }
    };

    while let Some(entry) = entries.next_entry().await.map_err(|e| {
        ServiceError::Internal(format!("Failed to read directory entry: {}", e))
    })? {
        let path = entry.path();

        if path.is_dir() {
            // Check if it's a Tantivy index directory
            if is_tantivy_index(&path).await {
                if let Some(db_info) = create_tantivy_info(&path).await {
                    databases.push(db_info);
                }
            } else {
                // Recurse into subdirectory
                Box::pin(scan_directory(&path, databases)).await?;
            }
        } else if path.extension().map_or(false, |ext| ext == "db" || ext == "sqlite" || ext == "sqlite3") {
            // SQLite database file
            if let Some(db_info) = create_sqlite_info(&path).await {
                databases.push(db_info);
            }
        }
    }

    Ok(())
}

/// Check if a directory is a Tantivy index
async fn is_tantivy_index(dir: &Path) -> bool {
    // Tantivy indexes have a meta.json file
    let meta_path = dir.join("meta.json");
    meta_path.exists()
}

/// Create database info for a SQLite database
async fn create_sqlite_info(path: &Path) -> Option<DatabaseInfo> {
    let metadata = fs::metadata(path).await.ok()?;
    let size = metadata.len();
    let modified = metadata.modified().ok().map(|t| {
        chrono::DateTime::<chrono::Utc>::from(t).to_rfc3339()
    });

    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "unknown".to_string());

    Some(DatabaseInfo {
        id: generate_db_id(path),
        name,
        db_type: DatabaseType::Sqlite,
        path: path.to_string_lossy().to_string(),
        size_bytes: size,
        size_display: format_size(size),
        modified,
        status: DatabaseStatus::Available,
        tables: None, // Populated on demand
    })
}

/// Create database info for a Tantivy index
async fn create_tantivy_info(dir: &Path) -> Option<DatabaseInfo> {
    // Calculate total size of the index directory
    let size = calculate_dir_size(dir).await.unwrap_or(0);

    let name = dir
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "unknown".to_string());

    let modified = fs::metadata(dir).await.ok().and_then(|m| {
        m.modified().ok().map(|t| {
            chrono::DateTime::<chrono::Utc>::from(t).to_rfc3339()
        })
    });

    Some(DatabaseInfo {
        id: generate_db_id(dir),
        name,
        db_type: DatabaseType::Tantivy,
        path: dir.to_string_lossy().to_string(),
        size_bytes: size,
        size_display: format_size(size),
        modified,
        status: DatabaseStatus::Available,
        tables: None,
    })
}

/// Calculate the total size of a directory
async fn calculate_dir_size(dir: &Path) -> Option<u64> {
    let mut total: u64 = 0;
    let mut entries = fs::read_dir(dir).await.ok()?;

    while let Some(entry) = entries.next_entry().await.ok()? {
        let path = entry.path();
        if path.is_file() {
            if let Ok(metadata) = fs::metadata(&path).await {
                total += metadata.len();
            }
        } else if path.is_dir() {
            if let Some(subdir_size) = Box::pin(calculate_dir_size(&path)).await {
                total += subdir_size;
            }
        }
    }

    Some(total)
}

/// Get schema for a SQLite database
pub async fn get_sqlite_schema(db_path: &str) -> ServiceResult<Vec<TableSchema>> {
    use sqlx::sqlite::SqlitePool;
    use sqlx::Row;

    let pool = SqlitePool::connect(&format!("sqlite:{}", db_path))
        .await
        .map_err(|e| ServiceError::Internal(format!("Failed to connect to database: {}", e)))?;

    // Get all tables
    let tables: Vec<String> = sqlx::query("SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%' ORDER BY name")
        .fetch_all(&pool)
        .await
        .map_err(|e| ServiceError::Internal(format!("Failed to query tables: {}", e)))?
        .iter()
        .map(|row| row.get::<String, _>("name"))
        .collect();

    let mut schemas = Vec::new();

    for table_name in tables {
        // Get column info
        let columns: Vec<ColumnInfo> = sqlx::query(&format!("PRAGMA table_info('{}')", table_name))
            .fetch_all(&pool)
            .await
            .map_err(|e| ServiceError::Internal(format!("Failed to get table info: {}", e)))?
            .iter()
            .map(|row| ColumnInfo {
                name: row.get::<String, _>("name"),
                data_type: row.get::<String, _>("type"),
                nullable: row.get::<i32, _>("notnull") == 0,
                primary_key: row.get::<i32, _>("pk") > 0,
                default_value: row.try_get::<String, _>("dflt_value").ok(),
            })
            .collect();

        // Get indexes
        let indexes: Vec<IndexInfo> = sqlx::query(&format!("PRAGMA index_list('{}')", table_name))
            .fetch_all(&pool)
            .await
            .map_err(|e| ServiceError::Internal(format!("Failed to get indexes: {}", e)))?
            .iter()
            .filter_map(|row| {
                let index_name: String = row.get("name");
                let unique: i32 = row.get("unique");

                // Skip auto-generated indexes
                if index_name.starts_with("sqlite_") {
                    return None;
                }

                Some(IndexInfo {
                    name: index_name,
                    columns: Vec::new(), // Would need another query to get columns
                    unique: unique == 1,
                })
            })
            .collect();

        // Get foreign keys
        let foreign_keys: Vec<ForeignKeyInfo> = sqlx::query(&format!("PRAGMA foreign_key_list('{}')", table_name))
            .fetch_all(&pool)
            .await
            .map_err(|e| ServiceError::Internal(format!("Failed to get foreign keys: {}", e)))?
            .iter()
            .map(|row| ForeignKeyInfo {
                column: row.get::<String, _>("from"),
                references_table: row.get::<String, _>("table"),
                references_column: row.get::<String, _>("to"),
                on_delete: row.try_get::<String, _>("on_delete").ok(),
                on_update: row.try_get::<String, _>("on_update").ok(),
            })
            .collect();

        // Get row count
        let row_count: Option<u64> = sqlx::query(&format!("SELECT COUNT(*) as count FROM '{}'", table_name))
            .fetch_one(&pool)
            .await
            .ok()
            .map(|row| row.get::<i64, _>("count") as u64);

        schemas.push(TableSchema {
            name: table_name,
            columns,
            row_count,
            indexes,
            foreign_keys,
        });
    }

    pool.close().await;

    Ok(schemas)
}

/// Execute a SQL query against a SQLite database
pub async fn execute_sqlite_query(
    db_path: &str,
    query: &str,
    limit: Option<u32>,
    offset: Option<u32>,
    safe_mode: bool,
) -> ServiceResult<QueryResult> {
    use sqlx::sqlite::SqlitePool;
    use sqlx::{Column, Row, TypeInfo};
    use std::time::Instant;

    let start = Instant::now();
    let warnings = Vec::new();

    // Determine query type
    let query_type = classify_query(query);

    // In safe mode, block destructive queries
    if safe_mode && matches!(query_type, QueryType::Delete | QueryType::Drop | QueryType::Update) {
        return Err(ServiceError::Validation(
            "Destructive queries are blocked in safe mode. Disable safe mode to execute this query.".to_string()
        ));
    }

    let pool = SqlitePool::connect(&format!("sqlite:{}", db_path))
        .await
        .map_err(|e| ServiceError::Internal(format!("Failed to connect to database: {}", e)))?;

    // Build the final query with limit/offset for SELECT queries
    let final_query = if query_type == QueryType::Select {
        let mut q = query.trim_end_matches(';').to_string();

        // Check if query already has LIMIT
        let query_lower = q.to_lowercase();
        if !query_lower.contains(" limit ") {
            if let Some(l) = limit {
                q.push_str(&format!(" LIMIT {}", l));
            }
        }

        // Check if query already has OFFSET
        if !query_lower.contains(" offset ") {
            if let Some(o) = offset {
                q.push_str(&format!(" OFFSET {}", o));
            }
        }

        q
    } else {
        query.to_string()
    };

    debug!("Executing query: {}", final_query);

    // Execute based on query type
    let result = if query_type == QueryType::Select {
        let rows = sqlx::query(&final_query)
            .fetch_all(&pool)
            .await
            .map_err(|e| ServiceError::Internal(format!("Query execution failed: {}", e)))?;

        let columns: Vec<String> = if let Some(first_row) = rows.first() {
            first_row.columns().iter().map(|c| c.name().to_string()).collect()
        } else {
            Vec::new()
        };

        let data_rows: Vec<Vec<serde_json::Value>> = rows
            .iter()
            .map(|row| {
                row.columns()
                    .iter()
                    .enumerate()
                    .map(|(i, col)| {
                        // Try to get value as different types
                        if let Ok(v) = row.try_get::<i64, _>(i) {
                            serde_json::Value::Number(v.into())
                        } else if let Ok(v) = row.try_get::<f64, _>(i) {
                            serde_json::Number::from_f64(v)
                                .map(serde_json::Value::Number)
                                .unwrap_or(serde_json::Value::Null)
                        } else if let Ok(v) = row.try_get::<String, _>(i) {
                            serde_json::Value::String(v)
                        } else if let Ok(v) = row.try_get::<bool, _>(i) {
                            serde_json::Value::Bool(v)
                        } else if let Ok(v) = row.try_get::<Vec<u8>, _>(i) {
                            // Binary data - show as hex
                            serde_json::Value::String(format!("0x{}", hex::encode(&v)))
                        } else {
                            // Check if it's NULL
                            let type_info = col.type_info();
                            if type_info.name() == "NULL" {
                                serde_json::Value::Null
                            } else {
                                serde_json::Value::String("[BLOB]".to_string())
                            }
                        }
                    })
                    .collect()
            })
            .collect();

        let row_count = data_rows.len();

        QueryResult {
            columns,
            rows: data_rows,
            row_count,
            execution_time_ms: start.elapsed().as_millis() as u64,
            affected_rows: None,
            query_type,
            warnings,
        }
    } else {
        // Execute non-SELECT query
        let result = sqlx::query(&final_query)
            .execute(&pool)
            .await
            .map_err(|e| ServiceError::Internal(format!("Query execution failed: {}", e)))?;

        QueryResult {
            columns: Vec::new(),
            rows: Vec::new(),
            row_count: 0,
            execution_time_ms: start.elapsed().as_millis() as u64,
            affected_rows: Some(result.rows_affected()),
            query_type,
            warnings,
        }
    };

    pool.close().await;

    Ok(result)
}

/// Classify a SQL query by type
fn classify_query(query: &str) -> QueryType {
    let normalized = query.trim().to_uppercase();

    if normalized.starts_with("SELECT") || normalized.starts_with("WITH") {
        QueryType::Select
    } else if normalized.starts_with("INSERT") {
        QueryType::Insert
    } else if normalized.starts_with("UPDATE") {
        QueryType::Update
    } else if normalized.starts_with("DELETE") {
        QueryType::Delete
    } else if normalized.starts_with("CREATE") {
        QueryType::Create
    } else if normalized.starts_with("DROP") {
        QueryType::Drop
    } else if normalized.starts_with("ALTER") {
        QueryType::Alter
    } else {
        QueryType::Other
    }
}

/// Get table data with pagination
pub async fn get_table_data(
    db_path: &str,
    table: &str,
    limit: u32,
    offset: u32,
    order_by: Option<&str>,
    order_dir: Option<&str>,
) -> ServiceResult<QueryResult> {
    // Validate table name to prevent SQL injection
    if !is_valid_identifier(table) {
        return Err(ServiceError::Validation(format!("Invalid table name: {}", table)));
    }

    let order_clause = if let Some(col) = order_by {
        if !is_valid_identifier(col) {
            return Err(ServiceError::Validation(format!("Invalid column name: {}", col)));
        }
        let dir = order_dir.unwrap_or("ASC").to_uppercase();
        if dir != "ASC" && dir != "DESC" {
            return Err(ServiceError::Validation("Order direction must be ASC or DESC".to_string()));
        }
        format!(" ORDER BY \"{}\" {}", col, dir)
    } else {
        String::new()
    };

    let query = format!(
        "SELECT * FROM \"{}\"{}",
        table, order_clause
    );

    execute_sqlite_query(db_path, &query, Some(limit), Some(offset), false).await
}

/// Validate an identifier (table/column name)
fn is_valid_identifier(name: &str) -> bool {
    !name.is_empty()
        && name.chars().all(|c| c.is_alphanumeric() || c == '_')
        && !name.chars().next().map_or(true, |c| c.is_numeric())
}

/// Get pre-built query templates
pub fn get_query_templates() -> Vec<QueryTemplate> {
    vec![
        QueryTemplate {
            id: "recent_entries".to_string(),
            name: "Recent Entries".to_string(),
            description: "Get the most recently created entries".to_string(),
            query: "SELECT * FROM entries ORDER BY created_at DESC LIMIT 100".to_string(),
            database_type: DatabaseType::Sqlite,
            category: "Entries".to_string(),
            parameters: Vec::new(),
        },
        QueryTemplate {
            id: "orphaned_pages".to_string(),
            name: "Orphaned Pages".to_string(),
            description: "Find pages without a corresponding entry".to_string(),
            query: "SELECT * FROM pages WHERE source_id NOT IN (SELECT id FROM entries)".to_string(),
            database_type: DatabaseType::Sqlite,
            category: "Maintenance".to_string(),
            parameters: Vec::new(),
        },
        QueryTemplate {
            id: "large_documents".to_string(),
            name: "Large Documents".to_string(),
            description: "Find entries with large content".to_string(),
            query: "SELECT id, title, LENGTH(content) as content_length FROM entries WHERE LENGTH(content) > {{min_size}} ORDER BY content_length DESC".to_string(),
            database_type: DatabaseType::Sqlite,
            category: "Analysis".to_string(),
            parameters: vec![
                TemplateParameter {
                    name: "min_size".to_string(),
                    description: "Minimum content size in characters".to_string(),
                    data_type: "integer".to_string(),
                    default_value: Some("50000".to_string()),
                },
            ],
        },
        QueryTemplate {
            id: "entries_by_domain".to_string(),
            name: "Entries by Domain".to_string(),
            description: "Get all entries from a specific source domain".to_string(),
            query: "SELECT * FROM entries WHERE source_domain = '{{domain}}' ORDER BY created_at DESC".to_string(),
            database_type: DatabaseType::Sqlite,
            category: "Entries".to_string(),
            parameters: vec![
                TemplateParameter {
                    name: "domain".to_string(),
                    description: "Source domain to filter by".to_string(),
                    data_type: "string".to_string(),
                    default_value: None,
                },
            ],
        },
        QueryTemplate {
            id: "domain_stats".to_string(),
            name: "Domain Statistics".to_string(),
            description: "Count entries per source domain".to_string(),
            query: "SELECT source_domain, COUNT(*) as count, SUM(LENGTH(content)) as total_content_size FROM entries GROUP BY source_domain ORDER BY count DESC".to_string(),
            database_type: DatabaseType::Sqlite,
            category: "Analysis".to_string(),
            parameters: Vec::new(),
        },
        QueryTemplate {
            id: "recent_jobs".to_string(),
            name: "Recent Jobs".to_string(),
            description: "Get recently completed indexing jobs".to_string(),
            query: "SELECT job_id, job_type, status, items_processed, chunks_created, duration_ms, created_at FROM jobs ORDER BY created_at DESC LIMIT 50".to_string(),
            database_type: DatabaseType::Sqlite,
            category: "Jobs".to_string(),
            parameters: Vec::new(),
        },
        QueryTemplate {
            id: "failed_jobs".to_string(),
            name: "Failed Jobs".to_string(),
            description: "Get jobs that failed or had errors".to_string(),
            query: "SELECT job_id, job_type, error_count, errors, created_at FROM jobs WHERE status = 'failed' OR error_count > 0 ORDER BY created_at DESC".to_string(),
            database_type: DatabaseType::Sqlite,
            category: "Jobs".to_string(),
            parameters: Vec::new(),
        },
        QueryTemplate {
            id: "project_issues".to_string(),
            name: "Project Issues".to_string(),
            description: "List all issues for a project".to_string(),
            query: "SELECT i.*, p.name as project_name FROM issues i JOIN projects p ON i.project_id = p.id WHERE p.slug = '{{project_slug}}' ORDER BY i.number DESC".to_string(),
            database_type: DatabaseType::Sqlite,
            category: "Projects".to_string(),
            parameters: vec![
                TemplateParameter {
                    name: "project_slug".to_string(),
                    description: "Project slug to filter by".to_string(),
                    data_type: "string".to_string(),
                    default_value: None,
                },
            ],
        },
        QueryTemplate {
            id: "storage_analysis".to_string(),
            name: "Storage Analysis".to_string(),
            description: "Analyze content storage by type".to_string(),
            query: r#"
SELECT
    entry_type,
    COUNT(*) as entry_count,
    SUM(LENGTH(content)) as total_content_bytes,
    AVG(LENGTH(content)) as avg_content_bytes
FROM entries
GROUP BY entry_type
ORDER BY total_content_bytes DESC
"#.to_string(),
            database_type: DatabaseType::Sqlite,
            category: "Analysis".to_string(),
            parameters: Vec::new(),
        },
        QueryTemplate {
            id: "linked_entries".to_string(),
            name: "Linked Entries".to_string(),
            description: "Find entries linked to projects".to_string(),
            query: r#"
SELECT
    e.id, e.title, e.entry_type,
    p.name as project_name,
    pe.relevance, pe.notes
FROM project_entries pe
JOIN entries e ON pe.entry_id = e.id
JOIN projects p ON pe.project_id = p.id
ORDER BY pe.linked_at DESC
"#.to_string(),
            database_type: DatabaseType::Sqlite,
            category: "Projects".to_string(),
            parameters: Vec::new(),
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_size() {
        assert_eq!(format_size(500), "500 bytes");
        assert_eq!(format_size(1024), "1.0 KB");
        assert_eq!(format_size(1536), "1.5 KB");
        assert_eq!(format_size(1048576), "1.0 MB");
        assert_eq!(format_size(234881024), "224.0 MB");
        assert_eq!(format_size(1073741824), "1.0 GB");
    }

    #[test]
    fn test_classify_query() {
        assert_eq!(classify_query("SELECT * FROM users"), QueryType::Select);
        assert_eq!(classify_query("select id from users"), QueryType::Select);
        assert_eq!(classify_query("WITH cte AS (SELECT 1) SELECT * FROM cte"), QueryType::Select);
        assert_eq!(classify_query("INSERT INTO users VALUES (1)"), QueryType::Insert);
        assert_eq!(classify_query("UPDATE users SET name = 'test'"), QueryType::Update);
        assert_eq!(classify_query("DELETE FROM users"), QueryType::Delete);
        assert_eq!(classify_query("CREATE TABLE test (id INT)"), QueryType::Create);
        assert_eq!(classify_query("DROP TABLE test"), QueryType::Drop);
        assert_eq!(classify_query("ALTER TABLE test ADD col INT"), QueryType::Alter);
        assert_eq!(classify_query("PRAGMA table_info(users)"), QueryType::Other);
    }

    #[test]
    fn test_is_valid_identifier() {
        assert!(is_valid_identifier("users"));
        assert!(is_valid_identifier("user_table"));
        assert!(is_valid_identifier("Users123"));
        assert!(!is_valid_identifier(""));
        assert!(!is_valid_identifier("123users"));
        assert!(!is_valid_identifier("user-table"));
        assert!(!is_valid_identifier("user;table"));
    }

    #[test]
    fn test_generate_db_id() {
        let path1 = Path::new("/data/kix.db");
        let path2 = Path::new("/data/other.db");

        let id1 = generate_db_id(path1);
        let id2 = generate_db_id(path2);

        assert!(id1.starts_with("db_"));
        assert!(id2.starts_with("db_"));
        assert_ne!(id1, id2);

        // Same path should generate same ID
        assert_eq!(generate_db_id(path1), id1);
    }

    #[test]
    fn test_query_templates() {
        let templates = get_query_templates();
        assert!(!templates.is_empty());

        // Check that all templates have required fields
        for template in &templates {
            assert!(!template.id.is_empty());
            assert!(!template.name.is_empty());
            assert!(!template.query.is_empty());
            assert!(!template.category.is_empty());
        }

        // Check specific templates exist
        assert!(templates.iter().any(|t| t.id == "recent_entries"));
        assert!(templates.iter().any(|t| t.id == "domain_stats"));
    }
}
