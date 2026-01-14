//! SQLite connection pool and migrations

use crate::error::{Result, SqliteError};
use sea_orm::{ConnectOptions, Database, DatabaseConnection};
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous};
use sqlx::SqlitePool;
use std::path::Path;
use std::str::FromStr;
use std::time::Duration;
use tracing::{debug, info};

/// Initial migration SQL
const INITIAL_MIGRATION: &str = include_str!("migrations/001_initial.sql");

/// Sync state migration SQL
const SYNC_STATE_MIGRATION: &str = include_str!("migrations/002_sync_state.sql");

/// Code extraction stats migration SQL
const CODE_EXTRACTION_STATS_MIGRATION: &str = include_str!("migrations/003_code_extraction_stats.sql");

/// Create a new SQLite connection pool
///
/// Creates the database file and parent directories if they don't exist.
/// Configures the pool with optimized settings for concurrent access.
pub async fn create_pool(db_path: &Path) -> Result<SqlitePool> {
    // Ensure parent directory exists
    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let db_url = format!("sqlite:{}?mode=rwc", db_path.display());
    debug!("Connecting to SQLite database at: {}", db_url);

    let options = SqliteConnectOptions::from_str(&db_url)?
        .journal_mode(SqliteJournalMode::Wal) // Write-ahead logging for better concurrency
        .synchronous(SqliteSynchronous::Normal) // Good balance of safety and speed
        .foreign_keys(true) // Enable foreign key constraints
        .busy_timeout(std::time::Duration::from_secs(30)); // Wait up to 30s for locks

    let pool = SqlitePoolOptions::new()
        .max_connections(10) // Allow multiple concurrent readers
        .min_connections(1)
        .acquire_timeout(std::time::Duration::from_secs(30))
        .connect_with(options)
        .await?;

    info!("SQLite connection pool created at: {}", db_path.display());
    Ok(pool)
}

/// Run all database migrations
///
/// This function runs the initial schema migration and any future migrations.
/// It's idempotent - safe to call multiple times.
pub async fn run_migrations(pool: &SqlitePool) -> Result<()> {
    debug!("Running SQLite migrations...");

    // Run migrations in order
    let migrations = vec![
        ("001_initial", INITIAL_MIGRATION),
        ("002_sync_state", SYNC_STATE_MIGRATION),
        ("003_code_extraction_stats", CODE_EXTRACTION_STATS_MIGRATION),
    ];

    for (name, migration_sql) in migrations {
        debug!("Running migration: {}", name);

        // Parse statements more carefully
        // Handle multi-line statements and avoid splitting inside trigger bodies
        let statements = parse_sql_statements(migration_sql);

        for statement in statements {
            let trimmed = statement.trim();
            if !trimmed.is_empty() {
                if let Err(e) = sqlx::query(trimmed).execute(pool).await {
                    // SQLite doesn't support "ADD COLUMN IF NOT EXISTS", so we need to
                    // handle the "duplicate column name" error gracefully for idempotency
                    let error_msg = e.to_string();
                    let is_duplicate_column = error_msg.contains("duplicate column name")
                        && trimmed.to_uppercase().contains("ADD COLUMN");

                    if is_duplicate_column {
                        debug!("Column already exists, skipping: {}", &trimmed[..trimmed.len().min(100)]);
                        continue;
                    }

                    tracing::error!("Migration {} statement failed: {}", name, &trimmed[..trimmed.len().min(200)]);
                    return Err(SqliteError::Database(e));
                }
            }
        }

        debug!("Migration {} completed", name);
    }

    info!("SQLite migrations completed successfully");
    Ok(())
}

/// Parse SQL statements, handling triggers and other multi-line constructs
fn parse_sql_statements(sql: &str) -> Vec<String> {
    let mut statements = Vec::new();
    let mut current = String::new();
    let mut in_trigger = false;

    for line in sql.lines() {
        let trimmed = line.trim();

        // Skip pure comments
        if trimmed.starts_with("--") && !trimmed.contains("CREATE") {
            continue;
        }

        // Track trigger blocks
        if trimmed.to_uppercase().starts_with("CREATE TRIGGER") {
            in_trigger = true;
        }

        current.push_str(line);
        current.push('\n');

        // End of trigger detected
        if in_trigger && trimmed.to_uppercase().starts_with("END;") {
            statements.push(current.trim().to_string());
            current.clear();
            in_trigger = false;
            continue;
        }

        // Normal statement ending
        if !in_trigger && trimmed.ends_with(';') {
            let stmt = current.trim().to_string();
            // Remove trailing semicolon for sqlx
            let stmt = stmt.trim_end_matches(';').to_string();
            if !stmt.is_empty() && !stmt.starts_with("--") {
                statements.push(stmt);
            }
            current.clear();
        }
    }

    // Handle any remaining content
    let remaining = current.trim();
    if !remaining.is_empty() && !remaining.starts_with("--") {
        statements.push(remaining.trim_end_matches(';').to_string());
    }

    statements
}

/// Create a SeaORM database connection
///
/// Creates a new SeaORM connection to the same database.
/// This is used alongside the sqlx pool for ORM-based operations.
pub async fn create_sea_orm_connection(db_path: &Path) -> Result<DatabaseConnection> {
    let db_url = format!("sqlite://{}?mode=rwc", db_path.display());
    debug!("Creating SeaORM connection to: {}", db_url);

    let mut opt = ConnectOptions::new(&db_url);
    opt.max_connections(10)
        .min_connections(1)
        .connect_timeout(Duration::from_secs(30))
        .idle_timeout(Duration::from_secs(300))
        .sqlx_logging(false); // Reduce noise in logs

    let db = Database::connect(opt).await.map_err(|e| {
        SqliteError::Configuration(format!("Failed to create SeaORM connection: {}", e))
    })?;

    // Set SQLite pragmas for performance and reliability
    use sea_orm::ConnectionTrait;
    db.execute_unprepared("PRAGMA journal_mode = WAL")
        .await
        .map_err(|e| SqliteError::Configuration(format!("Failed to set WAL mode: {}", e)))?;
    db.execute_unprepared("PRAGMA synchronous = NORMAL")
        .await
        .map_err(|e| SqliteError::Configuration(format!("Failed to set synchronous: {}", e)))?;
    db.execute_unprepared("PRAGMA foreign_keys = ON")
        .await
        .map_err(|e| SqliteError::Configuration(format!("Failed to enable foreign keys: {}", e)))?;
    db.execute_unprepared("PRAGMA busy_timeout = 30000")
        .await
        .map_err(|e| SqliteError::Configuration(format!("Failed to set busy timeout: {}", e)))?;
    // Performance optimizations
    db.execute_unprepared("PRAGMA cache_size = -64000") // 64MB page cache (default 2MB)
        .await
        .map_err(|e| SqliteError::Configuration(format!("Failed to set cache size: {}", e)))?;
    db.execute_unprepared("PRAGMA mmap_size = 268435456") // 256MB memory-mapped I/O
        .await
        .map_err(|e| SqliteError::Configuration(format!("Failed to set mmap size: {}", e)))?;
    db.execute_unprepared("PRAGMA temp_store = MEMORY") // Temp tables in RAM
        .await
        .map_err(|e| SqliteError::Configuration(format!("Failed to set temp store: {}", e)))?;

    info!("SeaORM connection created at: {}", db_path.display());
    Ok(db)
}

/// Get the current database version/schema info
pub async fn get_db_info(pool: &SqlitePool) -> Result<DbInfo> {
    let table_count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM sqlite_master WHERE type='table'")
        .fetch_one(pool)
        .await?;

    let page_size: (i64,) = sqlx::query_as("PRAGMA page_size").fetch_one(pool).await?;

    let page_count: (i64,) = sqlx::query_as("PRAGMA page_count").fetch_one(pool).await?;

    Ok(DbInfo {
        table_count: table_count.0 as usize,
        page_size: page_size.0 as usize,
        total_size_bytes: (page_size.0 * page_count.0) as usize,
    })
}

/// Database information
#[derive(Debug, Clone)]
pub struct DbInfo {
    pub table_count: usize,
    pub page_size: usize,
    pub total_size_bytes: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_create_pool_and_migrations() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.db");

        let pool = create_pool(&db_path).await.unwrap();
        run_migrations(&pool).await.unwrap();

        // Verify tables were created
        let info = get_db_info(&pool).await.unwrap();
        assert!(info.table_count > 0, "Tables should be created");

        // Verify we can run migrations again (idempotent)
        run_migrations(&pool).await.unwrap();
    }

    #[tokio::test]
    async fn test_creates_parent_directories() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("nested").join("dirs").join("test.db");

        let pool = create_pool(&db_path).await.unwrap();
        assert!(db_path.exists());

        pool.close().await;
    }
}
