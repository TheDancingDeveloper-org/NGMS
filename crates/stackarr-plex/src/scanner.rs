use anyhow::Result;
use sqlx::PgPool;

use crate::api::PlexApi;
use crate::guid;
use crate::types::*;

const PAGE_SIZE: i64 = 50;

/// Scans Plex libraries and updates local media availability.
pub struct PlexScanner {
    pool: PgPool,
}

impl PlexScanner {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Full scan of all enabled Plex libraries. Updates availability for every item.
    pub async fn full_scan(&self) -> Result<ScanReport> {
        let mut report = ScanReport::default();

        let servers = self.load_servers().await?;
        for server in &servers {
            let Some(api) = PlexApi::from_server(server) else {
                tracing::warn!(server_id = server.id, "no auth token for plex server, skipping");
                continue;
            };

            let libraries = self.load_enabled_libraries(server.id).await?;
            for lib in &libraries {
                tracing::info!(
                    server = %server.name,
                    library = %lib.name,
                    "starting full scan"
                );

                let mut start = 0i64;
                loop {
                    let container = api
                        .get_library_contents(&lib.section_id, start, PAGE_SIZE)
                        .await;

                    let container = match container {
                        Ok(c) => c,
                        Err(e) => {
                            tracing::error!(
                                library = %lib.name,
                                error = %e,
                                "failed to fetch library contents"
                            );
                            report.errors += 1;
                            break;
                        }
                    };

                    let count = container.metadata.len() as i64;
                    for item in &container.metadata {
                        self.process_item(item, &lib.library_type, &mut report)
                            .await;
                    }

                    start += count;
                    let total = container.total_size.unwrap_or(0);
                    if count == 0 || start >= total {
                        break;
                    }
                }

                // Update last_scan timestamp
                let _ = sqlx::query(
                    "UPDATE plex_libraries SET last_scan = NOW() WHERE id = $1",
                )
                .bind(lib.id)
                .execute(&self.pool)
                .await;

                tracing::info!(
                    library = %lib.name,
                    scanned = report.items_scanned,
                    updated = report.items_updated,
                    "full scan complete for library"
                );
            }
        }

        Ok(report)
    }

    /// Incremental scan — only items added since the last scan.
    pub async fn recent_scan(&self) -> Result<ScanReport> {
        let mut report = ScanReport::default();

        let servers = self.load_servers().await?;
        for server in &servers {
            let Some(api) = PlexApi::from_server(server) else {
                continue;
            };

            let libraries = self.load_enabled_libraries(server.id).await?;
            for lib in &libraries {
                let last_scan_ts = lib
                    .last_scan
                    .map(|dt| dt.timestamp())
                    .unwrap_or(0);

                // Add a 10-minute buffer to catch items that were being processed
                let since_ts = last_scan_ts - 600;

                let mut start = 0i64;
                let mut seen_keys = std::collections::HashSet::new();

                loop {
                    let container = match api
                        .get_recently_added(&lib.section_id, start, PAGE_SIZE)
                        .await
                    {
                        Ok(c) => c,
                        Err(e) => {
                            tracing::error!(
                                library = %lib.name,
                                error = %e,
                                "failed to fetch recently added"
                            );
                            report.errors += 1;
                            break;
                        }
                    };

                    let count = container.metadata.len() as i64;
                    let mut all_older = true;

                    for item in &container.metadata {
                        let added_at = item.added_at.unwrap_or(0);
                        if added_at < since_ts {
                            all_older = true;
                            continue;
                        }
                        all_older = false;

                        // Deduplicate by rating key
                        if !seen_keys.insert(item.rating_key.clone()) {
                            continue;
                        }

                        self.process_item(item, &lib.library_type, &mut report)
                            .await;
                    }

                    start += count;
                    if count == 0 || all_older {
                        break;
                    }
                }

                // Update last_scan
                let _ = sqlx::query(
                    "UPDATE plex_libraries SET last_scan = NOW() WHERE id = $1",
                )
                .bind(lib.id)
                .execute(&self.pool)
                .await;
            }
        }

        if report.items_updated > 0 {
            tracing::info!(
                scanned = report.items_scanned,
                updated = report.items_updated,
                "plex recent scan complete"
            );
        }

        Ok(report)
    }

    /// Process a single Plex metadata item — extract IDs and update local DB.
    async fn process_item(
        &self,
        item: &PlexMetadataItem,
        library_type: &str,
        report: &mut ScanReport,
    ) {
        report.items_scanned += 1;

        // Extract external IDs from Guid array
        let mut ids = guid::extract_ids(&item.guids);

        // Fallback to legacy guid field
        if ids.tmdb_id.is_none() && ids.imdb_id.is_none() && ids.tvdb_id.is_none() {
            if let Some(ref legacy) = item.guid {
                ids = guid::extract_ids_from_legacy_guid(legacy);
            }
        }

        // We need at least a TMDB ID to link
        let Some(tmdb_id) = ids.tmdb_id else {
            // Try IMDB/TVDB lookup later — for now skip
            tracing::debug!(
                rating_key = %item.rating_key,
                title = %item.title,
                "no TMDB ID found, skipping"
            );
            return;
        };

        let is_4k = is_4k(&item.media);
        let added_at = item
            .added_at
            .and_then(|ts| {
                chrono::DateTime::from_timestamp(ts, 0)
            });

        match library_type {
            "movie" => {
                let (rk_col, rk_val) = if is_4k {
                    ("plex_rating_key_4k", &item.rating_key)
                } else {
                    ("plex_rating_key", &item.rating_key)
                };

                let query = format!(
                    "UPDATE movies SET {rk_col} = $1, media_added_at = COALESCE($2, media_added_at) \
                     WHERE tmdb_id = $3"
                );
                let result = sqlx::query(&query)
                    .bind(rk_val)
                    .bind(added_at)
                    .bind(tmdb_id)
                    .execute(&self.pool)
                    .await;

                if let Ok(r) = result {
                    if r.rows_affected() > 0 {
                        report.items_updated += 1;
                    }
                }
            }
            "show" => {
                let (rk_col, rk_val) = if is_4k {
                    ("plex_rating_key_4k", &item.rating_key)
                } else {
                    ("plex_rating_key", &item.rating_key)
                };

                let query = format!(
                    "UPDATE series SET {rk_col} = $1, media_added_at = COALESCE($2, media_added_at) \
                     WHERE tmdb_id = $3"
                );
                let result = sqlx::query(&query)
                    .bind(rk_val)
                    .bind(added_at)
                    .bind(tmdb_id)
                    .execute(&self.pool)
                    .await;

                if let Ok(r) = result {
                    if r.rows_affected() > 0 {
                        report.items_updated += 1;
                    }
                }
            }
            _ => {}
        }
    }

    // ── DB helpers ──────────────────────────────────────────────────────────

    async fn load_servers(&self) -> Result<Vec<PlexServer>> {
        let servers = sqlx::query_as::<_, PlexServer>(
            "SELECT id, name, machine_id, ip, port, use_ssl, verify_tls, auth_token, web_app_url, webhook_secret, created_at, updated_at \
             FROM plex_servers ORDER BY id",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(servers)
    }

    async fn load_enabled_libraries(&self, server_id: i32) -> Result<Vec<PlexLibrary>> {
        let libs = sqlx::query_as::<_, PlexLibrary>(
            "SELECT id, plex_server_id, section_id, name, enabled, library_type, last_scan \
             FROM plex_libraries WHERE plex_server_id = $1 AND enabled = true ORDER BY id",
        )
        .bind(server_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(libs)
    }
}

// ── Report ─────────────────────────────────────────────────────────────────

#[derive(Debug, Default, serde::Serialize)]
pub struct ScanReport {
    pub items_scanned: u64,
    pub items_updated: u64,
    pub errors: u64,
}
