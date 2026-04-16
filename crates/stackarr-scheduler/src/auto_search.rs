//! Automatic search for missing/wanted media.
//!
//! Provides the canonical `search_and_grab` implementation used by both the
//! periodic scheduler and the web layer's search commands (EpisodeSearch,
//! MovieSearch, MissingSearch).

use std::collections::HashSet;
use std::sync::Arc;

use anyhow::Result;
use sqlx::PgPool;
use tokio::sync::RwLock;

use stackarr_core::models::{DownloadProtocol, QualityProfile, ReleaseInfo};
use stackarr_download::{DownloadClient, DownloadClientManager};
use stackarr_indexer::IndexerManager;
use stackarr_indexer::search::{MovieSearchCriteria, TvSearchCriteria};
use stackarr_parser::release::parse_release;
use stackarr_parser::title::title_matches;
use stackarr_quality::custom_formats::{CustomFormatDef, CustomFormatEngine, parse_specifications};
use stackarr_quality::{DecisionContext, DecisionEngine, GrabStrategy, rank_releases};

// ── Public types ─────────────────────────────────────────────────────────────

/// Result of a successful automatic grab.
#[derive(Debug)]
pub struct GrabResult {
    pub title: String,
    pub download_id: String,
    pub indexer_id: i64,
}

/// Statistics from an `auto_search_missing` run.
#[derive(Default)]
pub struct AutoSearchStats {
    pub searched: usize,
    pub grabbed: usize,
    pub errors: usize,
}

// ── Public search+grab API ───────────────────────────────────────────────────

/// Search indexers for a single media item and auto-grab the best approved release.
///
/// Returns `Ok(Some(result))` if a release was grabbed, `Ok(None)` if no approved
/// releases were found, or `Err` on failure.
#[allow(clippy::too_many_arguments)]
pub async fn search_and_grab(
    pool: &PgPool,
    indexer_manager: &Arc<RwLock<IndexerManager>>,
    download_manager: &Arc<RwLock<DownloadClientManager>>,
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
) -> Result<Option<GrabResult>> {
    // Guard against duplicate grabs: if this media already has an active
    // download in the queue (queued / downloading / importing / paused / failed-
    // pending-cleanup), skip the grab entirely. Without this check the scheduler
    // re-grabs the same episode every ~15 s because the "missing" state hasn't
    // flipped yet — producing hundreds of identical jobs that clog the usenet
    // engine and never complete.
    //
    // Only the terminal `completed` status indicates the download is finished
    // — any other state means we already have a job in flight, and the next
    // scheduler sync will either progress it or move it to history on its own.
    let already_queued: Option<(i64,)> = if is_movie {
        sqlx::query_as(
            "SELECT id FROM queue WHERE media_type = 'movie' AND media_id = $1 \
             AND status != 'completed' LIMIT 1",
        )
        .bind(media_id)
        .fetch_optional(pool)
        .await?
    } else if let Some(eid) = episode_id {
        sqlx::query_as(
            "SELECT id FROM queue WHERE media_type = 'series' AND episode_id = $1 \
             AND status != 'completed' LIMIT 1",
        )
        .bind(eid)
        .fetch_optional(pool)
        .await?
    } else {
        None
    };
    if let Some((queue_id,)) = already_queued {
        tracing::debug!(
            query = %query_term,
            is_movie,
            media_id,
            episode_id,
            queue_id,
            "search_and_grab: already in queue, skipping"
        );
        return Ok(None);
    }

    // Load quality profile for the media
    let profile: QualityProfile = if is_movie {
        sqlx::query_as::<_, QualityProfile>(
            "SELECT qp.* FROM movies m JOIN quality_profiles qp ON m.quality_profile_id = qp.id WHERE m.id = $1",
        )
        .bind(media_id)
        .fetch_optional(pool)
        .await?
    } else {
        let sid = series_id.unwrap_or(media_id);
        sqlx::query_as::<_, QualityProfile>(
            "SELECT qp.* FROM series s JOIN quality_profiles qp ON s.quality_profile_id = qp.id WHERE s.id = $1",
        )
        .bind(sid)
        .fetch_optional(pool)
        .await?
    }
    .ok_or_else(|| anyhow::anyhow!("no quality profile found"))?;

    // Clone the manager (cheap Arc bumps) and drop the lock before network I/O
    let mgr = indexer_manager.read().await.clone();
    let releases = if is_movie {
        let criteria = MovieSearchCriteria {
            query: Some(query_term.to_string()),
            tmdb_id,
            imdb_id,
            categories: vec![],
        };
        mgr.search_movies(&criteria).await?
    } else {
        let criteria = TvSearchCriteria {
            query: Some(query_term.to_string()),
            tvdb_id,
            season,
            episode,
            categories: vec![],
        };
        mgr.search_series(&criteria).await?
    };

    if releases.is_empty() {
        return Ok(None);
    }

    // Filter out releases whose parsed title doesn't match the searched media title.
    // Indexers may return results matching only season/episode numbers.
    // Uses fuzzy token-subset matching that tolerates &/and, year inclusion,
    // and leading articles — strict equality silently dropped valid grabs.
    //
    // For series searches, also reject releases with no episode information
    // (e.g. a movie named "My.People.My.Homeland.2020" matching a search for
    // "Homeland S01E03") — these are plainly the wrong type of release.
    let releases: Vec<_> = releases
        .into_iter()
        .filter(|r| {
            if !title_matches(query_term, &r.title) {
                tracing::debug!(
                    release = %r.title,
                    query = %query_term,
                    "search_and_grab: skipping release — title mismatch"
                );
                return false;
            }
            if !is_movie {
                let parsed = parse_release(&r.title);
                let ep = &parsed.episode_info;
                let has_episode_info = !ep.episode_numbers.is_empty()
                    || ep.is_full_season
                    || ep.is_multi_season
                    || ep.air_date.is_some();
                if !has_episode_info {
                    tracing::debug!(
                        release = %r.title,
                        "search_and_grab: skipping release — no episode info (likely a movie)"
                    );
                    return false;
                }
            }
            true
        })
        .collect();

    if releases.is_empty() {
        return Ok(None);
    }

    // Convert indexer releases to core model
    let core_releases: Vec<ReleaseInfo> = releases.into_iter().map(indexer_to_core).collect();

    // Look up movie's original language for LanguageSpec
    let original_language = if is_movie {
        if let Some(mid) = movie_id {
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

    evaluate_and_grab(
        pool,
        download_manager,
        &profile,
        core_releases,
        is_movie,
        media_id,
        episode_id,
        series_id,
        movie_id,
        original_language,
    )
    .await
}

/// Evaluate a set of releases through the decision engine and grab the best
/// approved one.
///
/// This is the core decision+grab pipeline extracted for testability — it takes
/// pre-fetched releases rather than querying indexers.
#[allow(clippy::too_many_arguments)]
pub async fn evaluate_and_grab(
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
) -> Result<Option<GrabResult>> {
    // Check queue/history/blocklist
    let guids: Vec<String> = releases.iter().map(|r| r.guid.clone()).collect();
    let queued_guids: HashSet<String> =
        sqlx::query_scalar("SELECT download_id FROM queue WHERE download_id = ANY($1)")
            .bind(&guids)
            .fetch_all(pool)
            .await?
            .into_iter()
            .collect();

    let history_guids: HashSet<String> = sqlx::query_scalar(
        "SELECT download_id FROM history WHERE download_id = ANY($1) AND event_type = 'grabbed'",
    )
    .bind(&guids)
    .fetch_all(pool)
    .await?
    .into_iter()
    .collect();

    let release_titles: Vec<String> = releases.iter().map(|r| r.title.clone()).collect();
    let blocklisted_titles: HashSet<String> =
        sqlx::query_scalar("SELECT source_title FROM blocklist WHERE source_title = ANY($1)")
            .bind(&release_titles)
            .fetch_all(pool)
            .await?
            .into_iter()
            .collect();

    // Load custom formats
    let cf_formats: Vec<CustomFormatDef> =
        sqlx::query_as::<_, stackarr_core::models::CustomFormat>("SELECT * FROM custom_formats")
            .fetch_all(pool)
            .await?
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
        pool,
        &cf_engine,
        &cf_formats,
        &cf_scores,
        is_movie,
        series_id,
        movie_id,
        episode_id,
    )
    .await;

    // Look up queued quality
    let queued_quality =
        lookup_queued_quality(pool, is_movie, series_id, movie_id, episode_id).await;

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

    // Find first approved release
    let best = match ranked.iter().find(|d| d.approved) {
        Some(d) => d.clone(),
        None => {
            // Log why every release was rejected so we can diagnose
            for d in &ranked {
                let reasons: Vec<&str> = d.rejections.iter().map(|r| r.reason.as_str()).collect();
                tracing::info!(
                    release = %d.release.title,
                    cf_score = d.custom_format_score,
                    reasons = ?reasons,
                    "search_and_grab: release rejected"
                );
            }
            return Ok(None);
        }
    };

    let download_url = match best.release.download_url.as_deref() {
        Some(url) if !url.is_empty() => url.to_string(),
        _ => anyhow::bail!("best release has no download URL"),
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
        password: best.release.password.clone(),
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

    Ok(Some(GrabResult {
        title: best.release.title.clone(),
        download_id,
        indexer_id: best.release.indexer_id,
    }))
}

// ── Scheduler entry point ────────────────────────────────────────────────────

/// A missing episode row from the database.
#[derive(sqlx::FromRow)]
struct MissingEpisode {
    episode_id: i64,
    series_id: i64,
    series_title: String,
    season_number: i32,
    episode_number: i32,
    tvdb_id: Option<i64>,
}

/// A missing movie row from the database.
#[derive(sqlx::FromRow)]
struct MissingMovie {
    movie_id: i64,
    movie_title: String,
    tmdb_id: Option<i64>,
    imdb_id: Option<String>,
}

/// Run one cycle of automatic search for all missing monitored media.
pub async fn auto_search_missing(
    pool: &PgPool,
    indexer_manager: &Arc<RwLock<IndexerManager>>,
    download_manager: &Arc<RwLock<DownloadClientManager>>,
    cancel_token: Option<&tokio_util::sync::CancellationToken>,
) -> Result<AutoSearchStats> {
    let mut stats = AutoSearchStats::default();

    // 1. Find all missing monitored episodes (aired, no file)
    let episodes: Vec<MissingEpisode> = sqlx::query_as(
        "SELECT e.id AS episode_id, e.series_id, s.title AS series_title,
                e.season_number, e.episode_number, s.tvdb_id
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

    // 2. Find all missing monitored movies (no file on disk)
    let movies: Vec<MissingMovie> = sqlx::query_as(
        "SELECT m.id AS movie_id, m.title AS movie_title,
                m.tmdb_id, m.imdb_id
         FROM movies m
         WHERE m.monitored = true
           AND m.movie_file_id IS NULL
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

    // Delay between searches to avoid hammering indexer APIs.
    let inter_search_delay = std::time::Duration::from_secs(2);

    // Process episodes
    for ep in &episodes {
        if cancel_token.is_some_and(|t: &tokio_util::sync::CancellationToken| t.is_cancelled()) {
            tracing::info!("auto search cancelled by user");
            return Ok(stats);
        }
        stats.searched += 1;
        match search_and_grab(
            pool,
            indexer_manager,
            download_manager,
            &ep.series_title,
            false,
            ep.series_id,
            Some(ep.episode_id),
            Some(ep.series_id),
            None,
            ep.tvdb_id,
            None,
            None,
            Some(ep.season_number),
            Some(ep.episode_number),
        )
        .await
        {
            Ok(Some(_)) => {
                stats.grabbed += 1;
                tracing::info!(
                    series = %ep.series_title,
                    season = ep.season_number,
                    episode = ep.episode_number,
                    "auto search: grabbed episode"
                );
            }
            Ok(None) => {}
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
        tokio::time::sleep(inter_search_delay).await;
    }

    // Process movies
    for movie in &movies {
        if cancel_token.is_some_and(|t: &tokio_util::sync::CancellationToken| t.is_cancelled()) {
            tracing::info!("auto search cancelled by user");
            return Ok(stats);
        }
        stats.searched += 1;
        match search_and_grab(
            pool,
            indexer_manager,
            download_manager,
            &movie.movie_title,
            true,
            movie.movie_id,
            None,
            None,
            Some(movie.movie_id),
            None,
            movie.tmdb_id,
            movie.imdb_id.clone(),
            None,
            None,
        )
        .await
        {
            Ok(Some(_)) => {
                stats.grabbed += 1;
                tracing::info!(movie = %movie.movie_title, "auto search: grabbed movie");
            }
            Ok(None) => {}
            Err(e) => {
                stats.errors += 1;
                tracing::debug!(
                    movie = %movie.movie_title,
                    error = %e,
                    "auto search: movie search failed"
                );
            }
        }
        tokio::time::sleep(inter_search_delay).await;
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

// ── Internal helpers ─────────────────────────────────────────────────────────

#[cfg(test)]
async fn load_quality_profile(pool: &PgPool, id: i32) -> Result<QualityProfile> {
    let profile =
        sqlx::query_as::<_, QualityProfile>("SELECT * FROM quality_profiles WHERE id = $1")
            .bind(id)
            .fetch_one(pool)
            .await?;
    Ok(profile)
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
        password: r.password,
    }
}

#[allow(clippy::too_many_arguments)]
pub async fn lookup_existing_quality_and_cf(
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
            // Integer format (normalized): {"quality": 11}
            q.as_i64()
                // Object format (Sonarr native): {"quality": {"id": 11, ...}}
                .or_else(|| q.get("id").and_then(|id| id.as_i64()))
                // Legacy string format: {"quality": "WEBDL1080p"}
                .or_else(|| {
                    serde_json::from_value::<stackarr_parser::Quality>(q.clone())
                        .ok()
                        .map(|pq| stackarr_quality::parser_quality_to_num(pq) as i64)
                })
        })
        .and_then(|v| i32::try_from(v).ok());

    let cf_score = scene_name.filter(|s| !s.is_empty()).map(|name| {
        cf_engine
            .score_release(name, cf_formats, cf_scores)
            .total_score
    });

    (quality_num, cf_score)
}

pub async fn lookup_queued_quality(
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
        .and_then(|q| {
            q.get("id")
                .and_then(|id| id.as_i64())
                .or_else(|| q.as_i64())
        })
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
                tracing::warn!(client = client.name(), error = ?e, url = %request.download_url, "download client failed, trying next");
            }
        }
    }
    anyhow::bail!("no {} download client available", request.protocol);
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use stackarr_core::test_helpers::TestDb;
    use stackarr_download::client::{ClientStatus, DownloadItem};

    // ── Mock download client ─────────────────────────────────────────────

    struct MockClient {
        name: String,
        proto: stackarr_download::DownloadProtocol,
    }

    impl MockClient {
        fn usenet() -> Self {
            Self {
                name: "mock-sab".into(),
                proto: stackarr_download::DownloadProtocol::Usenet,
            }
        }
    }

    #[async_trait::async_trait]
    impl DownloadClient for MockClient {
        fn name(&self) -> &str {
            &self.name
        }
        fn protocol(&self) -> stackarr_download::DownloadProtocol {
            self.proto
        }
        async fn add(&self, _req: &stackarr_download::GrabRequest) -> anyhow::Result<String> {
            Ok(format!("mock-dl-{}", self.name))
        }
        async fn get_items(&self) -> anyhow::Result<Vec<DownloadItem>> {
            Ok(vec![])
        }
        async fn remove(&self, _id: &str, _del: bool) -> anyhow::Result<()> {
            Ok(())
        }
        async fn pause(&self, _id: &str) -> anyhow::Result<()> {
            Ok(())
        }
        async fn resume(&self, _id: &str) -> anyhow::Result<()> {
            Ok(())
        }
        async fn test(&self) -> anyhow::Result<()> {
            Ok(())
        }
        async fn status(&self) -> anyhow::Result<ClientStatus> {
            Ok(ClientStatus {
                name: self.name.clone(),
                protocol: self.proto,
                version: "1.0".into(),
                is_connected: true,
            })
        }
    }

    // ── Helpers ──────────────────────────────────────────────────────────

    fn make_release(title: &str) -> ReleaseInfo {
        ReleaseInfo {
            guid: format!("guid-{title}"),
            title: title.to_string(),
            download_url: Some("http://example.com/dl".to_string()),
            info_url: None,
            indexer_id: 1,
            indexer_name: "TestIndexer".to_string(),
            protocol: DownloadProtocol::Usenet,
            size: 1_500_000_000,
            age_days: 1,
            publish_date: Utc::now(),
            info_hash: None,
            magnet_url: None,
            seeders: None,
            leechers: None,
            nzb_url: None,
            tvdb_id: None,
            imdb_id: None,
            tmdb_id: None,
            categories: vec![],
            indexer_flags: vec![],
            indexer_priority: 25,
            password: None,
        }
    }

    async fn seed_profile_with_quality(pool: &PgPool, allowed_quality: i32) -> i32 {
        let items = serde_json::json!([{"quality": allowed_quality, "allowed": true}]);
        let row: (i32,) = sqlx::query_as(
            "INSERT INTO quality_profiles (name, cutoff, upgrade_allowed, min_format_score, cutoff_format_score, items)
             VALUES ('Test Profile', $1, true, 0, 0, $2) RETURNING id",
        )
        .bind(allowed_quality)
        .bind(items)
        .fetch_one(pool)
        .await
        .expect("seed quality profile");
        row.0
    }

    fn dm_with_usenet() -> Arc<RwLock<DownloadClientManager>> {
        let mut mgr = DownloadClientManager::new();
        mgr.add_client(1, Box::new(MockClient::usenet()), 5);
        Arc::new(RwLock::new(mgr))
    }

    fn dm_empty() -> Arc<RwLock<DownloadClientManager>> {
        Arc::new(RwLock::new(DownloadClientManager::new()))
    }

    // ── Tests ────────────────────────────────────────────────────────────

    #[tokio::test]
    #[ignore = "requires running postgres"]
    async fn test_evaluate_no_releases_returns_none() {
        let db = TestDb::new().await;
        let profile_id = seed_profile_with_quality(&db.pool, 16).await; // WEBDL-2160p

        let profile = load_quality_profile(&db.pool, profile_id).await.unwrap();
        let dm = dm_with_usenet();

        let result = evaluate_and_grab(
            &db.pool,
            &dm,
            &profile,
            vec![], // no releases
            false,
            1,
            None,
            Some(1),
            None,
            None,
        )
        .await
        .unwrap();

        assert!(result.is_none());
        db.cleanup().await;
    }

    #[tokio::test]
    #[ignore = "requires running postgres"]
    async fn test_evaluate_all_rejected_returns_none() {
        let db = TestDb::new().await;
        // Profile only allows quality 6 (HDTV-720p), but release is 2160p (quality 16)
        let profile_id = seed_profile_with_quality(&db.pool, 6).await;
        let profile = load_quality_profile(&db.pool, profile_id).await.unwrap();
        let dm = dm_with_usenet();

        let release = make_release("Show.S01E01.2160p.AMZN.WEB-DL.DDP5.1.H.265-GROUP");
        let result = evaluate_and_grab(
            &db.pool,
            &dm,
            &profile,
            vec![release],
            false,
            1,
            None,
            Some(1),
            None,
            None,
        )
        .await
        .unwrap();

        assert!(
            result.is_none(),
            "should reject release with disallowed quality"
        );
        db.cleanup().await;
    }

    #[tokio::test]
    #[ignore = "requires running postgres"]
    async fn test_evaluate_picks_best_and_grabs() {
        let db = TestDb::new().await;
        // Allow WEBDL-1080p (quality 3)
        let profile_id = seed_profile_with_quality(&db.pool, 11).await;

        // Need a series row for the profile join
        let folder_id =
            stackarr_core::test_helpers::seed_media_library_folder(&db.pool, "/tv", "series").await;
        let series_id =
            stackarr_core::test_helpers::seed_series(&db.pool, "Test Show", profile_id, folder_id)
                .await;
        let ep_id = stackarr_core::test_helpers::seed_episode(&db.pool, series_id, 1, 1).await;

        let profile = load_quality_profile(&db.pool, profile_id).await.unwrap();
        let dm = dm_with_usenet();

        let release = make_release("Test.Show.S01E01.1080p.WEB-DL.x264-GROUP");
        let result = evaluate_and_grab(
            &db.pool,
            &dm,
            &profile,
            vec![release.clone()],
            false,
            series_id,
            Some(ep_id),
            Some(series_id),
            None,
            None,
        )
        .await
        .unwrap();

        assert!(result.is_some(), "should grab the approved release");
        let grab = result.unwrap();
        assert_eq!(grab.title, "Test.Show.S01E01.1080p.WEB-DL.x264-GROUP");
        assert!(grab.download_id.contains("mock-dl-"));
        db.cleanup().await;
    }

    #[tokio::test]
    #[ignore = "requires running postgres"]
    async fn test_evaluate_no_download_client_returns_err() {
        let db = TestDb::new().await;
        let profile_id = seed_profile_with_quality(&db.pool, 11).await;
        let profile = load_quality_profile(&db.pool, profile_id).await.unwrap();
        let dm = dm_empty(); // no clients

        let release = make_release("Show.S01E01.1080p.WEB-DL.x264-GROUP");
        let result = evaluate_and_grab(
            &db.pool,
            &dm,
            &profile,
            vec![release],
            false,
            1,
            None,
            Some(1),
            None,
            None,
        )
        .await;

        assert!(
            result.is_err(),
            "should error when no download client available"
        );
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("no") && err.contains("download client"),
            "error should mention no download client: {err}"
        );
        db.cleanup().await;
    }

    #[tokio::test]
    #[ignore = "requires running postgres"]
    async fn test_evaluate_inserts_queue_and_history() {
        let db = TestDb::new().await;
        let profile_id = seed_profile_with_quality(&db.pool, 11).await;

        let folder_id =
            stackarr_core::test_helpers::seed_media_library_folder(&db.pool, "/tv", "series").await;
        let series_id =
            stackarr_core::test_helpers::seed_series(&db.pool, "Test Show", profile_id, folder_id)
                .await;
        let ep_id = stackarr_core::test_helpers::seed_episode(&db.pool, series_id, 1, 1).await;

        let profile = load_quality_profile(&db.pool, profile_id).await.unwrap();
        let dm = dm_with_usenet();

        // Seed indexer and download_client rows to satisfy FK constraints
        sqlx::query("INSERT INTO indexers (id, name, indexer_type, base_url, protocol, priority) VALUES (1, 'Test', 'Newznab', 'http://localhost', 'usenet', 25)")
            .execute(&db.pool).await.unwrap();
        sqlx::query("INSERT INTO download_clients (id, name, client_type, protocol, config) VALUES (1, 'MockSab', 'SABnzbd', 'usenet', '{}'::jsonb)")
            .execute(&db.pool).await.unwrap();

        let release = make_release("Test.Show.S01E01.1080p.WEB-DL.x264-GROUP");
        let result = evaluate_and_grab(
            &db.pool,
            &dm,
            &profile,
            vec![release],
            false,
            series_id,
            Some(ep_id),
            Some(series_id),
            None,
            None,
        )
        .await
        .unwrap();
        assert!(result.is_some(), "should have grabbed a release");

        // Verify queue entry was created
        let queue_count: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM queue WHERE media_type = 'series' AND media_id = $1",
        )
        .bind(series_id)
        .fetch_one(&db.pool)
        .await
        .unwrap();
        assert_eq!(queue_count.0, 1, "should have 1 queue entry");

        // Verify history entry was created
        let history_count: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM history WHERE media_type = 'series' AND media_id = $1 AND event_type = 'grabbed'",
        )
        .bind(series_id)
        .fetch_one(&db.pool)
        .await
        .unwrap();
        assert_eq!(history_count.0, 1, "should have 1 history entry");

        db.cleanup().await;
    }

    #[tokio::test]
    #[ignore = "requires running postgres"]
    async fn test_evaluate_blocklisted_release_skipped() {
        let db = TestDb::new().await;
        let profile_id = seed_profile_with_quality(&db.pool, 11).await;
        let profile = load_quality_profile(&db.pool, profile_id).await.unwrap();
        let dm = dm_with_usenet();

        let blocked_title = "Show.S01E01.1080p.WEB-DL.x264-BLOCKED";

        // Insert blocklist entry
        sqlx::query("INSERT INTO blocklist (source_title, media_type, media_id, quality) VALUES ($1, 'series', 1, '{}'::jsonb)")
            .bind(blocked_title)
            .execute(&db.pool)
            .await
            .unwrap();

        let release = make_release(blocked_title);
        let result = evaluate_and_grab(
            &db.pool,
            &dm,
            &profile,
            vec![release],
            false,
            1,
            None,
            Some(1),
            None,
            None,
        )
        .await
        .unwrap();

        assert!(result.is_none(), "blocklisted release should be rejected");
        db.cleanup().await;
    }

    #[tokio::test]
    #[ignore = "requires running postgres"]
    async fn test_evaluate_picks_best_when_multiple_approved() {
        let db = TestDb::new().await;
        // Allow both WEBDL-720p (quality 7) and WEBDL-1080p (quality 11)
        let items = serde_json::json!([
            {"quality": 7, "allowed": true},
            {"quality": 11, "allowed": true}
        ]);
        let row: (i32,) = sqlx::query_as(
            "INSERT INTO quality_profiles (name, cutoff, upgrade_allowed, min_format_score, cutoff_format_score, items)
             VALUES ('Multi Profile', 11, true, 0, 0, $1) RETURNING id",
        )
        .bind(items)
        .fetch_one(&db.pool)
        .await
        .unwrap();
        let profile_id = row.0;

        let profile = load_quality_profile(&db.pool, profile_id).await.unwrap();
        let dm = dm_with_usenet();

        let release_720 = make_release("Show.S01E01.720p.WEB-DL.DD5.1.x264-GROUP1");
        let release_1080 = make_release("Show.S01E01.1080p.WEB-DL.DD5.1.x264-GROUP2");

        let result = evaluate_and_grab(
            &db.pool,
            &dm,
            &profile,
            vec![release_720, release_1080],
            false,
            1,
            None,
            Some(1),
            None,
            None,
        )
        .await
        .unwrap();

        // The decision engine should rank 1080p higher than 720p
        assert!(result.is_some(), "should grab one of the releases");
        let grab = result.unwrap();
        assert!(
            grab.title.contains("1080p"),
            "should pick the higher quality release, got: {}",
            grab.title
        );
        db.cleanup().await;
    }
}
