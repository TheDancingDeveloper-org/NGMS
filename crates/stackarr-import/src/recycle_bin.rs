// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 The StackArr Authors

use std::path::{Path, PathBuf};

use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::{FromRow, PgPool};

// ── Types ───────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct RecycleBinEntry {
    pub id: i64,
    pub original_path: String,
    pub recycle_path: String,
    pub media_file_id: Option<i64>,
    pub media_type: String,
    pub media_id: i64,
    pub size: i64,
    pub recycled_at: DateTime<Utc>,
}

// ── Config helpers ──────────────────────────────────────────────────────────

async fn get_recycle_bin_path(pool: &PgPool) -> Result<String> {
    let row: Option<(serde_json::Value,)> =
        sqlx::query_as("SELECT value FROM app_config WHERE key = 'recycle_bin_path'")
            .fetch_optional(pool)
            .await?;
    let path = row
        .and_then(|(v,)| v.as_str().map(|s| s.to_string()))
        .unwrap_or_default();
    Ok(path)
}

async fn get_cleanup_days(pool: &PgPool) -> Result<i32> {
    let row: Option<(serde_json::Value,)> =
        sqlx::query_as("SELECT value FROM app_config WHERE key = 'recycle_bin_cleanup_days'")
            .fetch_optional(pool)
            .await?;
    let days = row
        .and_then(|(v,)| v.as_i64().map(|n| n as i32))
        .unwrap_or(7);
    Ok(days)
}

// ── Core operations ─────────────────────────────────────────────────────────

/// Move a file to the recycle bin directory.
///
/// Returns the recycle path if the file was moved, or `None` if the recycle bin
/// is disabled (the file is permanently deleted instead).
pub async fn recycle_file(
    pool: &PgPool,
    file_path: &Path,
    media_file_id: i64,
    media_type: &str,
    media_id: i64,
) -> Result<Option<PathBuf>> {
    let bin_path = get_recycle_bin_path(pool).await?;

    // Get file size before any operation
    let size = tokio::fs::metadata(file_path)
        .await
        .map(|m| m.len() as i64)
        .unwrap_or(0);

    if bin_path.is_empty() {
        // No recycle bin configured — permanently delete.
        // Use async metadata() to avoid a sync stat() on the tokio worker
        // while recycle_file runs on the import/scheduler hot path.
        if tokio::fs::metadata(file_path).await.is_ok() {
            tokio::fs::remove_file(file_path).await?;
            tracing::info!(path = %file_path.display(), "permanently deleted file (no recycle bin configured)");
        }
        return Ok(None);
    }

    let bin_dir = PathBuf::from(&bin_path);
    tokio::fs::create_dir_all(&bin_dir).await?;

    // Build destination with collision handling
    let dest = unique_recycle_path(&bin_dir, file_path).await;

    // Move file using the shared helper
    super::move_file(file_path, &dest).await?;

    // Track in database
    let original = file_path.display().to_string();
    let recycled = dest.display().to_string();

    sqlx::query(
        "INSERT INTO recycle_bin (original_path, recycle_path, media_file_id, media_type, media_id, size) \
         VALUES ($1, $2, $3, $4, $5, $6)",
    )
    .bind(&original)
    .bind(&recycled)
    .bind(media_file_id)
    .bind(media_type)
    .bind(media_id)
    .bind(size)
    .execute(pool)
    .await?;

    tracing::info!(
        original = %original,
        recycled = %recycled,
        "moved file to recycle bin"
    );

    Ok(Some(dest))
}

/// Permanently delete all recycle bin entries older than the configured
/// `recycle_bin_cleanup_days`. Returns the number of files cleaned up.
pub async fn cleanup_expired_from_config(pool: PgPool) -> Result<usize> {
    let days = get_cleanup_days(&pool).await?;
    if days == 0 {
        return Ok(0); // 0 = keep forever
    }
    cleanup_expired(&pool, days).await
}

/// Permanently delete all recycle bin entries older than `days`.
pub async fn cleanup_expired(pool: &PgPool, days: i32) -> Result<usize> {
    let entries: Vec<(i64, String)> = sqlx::query_as(
        "SELECT id, recycle_path FROM recycle_bin WHERE recycled_at < NOW() - make_interval(days => $1)",
    )
    .bind(days)
    .fetch_all(pool)
    .await?;

    let mut cleaned = 0usize;
    for (id, path) in &entries {
        let p = Path::new(path);
        // Skip delete if file is already gone; use async metadata() to keep
        // the stat() off the tokio worker thread.
        if tokio::fs::metadata(p).await.is_ok()
            && let Err(e) = tokio::fs::remove_file(p).await
        {
            tracing::warn!(path = %path, error = %e, "failed to delete expired recycle bin file");
            continue;
        }
        sqlx::query("DELETE FROM recycle_bin WHERE id = $1")
            .bind(id)
            .execute(pool)
            .await?;
        cleaned += 1;
    }

    Ok(cleaned)
}

/// List all entries currently in the recycle bin.
pub async fn list_entries(pool: &PgPool) -> Result<Vec<RecycleBinEntry>> {
    let entries =
        sqlx::query_as::<_, RecycleBinEntry>("SELECT * FROM recycle_bin ORDER BY recycled_at DESC")
            .fetch_all(pool)
            .await?;
    Ok(entries)
}

/// Permanently delete a specific recycle bin entry by ID.
pub async fn delete_entry(pool: &PgPool, id: i64) -> Result<()> {
    let row: Option<(String,)> =
        sqlx::query_as("SELECT recycle_path FROM recycle_bin WHERE id = $1")
            .bind(id)
            .fetch_optional(pool)
            .await?;

    if let Some((path,)) = row {
        let p = Path::new(&path);
        if tokio::fs::metadata(p).await.is_ok() {
            tokio::fs::remove_file(p).await?;
        }
        sqlx::query("DELETE FROM recycle_bin WHERE id = $1")
            .bind(id)
            .execute(pool)
            .await?;
    }

    Ok(())
}

/// Empty the entire recycle bin. Returns the number of entries removed.
pub async fn empty_bin(pool: &PgPool) -> Result<usize> {
    let entries: Vec<(i64, String)> = sqlx::query_as("SELECT id, recycle_path FROM recycle_bin")
        .fetch_all(pool)
        .await?;

    let count = entries.len();
    for (_, path) in &entries {
        let p = Path::new(path);
        if tokio::fs::metadata(p).await.is_ok() {
            let _ = tokio::fs::remove_file(p).await;
        }
    }

    sqlx::query("DELETE FROM recycle_bin").execute(pool).await?;

    Ok(count)
}

// ── Helpers ─────────────────────────────────────────────────────────────────

/// Build a unique path inside the recycle bin, appending `_2`, `_3`, etc.
/// on collision. Async so the existence probes don't block the tokio
/// worker thread — the call site is in an async fn on the import hot path.
async fn unique_recycle_path(bin_dir: &Path, original: &Path) -> PathBuf {
    let file_name = original.file_name().unwrap_or_default().to_string_lossy();

    let candidate = bin_dir.join(file_name.as_ref());
    if tokio::fs::metadata(&candidate).await.is_err() {
        return candidate;
    }

    let stem = original.file_stem().unwrap_or_default().to_string_lossy();
    let ext = original
        .extension()
        .map(|e| format!(".{}", e.to_string_lossy()))
        .unwrap_or_default();

    for i in 2..=1000 {
        let path = bin_dir.join(format!("{stem}_{i}{ext}"));
        if tokio::fs::metadata(&path).await.is_err() {
            return path;
        }
    }

    // Fallback: use timestamp
    let ts = chrono::Utc::now().timestamp();
    bin_dir.join(format!("{stem}_{ts}{ext}"))
}
