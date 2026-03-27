use std::sync::Arc;

use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::get;
use axum::{Json, Router};
use serde::Deserialize;

use stackarr_core::models::DownloadProtocol;
use stackarr_indexer::newznab::Protocol;
use stackarr_indexer::search::TextSearchCriteria;

use crate::AppState;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SearchQuery {
    #[serde(default)]
    query: String,
    /// Comma-separated category IDs (e.g. "2000,5000")
    #[serde(default)]
    categories: Option<String>,
}

/// Response shape for each search result.
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct SearchResult {
    guid: String,
    title: String,
    download_url: Option<String>,
    info_url: Option<String>,
    indexer_id: i64,
    indexer_name: String,
    protocol: DownloadProtocol,
    size: i64,
    age_days: i64,
    #[serde(with = "chrono::serde::ts_seconds")]
    publish_date: chrono::DateTime<chrono::Utc>,
    // Torrent-specific
    info_hash: Option<String>,
    magnet_url: Option<String>,
    seeders: Option<i32>,
    leechers: Option<i32>,
    // Usenet-specific
    nzb_url: Option<String>,
    categories: Vec<i32>,
    indexer_flags: Vec<String>,
}

fn to_search_result(r: stackarr_indexer::ReleaseInfo) -> SearchResult {
    SearchResult {
        guid: r.guid,
        title: r.title,
        download_url: r.download_url,
        info_url: r.info_url,
        indexer_id: r.indexer_id,
        indexer_name: r.indexer_name,
        protocol: match r.protocol {
            Protocol::Torrent => DownloadProtocol::Torrent,
            Protocol::Usenet => DownloadProtocol::Usenet,
        },
        size: r.size,
        age_days: r.age_days,
        publish_date: r.publish_date,
        info_hash: r.info_hash,
        magnet_url: r.magnet_url,
        seeders: r.seeders,
        leechers: r.leechers,
        nzb_url: r.nzb_url,
        categories: r.categories,
        indexer_flags: r.indexer_flags,
    }
}

async fn search(
    State(state): State<Arc<AppState>>,
    Query(query): Query<SearchQuery>,
) -> impl IntoResponse {
    tracing::info!(query = %query.query, "freehand search requested");

    if query.query.is_empty() {
        return Json(serde_json::json!([])).into_response();
    }

    let categories: Vec<i32> = query
        .categories
        .as_deref()
        .unwrap_or("")
        .split(',')
        .filter_map(|s| s.trim().parse().ok())
        .collect();

    let criteria = TextSearchCriteria {
        query: query.query,
        categories,
    };

    let mgr = state.indexer_manager.read().await;
    let results = match mgr.search_text(&criteria).await {
        Ok(r) => r,
        Err(e) => {
            tracing::error!(error = %e, "freehand search failed");
            return (
                StatusCode::BAD_GATEWAY,
                Json(serde_json::json!({"error": "search failed"})),
            )
                .into_response();
        }
    };
    drop(mgr);

    let out: Vec<SearchResult> = results.into_iter().map(to_search_result).collect();
    Json(out).into_response()
}

pub fn router() -> Router<Arc<AppState>> {
    Router::new().route("/api/v1/search", get(search))
}
