//! Review + accept/reject API for rows written by the disk scanner when it
//! finds media files on disk that don't match an existing series or movie.
//!
//! Flow:
//!   1. `disk_scan_in_folder` walks a media library folder and emits pending
//!      `import_candidates` rows for anything unmatched.
//!   2. A periodic match pass calls TMDB and fills in suggestions.
//!   3. The UI lists pending candidates via `GET /api/v1/import-candidates`.
//!   4. The user accepts or rejects each one. On accept we create the
//!      corresponding Series/Movie (populating from TMDB) and immediately
//!      run a folder rescan to link the discovered files to the new entity.
//!   5. A second scan resolves everything previously emitted as a candidate.

use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use serde_json::json;

use stackarr_core::models::ImportCandidate;
use stackarr_metadata::TmdbClient;

use crate::AppState;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListQuery {
    pub media_type: Option<String>,
    pub limit: Option<i64>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ListResponse {
    pub items: Vec<ImportCandidate>,
    pub count: usize,
}

pub async fn list(
    State(state): State<Arc<AppState>>,
    Query(q): Query<ListQuery>,
) -> impl IntoResponse {
    let pool = state.db.pool();
    let limit = q.limit.unwrap_or(200).clamp(1, 1000);
    match ImportCandidate::list_pending(pool, q.media_type.as_deref(), limit).await {
        Ok(items) => {
            let count = items.len();
            Json(ListResponse { items, count }).into_response()
        }
        Err(e) => super::api_error(StatusCode::INTERNAL_SERVER_ERROR, e),
    }
}

pub async fn reject(State(state): State<Arc<AppState>>, Path(id): Path<i64>) -> impl IntoResponse {
    let pool = state.db.pool();
    match ImportCandidate::mark_rejected(pool, id).await {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => super::api_error(StatusCode::INTERNAL_SERVER_ERROR, e),
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AcceptBody {
    /// Override the media_library_folder_id from the candidate row. If omitted
    /// we use the one the candidate was discovered in.
    pub media_library_folder_id: Option<i32>,
    /// Which TMDB id to use. Defaults to `candidate.suggested_tmdb_id`.
    pub tmdb_id: Option<i64>,
    /// Initial quality profile (0 → first available).
    #[serde(default)]
    pub quality_profile_id: i32,
    /// Whether the created entity should be monitored. Defaults to true.
    #[serde(default = "default_true")]
    pub monitored: bool,
}

fn default_true() -> bool {
    true
}

pub async fn accept(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    Json(body): Json<AcceptBody>,
) -> impl IntoResponse {
    let pool = state.db.pool();

    let candidate = match ImportCandidate::get(pool, id).await {
        Ok(Some(c)) => c,
        Ok(None) => {
            return super::api_error(StatusCode::NOT_FOUND, "candidate not found");
        }
        Err(e) => return super::api_error(StatusCode::INTERNAL_SERVER_ERROR, e),
    };
    if candidate.status != "pending" {
        return super::api_error(
            StatusCode::CONFLICT,
            format!("candidate is not pending (status={})", candidate.status),
        );
    }

    let tmdb_id = body
        .tmdb_id
        .or(candidate.suggested_tmdb_id.map(|v| v as i64));
    let Some(tmdb_id) = tmdb_id else {
        return super::api_error(
            StatusCode::BAD_REQUEST,
            "no tmdb_id provided and candidate has no suggestion — either retry \
             match or pass tmdb_id in the body",
        );
    };

    let folder_id = body
        .media_library_folder_id
        .or(candidate.media_library_folder_id);
    let Some(folder_id) = folder_id else {
        return super::api_error(
            StatusCode::BAD_REQUEST,
            "candidate has no media_library_folder_id — pass one in the body",
        );
    };

    // Look up folder (for media_type + root path needed by the rescan).
    let folder: Option<(String, String)> =
        match sqlx::query_as("SELECT path, media_type FROM media_library_folders WHERE id = $1")
            .bind(folder_id)
            .fetch_optional(pool)
            .await
        {
            Ok(r) => r,
            Err(e) => return super::api_error(StatusCode::INTERNAL_SERVER_ERROR, e),
        };
    let Some((folder_path, folder_media_type)) = folder else {
        return super::api_error(StatusCode::NOT_FOUND, "media library folder not found");
    };

    // Default quality profile to the first one in the DB.
    let quality_profile_id = if body.quality_profile_id == 0 {
        let qp: Option<(i32,)> =
            sqlx::query_as("SELECT id FROM quality_profiles ORDER BY id LIMIT 1")
                .fetch_optional(pool)
                .await
                .ok()
                .flatten();
        qp.map(|r| r.0).unwrap_or(1)
    } else {
        body.quality_profile_id
    };

    // Resolve TMDB client (may not be configured).
    let tmdb_client: Option<Arc<TmdbClient>> = state.tmdb_client.clone();

    let outcome: Result<AcceptOutcome, String> = match candidate.media_type.as_str() {
        "series" => {
            accept_series(
                pool,
                &candidate,
                tmdb_id,
                folder_id,
                quality_profile_id,
                body.monitored,
                tmdb_client.clone(),
            )
            .await
        }
        "movie" => {
            accept_movie(
                pool,
                &candidate,
                tmdb_id,
                folder_id,
                quality_profile_id,
                body.monitored,
                tmdb_client.clone(),
            )
            .await
        }
        other => Err(format!("unknown media_type '{other}'")),
    };

    match outcome {
        Ok(outcome) => {
            // Run disk_scan on the media library folder to link the newly
            // registered media files. This also resolves any other pending
            // candidates that happen to match the newly-created entity.
            let scan_root = std::path::Path::new(&folder_path);
            if let Err(e) = stackarr_import::disk_scan_in_folder(
                pool,
                Some(folder_id),
                scan_root,
                &folder_media_type,
            )
            .await
            {
                tracing::warn!(error = %e, "accept: post-create disk scan failed (entity still created)");
            }

            if let Err(e) = ImportCandidate::mark_accepted(
                pool,
                candidate.id,
                outcome.series_id,
                outcome.movie_id,
            )
            .await
            {
                tracing::warn!(error = %e, "failed to mark candidate accepted");
            }

            Json(json!({
                "status": "accepted",
                "seriesId": outcome.series_id,
                "movieId": outcome.movie_id,
            }))
            .into_response()
        }
        Err(e) => {
            let _ = ImportCandidate::mark_failed(pool, candidate.id, &e).await;
            super::api_error(StatusCode::BAD_REQUEST, e)
        }
    }
}

struct AcceptOutcome {
    series_id: Option<i64>,
    movie_id: Option<i64>,
}

async fn accept_series(
    pool: &sqlx::PgPool,
    candidate: &ImportCandidate,
    tmdb_id: i64,
    folder_id: i32,
    quality_profile_id: i32,
    monitored: bool,
    tmdb_client: Option<Arc<TmdbClient>>,
) -> Result<AcceptOutcome, String> {
    let title = candidate
        .suggested_title
        .clone()
        .or_else(|| candidate.parsed_title.clone())
        .unwrap_or_else(|| "Unknown Series".to_string());
    let clean = stackarr_parser::clean_title(&title);

    // Insert minimal series row pointing at the on-disk folder.
    let row: (i64,) = sqlx::query_as(
        "INSERT INTO series (
            title, clean_title, sort_title, path, quality_profile_id, monitored,
            media_library_folder_id, tmdb_id
         ) VALUES ($1, $2, $2, $3, $4, $5, $6, $7)
         RETURNING id",
    )
    .bind(&title)
    .bind(&clean)
    .bind(&candidate.discovered_path)
    .bind(quality_profile_id)
    .bind(monitored)
    .bind(folder_id)
    .bind(tmdb_id)
    .fetch_one(pool)
    .await
    .map_err(|e| format!("failed to insert series: {e}"))?;
    let series_id = row.0;

    // Inline TMDB enrichment — populate metadata + episodes so the rescan
    // can actually match season/episode from filenames.
    if let Some(client) = tmdb_client.as_ref()
        && let Ok(detail) = client.get_series(tmdb_id).await
    {
        let mut images: Vec<serde_json::Value> = Vec::new();
        if let Some(ref p) = detail.poster_path {
            images.push(json!({"coverType": "poster", "remoteUrl": format!("https://image.tmdb.org/t/p/w342{p}")}));
        }
        if let Some(ref b) = detail.backdrop_path {
            images.push(json!({"coverType": "fanart", "remoteUrl": format!("https://image.tmdb.org/t/p/original{b}")}));
        }
        let images_json = serde_json::Value::Array(images);
        let status_str = match detail.status.as_deref() {
            Some("Returning Series") | Some("In Production") => "continuing",
            Some("Ended") | Some("Canceled") | Some("Cancelled") => "ended",
            Some("Planned") => "upcoming",
            _ => "continuing",
        };
        let network = detail
            .networks
            .first()
            .map(|n| n.name.as_str())
            .unwrap_or("");
        let genres: Vec<String> = detail.genres.iter().map(|g| g.name.clone()).collect();
        let year = detail
            .first_air_date
            .as_deref()
            .and_then(|d| d.get(..4))
            .and_then(|y| y.parse::<i32>().ok());
        let runtime = detail.episode_run_time.first().copied();
        let tvdb_id = detail.external_ids.as_ref().and_then(|e| e.tvdb_id);
        let imdb_id = detail.external_ids.as_ref().and_then(|e| e.imdb_id.clone());
        let _ = sqlx::query(
            "UPDATE series SET overview = $1, status = $2::text::series_status, network = $3,
             images = $4, genres = $5, year = $6, runtime = $7, tvdb_id = COALESCE($8, tvdb_id),
             imdb_id = COALESCE($9, imdb_id), last_info_sync = NOW()
             WHERE id = $10",
        )
        .bind(&detail.overview)
        .bind(status_str)
        .bind(network)
        .bind(&images_json)
        .bind(&genres)
        .bind(year)
        .bind(runtime)
        .bind(tvdb_id)
        .bind(&imdb_id)
        .bind(series_id)
        .execute(pool)
        .await;

        // Populate episodes for all seasons.
        let num_seasons = detail.number_of_seasons.unwrap_or(0);
        for season_num in 0..=num_seasons {
            if let Ok(season) = client.get_season(tmdb_id, season_num).await {
                for ep in &season.episodes {
                    let _ = sqlx::query(
                        "INSERT INTO episodes (
                            series_id, season_number, episode_number, title, overview,
                            air_date, runtime, monitored
                         ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
                         ON CONFLICT (series_id, season_number, episode_number) DO NOTHING",
                    )
                    .bind(series_id)
                    .bind(ep.season_number)
                    .bind(ep.episode_number)
                    .bind(&ep.name)
                    .bind(&ep.overview)
                    .bind(ep.air_date)
                    .bind(ep.runtime)
                    .bind(season_num > 0)
                    .execute(pool)
                    .await;
                }
            }
        }
    }

    Ok(AcceptOutcome {
        series_id: Some(series_id),
        movie_id: None,
    })
}

async fn accept_movie(
    pool: &sqlx::PgPool,
    candidate: &ImportCandidate,
    tmdb_id: i64,
    folder_id: i32,
    quality_profile_id: i32,
    monitored: bool,
    tmdb_client: Option<Arc<TmdbClient>>,
) -> Result<AcceptOutcome, String> {
    let title = candidate
        .suggested_title
        .clone()
        .or_else(|| candidate.parsed_title.clone())
        .unwrap_or_else(|| "Unknown Movie".to_string());
    let clean = stackarr_parser::clean_title(&title);

    // For movies we use the parent folder of the discovered file as the path
    // (the disk_scan walker expects `{root}/{Movie Folder}/file.mkv`).
    let discovered = std::path::Path::new(&candidate.discovered_path);
    let movie_path = discovered
        .parent()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|| candidate.discovered_path.clone());

    let year = candidate.suggested_year.or(candidate.parsed_year);

    let row: (i64,) = sqlx::query_as(
        "INSERT INTO movies (
            title, clean_title, sort_title, path, quality_profile_id, monitored,
            media_library_folder_id, tmdb_id, year
         ) VALUES ($1, $2, $2, $3, $4, $5, $6, $7, $8)
         RETURNING id",
    )
    .bind(&title)
    .bind(&clean)
    .bind(&movie_path)
    .bind(quality_profile_id)
    .bind(monitored)
    .bind(folder_id)
    .bind(tmdb_id)
    .bind(year)
    .fetch_one(pool)
    .await
    .map_err(|e| format!("failed to insert movie: {e}"))?;
    let movie_id = row.0;

    // Inline TMDB enrichment.
    if let Some(client) = tmdb_client.as_ref()
        && let Ok(detail) = client.get_movie(tmdb_id).await
    {
        let mut images: Vec<serde_json::Value> = Vec::new();
        if let Some(ref p) = detail.poster_path {
            images.push(json!({"coverType": "poster", "remoteUrl": format!("https://image.tmdb.org/t/p/w342{p}")}));
        }
        if let Some(ref b) = detail.backdrop_path {
            images.push(json!({"coverType": "fanart", "remoteUrl": format!("https://image.tmdb.org/t/p/original{b}")}));
        }
        let images_json = serde_json::Value::Array(images);
        let genres: Vec<String> = detail.genres.iter().map(|g| g.name.clone()).collect();
        let imdb_id = detail.imdb_id.clone();
        let runtime = detail.runtime;
        let _ = sqlx::query(
            "UPDATE movies SET overview = $1, images = $2, genres = $3,
             runtime = $4, imdb_id = COALESCE($5, imdb_id), last_info_sync = NOW()
             WHERE id = $6",
        )
        .bind(&detail.overview)
        .bind(&images_json)
        .bind(&genres)
        .bind(runtime)
        .bind(&imdb_id)
        .bind(movie_id)
        .execute(pool)
        .await;
    }

    Ok(AcceptOutcome {
        series_id: None,
        movie_id: Some(movie_id),
    })
}

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/v1/import-candidates", get(list))
        .route("/api/v1/import-candidates/{id}/accept", post(accept))
        .route("/api/v1/import-candidates/{id}/reject", post(reject))
}
