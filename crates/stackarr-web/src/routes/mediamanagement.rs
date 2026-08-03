// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 The StackArr Authors

use std::sync::Arc;

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::{delete, get};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};

use crate::AppState;

// ── Config types ────────────────────────────────────────────────────────────

#[derive(Serialize, Default)]
#[serde(rename_all = "camelCase")]
struct MediaManagementConfig {
    recycle_bin_path: String,
    recycle_bin_cleanup_days: i32,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct UpdateMediaManagementConfig {
    recycle_bin_path: Option<String>,
    recycle_bin_cleanup_days: Option<i32>,
}

// ── Config endpoints ────────────────────────────────────────────────────────

async fn get_config(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let rows = sqlx::query_as::<_, (String, serde_json::Value)>(
        "SELECT key, value FROM app_config WHERE key IN ('recycle_bin_path', 'recycle_bin_cleanup_days')",
    )
    .fetch_all(state.db.pool())
    .await
    .unwrap_or_default();

    let mut config = MediaManagementConfig {
        recycle_bin_path: String::new(),
        recycle_bin_cleanup_days: 7,
    };

    for (key, value) in rows {
        match key.as_str() {
            "recycle_bin_path" => {
                if let Some(s) = value.as_str() {
                    config.recycle_bin_path = s.to_string();
                }
            }
            "recycle_bin_cleanup_days" => {
                if let Some(n) = value.as_i64() {
                    config.recycle_bin_cleanup_days = n as i32;
                }
            }
            _ => {}
        }
    }

    Json(config)
}

async fn put_config(
    State(state): State<Arc<AppState>>,
    Json(body): Json<UpdateMediaManagementConfig>,
) -> impl IntoResponse {
    let pool = state.db.pool();

    if let Some(path) = &body.recycle_bin_path {
        let _ = sqlx::query(
            "INSERT INTO app_config (key, value) VALUES ('recycle_bin_path', $1::jsonb) \
             ON CONFLICT (key) DO UPDATE SET value = $1::jsonb",
        )
        .bind(serde_json::json!(path))
        .execute(pool)
        .await;
    }

    if let Some(days) = body.recycle_bin_cleanup_days {
        let _ = sqlx::query(
            "INSERT INTO app_config (key, value) VALUES ('recycle_bin_cleanup_days', $1::jsonb) \
             ON CONFLICT (key) DO UPDATE SET value = $1::jsonb",
        )
        .bind(serde_json::json!(days))
        .execute(pool)
        .await;
    }

    Json(serde_json::json!({"success": true}))
}

// ── Recycle bin endpoints ───────────────────────────────────────────────────

async fn list_recycle_bin(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    match stackarr_import::recycle_bin::list_entries(state.db.pool()).await {
        Ok(entries) => Json(entries).into_response(),
        Err(e) => super::api_error(StatusCode::INTERNAL_SERVER_ERROR, e),
    }
}

async fn delete_recycle_entry(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    match stackarr_import::recycle_bin::delete_entry(state.db.pool(), id).await {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => super::api_error(StatusCode::INTERNAL_SERVER_ERROR, e),
    }
}

async fn empty_recycle_bin(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    match stackarr_import::recycle_bin::empty_bin(state.db.pool()).await {
        Ok(count) => Json(serde_json::json!({"deleted": count})).into_response(),
        Err(e) => super::api_error(StatusCode::INTERNAL_SERVER_ERROR, e),
    }
}

// ── Router ──────────────────────────────────────────────────────────────────

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route(
            "/api/v1/config/mediamanagement",
            get(get_config).put(put_config),
        )
        .route(
            "/api/v1/recyclebin",
            get(list_recycle_bin).delete(empty_recycle_bin),
        )
        .route("/api/v1/recyclebin/{id}", delete(delete_recycle_entry))
}
