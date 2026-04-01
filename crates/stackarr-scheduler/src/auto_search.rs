//! Automatic search for missing/wanted media.
//!
//! Periodically searches indexers for all monitored missing episodes and movies,
//! runs the decision engine, and auto-grabs the best approved release.

use std::sync::Arc;

use anyhow::Result;
use sqlx::PgPool;
use tokio::sync::RwLock;

use stackarr_core::models::{DownloadProtocol, QualityProfile, ReleaseInfo};
use stackarr_download::{DownloadClient, DownloadClientManager};
use stackarr_indexer::search::TvSearchCriteria;
use stackarr_indexer::IndexerManager;
use stackarr_quality::custom_formats::{CustomFormatDef, CustomFormatEngine};
use stackarr_quality::{DecisionContext, DecisionEngine, GrabStrategy, rank_releases};

/// A missing episode row from the database.
#[derive(sqlx::FromRow)]
struct MissingEpisode {
    episode_id: i64,
    series_id: i64,
    series_title: String,
    season_number: i32,
    episode_number: i32,
    tvdb_id: Option<i64>,
    quality_profile_id: i32,
}

/// A missing movie row from the database.
#[derive(sqlx::FromRow)]
struct MissingMovie {
    movie_id: i64,
    movie_title: String,
    tmdb_id: Option<i64>,
    imdb_id: Option<String>,
    quality_profile_id: i32,
    /// Radarr language ID for the movie's original language (1=English, etc.).
    /// Used by LanguageSpec when the profile language is -2 (Original).
    original_language: Option<i32>,
}

/// Run one cycle of automatic search for all missing monitored media.
pub async fn auto_search_missing(
    pool: &PgPool,
    indexer_manager: &Arc<RwLock<IndexerManager>>,
    download_manager: &Arc<RwLock<DownloadClientManager>>,
) -> Result<AutoSearchStats> {
    let mut stats = AutoSearchStats::default();

    // 1. Find all missing monitored episodes (aired, no file)
    let episodes: Vec<MissingEpisode> = sqlx::query_as(
        "SELECT e.id AS episode_id, e.series_id, s.title AS series_title,
                e.season_number, e.episode_number, s.tvdb_id, s.quality_profile_id
         FROM episodes e
         JOIN series s ON e.series_id = s.id
         WHERE e.monitored = true
           AND s.monitored = true
           AND e.episode_file_id IS NULL
           AND e.season_number > 0
           AND (e.air_date IS NULL OR e.air_date <= CURRENT_DATE)
         ORDER BY e.air_date DESC NULLS LAST
         LIMIT 100",
    )
    .fetch_all(pool)
    .await?;

    // 2. Find all missing monitored movies
    let movies: Vec<MissingMovie> = sqlx::query_as(
        "SELECT m.id AS movie_id, m.title AS movie_title,
                m.tmdb_id, m.imdb_id, m.quality_profile_id, m.original_language
         FROM movies m
         LEFT JOIN media_files mf ON mf.id = (
             SELECT episode_file_id FROM episodes WHERE series_id = m.id LIMIT 1
         )
         WHERE m.monitored = true
           AND NOT EXISTS (
               SELECT 1 FROM media_files mf2
               JOIN history h ON h.media_id = m.id AND h.media_type = 'movie' AND h.event_type = 'imported'
               LIMIT 1
           )
           AND NOT EXISTS (
               SELECT 1 FROM queue q WHERE q.media_type = 'movie' AND q.media_id = m.id
           )
         ORDER BY m.added_at DESC
         LIMIT 50",
    )
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    let total = episodes.len() + movies.len();
    if total == 0 {
        tracing::debug!("auto search: no missing monitored media found");
        return Ok(stats);
    }

    tracing::info!(
        episodes = episodes.len(),
        movies = movies.len(),
        "auto search: searching for missing media"
    );

    // Process episodes
    for ep in &episodes {
        stats.searched += 1;
        match search_and_grab_episode(pool, indexer_manager, download_manager, ep).await {
            Ok(true) => {
                stats.grabbed += 1;
                tracing::info!(
                    series = %ep.series_title,
                    season = ep.season_number,
                    episode = ep.episode_number,
                    "auto search: grabbed episode"
                );
            }
            Ok(false) => {}
            Err(e) => {
                stats.errors += 1;
                tracing::debug!(
                    series = %ep.series_title,
                    season = ep.season_number,
                    episode = ep.episode_number,
                    error = %e,
                    "auto search: episode search failed"
                );
            }
        }
    }

    // Process movies
    for movie in &movies {
        stats.searched += 1;
        match search_and_grab_movie(pool, indexer_manager, download_manager, movie).await {
            Ok(true) => {
                stats.grabbed += 1;
                tracing::info!(movie = %movie.movie_title, "auto search: grabbed movie");
            }
            Ok(false) => {}
            Err(e) => {
                stats.errors += 1;
                tracing::debug!(
                    movie = %movie.movie_title,
                    error = %e,
                    "auto search: movie search failed"
                );
            }
        }
    }

    if stats.grabbed > 0 {
        tracing::info!(
            searched = stats.searched,
            grabbed = stats.grabbed,
            errors = stats.errors,
            "auto search completed"
        );
    }

    Ok(stats)
}

#[derive(Default)]
pub struct AutoSearchStats {
    pub searched: usize,
    pub grabbed: usize,
    pub errors: usize,
}

async fn search_and_grab_episode(
    pool: &PgPool,
    indexer_manager: &Arc<RwLock<IndexerManager>>,
    download_manager: &Arc<RwLock<DownloadClientManager>>,
    ep: &MissingEpisode,
) -> Result<bool> {
    let profile = load_quality_profile(pool, ep.quality_profile_id).await?;

    // Clone the manager (cheap Arc bumps) and drop the lock before network I/O
    let mgr = indexer_manager.read().await.clone();
    let criteria = TvSearchCriteria {
        query: Some(ep.series_title.clone()),
        tvdb_id: ep.tvdb_id,
        season: Some(ep.season_number),
        episode: Some(ep.episode_number),
        categories: vec![],
    };
    let releases = mgr.search_series(&criteria).await?;

    if releases.is_empty() {
        return Ok(false);
    }

    // Run decision engine and grab
    let core_releases: Vec<ReleaseInfo> = releases.into_iter().map(indexer_to_core).collect();
    try_grab_best(
        pool,
        download_manager,
        &profile,
        core_releases,
        false, // is_movie
        ep.series_id,
        Some(ep.episode_id),
        Some(ep.series_id),
        None,
        None, // episodes don't have original_language
    )
    .await
}

async fn search_and_grab_movie(
    pool: &PgPool,
    indexer_manager: &Arc<RwLock<IndexerManager>>,
    download_manager: &Arc<RwLock<DownloadClientManager>>,
    movie: &MissingMovie,
) -> Result<bool> {
    let profile = load_quality_profile(pool, movie.quality_profile_id).await?;

    // Clone the manager (cheap Arc bumps) and drop the lock before network I/O
    let mgr = indexer_manager.read().await.clone();
    let criteria = stackarr_indexer::search::MovieSearchCriteria {
        query: Some(movie.movie_title.clone()),
        tmdb_id: movie.tmdb_id,
        imdb_id: movie.imdb_id.clone(),
        categories: vec![],
    };
    let releases = mgr.search_movies(&criteria).await?;

    if releases.is_empty() {
        return Ok(false);
    }

    let core_releases: Vec<ReleaseInfo> = releases.into_iter().map(indexer_to_core).collect();
    try_grab_best(
        pool,
        download_manager,
        &profile,
        core_releases,
        true, // is_movie
        movie.movie_id,
        None,
        None,
        Some(movie.movie_id),
        movie.original_language,
    )
    .await
}

/// Run the decision engine on releases and grab the best approved one.
#[allow(clippy::too_many_arguments)]
async fn try_grab_best(
    pool: &PgPool,
    download_manager: &Arc<RwLock<DownloadClientManager>>,
    profile: &QualityProfile,
    releases: Vec<ReleaseInfo>,
    is_movie: bool,
    media_id: i64,
    episode_id: Option<i64>,
    series_id: Option<i64>,
    movie_id: Option<i64>,
    original_language: Option<i32>,
) -> Result<bool> {
    // Check queue/history/blocklist
    let guids: Vec<String> = releases.iter().map(|r| r.guid.clone()).collect();
    let queued_guids: std::collections::HashSet<String> = sqlx::query_scalar(
        "SELECT download_id FROM queue WHERE download_id = ANY($1)",
    )
    .bind(&guids)
    .fetch_all(pool)
    .await?
    .into_iter()
    .collect();

    let history_guids: std::collections::HashSet<String> = sqlx::query_scalar(
        "SELECT download_id FROM history WHERE download_id = ANY($1) AND event_type = 'grabbed'",
    )
    .bind(&guids)
    .fetch_all(pool)
    .await?
    .into_iter()
    .collect();

    let release_titles: Vec<String> = releases.iter().map(|r| r.title.clone()).collect();
    let blocklisted_titles: std::collections::HashSet<String> = sqlx::query_scalar(
        "SELECT source_title FROM blocklist WHERE source_title = ANY($1)",
    )
    .bind(&release_titles)
    .fetch_all(pool)
    .await?
    .into_iter()
    .collect();

    // Load custom formats
    let cf_formats: Vec<CustomFormatDef> = sqlx::query_as::<_, stackarr_core::models::CustomFormat>(
        "SELECT * FROM custom_formats",
    )
    .fetch_all(pool)
    .await?
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
    .await?
    .into_iter()
    .map(|(fid, score)| (fid as i64, score))
    .collect();

    let cf_engine = CustomFormatEngine::new();

    // Look up existing file quality and custom format score
    let (existing_quality, existing_cf_score) = lookup_existing_quality_and_cf(
        pool, &cf_engine, &cf_formats, &cf_scores, is_movie, series_id, movie_id, episode_id,
    )
    .await;

    // Look up queued quality
    let queued_quality = lookup_queued_quality(pool, is_movie, series_id, movie_id, episode_id).await;

    // Run decision engine
    let engine = DecisionEngine::new();
    let decisions: Vec<stackarr_quality::DownloadDecision> = releases
        .into_iter()
        .map(|r| {
            let guid = r.guid.clone();
            let title = r.title.clone();
            let cf_result = cf_engine.score_release(&title, &cf_formats, &cf_scores);
            let ctx = DecisionContext {
                release: r,
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

    // Rank and pick best
    let strategy: GrabStrategy = sqlx::query_scalar::<_, String>(
        "SELECT value #>> '{}' FROM app_config WHERE key = 'grab_strategy'",
    )
    .fetch_optional(pool)
    .await?
    .and_then(|s| serde_json::from_str(&format!("\"{s}\"")).ok())
    .unwrap_or_default();

    let ranked = rank_releases(decisions, strategy);
    let best = match ranked.into_iter().find(|d| d.approved) {
        Some(d) => d,
        None => return Ok(false),
    };

    let download_url = match best.release.download_url.as_deref() {
        Some(url) if !url.is_empty() => url.to_string(),
        _ => return Ok(false),
    };

    let protocol = match best.release.protocol {
        DownloadProtocol::Torrent => stackarr_download::DownloadProtocol::Torrent,
        DownloadProtocol::Usenet => stackarr_download::DownloadProtocol::Usenet,
    };

    let grab_req = stackarr_download::GrabRequest {
        title: best.release.title.clone(),
        download_url,
        category: None,
        protocol,
    };

    // Extract candidates from behind the lock, then drop it before network I/O
    let candidates = {
        let mgr = download_manager.read().await;
        mgr.grab_candidates(grab_req.protocol)
    };
    let (client_id, download_id) = grab_with_candidates(&candidates, &grab_req).await?;

    // Insert queue entry
    let media_type_str = if is_movie { "movie" } else { "series" };
    let protocol_str = match protocol {
        stackarr_download::DownloadProtocol::Torrent => "torrent",
        stackarr_download::DownloadProtocol::Usenet => "usenet",
    };

    let _ = sqlx::query(
        "INSERT INTO queue (media_type, media_id, episode_id, title, quality, size, status, download_id, download_client_id, indexer_id, protocol)
         VALUES ($1, $2, $3, $4, '{}'::jsonb, $5, 'queued', $6, $7, $8, $9)",
    )
    .bind(media_type_str)
    .bind(media_id)
    .bind(episode_id)
    .bind(&best.release.title)
    .bind(best.release.size)
    .bind(&download_id)
    .bind(if client_id < 0 { None } else { Some(client_id as i32) })
    .bind(best.release.indexer_id as i32)
    .bind(protocol_str)
    .execute(pool)
    .await;

    // Insert history entry
    let _ = sqlx::query(
        "INSERT INTO history (media_type, media_id, episode_id, event_type, quality, source_title, download_id, indexer_id, download_client)
         VALUES ($1, $2, $3, 'grabbed', '{}'::jsonb, $4, $5, $6, $7)",
    )
    .bind(media_type_str)
    .bind(media_id)
    .bind(episode_id)
    .bind(&best.release.title)
    .bind(&download_id)
    .bind(best.release.indexer_id as i32)
    .bind(client_id.to_string())
    .execute(pool)
    .await;

    Ok(true)
}

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

async fn load_quality_profile(pool: &PgPool, id: i32) -> Result<QualityProfile> {
    let profile = sqlx::query_as::<_, QualityProfile>(
        "SELECT * FROM quality_profiles WHERE id = $1",
    )
    .bind(id)
    .fetch_one(pool)
    .await?;
    Ok(profile)
}

/// Look up the quality (and custom format score) of an existing file on disk
/// for the given media context. Returns `(existing_quality_num, existing_cf_score)`.
#[allow(clippy::too_many_arguments)]
async fn lookup_existing_quality_and_cf(
    pool: &PgPool,
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

    // For series: look up the episode's existing file
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

/// Parse quality JSONB and scene name into a quality number and CF score.
fn parse_existing_file_context(
    quality_json: &serde_json::Value,
    scene_name: Option<&str>,
    cf_engine: &CustomFormatEngine,
    cf_formats: &[CustomFormatDef],
    cf_scores: &[(i64, i32)],
) -> (Option<i32>, Option<i32>) {
    let quality_num = quality_json
        .get("quality")
        .and_then(|q| {
            q.get("id")
                .and_then(|id| id.as_i64())
                .or_else(|| q.as_i64())
        })
        .and_then(|v| i32::try_from(v).ok());

    let cf_score = scene_name
        .filter(|s| !s.is_empty())
        .map(|name| cf_engine.score_release(name, cf_formats, cf_scores).total_score);

    (quality_num, cf_score)
}

async fn lookup_queued_quality(
    pool: &PgPool,
    is_movie: bool,
    series_id: Option<i64>,
    movie_id: Option<i64>,
    episode_id: Option<i64>,
) -> Option<i32> {
    let media_type = if is_movie { "movie" } else { "series" };
    let media_id = if is_movie { movie_id? } else { series_id? };

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
            .ok()
            .flatten()
        } else {
            sqlx::query_scalar(
                "SELECT quality FROM queue WHERE media_type = $1 AND media_id = $2 ORDER BY id DESC LIMIT 1",
            )
            .bind(media_type)
            .bind(media_id)
            .fetch_optional(pool)
            .await
            .ok()
            .flatten()
        }
    } else {
        sqlx::query_scalar(
            "SELECT quality FROM queue WHERE media_type = $1 AND media_id = $2 ORDER BY id DESC LIMIT 1",
        )
        .bind(media_type)
        .bind(media_id)
        .fetch_optional(pool)
        .await
        .ok()
        .flatten()
    };

    quality_json?
        .get("quality")
        .and_then(|q| q.get("id").and_then(|id| id.as_i64()).or_else(|| q.as_i64()))
        .and_then(|v| i32::try_from(v).ok())
}

/// Try to grab a download using pre-extracted candidates (outside the lock).
async fn grab_with_candidates(
    candidates: &[(i64, Arc<dyn DownloadClient>)],
    request: &stackarr_download::GrabRequest,
) -> Result<(i64, String)> {
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
