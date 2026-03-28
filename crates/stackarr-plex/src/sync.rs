use anyhow::Result;
use sqlx::PgPool;

use crate::api::{PlexApi, PlexTvApi};
use crate::guid;
use crate::types::*;

// ── Availability Sync ──────────────────────────────────────────────────────

/// Verifies that media marked as available in Plex still exists.
/// Runs every 24 hours to detect items removed from Plex libraries.
pub struct AvailabilitySync {
    pool: PgPool,
}

impl AvailabilitySync {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn run(&self) -> Result<AvailabilitySyncReport> {
        let mut report = AvailabilitySyncReport::default();

        let servers = sqlx::query_as::<_, PlexServer>(
            "SELECT id, name, machine_id, ip, port, use_ssl, verify_tls, auth_token, web_app_url, created_at, updated_at \
             FROM plex_servers ORDER BY id",
        )
        .fetch_all(&self.pool)
        .await?;

        for server in &servers {
            let Some(api) = PlexApi::from_server(server) else {
                continue;
            };

            // Check movies with plex_rating_key set
            let movies: Vec<(i64, String, Option<String>)> = sqlx::query_as(
                "SELECT id, COALESCE(plex_rating_key, ''), plex_rating_key_4k \
                 FROM movies WHERE plex_rating_key IS NOT NULL OR plex_rating_key_4k IS NOT NULL",
            )
            .fetch_all(&self.pool)
            .await?;

            for (movie_id, rk, rk_4k) in &movies {
                report.checked += 1;

                // Check standard quality
                if !rk.is_empty() {
                    if !self.item_exists_in_plex(&api, rk, false).await {
                        tracing::info!(movie_id, rating_key = %rk, "movie no longer in Plex, clearing");
                        let _ = sqlx::query(
                            "UPDATE movies SET plex_rating_key = NULL WHERE id = $1",
                        )
                        .bind(movie_id)
                        .execute(&self.pool)
                        .await;
                        report.removed += 1;
                    }
                }

                // Check 4K
                if let Some(rk4) = rk_4k {
                    if !self.item_exists_in_plex(&api, rk4, true).await {
                        let _ = sqlx::query(
                            "UPDATE movies SET plex_rating_key_4k = NULL WHERE id = $1",
                        )
                        .bind(movie_id)
                        .execute(&self.pool)
                        .await;
                        report.removed += 1;
                    }
                }
            }

            // Check series
            let series: Vec<(i64, String, Option<String>)> = sqlx::query_as(
                "SELECT id, COALESCE(plex_rating_key, ''), plex_rating_key_4k \
                 FROM series WHERE plex_rating_key IS NOT NULL OR plex_rating_key_4k IS NOT NULL",
            )
            .fetch_all(&self.pool)
            .await?;

            for (series_id, rk, rk_4k) in &series {
                report.checked += 1;

                if !rk.is_empty() {
                    if !self.item_exists_in_plex(&api, rk, false).await {
                        tracing::info!(series_id, rating_key = %rk, "series no longer in Plex, clearing");
                        let _ = sqlx::query(
                            "UPDATE series SET plex_rating_key = NULL WHERE id = $1",
                        )
                        .bind(series_id)
                        .execute(&self.pool)
                        .await;
                        report.removed += 1;
                    }
                }

                if let Some(rk4) = rk_4k {
                    if !self.item_exists_in_plex(&api, rk4, true).await {
                        let _ = sqlx::query(
                            "UPDATE series SET plex_rating_key_4k = NULL WHERE id = $1",
                        )
                        .bind(series_id)
                        .execute(&self.pool)
                        .await;
                        report.removed += 1;
                    }
                }
            }
        }

        if report.removed > 0 {
            tracing::info!(
                checked = report.checked,
                removed = report.removed,
                "availability sync complete"
            );
        }

        Ok(report)
    }

    async fn item_exists_in_plex(&self, api: &PlexApi, rating_key: &str, require_4k: bool) -> bool {
        match api.get_metadata(rating_key).await {
            Ok(item) => {
                if require_4k {
                    is_4k(&item.media)
                } else {
                    true
                }
            }
            Err(_) => false,
        }
    }
}

#[derive(Debug, Default, serde::Serialize)]
pub struct AvailabilitySyncReport {
    pub checked: u64,
    pub removed: u64,
}

// ── Watchlist Sync ─────────────────────────────────────────────────────────

/// Syncs Plex watchlists and optionally auto-adds items to the library.
pub struct WatchlistSync {
    pool: PgPool,
}

impl WatchlistSync {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn run(&self) -> Result<WatchlistSyncReport> {
        let mut report = WatchlistSyncReport::default();

        // Get all plex servers with tokens
        let tokens: Vec<(String,)> = sqlx::query_as(
            "SELECT DISTINCT auth_token FROM plex_servers WHERE auth_token IS NOT NULL",
        )
        .fetch_all(&self.pool)
        .await?;

        for (token,) in &tokens {
            let tv_api = PlexTvApi::new(token);

            let watchlist = match tv_api.get_watchlist().await {
                Ok(wl) => wl,
                Err(e) => {
                    tracing::warn!(error = %e, "failed to fetch Plex watchlist");
                    continue;
                }
            };

            for item in &watchlist {
                report.total += 1;

                // Extract TMDB ID from watchlist item GUIDs
                let ids = guid::extract_ids(&item.guids);
                let Some(tmdb_id) = ids.tmdb_id else {
                    // Try legacy guid
                    let ids = item
                        .guid
                        .as_deref()
                        .map(guid::extract_ids_from_legacy_guid)
                        .unwrap_or_default();
                    let Some(tmdb_id) = ids.tmdb_id else {
                        continue;
                    };
                    self.upsert_watchlist_entry(
                        tmdb_id,
                        &item.item_type,
                        &item.rating_key,
                        &mut report,
                    )
                    .await;
                    continue;
                };

                self.upsert_watchlist_entry(
                    tmdb_id,
                    &item.item_type,
                    &item.rating_key,
                    &mut report,
                )
                .await;
            }
        }

        if report.new_entries > 0 {
            tracing::info!(
                total = report.total,
                new = report.new_entries,
                "watchlist sync complete"
            );
        }

        Ok(report)
    }

    async fn upsert_watchlist_entry(
        &self,
        tmdb_id: i64,
        media_type: &str,
        rating_key: &str,
        report: &mut WatchlistSyncReport,
    ) {
        let media_type_normalized = match media_type {
            "show" => "tv",
            other => other,
        };

        let result = sqlx::query(
            "INSERT INTO watchlist (tmdb_id, media_type, plex_rating_key) \
             VALUES ($1, $2, $3) \
             ON CONFLICT (tmdb_id, media_type) DO NOTHING",
        )
        .bind(tmdb_id)
        .bind(media_type_normalized)
        .bind(rating_key)
        .execute(&self.pool)
        .await;

        if let Ok(r) = result {
            if r.rows_affected() > 0 {
                report.new_entries += 1;
            }
        }
    }
}

#[derive(Debug, Default, serde::Serialize)]
pub struct WatchlistSyncReport {
    pub total: u64,
    pub new_entries: u64,
}

// ── Token Refresh ──────────────────────────────────────────────────────────

/// Periodically pings plex.tv to keep auth tokens from expiring.
pub struct TokenRefresh {
    pool: PgPool,
}

impl TokenRefresh {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn run(&self) -> Result<u64> {
        let servers: Vec<(i64, String)> = sqlx::query_as(
            "SELECT id, auth_token FROM plex_servers WHERE auth_token IS NOT NULL",
        )
        .fetch_all(&self.pool)
        .await?;

        let mut refreshed = 0u64;

        for (server_id, token) in &servers {
            let tv_api = PlexTvApi::new(token);
            match tv_api.ping_token().await {
                Ok(true) => {
                    refreshed += 1;
                    tracing::debug!(server_id, "plex token refreshed successfully");
                }
                Ok(false) => {
                    tracing::warn!(server_id, "plex token ping returned non-success");
                }
                Err(e) => {
                    tracing::warn!(server_id, error = %e, "failed to refresh plex token");
                }
            }
        }

        Ok(refreshed)
    }
}
