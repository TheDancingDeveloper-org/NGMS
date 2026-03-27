use std::sync::Arc;

use axum::extract::State;
use axum::response::IntoResponse;
use axum::routing::get;
use axum::{Json, Router};
use serde::{Deserialize, Serialize};

use crate::AppState;

#[derive(Serialize, Default)]
#[serde(rename_all = "camelCase")]
struct GeneralConfig {
    instance_name: String,
    auth_method: String,
    grab_strategy: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct UpdateGeneralConfig {
    instance_name: Option<String>,
    auth_method: Option<String>,
    grab_strategy: Option<String>,
}

async fn get_general(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let rows = sqlx::query_as::<_, (String, serde_json::Value)>(
        "SELECT key, value FROM app_config WHERE key IN ('instance_name', 'auth_method', 'grab_strategy')",
    )
    .fetch_all(state.db.pool())
    .await
    .unwrap_or_default();

    let mut config = GeneralConfig {
        instance_name: String::new(),
        auth_method: "none".to_string(),
        grab_strategy: "best_quality".to_string(),
    };

    for (key, value) in rows {
        let s = value
            .as_str()
            .map(|s| s.to_string())
            .unwrap_or_else(|| value.to_string().trim_matches('"').to_string());
        match key.as_str() {
            "instance_name" => config.instance_name = s,
            "auth_method" => config.auth_method = s,
            "grab_strategy" => config.grab_strategy = s,
            _ => {}
        }
    }

    Json(config)
}

async fn put_general(
    State(state): State<Arc<AppState>>,
    Json(body): Json<UpdateGeneralConfig>,
) -> impl IntoResponse {
    let pool = state.db.pool();

    if let Some(name) = &body.instance_name {
        let _ = sqlx::query(
            "INSERT INTO app_config (key, value) VALUES ('instance_name', $1::jsonb)
             ON CONFLICT (key) DO UPDATE SET value = $1::jsonb",
        )
        .bind(serde_json::json!(name))
        .execute(pool)
        .await;
    }

    if let Some(method) = &body.auth_method {
        let _ = sqlx::query(
            "INSERT INTO app_config (key, value) VALUES ('auth_method', $1::jsonb)
             ON CONFLICT (key) DO UPDATE SET value = $1::jsonb",
        )
        .bind(serde_json::json!(method))
        .execute(pool)
        .await;
    }

    if let Some(strategy) = &body.grab_strategy {
        let _ = sqlx::query(
            "INSERT INTO app_config (key, value) VALUES ('grab_strategy', $1::jsonb)
             ON CONFLICT (key) DO UPDATE SET value = $1::jsonb",
        )
        .bind(serde_json::json!(strategy))
        .execute(pool)
        .await;
    }

    Json(serde_json::json!({"success": true}))
}

pub fn router() -> Router<Arc<AppState>> {
    Router::new().route("/api/v1/config/general", get(get_general).put(put_general))
}
