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

// ---------------------------------------------------------------------------
// Bootstrap config
// ---------------------------------------------------------------------------

#[derive(Serialize, Default)]
#[serde(rename_all = "camelCase")]
struct BootstrapConfigResponse {
    enabled: bool,
    url: String,
    token: String,
    advertise_port: Option<u16>,
    upnp_enabled: bool,
    discovery_name: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct UpdateBootstrapConfig {
    enabled: Option<bool>,
    url: Option<String>,
    token: Option<String>,
    advertise_port: Option<Option<u16>>,
    upnp_enabled: Option<bool>,
    discovery_name: Option<String>,
}

async fn get_bootstrap_config(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let rows = sqlx::query_as::<_, (String, serde_json::Value)>(
        "SELECT key, value FROM app_config WHERE key IN ('bootstrap_enabled', 'bootstrap_url', 'bootstrap_token', 'bootstrap_advertise_port', 'bootstrap_upnp_enabled', 'discovery_name')",
    )
    .fetch_all(state.db.pool())
    .await
    .unwrap_or_default();

    // Fall back to TOML config values
    let toml_config = state.config.load();
    let mut config = BootstrapConfigResponse {
        enabled: toml_config.bootstrap.enabled,
        url: toml_config.bootstrap.url.clone().unwrap_or_default(),
        token: toml_config.bootstrap.token.clone().unwrap_or_default(),
        advertise_port: toml_config.bootstrap.advertise_port,
        upnp_enabled: toml_config.bootstrap.upnp_enabled,
        discovery_name: String::new(),
    };

    // Override with DB values where present
    for (key, value) in &rows {
        match key.as_str() {
            "bootstrap_enabled" => {
                if let Some(b) = value.as_bool() {
                    config.enabled = b;
                }
            }
            "bootstrap_url" => {
                if let Some(s) = value.as_str() {
                    config.url = s.to_string();
                }
            }
            "bootstrap_token" => {
                if let Some(s) = value.as_str() {
                    config.token = s.to_string();
                }
            }
            "bootstrap_advertise_port" => {
                if value.is_null() {
                    config.advertise_port = None;
                } else if let Some(n) = value.as_u64() {
                    config.advertise_port = Some(n as u16);
                }
            }
            "bootstrap_upnp_enabled" => {
                if let Some(b) = value.as_bool() {
                    config.upnp_enabled = b;
                }
            }
            "discovery_name" => {
                if let Some(s) = value.as_str() {
                    config.discovery_name = s.to_string();
                }
            }
            _ => {}
        }
    }

    Json(config)
}

async fn put_bootstrap_config(
    State(state): State<Arc<AppState>>,
    Json(body): Json<UpdateBootstrapConfig>,
) -> impl IntoResponse {
    let pool = state.db.pool();

    if let Some(enabled) = body.enabled {
        let _ = sqlx::query(
            "INSERT INTO app_config (key, value) VALUES ('bootstrap_enabled', $1::jsonb)
             ON CONFLICT (key) DO UPDATE SET value = $1::jsonb",
        )
        .bind(serde_json::json!(enabled))
        .execute(pool)
        .await;
    }

    if let Some(url) = &body.url {
        let _ = sqlx::query(
            "INSERT INTO app_config (key, value) VALUES ('bootstrap_url', $1::jsonb)
             ON CONFLICT (key) DO UPDATE SET value = $1::jsonb",
        )
        .bind(serde_json::json!(url))
        .execute(pool)
        .await;
    }

    if let Some(token) = &body.token {
        let _ = sqlx::query(
            "INSERT INTO app_config (key, value) VALUES ('bootstrap_token', $1::jsonb)
             ON CONFLICT (key) DO UPDATE SET value = $1::jsonb",
        )
        .bind(serde_json::json!(token))
        .execute(pool)
        .await;
    }

    if let Some(port) = body.advertise_port {
        let _ = sqlx::query(
            "INSERT INTO app_config (key, value) VALUES ('bootstrap_advertise_port', $1::jsonb)
             ON CONFLICT (key) DO UPDATE SET value = $1::jsonb",
        )
        .bind(serde_json::json!(port))
        .execute(pool)
        .await;
    }

    if let Some(upnp) = body.upnp_enabled {
        let _ = sqlx::query(
            "INSERT INTO app_config (key, value) VALUES ('bootstrap_upnp_enabled', $1::jsonb)
             ON CONFLICT (key) DO UPDATE SET value = $1::jsonb",
        )
        .bind(serde_json::json!(upnp))
        .execute(pool)
        .await;
    }

    if let Some(name) = &body.discovery_name {
        let _ = sqlx::query(
            "INSERT INTO app_config (key, value) VALUES ('discovery_name', $1::jsonb)
             ON CONFLICT (key) DO UPDATE SET value = $1::jsonb",
        )
        .bind(serde_json::json!(name))
        .execute(pool)
        .await;
    }

    Json(serde_json::json!({"success": true}))
}

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/v1/config/general", get(get_general).put(put_general))
        .route(
            "/api/v1/config/bootstrap",
            get(get_bootstrap_config).put(put_bootstrap_config),
        )
}
