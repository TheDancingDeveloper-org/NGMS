use std::sync::Arc;

use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::get;
use axum::{Json, Router};
use serde::Deserialize;

use crate::AppState;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SearchQuery {
    #[serde(default)]
    term: String,
}

async fn search_releases(
    State(_state): State<Arc<AppState>>,
    Query(query): Query<SearchQuery>,
) -> impl IntoResponse {
    // TODO: query configured indexers via stackarr_indexer and return results
    // run through the decision engine.
    tracing::info!(term = %query.term, "release search requested");
    Json(serde_json::json!([])).into_response()
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GrabRequest {
    guid: String,
    indexer_id: i64,
}

async fn grab_release(
    State(_state): State<Arc<AppState>>,
    Json(body): Json<GrabRequest>,
) -> impl IntoResponse {
    // TODO: look up release by guid from indexer, send to download client,
    // add queue entry.
    tracing::info!(guid = %body.guid, indexer_id = body.indexer_id, "release grab requested");
    (
        StatusCode::OK,
        Json(serde_json::json!({"success": true})),
    )
        .into_response()
}

pub fn router() -> Router<Arc<AppState>> {
    Router::new().route("/api/v1/release", get(search_releases).post(grab_release))
}
