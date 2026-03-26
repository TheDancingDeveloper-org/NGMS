use std::sync::Arc;

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::get;
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::AppState;

#[derive(Serialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
struct NamingConfigRow {
    id: i32,
    media_type: String,
    rename_files: bool,
    standard_format: Option<String>,
    daily_format: Option<String>,
    anime_format: Option<String>,
    season_folder_format: Option<String>,
    movie_format: Option<String>,
    movie_folder_format: Option<String>,
    colon_replacement: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct NamingConfigResponse {
    series: Option<NamingConfigRow>,
    movie: Option<NamingConfigRow>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct UpdateNamingConfigRequest {
    series: Option<UpdateNamingEntry>,
    movie: Option<UpdateNamingEntry>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct UpdateNamingEntry {
    rename_files: Option<bool>,
    standard_format: Option<String>,
    daily_format: Option<String>,
    anime_format: Option<String>,
    season_folder_format: Option<String>,
    movie_format: Option<String>,
    movie_folder_format: Option<String>,
    colon_replacement: Option<String>,
}

async fn get_naming_config(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let pool = state.db.pool();

    match sqlx::query_as::<_, NamingConfigRow>(
        "SELECT id, media_type, rename_files, standard_format, daily_format, anime_format,
                season_folder_format, movie_format, movie_folder_format, colon_replacement
         FROM naming_config ORDER BY media_type",
    )
    .fetch_all(pool)
    .await
    {
        Ok(rows) => {
            let mut series = None;
            let mut movie = None;
            for row in rows {
                match row.media_type.as_str() {
                    "series" => series = Some(row),
                    "movie" => movie = Some(row),
                    _ => {}
                }
            }
            Json(NamingConfigResponse { series, movie }).into_response()
        }
        Err(e) => {
            tracing::error!(error = %e, "failed to fetch naming config");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "internal server error"})),
            )
                .into_response()
        }
    }
}

async fn update_naming_config(
    State(state): State<Arc<AppState>>,
    Json(body): Json<UpdateNamingConfigRequest>,
) -> impl IntoResponse {
    let pool = state.db.pool();

    if let Some(series) = &body.series {
        if let Err(e) = sqlx::query(
            "UPDATE naming_config SET
                rename_files = COALESCE($1, rename_files),
                standard_format = COALESCE($2, standard_format),
                daily_format = COALESCE($3, daily_format),
                anime_format = COALESCE($4, anime_format),
                season_folder_format = COALESCE($5, season_folder_format),
                colon_replacement = COALESCE($6, colon_replacement)
             WHERE media_type = 'series'",
        )
        .bind(series.rename_files)
        .bind(&series.standard_format)
        .bind(&series.daily_format)
        .bind(&series.anime_format)
        .bind(&series.season_folder_format)
        .bind(&series.colon_replacement)
        .execute(pool)
        .await
        {
            tracing::error!(error = %e, "failed to update series naming config");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "internal server error"})),
            )
                .into_response();
        }
    }

    if let Some(movie) = &body.movie {
        if let Err(e) = sqlx::query(
            "UPDATE naming_config SET
                rename_files = COALESCE($1, rename_files),
                movie_format = COALESCE($2, movie_format),
                movie_folder_format = COALESCE($3, movie_folder_format),
                colon_replacement = COALESCE($4, colon_replacement)
             WHERE media_type = 'movie'",
        )
        .bind(movie.rename_files)
        .bind(&movie.movie_format)
        .bind(&movie.movie_folder_format)
        .bind(&movie.colon_replacement)
        .execute(pool)
        .await
        {
            tracing::error!(error = %e, "failed to update movie naming config");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "internal server error"})),
            )
                .into_response();
        }
    }

    // Return the updated config
    get_naming_config(State(state)).await.into_response()
}

pub fn router() -> Router<Arc<AppState>> {
    Router::new().route(
        "/api/v1/config/naming",
        get(get_naming_config).put(update_naming_config),
    )
}
