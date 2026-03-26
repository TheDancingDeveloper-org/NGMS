use std::sync::Arc;

use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::get;
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::AppState;

#[derive(Debug, Deserialize)]
struct CalendarParams {
    start: Option<String>,
    end: Option<String>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
struct CalendarEntry {
    episode_id: i64,
    series_id: i64,
    series_title: String,
    season_number: i32,
    episode_number: i32,
    episode_title: Option<String>,
    air_date_utc: Option<chrono::DateTime<chrono::Utc>>,
    has_file: bool,
    monitored: bool,
}

async fn get_calendar(
    State(state): State<Arc<AppState>>,
    Query(params): Query<CalendarParams>,
) -> impl IntoResponse {
    let pool = state.db.pool();

    // Default: next 14 days if no params
    let start = params
        .start
        .unwrap_or_else(|| chrono::Utc::now().format("%Y-%m-%d").to_string());
    let end = params.end.unwrap_or_else(|| {
        (chrono::Utc::now() + chrono::Duration::days(14))
            .format("%Y-%m-%d")
            .to_string()
    });

    let rows = sqlx::query_as::<_, CalendarEntry>(
        "SELECT e.id as episode_id, e.series_id, s.title as series_title,
                e.season_number, e.episode_number, e.title as episode_title,
                e.air_date_utc, (e.episode_file_id IS NOT NULL) as has_file,
                e.monitored
         FROM episodes e
         JOIN series s ON e.series_id = s.id
         WHERE e.air_date_utc >= $1::timestamptz
         AND e.air_date_utc <= $2::timestamptz
         AND s.monitored = true
         ORDER BY e.air_date_utc",
    )
    .bind(&start)
    .bind(&end)
    .fetch_all(pool)
    .await;

    match rows {
        Ok(entries) => Json(entries).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

pub fn router() -> Router<Arc<AppState>> {
    Router::new().route("/api/v1/calendar", get(get_calendar))
}
