use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use sqlx::PgPool;
use tokio::sync::RwLock;

use stackarr_download::DownloadClientManager;
use stackarr_indexer::IndexerManager;

/// Maximum consecutive failures before auto-disabling a service.
const FAILURE_THRESHOLD: i32 = 3;

/// Timeout for each individual health check.
const CHECK_TIMEOUT: Duration = Duration::from_secs(15);

#[derive(sqlx::FromRow)]
struct DownloadClientRow {
    id: i32,
    name: String,
    client_type: String,
    enabled: bool,
    auto_disabled: bool,
    consecutive_failures: i32,
    priority: i32,
}

#[derive(sqlx::FromRow)]
struct IndexerRow {
    id: i64,
    name: String,
    enabled: bool,
    auto_disabled: bool,
    consecutive_failures: i32,
}

/// Run a single pass of health checks against all download clients and indexers.
pub async fn health_check_task(
    pool: PgPool,
    download_manager: Arc<RwLock<DownloadClientManager>>,
    indexer_manager: Arc<RwLock<IndexerManager>>,
) -> Result<()> {
    check_download_clients(&pool, &download_manager).await;
    check_indexers(&pool, &indexer_manager).await;
    Ok(())
}

async fn check_download_clients(
    pool: &PgPool,
    download_manager: &Arc<RwLock<DownloadClientManager>>,
) {
    let rows: Vec<DownloadClientRow> = match sqlx::query_as(
        "SELECT id, name, client_type, enabled, auto_disabled, consecutive_failures, priority \
         FROM download_clients \
         WHERE client_type != 'embedded_usenet'",
    )
    .fetch_all(pool)
    .await
    {
        Ok(r) => r,
        Err(e) => {
            tracing::error!(error = %e, "health check: failed to query download clients");
            return;
        }
    };

    for row in rows {
        // Skip user-disabled clients (enabled=false AND auto_disabled=false)
        if !row.enabled && !row.auto_disabled {
            continue;
        }

        let test_result = {
            let mgr = download_manager.read().await;
            match mgr.client_by_id(row.id as i64) {
                Some(client) => {
                    tokio::time::timeout(CHECK_TIMEOUT, client.test()).await
                }
                None => {
                    // Client not in manager — might need rebuilding for auto-disabled
                    if row.auto_disabled {
                        try_rebuild_client(pool, download_manager, &row).await;
                    }
                    continue;
                }
            }
        };

        match test_result {
            Ok(Ok(())) => {
                // Healthy
                if row.auto_disabled || row.consecutive_failures > 0 {
                    if row.auto_disabled {
                        tracing::info!(
                            id = row.id, name = %row.name,
                            "health check: download client recovered, re-enabling"
                        );
                        let mut mgr = download_manager.write().await;
                        mgr.set_enabled(row.id as i64, true);
                    }
                    let _ = sqlx::query(
                        "UPDATE download_clients \
                         SET enabled = true, auto_disabled = false, \
                             health_status = 'healthy', consecutive_failures = 0, \
                             last_health_check = NOW() \
                         WHERE id = $1",
                    )
                    .bind(row.id)
                    .execute(pool)
                    .await;
                } else {
                    let _ = sqlx::query(
                        "UPDATE download_clients \
                         SET health_status = 'healthy', last_health_check = NOW() \
                         WHERE id = $1",
                    )
                    .bind(row.id)
                    .execute(pool)
                    .await;
                }
            }
            Ok(Err(e)) => {
                handle_dl_failure(pool, download_manager, &row, &e.to_string()).await;
            }
            Err(_) => {
                handle_dl_failure(pool, download_manager, &row, "connection test timed out").await;
            }
        }
    }
}

async fn handle_dl_failure(
    pool: &PgPool,
    download_manager: &Arc<RwLock<DownloadClientManager>>,
    row: &DownloadClientRow,
    error_msg: &str,
) {
    let new_failures = row.consecutive_failures + 1;

    if new_failures >= FAILURE_THRESHOLD && !row.auto_disabled {
        tracing::warn!(
            id = row.id, name = %row.name, failures = new_failures,
            error = error_msg,
            "health check: auto-disabling download client after {} consecutive failures",
            FAILURE_THRESHOLD
        );
        let mut mgr = download_manager.write().await;
        mgr.set_enabled(row.id as i64, false);
        let _ = sqlx::query(
            "UPDATE download_clients \
             SET enabled = false, auto_disabled = true, \
                 health_status = 'auto_disabled', \
                 consecutive_failures = $1, last_health_check = NOW() \
             WHERE id = $2",
        )
        .bind(new_failures)
        .bind(row.id)
        .execute(pool)
        .await;
    } else {
        tracing::debug!(
            id = row.id, name = %row.name, failures = new_failures,
            error = error_msg,
            "health check: download client failed"
        );
        let _ = sqlx::query(
            "UPDATE download_clients \
             SET health_status = 'unhealthy', \
                 consecutive_failures = $1, last_health_check = NOW() \
             WHERE id = $2",
        )
        .bind(new_failures)
        .bind(row.id)
        .execute(pool)
        .await;
    }
}

async fn try_rebuild_client(
    pool: &PgPool,
    download_manager: &Arc<RwLock<DownloadClientManager>>,
    row: &DownloadClientRow,
) {
    // Try to rebuild the client from DB config
    let config_row: Option<(serde_json::Value,)> = sqlx::query_as(
        "SELECT config FROM download_clients WHERE id = $1",
    )
    .bind(row.id)
    .fetch_optional(pool)
    .await
    .unwrap_or(None);

    if let Some((config,)) = config_row {
        match stackarr_download::build_from_config(&row.client_type, &config) {
            Ok(client) => {
                // Test it before adding
                match tokio::time::timeout(CHECK_TIMEOUT, client.test()).await {
                    Ok(Ok(())) => {
                        tracing::info!(
                            id = row.id, name = %row.name,
                            "health check: rebuilt and re-enabled download client"
                        );
                        let mut mgr = download_manager.write().await;
                        mgr.add_client(row.id as i64, client, row.priority);
                        let _ = sqlx::query(
                            "UPDATE download_clients \
                             SET enabled = true, auto_disabled = false, \
                                 health_status = 'healthy', consecutive_failures = 0, \
                                 last_health_check = NOW() \
                             WHERE id = $1",
                        )
                        .bind(row.id)
                        .execute(pool)
                        .await;
                    }
                    _ => {
                        tracing::debug!(
                            id = row.id, name = %row.name,
                            "health check: rebuilt download client still unhealthy"
                        );
                    }
                }
            }
            Err(e) => {
                tracing::debug!(
                    id = row.id, name = %row.name, error = %e,
                    "health check: could not rebuild download client"
                );
            }
        }
    }
}

async fn check_indexers(
    pool: &PgPool,
    indexer_manager: &Arc<RwLock<IndexerManager>>,
) {
    let rows: Vec<IndexerRow> = match sqlx::query_as(
        "SELECT id, name, enabled, auto_disabled, consecutive_failures FROM indexers",
    )
    .fetch_all(pool)
    .await
    {
        Ok(r) => r,
        Err(e) => {
            tracing::error!(error = %e, "health check: failed to query indexers");
            return;
        }
    };

    for row in rows {
        // Skip user-disabled indexers
        if !row.enabled && !row.auto_disabled {
            continue;
        }

        let test_result = {
            let mgr = indexer_manager.read().await;
            match mgr.get_client(row.id) {
                Some(client) => {
                    Some(tokio::time::timeout(CHECK_TIMEOUT, client.caps()).await)
                }
                None => None,
            }
        };

        let test_result = match test_result {
            Some(r) => r,
            None => continue, // not a Newznab indexer or not registered
        };

        match test_result {
            Ok(Ok(_)) => {
                // Healthy
                if row.auto_disabled || row.consecutive_failures > 0 {
                    if row.auto_disabled {
                        tracing::info!(
                            id = row.id, name = %row.name,
                            "health check: indexer recovered, re-enabling"
                        );
                        let mut mgr = indexer_manager.write().await;
                        mgr.set_enabled(row.id, true);
                    }
                    let _ = sqlx::query(
                        "UPDATE indexers \
                         SET enabled = true, auto_disabled = false, \
                             health_status = 'healthy', consecutive_failures = 0, \
                             last_health_check = NOW() \
                         WHERE id = $1",
                    )
                    .bind(row.id)
                    .execute(pool)
                    .await;
                } else {
                    let _ = sqlx::query(
                        "UPDATE indexers \
                         SET health_status = 'healthy', last_health_check = NOW() \
                         WHERE id = $1",
                    )
                    .bind(row.id)
                    .execute(pool)
                    .await;
                }
            }
            Ok(Err(e)) => {
                handle_indexer_failure(pool, indexer_manager, &row, &e.to_string()).await;
            }
            Err(_) => {
                handle_indexer_failure(pool, indexer_manager, &row, "connection test timed out").await;
            }
        }
    }
}

async fn handle_indexer_failure(
    pool: &PgPool,
    indexer_manager: &Arc<RwLock<IndexerManager>>,
    row: &IndexerRow,
    error_msg: &str,
) {
    let new_failures = row.consecutive_failures + 1;

    if new_failures >= FAILURE_THRESHOLD && !row.auto_disabled {
        tracing::warn!(
            id = row.id, name = %row.name, failures = new_failures,
            error = error_msg,
            "health check: auto-disabling indexer after {} consecutive failures",
            FAILURE_THRESHOLD
        );
        let mut mgr = indexer_manager.write().await;
        mgr.set_enabled(row.id, false);
        let _ = sqlx::query(
            "UPDATE indexers \
             SET enabled = false, auto_disabled = true, \
                 health_status = 'auto_disabled', \
                 consecutive_failures = $1, last_health_check = NOW() \
             WHERE id = $2",
        )
        .bind(new_failures)
        .bind(row.id)
        .execute(pool)
        .await;
    } else {
        tracing::debug!(
            id = row.id, name = %row.name, failures = new_failures,
            error = error_msg,
            "health check: indexer failed"
        );
        let _ = sqlx::query(
            "UPDATE indexers \
             SET health_status = 'unhealthy', \
                 consecutive_failures = $1, last_health_check = NOW() \
             WHERE id = $2",
        )
        .bind(new_failures)
        .bind(row.id)
        .execute(pool)
        .await;
    }
}
