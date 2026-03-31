use std::collections::HashMap;
use std::sync::Arc;

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::middleware::redact_sensitive_fields;
use crate::AppState;

#[derive(Serialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
struct IndexerResponse {
    id: i32,
    name: String,
    indexer_type: String,
    base_url: String,
    api_key: Option<String>,
    protocol: String,
    categories: Option<Vec<i32>>,
    enabled: bool,
    priority: i32,
    supports_search: bool,
    supports_rss: bool,
    config: Option<serde_json::Value>,
    last_rss_sync: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateIndexerRequest {
    name: String,
    indexer_type: String,
    base_url: String,
    api_key: Option<String>,
    protocol: String,
    categories: Option<Vec<i32>>,
    enabled: Option<bool>,
    priority: Option<i32>,
    supports_search: Option<bool>,
    supports_rss: Option<bool>,
    config: Option<serde_json::Value>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct UpdateIndexerRequest {
    name: Option<String>,
    indexer_type: Option<String>,
    base_url: Option<String>,
    api_key: Option<String>,
    protocol: Option<String>,
    categories: Option<Vec<i32>>,
    enabled: Option<bool>,
    priority: Option<i32>,
    supports_search: Option<bool>,
    supports_rss: Option<bool>,
    config: Option<serde_json::Value>,
}

// ---------------------------------------------------------------------------
// CRUD endpoints (existing)
// ---------------------------------------------------------------------------

async fn list_indexers(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let pool = state.db.pool();

    match sqlx::query_as::<_, IndexerResponse>(
        "SELECT id, name, indexer_type, base_url, api_key, protocol, categories,
                enabled, priority, supports_search, supports_rss, config, last_rss_sync
         FROM indexers ORDER BY priority, id",
    )
    .fetch_all(pool)
    .await
    {
        Ok(indexers) => {
            let mut value = serde_json::to_value(&indexers).unwrap_or_default();
            redact_sensitive_fields(&mut value);
            Json(value).into_response()
        }
        Err(e) => {
            tracing::error!(error = %e, "failed to list indexers");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "internal server error"})),
            )
                .into_response()
        }
    }
}

async fn create_indexer(
    State(state): State<Arc<AppState>>,
    Json(body): Json<CreateIndexerRequest>,
) -> impl IntoResponse {
    let pool = state.db.pool();

    if body.name.trim().is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "name cannot be empty"})),
        )
            .into_response();
    }

    if body.base_url.trim().is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "base_url cannot be empty"})),
        )
            .into_response();
    }

    let enabled = body.enabled.unwrap_or(true);
    let priority = body.priority.unwrap_or(25);
    let supports_search = body.supports_search.unwrap_or(true);
    let supports_rss = body.supports_rss.unwrap_or(true);

    match sqlx::query_as::<_, IndexerResponse>(
        "INSERT INTO indexers (name, indexer_type, base_url, api_key, protocol, categories,
                               enabled, priority, supports_search, supports_rss, config)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
         RETURNING id, name, indexer_type, base_url, api_key, protocol, categories,
                   enabled, priority, supports_search, supports_rss, config, last_rss_sync",
    )
    .bind(body.name.trim())
    .bind(&body.indexer_type)
    .bind(body.base_url.trim())
    .bind(&body.api_key)
    .bind(&body.protocol)
    .bind(&body.categories)
    .bind(enabled)
    .bind(priority)
    .bind(supports_search)
    .bind(supports_rss)
    .bind(&body.config)
    .fetch_one(pool)
    .await
    {
        Ok(indexer) => {
            // Register the new indexer in the manager for immediate search
            register_indexer_in_manager(&state, &indexer).await;
            let mut value = serde_json::to_value(&indexer).unwrap_or_default();
            redact_sensitive_fields(&mut value);
            (StatusCode::CREATED, Json(value)).into_response()
        }
        Err(e) => {
            tracing::error!(error = %e, "failed to create indexer");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "internal server error"})),
            )
                .into_response()
        }
    }
}

async fn update_indexer(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    Json(body): Json<UpdateIndexerRequest>,
) -> impl IntoResponse {
    let pool = state.db.pool();

    match sqlx::query_as::<_, IndexerResponse>(
        "UPDATE indexers SET
            name = COALESCE($1, name),
            indexer_type = COALESCE($2, indexer_type),
            base_url = COALESCE($3, base_url),
            api_key = COALESCE($4, api_key),
            protocol = COALESCE($5, protocol),
            categories = COALESCE($6, categories),
            enabled = COALESCE($7, enabled),
            priority = COALESCE($8, priority),
            supports_search = COALESCE($9, supports_search),
            supports_rss = COALESCE($10, supports_rss),
            config = COALESCE($11, config)
         WHERE id = $12
         RETURNING id, name, indexer_type, base_url, api_key, protocol, categories,
                   enabled, priority, supports_search, supports_rss, config, last_rss_sync",
    )
    .bind(body.name.as_deref().map(str::trim))
    .bind(&body.indexer_type)
    .bind(body.base_url.as_deref().map(str::trim))
    .bind(&body.api_key)
    .bind(&body.protocol)
    .bind(&body.categories)
    .bind(body.enabled)
    .bind(body.priority)
    .bind(body.supports_search)
    .bind(body.supports_rss)
    .bind(&body.config)
    .bind(id as i32)
    .fetch_optional(pool)
    .await
    {
        Ok(Some(indexer)) => {
            // Update the indexer in the manager
            let mut mgr = state.indexer_manager.write().await;
            mgr.remove_indexer(id);
            drop(mgr);
            register_indexer_in_manager(&state, &indexer).await;
            let mut value = serde_json::to_value(&indexer).unwrap_or_default();
            redact_sensitive_fields(&mut value);
            Json(value).into_response()
        }
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "indexer not found"})),
        )
            .into_response(),
        Err(e) => {
            tracing::error!(error = %e, "failed to update indexer");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "internal server error"})),
            )
                .into_response()
        }
    }
}

async fn delete_indexer(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    let pool = state.db.pool();

    match sqlx::query("DELETE FROM indexers WHERE id = $1")
        .bind(id as i32)
        .execute(pool)
        .await
    {
        Ok(result) => {
            if result.rows_affected() == 0 {
                (
                    StatusCode::NOT_FOUND,
                    Json(json!({"error": "indexer not found"})),
                )
                    .into_response()
            } else {
                // Remove from live manager
                let mut mgr = state.indexer_manager.write().await;
                mgr.remove_indexer(id);
                StatusCode::NO_CONTENT.into_response()
            }
        }
        Err(e) => {
            tracing::error!(error = %e, "failed to delete indexer");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "internal server error"})),
            )
                .into_response()
        }
    }
}

async fn test_indexer(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    let pool = state.db.pool();

    // Load indexer config from DB
    let row: Option<(String, String, Option<String>, String, Option<serde_json::Value>)> =
        match sqlx::query_as(
            "SELECT indexer_type, base_url, api_key, protocol, config FROM indexers WHERE id = $1",
        )
        .bind(id as i32)
        .fetch_optional(pool)
        .await
        {
            Ok(r) => r,
            Err(e) => {
                return Json(json!({
                    "success": false,
                    "message": format!("database error: {e}")
                }))
                .into_response();
            }
        };

    let (indexer_type, base_url, api_key, protocol, config) = match row {
        Some(r) => r,
        None => {
            return Json(json!({
                "success": false,
                "message": "indexer not found"
            }))
            .into_response();
        }
    };

    // For Cardigann/custom indexers, basic connectivity check via HTTP HEAD
    if indexer_type == "cardigann" {
        let test_url = base_url.trim_end_matches('/').to_string();
        match tokio::time::timeout(
            std::time::Duration::from_secs(15),
            reqwest::get(&test_url),
        )
        .await
        {
            Ok(Ok(resp)) if resp.status().is_success() || resp.status().is_redirection() => {
                return Json(json!({
                    "success": true,
                    "message": format!("site reachable (HTTP {})", resp.status().as_u16())
                }))
                .into_response();
            }
            Ok(Ok(resp)) => {
                return Json(json!({
                    "success": false,
                    "message": format!("site returned HTTP {}", resp.status())
                }))
                .into_response();
            }
            Ok(Err(e)) => {
                return Json(json!({
                    "success": false,
                    "message": format!("connection failed: {e}")
                }))
                .into_response();
            }
            Err(_) => {
                return Json(json!({
                    "success": false,
                    "message": "connection timed out after 15 seconds"
                }))
                .into_response();
            }
        }
    }

    // For Newznab/Torznab indexers, test by fetching caps
    let api_key_str = api_key.unwrap_or_default();
    let proto = if protocol == "torrent" {
        stackarr_indexer::newznab::Protocol::Torrent
    } else {
        stackarr_indexer::newznab::Protocol::Usenet
    };

    let client =
        stackarr_indexer::newznab::NewznabClient::new(&base_url, &api_key_str, id, "test", proto);

    match tokio::time::timeout(std::time::Duration::from_secs(15), client.caps()).await {
        Ok(Ok(caps)) => {
            let cat_count = caps.categories.len();
            Json(json!({
                "success": true,
                "message": format!("OK — {cat_count} categories available")
            }))
            .into_response()
        }
        Ok(Err(e)) => {
            let msg = e.to_string();
            Json(json!({
                "success": false,
                "message": msg
            }))
            .into_response()
        }
        Err(_) => Json(json!({
            "success": false,
            "message": "connection timed out after 15 seconds"
        }))
        .into_response(),
    }
}

// ---------------------------------------------------------------------------
// Available indexer catalog (Cardigann definitions)
// ---------------------------------------------------------------------------

/// Summary of an available indexer definition.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AvailableIndexer {
    id: String,
    name: String,
    description: Option<String>,
    privacy: String,
    language: Option<String>,
    protocol: String,
    urls: Vec<String>,
    /// Setting fields the user needs to fill in (empty for public indexers).
    settings: Vec<AvailableSetting>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AvailableSetting {
    name: String,
    field_type: String,
    label: Option<String>,
    default: Option<String>,
    options: Option<Vec<SettingOption>>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SettingOption {
    value: String,
    label: String,
}

/// List all available Cardigann indexer definitions.
/// Supports optional `?privacy=public` query filter.
async fn list_available_indexers(
    State(state): State<Arc<AppState>>,
    axum::extract::Query(params): axum::extract::Query<HashMap<String, String>>,
) -> impl IntoResponse {
    let engine = &state.cardigann_engine;
    let privacy_filter = params.get("privacy").map(String::as_str);

    let mut available: Vec<AvailableIndexer> = engine
        .definitions()
        .values()
        .filter(|def| {
            if let Some(filter) = privacy_filter {
                def.privacy.as_deref() == Some(filter)
            } else {
                true
            }
        })
        .map(|def| {
            let settings: Vec<AvailableSetting> = def
                .settings
                .as_ref()
                .map(|fields| {
                    fields
                        .iter()
                        .filter(|f| f.field_type != "info" && f.field_type != "info_flaresolverr")
                        .map(|f| {
                            let default_str = f.default.as_ref().map(|v| match v {
                                serde_yaml::Value::String(s) => s.clone(),
                                serde_yaml::Value::Bool(b) => b.to_string(),
                                serde_yaml::Value::Number(n) => n.to_string(),
                                _ => String::new(),
                            });
                            AvailableSetting {
                                name: f.name.clone(),
                                field_type: f.field_type.clone(),
                                label: f.label.clone(),
                                default: default_str,
                                options: f.options.as_ref().map(|opts| {
                                    opts.iter()
                                        .map(|(k, v)| SettingOption {
                                            value: k.clone(),
                                            label: v.clone(),
                                        })
                                        .collect()
                                }),
                            }
                        })
                        .collect()
                })
                .unwrap_or_default();

            AvailableIndexer {
                id: def.id.clone(),
                name: def.name.clone(),
                description: def.description.clone(),
                privacy: def.privacy.clone().unwrap_or_else(|| "unknown".into()),
                language: def.language.clone(),
                protocol: "torrent".into(),
                urls: def.links.clone(),
                settings,
            }
        })
        .collect();

    available.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    Json(available).into_response()
}

/// Get a single available indexer definition by ID.
async fn get_available_indexer(
    State(state): State<Arc<AppState>>,
    Path(definition_id): Path<String>,
) -> impl IntoResponse {
    let engine = &state.cardigann_engine;

    match engine.get_definition(&definition_id) {
        Some(def) => {
            let settings: Vec<AvailableSetting> = def
                .settings
                .as_ref()
                .map(|fields| {
                    fields
                        .iter()
                        .filter(|f| f.field_type != "info" && f.field_type != "info_flaresolverr")
                        .map(|f| {
                            let default_str = f.default.as_ref().map(|v| match v {
                                serde_yaml::Value::String(s) => s.clone(),
                                serde_yaml::Value::Bool(b) => b.to_string(),
                                serde_yaml::Value::Number(n) => n.to_string(),
                                _ => String::new(),
                            });
                            AvailableSetting {
                                name: f.name.clone(),
                                field_type: f.field_type.clone(),
                                label: f.label.clone(),
                                default: default_str,
                                options: f.options.as_ref().map(|opts| {
                                    opts.iter()
                                        .map(|(k, v)| SettingOption {
                                            value: k.clone(),
                                            label: v.clone(),
                                        })
                                        .collect()
                                }),
                            }
                        })
                        .collect()
                })
                .unwrap_or_default();

            let resp = AvailableIndexer {
                id: def.id.clone(),
                name: def.name.clone(),
                description: def.description.clone(),
                privacy: def.privacy.clone().unwrap_or_else(|| "unknown".into()),
                language: def.language.clone(),
                protocol: "torrent".into(),
                urls: def.links.clone(),
                settings,
            };
            Json(json!(resp)).into_response()
        }
        None => (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "definition not found"})),
        )
            .into_response(),
    }
}

// ---------------------------------------------------------------------------
// Helper: register an indexer row into the live IndexerManager
// ---------------------------------------------------------------------------

async fn register_indexer_in_manager(state: &AppState, row: &IndexerResponse) {
    let mut mgr = state.indexer_manager.write().await;

    if row.indexer_type == "Cardigann" {
        // Extract definition file from config
        let definition_file = row
            .config
            .as_ref()
            .and_then(|c| c.get("definitionFile"))
            .and_then(|v| v.as_str())
            .unwrap_or("");

        if let Some(def) = state.cardigann_engine.get_definition(definition_file) {
            // Build config map from the stored settings
            let mut config = HashMap::new();
            config.insert("baseUrl".into(), row.base_url.clone());
            if let Some(ref key) = row.api_key {
                config.insert("apiKey".into(), key.clone());
            }
            // Merge any additional settings from config JSONB
            if let Some(serde_json::Value::Object(map)) = row.config.as_ref() {
                for (k, v) in map {
                    if k != "definitionFile" {
                        let val = match v {
                            serde_json::Value::String(s) => s.clone(),
                            serde_json::Value::Number(n) => n.to_string(),
                            serde_json::Value::Bool(b) => b.to_string(),
                            _ => continue,
                        };
                        config.insert(k.clone(), val);
                    }
                }
            }

            match stackarr_cardigann::search::CardigannIndexer::new(
                def.clone(),
                config,
                row.id as i64,
            ) {
                Ok(indexer) => {
                    mgr.add_cardigann_indexer(row.id as i64, &row.name, indexer, row.priority);
                    tracing::debug!(name = %row.name, "registered Cardigann indexer");
                }
                Err(e) => {
                    tracing::warn!(name = %row.name, error = %e, "failed to create Cardigann indexer");
                }
            }
        } else {
            tracing::warn!(
                name = %row.name,
                definition = definition_file,
                "Cardigann definition not found"
            );
        }
    } else {
        // Newznab/Torznab
        let protocol = match row.protocol.as_str() {
            "usenet" => stackarr_indexer::newznab::Protocol::Usenet,
            _ => stackarr_indexer::newznab::Protocol::Torrent,
        };
        mgr.add_indexer(
            row.id as i64,
            &row.name,
            &row.base_url,
            row.api_key.as_deref().unwrap_or(""),
            protocol,
            row.priority,
        );
    }
}

// ---------------------------------------------------------------------------
// Router
// ---------------------------------------------------------------------------

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route(
            "/api/v1/indexer",
            get(list_indexers).post(create_indexer),
        )
        .route(
            "/api/v1/indexer/available",
            get(list_available_indexers),
        )
        .route(
            "/api/v1/indexer/available/{id}",
            get(get_available_indexer),
        )
        .route(
            "/api/v1/indexer/{id}",
            axum::routing::put(update_indexer).delete(delete_indexer),
        )
        .route("/api/v1/indexer/{id}/test", post(test_indexer))
}
