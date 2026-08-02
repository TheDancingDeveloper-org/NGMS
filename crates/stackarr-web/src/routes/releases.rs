use std::sync::Arc;

use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::get;
use axum::{Json, Router};
use serde::Deserialize;

use stackarr_core::models::{DownloadProtocol, QualityProfile, ReleaseInfo};
use stackarr_download::DownloadClient;
use stackarr_indexer::search::{MovieSearchCriteria, TvSearchCriteria};
use stackarr_quality::custom_formats::{CustomFormatDef, CustomFormatEngine, parse_specifications};
use stackarr_quality::{
    DecisionContext, DecisionEngine, DownloadDecision, GrabStrategy, rank_releases,
};
use stackarr_scheduler::auto_search;

use crate::AppState;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SearchQuery {
    #[serde(default)]
    term: String,
    /// Optional media type hint: "series" or "movie" (defaults to series)
    #[serde(default)]
    media_type: Option<String>,
    /// Optional quality profile ID to use for decision engine
    #[serde(default)]
    quality_profile_id: Option<i64>,
    /// Optional series ID — enables existing file quality context for episodes
    #[serde(default)]
    series_id: Option<i64>,
    /// Optional movie ID — enables existing file quality context
    #[serde(default)]
    movie_id: Option<i64>,
    /// Optional episode ID — narrows existing file lookup to a specific episode
    #[serde(default)]
    episode_id: Option<i64>,
}

/// Convert an indexer ReleaseInfo into a core model ReleaseInfo.
fn indexer_to_core(r: stackarr_indexer::ReleaseInfo) -> ReleaseInfo {
    ReleaseInfo {
        guid: r.guid,
        title: r.title,
        download_url: r.download_url,
        info_url: r.info_url,
        indexer_id: r.indexer_id,
        indexer_name: r.indexer_name,
        protocol: match r.protocol {
            stackarr_indexer::newznab::Protocol::Torrent => DownloadProtocol::Torrent,
            stackarr_indexer::newznab::Protocol::Usenet => DownloadProtocol::Usenet,
        },
        size: r.size,
        age_days: r.age_days,
        publish_date: r.publish_date,
        info_hash: r.info_hash,
        magnet_url: r.magnet_url,
        seeders: r.seeders,
        leechers: r.leechers,
        nzb_url: r.nzb_url,
        tvdb_id: r.tvdb_id,
        imdb_id: r.imdb_id,
        tmdb_id: r.tmdb_id,
        categories: r.categories,
        indexer_flags: r.indexer_flags,
        indexer_priority: r.indexer_priority,
        password: r.password,
    }
}

async fn search_releases(
    State(state): State<Arc<AppState>>,
    Query(query): Query<SearchQuery>,
) -> impl IntoResponse {
    tracing::info!(term = %query.term, "release search requested");

    if query.term.is_empty() {
        return Json(serde_json::json!([])).into_response();
    }

    // Load quality profile: explicit id > media's profile > first available
    let pool = state.db.pool();
    let profile: QualityProfile = if let Some(id) = query.quality_profile_id {
        match sqlx::query_as::<_, QualityProfile>("SELECT * FROM quality_profiles WHERE id = ?")
            .bind(id as i32)
            .fetch_optional(pool)
            .await
        {
            Ok(Some(p)) => p,
            Ok(None) => {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(serde_json::json!({"error": "quality profile not found"})),
                )
                    .into_response();
            }
            Err(e) => {
                tracing::error!(error = %e, "failed to load quality profile");
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({"error": "failed to load quality profile"})),
                )
                    .into_response();
            }
        }
    } else {
        // Try the media's assigned profile first so interactive search
        // uses the same profile as automatic search.
        let media_profile = if let Some(sid) = query.series_id {
            sqlx::query_as::<_, QualityProfile>(
                "SELECT qp.* FROM series s JOIN quality_profiles qp ON s.quality_profile_id = qp.id WHERE s.id = ?",
            )
            .bind(sid)
            .fetch_optional(pool)
            .await
            .ok()
            .flatten()
        } else if let Some(mid) = query.movie_id {
            sqlx::query_as::<_, QualityProfile>(
                "SELECT qp.* FROM movies m JOIN quality_profiles qp ON m.quality_profile_id = qp.id WHERE m.id = ?",
            )
            .bind(mid)
            .fetch_optional(pool)
            .await
            .ok()
            .flatten()
        } else {
            None
        };

        match media_profile {
            Some(p) => p,
            None => {
                // Fall back to first available profile
                match sqlx::query_as::<_, QualityProfile>(
                    "SELECT * FROM quality_profiles ORDER BY id LIMIT 1",
                )
                .fetch_optional(pool)
                .await
                {
                    Ok(Some(p)) => p,
                    Ok(None) => {
                        // No profiles configured — return raw results without decision engine
                        let mgr = state.indexer_manager.read().await.clone();
                        let criteria = TvSearchCriteria {
                            query: Some(query.term.clone()),
                            tvdb_id: None,
                            season: None,
                            episode: None,
                            categories: vec![],
                        };
                        let releases = mgr.search_series(&criteria).await.unwrap_or_default();
                        let core_releases: Vec<ReleaseInfo> =
                            releases.into_iter().map(indexer_to_core).collect();
                        return Json(core_releases).into_response();
                    }
                    Err(e) => {
                        tracing::error!(error = %e, "failed to load quality profiles");
                        return (
                            StatusCode::INTERNAL_SERVER_ERROR,
                            Json(serde_json::json!({"error": "failed to load quality profiles"})),
                        )
                            .into_response();
                    }
                }
            }
        }
    };

    // Clone the manager (cheap Arc bumps) and drop the lock before network I/O
    let mgr = state.indexer_manager.read().await.clone();
    let is_movie = query
        .media_type
        .as_deref()
        .is_some_and(|t| t.eq_ignore_ascii_case("movie"));

    // Look up media IDs so interactive search uses the same criteria as
    // automatic search (tvdb_id/tmdb_id + season/episode when available).
    let indexer_results = if is_movie {
        let (tmdb_id, imdb_id) = if let Some(mid) = query.movie_id {
            sqlx::query_as::<_, (Option<i64>, Option<String>)>(
                "SELECT tmdb_id, imdb_id FROM movies WHERE id = ?",
            )
            .bind(mid)
            .fetch_optional(pool)
            .await
            .ok()
            .flatten()
            .unwrap_or((None, None))
        } else {
            (None, None)
        };
        let criteria = MovieSearchCriteria {
            query: Some(query.term.clone()),
            tmdb_id,
            imdb_id,
            categories: vec![],
        };
        mgr.search_movies(&criteria).await
    } else {
        let (tvdb_id, season, episode) = if let Some(sid) = query.series_id {
            let tvdb =
                sqlx::query_scalar::<_, Option<i64>>("SELECT tvdb_id FROM series WHERE id = ?")
                    .bind(sid)
                    .fetch_optional(pool)
                    .await
                    .ok()
                    .flatten()
                    .flatten();
            let (s, e) = if let Some(eid) = query.episode_id {
                sqlx::query_as::<_, (i32, i32)>(
                    "SELECT season_number, episode_number FROM episodes WHERE id = ?",
                )
                .bind(eid)
                .fetch_optional(pool)
                .await
                .ok()
                .flatten()
                .map(|(s, e)| (Some(s), Some(e)))
                .unwrap_or((None, None))
            } else {
                (None, None)
            };
            (tvdb, s, e)
        } else {
            (None, None, None)
        };
        let criteria = TvSearchCriteria {
            query: Some(query.term.clone()),
            tvdb_id,
            season,
            episode,
            categories: vec![],
        };
        mgr.search_series(&criteria).await
    };

    let releases = match indexer_results {
        Ok(r) => r,
        Err(e) => {
            tracing::error!(error = %e, "indexer search failed");
            return (
                StatusCode::BAD_GATEWAY,
                Json(serde_json::json!({"error": "indexer search failed"})),
            )
                .into_response();
        }
    };

    // Check which guids are already in queue or history
    let guids: Vec<String> = releases.iter().map(|r| r.guid.clone()).collect();

    let queued_guids: std::collections::HashSet<String> = if guids.is_empty() {
        std::collections::HashSet::new()
    } else {
        let mut query =
            sqlx::QueryBuilder::new("SELECT download_id FROM queue WHERE download_id IN (");
        let mut ids = query.separated(", ");
        for guid in &guids {
            ids.push_bind(guid);
        }
        ids.push_unseparated(")");
        query
            .build_query_scalar::<String>()
            .fetch_all(pool)
            .await
            .unwrap_or_default()
            .into_iter()
            .collect()
    };

    let history_guids: std::collections::HashSet<String> = if guids.is_empty() {
        std::collections::HashSet::new()
    } else {
        let mut query =
            sqlx::QueryBuilder::new("SELECT download_id FROM history WHERE download_id IN (");
        let mut ids = query.separated(", ");
        for guid in &guids {
            ids.push_bind(guid);
        }
        ids.push_unseparated(") AND event_type = 'grabbed'");
        query
            .build_query_scalar::<String>()
            .fetch_all(pool)
            .await
            .unwrap_or_default()
            .into_iter()
            .collect()
    };

    // Check which release titles are blocklisted
    let release_titles: Vec<String> = releases.iter().map(|r| r.title.clone()).collect();
    let blocklisted_titles: std::collections::HashSet<String> = if release_titles.is_empty() {
        std::collections::HashSet::new()
    } else {
        let mut query =
            sqlx::QueryBuilder::new("SELECT source_title FROM blocklist WHERE source_title IN (");
        let mut titles = query.separated(", ");
        for title in &release_titles {
            titles.push_bind(title);
        }
        titles.push_unseparated(")");
        query
            .build_query_scalar::<String>()
            .fetch_all(pool)
            .await
            .unwrap_or_default()
            .into_iter()
            .collect()
    };

    // Load custom formats and profile scores for CF scoring
    let cf_formats: Vec<CustomFormatDef> =
        sqlx::query_as::<_, stackarr_core::models::CustomFormat>("SELECT * FROM custom_formats")
            .fetch_all(pool)
            .await
            .unwrap_or_default()
            .into_iter()
            .filter_map(|cf| {
                let specs = parse_specifications(cf.specifications)?;
                Some(CustomFormatDef {
                    id: cf.id as i64,
                    name: cf.name,
                    specifications: specs,
                })
            })
            .collect();

    let cf_scores: Vec<(i64, i32)> = sqlx::query_as::<_, (i32, i32)>(
        "SELECT format_id, score FROM custom_format_scores WHERE profile_id = ?",
    )
    .bind(profile.id)
    .fetch_all(pool)
    .await
    .unwrap_or_default()
    .into_iter()
    .map(|(fid, score)| (fid as i64, score))
    .collect();

    let cf_engine = CustomFormatEngine::new();

    // Look up existing file quality from the database when media context is provided.
    // This allows QualityCutoffSpec and CustomFormatCutoffSpec to reject releases
    // when the existing file already meets the cutoff (matching Sonarr/Radarr behavior).
    let (existing_quality, existing_cf_score) = auto_search::lookup_existing_quality_and_cf(
        pool,
        &cf_engine,
        &cf_formats,
        &cf_scores,
        is_movie,
        query.series_id,
        query.movie_id,
        query.episode_id,
    )
    .await;

    // Look up highest quality item in queue for this media item (not just by guid).
    // This allows QueueConflictSpec to reject releases when the same episode/movie
    // already has a queued download at equal or higher quality.
    let queued_quality = auto_search::lookup_queued_quality(
        pool,
        is_movie,
        query.series_id,
        query.movie_id,
        query.episode_id,
    )
    .await;

    // Look up movie's original language for LanguageSpec (Radarr -2/Original profiles)
    let original_language = if is_movie {
        if let Some(mid) = query.movie_id {
            sqlx::query_scalar::<_, Option<i32>>(
                "SELECT original_language FROM movies WHERE id = ?",
            )
            .bind(mid)
            .fetch_optional(pool)
            .await
            .ok()
            .flatten()
            .flatten()
        } else {
            None
        }
    } else {
        None
    };

    // Run decision engine on each release
    let engine = DecisionEngine::new();
    let decisions: Vec<DownloadDecision> = releases
        .into_iter()
        .map(|r| {
            let guid = r.guid.clone();
            let core_release = indexer_to_core(r);
            let title = core_release.title.clone();

            // Score this release against custom formats
            let cf_result = cf_engine.score_release(&title, &cf_formats, &cf_scores);

            let ctx = DecisionContext {
                release: core_release,
                profile: profile.clone(),
                existing_quality,
                existing_custom_format_score: existing_cf_score,
                release_custom_format_score: cf_result.total_score,
                matched_formats: cf_result.matched_formats,
                in_queue: queued_guids.contains(&guid),
                in_blocklist: blocklisted_titles.contains(&title),
                already_grabbed: history_guids.contains(&guid),
                queued_quality,
                original_language,
            };
            engine.decide(ctx)
        })
        .collect();

    // Load grab strategy from app_config
    let strategy: GrabStrategy = sqlx::query_scalar::<_, String>(
        "SELECT value #>> '{}' FROM app_config WHERE key = 'grab_strategy'",
    )
    .fetch_optional(state.db.pool())
    .await
    .ok()
    .flatten()
    .and_then(|s| serde_json::from_str(&format!("\"{s}\"")).ok())
    .unwrap_or_default();

    let ranked = rank_releases(decisions, strategy);
    Json(ranked).into_response()
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GrabRequest {
    guid: String,
    indexer_id: i64,
    /// Release title (used for the download request and queue entry)
    #[serde(default)]
    title: String,
    /// Download URL
    #[serde(default)]
    download_url: Option<String>,
    /// Protocol: "usenet" or "torrent"
    #[serde(default)]
    protocol: Option<String>,
    /// Size in bytes
    #[serde(default)]
    size: Option<i64>,
    /// Associated media ID
    #[serde(default)]
    media_id: Option<i64>,
    /// Media type: "series" or "movie"
    #[serde(default)]
    media_type: Option<String>,
    /// Episode ID (for series, enables queue conflict checking by episode)
    #[serde(default)]
    episode_id: Option<i64>,
    /// Archive password (from indexer API)
    #[serde(default)]
    password: Option<String>,
}

async fn grab_release(
    State(state): State<Arc<AppState>>,
    Json(body): Json<GrabRequest>,
) -> impl IntoResponse {
    tracing::info!(guid = %body.guid, indexer_id = body.indexer_id, "release grab requested");

    let download_url = match body.download_url {
        Some(ref url) if !url.is_empty() => url.clone(),
        _ => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": "download_url is required"})),
            )
                .into_response();
        }
    };

    let protocol = match body.protocol.as_deref() {
        Some("torrent") => stackarr_download::DownloadProtocol::Torrent,
        _ => stackarr_download::DownloadProtocol::Usenet,
    };

    // For Cardigann torrent indexers, pre-fetch the torrent file using the
    // indexer's authenticated session so the download client receives raw bytes
    // rather than a URL that would be fetched without auth cookies.
    let torrent_bytes = if protocol == stackarr_download::DownloadProtocol::Torrent {
        let cardigann_indexer = {
            let mgr = state.indexer_manager.read().await;
            mgr.get_cardigann_indexer(body.indexer_id)
        };
        if let Some(indexer) = cardigann_indexer {
            match indexer.fetch_torrent_bytes(&download_url).await {
                Ok(bytes) => {
                    tracing::debug!(
                        indexer_id = body.indexer_id,
                        bytes = bytes.len(),
                        "pre-fetched torrent bytes via Cardigann session"
                    );
                    Some(bytes)
                }
                Err(e) => {
                    tracing::warn!(indexer_id = body.indexer_id, error = %e, "Cardigann torrent fetch failed, falling back to URL");
                    None
                }
            }
        } else {
            None
        }
    } else {
        None
    };

    let grab_req = stackarr_download::GrabRequest {
        title: if body.title.is_empty() {
            body.guid.clone()
        } else {
            body.title.clone()
        },
        download_url,
        category: None,
        protocol,
        password: body.password.clone(),
        torrent_bytes,
    };

    // Extract candidates from behind the lock, then drop it before network I/O
    let candidates = {
        let mgr = state.download_manager.read().await;
        mgr.grab_candidates(grab_req.protocol)
    };
    let (client_id, download_id) = match grab_with_candidates(&candidates, &grab_req).await {
        Ok(result) => result,
        Err(e) => {
            tracing::error!(error = %e, "grab failed");
            return (
                StatusCode::BAD_GATEWAY,
                Json(serde_json::json!({"error": format!("grab failed: {e}")})),
            )
                .into_response();
        }
    };

    // Create queue entry
    let pool = state.db.pool();
    let media_type = body.media_type.as_deref().unwrap_or("series");
    let media_id = body.media_id.unwrap_or(0);
    let core_protocol = match protocol {
        stackarr_download::DownloadProtocol::Torrent => "torrent",
        stackarr_download::DownloadProtocol::Usenet => "usenet",
    };
    let title = if body.title.is_empty() {
        &body.guid
    } else {
        &body.title
    };

    if let Err(e) = sqlx::query(
        "INSERT INTO queue (media_type, media_id, episode_id, title, quality, size, status, download_id, download_client_id, indexer_id, protocol)
         VALUES (?, ?, ?, ?, JSON_OBJECT(), ?, 'queued', ?, ?, ?, ?)",
    )
    .bind(media_type)
    .bind(media_id)
    .bind(body.episode_id)
    .bind(title)
    .bind(body.size)
    .bind(&download_id)
    .bind(if client_id < 0 { None } else { Some(client_id as i32) })
    .bind(body.indexer_id as i32)
    .bind(core_protocol)
    .execute(pool)
    .await
    {
        tracing::warn!(error = %e, "failed to insert queue entry (download still dispatched)");
    }

    // Record in history
    if let Err(e) = sqlx::query(
        "INSERT INTO history (media_type, media_id, event_type, quality, source_title, download_id, indexer_id, download_client)
         VALUES (?, ?, 'grabbed', JSON_OBJECT(), ?, ?, ?, ?)",
    )
    .bind(media_type)
    .bind(media_id)
    .bind(title)
    .bind(&download_id)
    .bind(body.indexer_id as i32)
    .bind(client_id.to_string())
    .execute(pool)
    .await
    {
        tracing::warn!(error = %e, "failed to insert history entry");
    }

    // Dispatch grab notification
    let indexer_name = sqlx::query_scalar::<_, String>("SELECT name FROM indexers WHERE id = ?")
        .bind(body.indexer_id as i32)
        .fetch_optional(pool)
        .await
        .ok()
        .flatten()
        .unwrap_or_else(|| format!("Indexer #{}", body.indexer_id));

    stackarr_notify::dispatch_event(
        pool,
        &stackarr_notify::NotificationEvent::Grab {
            title: title.to_string(),
            quality: String::new(),
            indexer: indexer_name,
        },
    )
    .await;

    (
        StatusCode::OK,
        Json(serde_json::json!({
            "success": true,
            "downloadClientId": client_id,
            "downloadId": download_id,
        })),
    )
        .into_response()
}

/// Try to grab a download using pre-extracted candidates (outside the lock).
/// Mirrors `DownloadClientManager::grab` but operates on cloned `Arc`s.
async fn grab_with_candidates(
    candidates: &[(i64, Arc<dyn DownloadClient>)],
    request: &stackarr_download::GrabRequest,
) -> anyhow::Result<(i64, String)> {
    for (id, client) in candidates {
        match client.add(request).await {
            Ok(download_id) => {
                tracing::info!(
                    client = client.name(),
                    title = %request.title,
                    download_id = %download_id,
                    "download grabbed successfully"
                );
                return Ok((*id, download_id));
            }
            Err(e) => {
                tracing::warn!(client = client.name(), error = %e, "download client failed, trying next");
            }
        }
    }
    anyhow::bail!("no {} download client available", request.protocol);
}

pub fn router() -> Router<Arc<AppState>> {
    Router::new().route("/api/v1/release", get(search_releases).post(grab_release))
}

// ── Public auto-grab helper (used by search commands) ──────────────────────

/// Type alias for the canonical grab result from the scheduler crate.
pub type AutoGrabResult = auto_search::GrabResult;

/// Search indexers for a single media item and auto-grab the best approved release.
///
/// Thin wrapper around `stackarr_scheduler::auto_search::search_and_grab` that
/// extracts the pool, indexer manager, and download manager from `AppState`.
#[allow(clippy::too_many_arguments)]
pub async fn search_and_grab(
    state: &AppState,
    query_term: &str,
    is_movie: bool,
    media_id: i64,
    episode_id: Option<i64>,
    series_id: Option<i64>,
    movie_id: Option<i64>,
    tvdb_id: Option<i64>,
    tmdb_id: Option<i64>,
    imdb_id: Option<String>,
    season: Option<i32>,
    episode: Option<i32>,
) -> Result<Option<AutoGrabResult>, String> {
    auto_search::search_and_grab(
        state.db.pool(),
        &state.indexer_manager,
        &state.download_manager,
        query_term,
        is_movie,
        media_id,
        episode_id,
        series_id,
        movie_id,
        tvdb_id,
        tmdb_id,
        imdb_id,
        season,
        episode,
    )
    .await
    .map_err(|e| e.to_string())
}
