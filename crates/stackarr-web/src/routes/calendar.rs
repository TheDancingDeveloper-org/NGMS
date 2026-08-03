// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 The StackArr Authors

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

#[derive(Debug, sqlx::FromRow)]
struct CalendarRow {
    episode_id: i64,
    series_id: i64,
    series_title: String,
    season_number: i32,
    episode_number: i32,
    episode_title: Option<String>,
    air_date_utc: Option<chrono::DateTime<chrono::Utc>>,
    has_file: bool,
    monitored: bool,
    images: Option<serde_json::Value>,
}

#[derive(Debug, Serialize)]
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
    poster_url: Option<String>,
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

    let rows = sqlx::query_as::<_, CalendarRow>(
        "SELECT e.id as episode_id, e.series_id, s.title as series_title,
                e.season_number, e.episode_number, e.title as episode_title,
                e.air_date_utc, (e.episode_file_id IS NOT NULL) as has_file,
                e.monitored, s.images
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
        Ok(rows) => {
            let entries: Vec<CalendarEntry> = rows
                .into_iter()
                .map(|r| {
                    let poster_url = super::extract_image_url(&r.images, "poster");
                    CalendarEntry {
                        episode_id: r.episode_id,
                        series_id: r.series_id,
                        series_title: r.series_title,
                        season_number: r.season_number,
                        episode_number: r.episode_number,
                        episode_title: r.episode_title,
                        air_date_utc: r.air_date_utc,
                        has_file: r.has_file,
                        monitored: r.monitored,
                        poster_url,
                    }
                })
                .collect();
            Json(entries).into_response()
        }
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
