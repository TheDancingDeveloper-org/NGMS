use std::sync::Arc;

use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::get;
use axum::{Json, Router};
use serde::Deserialize;

use stackarr_core::models::{DownloadProtocol, QualityProfile, ReleaseInfo};
use stackarr_indexer::search::{MovieSearchCriteria, TvSearchCriteria};
use stackarr_quality::custom_formats::{CustomFormatDef, CustomFormatEngine};
use stackarr_quality::{DecisionContext, DecisionEngine, DownloadDecision, GrabStrategy, rank_releases};

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

    // Load quality profile (use requested or first available)
    let pool = state.db.pool();
    let profile: QualityProfile = match query.quality_profile_id {
        Some(id) => {
            match sqlx::query_as::<_, QualityProfile>(
                "SELECT * FROM quality_profiles WHERE id = $1",
            )
            .bind(id)
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
        }
        None => {
            match sqlx::query_as::<_, QualityProfile>(
                "SELECT * FROM quality_profiles ORDER BY id LIMIT 1",
            )
            .fetch_optional(pool)
            .await
            {
                Ok(Some(p)) => p,
                Ok(None) => {
                    // No profiles configured — return raw results without decision engine
                    let mgr = state.indexer_manager.read().await;
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
    };

    // Search indexers
    let mgr = state.indexer_manager.read().await;
    let is_movie = query
        .media_type
        .as_deref()
        .is_some_and(|t| t.eq_ignore_ascii_case("movie"));

    let indexer_results = if is_movie {
        let criteria = MovieSearchCriteria {
            query: Some(query.term.clone()),
            tmdb_id: None,
            imdb_id: None,
            categories: vec![],
        };
        mgr.search_movies(&criteria).await
    } else {
        let criteria = TvSearchCriteria {
            query: Some(query.term.clone()),
            tvdb_id: None,
            season: None,
            episode: None,
            categories: vec![],
        };
        mgr.search_series(&criteria).await
    };
    drop(mgr);

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

    let queued_guids: std::collections::HashSet<String> = sqlx::query_scalar(
        "SELECT download_id FROM queue WHERE download_id = ANY($1)",
    )
    .bind(&guids)
    .fetch_all(pool)
    .await
    .unwrap_or_default()
    .into_iter()
    .collect();

    let history_guids: std::collections::HashSet<String> = sqlx::query_scalar(
        "SELECT download_id FROM history WHERE download_id = ANY($1) AND event_type = 'grabbed'",
    )
    .bind(&guids)
    .fetch_all(pool)
    .await
    .unwrap_or_default()
    .into_iter()
    .collect();

    // Check which release titles are blocklisted
    let release_titles: Vec<String> = releases.iter().map(|r| r.title.clone()).collect();
    let blocklisted_titles: std::collections::HashSet<String> = sqlx::query_scalar(
        "SELECT source_title FROM blocklist WHERE source_title = ANY($1)",
    )
    .bind(&release_titles)
    .fetch_all(pool)
    .await
    .unwrap_or_default()
    .into_iter()
    .collect();

    // Load custom formats and profile scores for CF scoring
    let cf_formats: Vec<CustomFormatDef> = sqlx::query_as::<_, stackarr_core::models::CustomFormat>(
        "SELECT * FROM custom_formats",
    )
    .fetch_all(pool)
    .await
    .unwrap_or_default()
    .into_iter()
    .filter_map(|cf| {
        let specs = serde_json::from_value(cf.specifications).ok()?;
        Some(CustomFormatDef {
            id: cf.id as i64,
            name: cf.name,
            specifications: specs,
        })
    })
    .collect();

    let cf_scores: Vec<(i64, i32)> = sqlx::query_as::<_, (i32, i32)>(
        "SELECT format_id, score FROM custom_format_scores WHERE profile_id = $1",
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
    let (existing_quality, existing_cf_score) = lookup_existing_file_quality(
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
    let queued_quality = lookup_queued_quality(
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
                "SELECT original_language FROM movies WHERE id = $1",
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
            let cf_result = cf_engine.score_release(
                &title,
                &cf_formats,
                &cf_scores,
            );

            let ctx = DecisionContext {
                release: core_release,
                profile: profile.clone(),
                existing_quality,
                existing_custom_format_score: existing_cf_score,
                release_custom_format_score: cf_result.total_score,
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

/// Look up the highest quality item in the download queue for a specific media item.
/// Returns Some(quality_num) if there's a queued item, None otherwise.
async fn lookup_queued_quality(
    pool: &sqlx::PgPool,
    is_movie: bool,
    series_id: Option<i64>,
    movie_id: Option<i64>,
    episode_id: Option<i64>,
) -> Option<i32> {
    let media_type = if is_movie { "movie" } else { "series" };

    let media_id = if is_movie {
        movie_id?
    } else {
        series_id?
    };

    // Query the queue for the highest quality item matching this media item.
    // For episodes, also filter by episode_id when available.
    let quality_json: Option<serde_json::Value> = if !is_movie {
        if let Some(eid) = episode_id {
            sqlx::query_scalar(
                "SELECT quality FROM queue WHERE media_type = $1 AND media_id = $2 AND episode_id = $3 ORDER BY id DESC LIMIT 1",
            )
            .bind(media_type)
            .bind(media_id)
            .bind(eid)
            .fetch_optional(pool)
            .await
            .unwrap_or(None)
        } else {
            sqlx::query_scalar(
                "SELECT quality FROM queue WHERE media_type = $1 AND media_id = $2 ORDER BY id DESC LIMIT 1",
            )
            .bind(media_type)
            .bind(media_id)
            .fetch_optional(pool)
            .await
            .unwrap_or(None)
        }
    } else {
        sqlx::query_scalar(
            "SELECT quality FROM queue WHERE media_type = $1 AND media_id = $2 ORDER BY id DESC LIMIT 1",
        )
        .bind(media_type)
        .bind(media_id)
        .fetch_optional(pool)
        .await
        .unwrap_or(None)
    };

    // Parse quality ID from the queue item's quality JSONB
    quality_json.and_then(|qj| {
        qj.get("quality")
            .and_then(|q| q.get("id").and_then(|id| id.as_i64()).or_else(|| q.as_i64()))
            .and_then(|v| i32::try_from(v).ok())
    })
}

/// Look up the quality (and custom format score) of an existing file on disk
/// for the given media context. Returns (existing_quality_num, existing_cf_score).
#[allow(clippy::too_many_arguments)]
async fn lookup_existing_file_quality(
    pool: &sqlx::PgPool,
    cf_engine: &CustomFormatEngine,
    cf_formats: &[CustomFormatDef],
    cf_scores: &[(i64, i32)],
    is_movie: bool,
    series_id: Option<i64>,
    movie_id: Option<i64>,
    episode_id: Option<i64>,
) -> (Option<i32>, Option<i32>) {
    // For movies: look up the movie's existing file
    if is_movie {
        if let Some(mid) = movie_id {
            let row: Option<(serde_json::Value, Option<String>)> = sqlx::query_as(
                "SELECT mf.quality, mf.scene_name
                 FROM movies m
                 JOIN media_files mf ON mf.id = m.movie_file_id
                 WHERE m.id = $1 AND m.movie_file_id IS NOT NULL",
            )
            .bind(mid)
            .fetch_optional(pool)
            .await
            .unwrap_or(None);

            if let Some((quality_json, scene_name)) = row {
                return parse_existing_file_context(
                    &quality_json,
                    scene_name.as_deref(),
                    cf_engine,
                    cf_formats,
                    cf_scores,
                );
            }
        }
        return (None, None);
    }

    // For series: look up the episode's existing file (or best existing file for the series)
    if let Some(eid) = episode_id {
        let row: Option<(serde_json::Value, Option<String>)> = sqlx::query_as(
            "SELECT mf.quality, mf.scene_name
             FROM episodes e
             JOIN media_files mf ON mf.id = e.episode_file_id
             WHERE e.id = $1 AND e.episode_file_id IS NOT NULL",
        )
        .bind(eid)
        .fetch_optional(pool)
        .await
        .unwrap_or(None);

        if let Some((quality_json, scene_name)) = row {
            return parse_existing_file_context(
                &quality_json,
                scene_name.as_deref(),
                cf_engine,
                cf_formats,
                cf_scores,
            );
        }
    } else if let Some(sid) = series_id {
        // No specific episode — use the highest quality file across the series
        let row: Option<(serde_json::Value, Option<String>)> = sqlx::query_as(
            "SELECT mf.quality, mf.scene_name
             FROM episodes e
             JOIN media_files mf ON mf.id = e.episode_file_id
             WHERE e.series_id = $1 AND e.episode_file_id IS NOT NULL
             ORDER BY mf.id DESC LIMIT 1",
        )
        .bind(sid)
        .fetch_optional(pool)
        .await
        .unwrap_or(None);

        if let Some((quality_json, scene_name)) = row {
            return parse_existing_file_context(
                &quality_json,
                scene_name.as_deref(),
                cf_engine,
                cf_formats,
                cf_scores,
            );
        }
    }

    (None, None)
}

/// Parse the quality JSONB from a media_file row into a quality number and CF score.
/// The quality column stores Sonarr/Radarr-format JSON like:
///   `{"quality": {"id": 16, "name": "WEBDL-2160p"}, "revision": {"version": 1, "real": 0}}`
/// or bare `{"quality": 16}`.
fn parse_existing_file_context(
    quality_json: &serde_json::Value,
    scene_name: Option<&str>,
    cf_engine: &CustomFormatEngine,
    cf_formats: &[CustomFormatDef],
    cf_scores: &[(i64, i32)],
) -> (Option<i32>, Option<i32>) {
    // Extract quality ID from the JSONB
    let quality_num = quality_json
        .get("quality")
        .and_then(|q| {
            // Object format: {"quality": {"id": 16}}
            q.get("id")
                .and_then(|id| id.as_i64())
                .or_else(|| q.as_i64()) // Bare integer: {"quality": 16}
        })
        .and_then(|v| i32::try_from(v).ok());

    // Compute CF score for the existing file using its scene name
    let cf_score = scene_name
        .filter(|s| !s.is_empty())
        .map(|name| cf_engine.score_release(name, cf_formats, cf_scores).total_score);

    (quality_num, cf_score)
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

    let grab_req = stackarr_download::GrabRequest {
        title: if body.title.is_empty() {
            body.guid.clone()
        } else {
            body.title.clone()
        },
        download_url,
        category: None,
        protocol,
    };

    // Send to download client
    let mgr = state.download_manager.read().await;
    let (client_id, download_id) = match mgr.grab(&grab_req).await {
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
    drop(mgr);

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
         VALUES ($1, $2, $3, $4, '{}'::jsonb, $5, 'queued', $6, $7, $8, $9)",
    )
    .bind(media_type)
    .bind(media_id)
    .bind(body.episode_id)
    .bind(title)
    .bind(body.size)
    .bind(&download_id)
    .bind(client_id as i32)
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
         VALUES ($1, $2, 'grabbed', '{}'::jsonb, $3, $4, $5, $6)",
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

pub fn router() -> Router<Arc<AppState>> {
    Router::new().route("/api/v1/release", get(search_releases).post(grab_release))
}
