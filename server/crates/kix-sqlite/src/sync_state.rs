//! Sync state operations for issue synchronization

use crate::error::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json;
use sqlx::{FromRow, SqlitePool};

/// Issue sync state record from database
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct SyncStateRecord {
    pub issue_id: String,
    pub content_hash: String,
    pub last_sync_hash: String,
    pub local_version: i64,
    pub github_version: Option<i64>,
    pub github_etag: Option<String>,
    pub sync_status: String,
    pub last_sync_at: String,
    pub last_sync_direction: Option<String>,
    pub last_sync_result: Option<String>,
    pub conflict_count: i32,
    pub created_at: String,
    pub updated_at: String,
}

/// Sync state operations
pub struct SyncStateStore;

impl SyncStateStore {
    /// Create or update sync state for an issue
    pub async fn upsert_sync_state(
        pool: &SqlitePool,
        issue_id: &str,
        content_hash: &str,
        sync_status: &str,
    ) -> Result<()> {
        let now = Utc::now().to_rfc3339();

        sqlx::query(
            r#"
            INSERT INTO issue_sync_state (
                issue_id, content_hash, last_sync_hash, local_version,
                sync_status, last_sync_at, created_at, updated_at
            ) VALUES (?, ?, ?, 1, ?, ?, ?, ?)
            ON CONFLICT(issue_id) DO UPDATE SET
                content_hash = excluded.content_hash,
                sync_status = excluded.sync_status,
                updated_at = excluded.updated_at
            "#,
        )
        .bind(issue_id)
        .bind(content_hash)
        .bind(content_hash) // Initially, last_sync_hash equals content_hash
        .bind(sync_status)
        .bind(&now)
        .bind(&now)
        .bind(&now)
        .execute(pool)
        .await?;

        Ok(())
    }

    /// Get sync state for an issue
    pub async fn get_sync_state(
        pool: &SqlitePool,
        issue_id: &str,
    ) -> Result<Option<SyncStateRecord>> {
        let state = sqlx::query_as::<_, SyncStateRecord>(
            r#"
            SELECT * FROM issue_sync_state
            WHERE issue_id = ?
            "#,
        )
        .bind(issue_id)
        .fetch_optional(pool)
        .await?;

        Ok(state)
    }

    /// Update sync state after successful sync
    pub async fn update_after_sync(
        pool: &SqlitePool,
        issue_id: &str,
        content_hash: &str,
        github_version: Option<i64>,
        github_etag: Option<&str>,
        sync_direction: &str,
        sync_result: Option<&serde_json::Value>,
    ) -> Result<()> {
        let now = Utc::now().to_rfc3339();
        let sync_result_str = sync_result.map(|r| r.to_string());

        sqlx::query(
            r#"
            UPDATE issue_sync_state SET
                content_hash = ?,
                last_sync_hash = ?,
                github_version = ?,
                github_etag = ?,
                sync_status = 'synced',
                last_sync_at = ?,
                last_sync_direction = ?,
                last_sync_result = ?,
                updated_at = ?
            WHERE issue_id = ?
            "#,
        )
        .bind(content_hash)
        .bind(content_hash) // After sync, both hashes should match
        .bind(github_version)
        .bind(github_etag)
        .bind(&now)
        .bind(sync_direction)
        .bind(sync_result_str)
        .bind(&now)
        .bind(issue_id)
        .execute(pool)
        .await?;

        Ok(())
    }

    /// Increment local version when issue is modified locally
    pub async fn increment_local_version(
        pool: &SqlitePool,
        issue_id: &str,
        new_content_hash: &str,
    ) -> Result<()> {
        let now = Utc::now().to_rfc3339();

        sqlx::query(
            r#"
            UPDATE issue_sync_state SET
                content_hash = ?,
                local_version = local_version + 1,
                sync_status = CASE
                    WHEN sync_status = 'synced' THEN 'local_ahead'
                    WHEN sync_status = 'github_ahead' THEN 'conflict'
                    ELSE sync_status
                END,
                updated_at = ?
            WHERE issue_id = ?
            "#,
        )
        .bind(new_content_hash)
        .bind(&now)
        .bind(issue_id)
        .execute(pool)
        .await?;

        Ok(())
    }

    /// Mark issue as having GitHub changes
    pub async fn mark_github_ahead(
        pool: &SqlitePool,
        issue_id: &str,
        github_version: i64,
        github_etag: Option<&str>,
    ) -> Result<()> {
        let now = Utc::now().to_rfc3339();

        sqlx::query(
            r#"
            UPDATE issue_sync_state SET
                github_version = ?,
                github_etag = ?,
                sync_status = CASE
                    WHEN sync_status = 'synced' THEN 'github_ahead'
                    WHEN sync_status = 'local_ahead' THEN 'conflict'
                    ELSE sync_status
                END,
                updated_at = ?
            WHERE issue_id = ?
            "#,
        )
        .bind(github_version)
        .bind(github_etag)
        .bind(&now)
        .bind(issue_id)
        .execute(pool)
        .await?;

        Ok(())
    }

    /// Record a conflict
    pub async fn record_conflict(
        pool: &SqlitePool,
        issue_id: &str,
        local_snapshot: &serde_json::Value,
        github_snapshot: &serde_json::Value,
        affected_fields: &[String],
    ) -> Result<()> {
        let now = Utc::now().to_rfc3339();

        // Update sync state
        sqlx::query(
            r#"
            UPDATE issue_sync_state SET
                sync_status = 'conflict',
                conflict_count = conflict_count + 1,
                updated_at = ?
            WHERE issue_id = ?
            "#,
        )
        .bind(&now)
        .bind(issue_id)
        .execute(pool)
        .await?;

        // Get project_id for conflict record
        let project_id: String = sqlx::query_scalar(
            r#"
            SELECT project_id FROM issues WHERE id = ?
            "#,
        )
        .bind(issue_id)
        .fetch_one(pool)
        .await?;

        // Insert conflict record
        sqlx::query(
            r#"
            INSERT INTO sync_conflicts (
                id, issue_id, project_id, detected_at, conflict_type,
                local_snapshot, github_snapshot, affected_fields
            ) VALUES (?, ?, ?, ?, 'content', ?, ?, ?)
            ON CONFLICT(issue_id) DO UPDATE SET
                detected_at = excluded.detected_at,
                local_snapshot = excluded.local_snapshot,
                github_snapshot = excluded.github_snapshot,
                affected_fields = excluded.affected_fields
            "#,
        )
        .bind(uuid::Uuid::new_v4().to_string())
        .bind(issue_id)
        .bind(project_id)
        .bind(&now)
        .bind(local_snapshot.to_string())
        .bind(github_snapshot.to_string())
        .bind(serde_json::to_string(affected_fields).unwrap())
        .execute(pool)
        .await?;

        Ok(())
    }

    /// Resolve a conflict
    pub async fn resolve_conflict(
        pool: &SqlitePool,
        issue_id: &str,
        resolution: &serde_json::Value,
        resolved_by: &str,
    ) -> Result<()> {
        let now = Utc::now().to_rfc3339();

        // Update conflict record
        sqlx::query(
            r#"
            UPDATE sync_conflicts SET
                manual_resolution = ?,
                resolved_at = ?,
                resolved_by = ?
            WHERE issue_id = ? AND resolved_at IS NULL
            "#,
        )
        .bind(resolution.to_string())
        .bind(&now)
        .bind(resolved_by)
        .bind(issue_id)
        .execute(pool)
        .await?;

        // Update sync state
        sqlx::query(
            r#"
            UPDATE issue_sync_state SET
                sync_status = 'synced',
                updated_at = ?
            WHERE issue_id = ?
            "#,
        )
        .bind(&now)
        .bind(issue_id)
        .execute(pool)
        .await?;

        Ok(())
    }

    /// Get all issues needing sync for a project
    pub async fn get_issues_needing_sync(
        pool: &SqlitePool,
        project_id: &str,
    ) -> Result<Vec<String>> {
        let issue_ids = sqlx::query_scalar::<_, String>(
            r#"
            SELECT iss.issue_id
            FROM issue_sync_state iss
            JOIN issues i ON i.id = iss.issue_id
            WHERE i.project_id = ?
                AND iss.sync_status IN ('local_ahead', 'github_ahead', 'conflict', 'pending')
            ORDER BY iss.updated_at DESC
            "#,
        )
        .bind(project_id)
        .fetch_all(pool)
        .await?;

        Ok(issue_ids)
    }

    /// Get sync statistics for a project
    pub async fn get_sync_stats(
        pool: &SqlitePool,
        project_id: &str,
    ) -> Result<SyncStats> {
        let row = sqlx::query_as::<_, SyncStatsRow>(
            r#"
            SELECT
                COUNT(CASE WHEN iss.sync_status = 'synced' THEN 1 END) as synced,
                COUNT(CASE WHEN iss.sync_status = 'local_ahead' THEN 1 END) as local_ahead,
                COUNT(CASE WHEN iss.sync_status = 'github_ahead' THEN 1 END) as github_ahead,
                COUNT(CASE WHEN iss.sync_status = 'conflict' THEN 1 END) as conflicts,
                COUNT(CASE WHEN iss.sync_status = 'pending' THEN 1 END) as pending
            FROM issue_sync_state iss
            JOIN issues i ON i.id = iss.issue_id
            WHERE i.project_id = ?
            "#,
        )
        .bind(project_id)
        .fetch_one(pool)
        .await?;

        Ok(SyncStats {
            synced: row.synced as usize,
            local_ahead: row.local_ahead as usize,
            github_ahead: row.github_ahead as usize,
            conflicts: row.conflicts as usize,
            pending: row.pending as usize,
        })
    }

    /// Clear sync state for an issue (used when issue is deleted)
    pub async fn clear_sync_state(pool: &SqlitePool, issue_id: &str) -> Result<()> {
        sqlx::query(
            r#"
            DELETE FROM issue_sync_state WHERE issue_id = ?
            "#,
        )
        .bind(issue_id)
        .execute(pool)
        .await?;

        Ok(())
    }
}

/// Sync statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncStats {
    pub synced: usize,
    pub local_ahead: usize,
    pub github_ahead: usize,
    pub conflicts: usize,
    pub pending: usize,
}

#[derive(FromRow)]
struct SyncStatsRow {
    synced: i64,
    local_ahead: i64,
    github_ahead: i64,
    conflicts: i64,
    pending: i64,
}

/// Sync history record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncHistoryRecord {
    pub id: String,
    pub project_id: String,
    pub sync_type: String,
    pub direction: String,
    pub started_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub status: String,
    pub stats: Option<serde_json::Value>,
    pub issues_pulled: i32,
    pub issues_pushed: i32,
    pub issues_merged: i32,
    pub conflicts_resolved: i32,
    pub conflicts_failed: i32,
    pub errors: Option<Vec<String>>,
}

impl SyncStateStore {
    /// Create sync history entry
    pub async fn create_sync_history(
        pool: &SqlitePool,
        project_id: &str,
        sync_type: &str,
        direction: &str,
    ) -> Result<String> {
        let id = uuid::Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();

        sqlx::query(
            r#"
            INSERT INTO sync_history (
                id, project_id, sync_type, direction,
                started_at, status
            ) VALUES (?, ?, ?, ?, ?, 'running')
            "#,
        )
        .bind(&id)
        .bind(project_id)
        .bind(sync_type)
        .bind(direction)
        .bind(&now)
        .execute(pool)
        .await?;

        Ok(id)
    }

    /// Complete sync history entry
    pub async fn complete_sync_history(
        pool: &SqlitePool,
        history_id: &str,
        stats: &SyncHistoryStats,
        errors: Option<&[String]>,
    ) -> Result<()> {
        let now = Utc::now().to_rfc3339();
        let status = if errors.is_some() && !errors.unwrap().is_empty() {
            "partial"
        } else {
            "success"
        };

        let stats_json = serde_json::to_string(&stats)?;
        let errors_json = errors.map(|e| serde_json::to_string(e).unwrap());

        sqlx::query(
            r#"
            UPDATE sync_history SET
                completed_at = ?,
                status = ?,
                stats = ?,
                issues_pulled = ?,
                issues_pushed = ?,
                issues_merged = ?,
                conflicts_resolved = ?,
                conflicts_failed = ?,
                errors = ?
            WHERE id = ?
            "#,
        )
        .bind(&now)
        .bind(status)
        .bind(&stats_json)
        .bind(stats.issues_pulled)
        .bind(stats.issues_pushed)
        .bind(stats.issues_merged)
        .bind(stats.conflicts_resolved)
        .bind(stats.conflicts_failed)
        .bind(errors_json)
        .bind(history_id)
        .execute(pool)
        .await?;

        Ok(())
    }
}

/// Stats for sync history
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncHistoryStats {
    pub issues_pulled: i32,
    pub issues_pushed: i32,
    pub issues_merged: i32,
    pub conflicts_resolved: i32,
    pub conflicts_failed: i32,
}