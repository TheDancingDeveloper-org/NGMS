use std::sync::Arc;

use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::get;
use axum::{Json, Router};
use serde::Deserialize;

use stackarr_core::models::{DownloadProtocol, QualityProfile, ReleaseInfo};
use stackarr_indexer::search::{MovieSearchCriteria, TvSearchCriteria};
use stackarr_quality::{DecisionContext, DecisionEngine, DownloadDecision, rank_releases};

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

    // Run decision engine on each release
    let engine = DecisionEngine::new();
    let decisions: Vec<DownloadDecision> = releases
        .into_iter()
        .map(|r| {
            let guid = r.guid.clone();
            let core_release = indexer_to_core(r);
            let ctx = DecisionContext {
                release: core_release,
                profile: profile.clone(),
                existing_quality: None,
                existing_custom_format_score: None,
                release_custom_format_score: 0,
                in_queue: queued_guids.contains(&guid),
                in_blocklist: false,
                already_grabbed: history_guids.contains(&guid),
            };
            engine.decide(ctx)
        })
        .collect();

    let ranked = rank_releases(decisions);
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
        "INSERT INTO queue (media_type, media_id, title, quality, size, status, download_id, download_client_id, indexer_id, protocol)
         VALUES ($1, $2, $3, '{}'::jsonb, $4, 'queued', $5, $6, $7, $8)",
    )
    .bind(media_type)
    .bind(media_id)
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
