// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 The StackArr Authors

use std::collections::HashMap;
use std::sync::Arc;

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::AppState;
use crate::middleware::redact_sensitive_fields;

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
        Ok(mut indexers) => {
            // Inject synthetic Indexarr sidecar entry if the module is enabled
            let modules = state.db.load_enabled_modules().await.unwrap_or_default();
            if modules.indexarr_sidecar {
                let priority = sqlx::query_scalar::<_, serde_json::Value>(
                    "SELECT value FROM app_config WHERE key = 'indexarr_priority'",
                )
                .fetch_optional(pool)
                .await
                .ok()
                .flatten()
                .and_then(|v| v.as_i64())
                .unwrap_or(1) as i32;

                let config = state.config.load();
                indexers.push(IndexerResponse {
                    id: -1,
                    name: "Indexarr".to_string(),
                    indexer_type: "Indexarr".to_string(),
                    base_url: config.indexarr.url.clone(),
                    api_key: None,
                    protocol: "usenet".to_string(),
                    categories: None,
                    enabled: state.indexarr_client.is_some(),
                    priority,
                    supports_search: true,
                    supports_rss: true,
                    config: None,
                    last_rss_sync: None,
                });
            }

            indexers.sort_by(|a, b| a.priority.cmp(&b.priority).then(a.id.cmp(&b.id)));
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

    // Handle synthetic Indexarr entry (priority update only)
    if id == -1 {
        if let Some(priority) = body.priority {
            let val = serde_json::Value::Number(serde_json::Number::from(priority));
            let _ = sqlx::query(
                "INSERT INTO app_config (key, value) VALUES ('indexarr_priority', $1)
                 ON CONFLICT (key) DO UPDATE SET value = $1",
            )
            .bind(&val)
            .execute(pool)
            .await;
        }
        return Json(json!({"id": -1, "name": "Indexarr", "priority": body.priority}))
            .into_response();
    }

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
    // Synthetic entries (Indexarr) cannot be deleted
    if id < 0 {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "cannot delete embedded indexer — disable the module instead"})),
        )
            .into_response();
    }

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
    #[allow(clippy::type_complexity)]
    let row: Option<(
        String,
        String,
        Option<String>,
        String,
        Option<serde_json::Value>,
    )> = match sqlx::query_as(
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

    let (indexer_type, base_url, api_key, protocol, _config) = match row {
        Some(r) => r,
        None => {
            return Json(json!({
                "success": false,
                "message": "indexer not found"
            }))
            .into_response();
        }
    };

    // For Cardigann/custom indexers: test login + sample search
    if indexer_type.eq_ignore_ascii_case("cardigann") {
        // Load config from DB to build the Cardigann indexer
        let config_json: Option<serde_json::Value> =
            sqlx::query_scalar("SELECT config FROM indexers WHERE id = $1")
                .bind(id as i32)
                .fetch_optional(pool)
                .await
                .ok()
                .flatten();

        let def_file = config_json
            .as_ref()
            .and_then(|c| c.get("definitionFile"))
            .and_then(|v| v.as_str())
            .unwrap_or("");

        let def = match state.cardigann_engine.get_definition(def_file) {
            Some(d) => d.clone(),
            None => {
                return Json(json!({
                    "success": false,
                    "message": format!("Cardigann definition '{}' not found", def_file)
                }))
                .into_response();
            }
        };

        // Build config map the same way startup does
        let mut idx_config = HashMap::new();
        idx_config.insert("baseUrl".into(), base_url.clone());
        if let Some(ref key) = api_key {
            idx_config.insert("apiKey".into(), key.clone());
        }
        if let Some(serde_json::Value::Object(map)) = config_json.as_ref() {
            for (k, v) in map {
                if k != "definitionFile" {
                    let val = match v {
                        serde_json::Value::String(s) => s.clone(),
                        serde_json::Value::Number(n) => n.to_string(),
                        serde_json::Value::Bool(b) => b.to_string(),
                        _ => continue,
                    };
                    idx_config.insert(k.clone(), val);
                }
            }
        }

        let indexer = match stackarr_cardigann::search::CardigannIndexer::new(def, idx_config, id) {
            Ok(i) => i,
            Err(e) => {
                return Json(json!({
                    "success": false,
                    "message": format!("failed to create indexer: {e}")
                }))
                .into_response();
            }
        };

        // Try a real search — this tests login + search + response parsing
        let sq = stackarr_cardigann::search::SearchQuery {
            query: "test".into(),
            ..Default::default()
        };
        match tokio::time::timeout(std::time::Duration::from_secs(30), indexer.search(&sq)).await {
            Ok(Ok(results)) => {
                return Json(json!({
                    "success": true,
                    "message": format!("search OK — {} results for test query", results.len())
                }))
                .into_response();
            }
            Ok(Err(e)) => {
                return Json(json!({
                    "success": false,
                    "message": format!("search failed: {e}")
                }))
                .into_response();
            }
            Err(_) => {
                return Json(json!({
                    "success": false,
                    "message": "search timed out after 30 seconds"
                }))
                .into_response();
            }
        }
    }

    // For Newznab/Torznab indexers: probe candidate URLs, test caps + sample search
    let api_key_str = api_key.unwrap_or_default();
    let proto = if protocol == "torrent" {
        stackarr_indexer::newznab::Protocol::Torrent
    } else {
        stackarr_indexer::newznab::Protocol::Usenet
    };

    let candidates = stackarr_indexer::newznab::candidate_base_urls(&base_url);
    let mut last_error = String::from("no candidates");

    for candidate_url in &candidates {
        let client = stackarr_indexer::newznab::NewznabClient::new(
            candidate_url,
            &api_key_str,
            id,
            "test",
            proto,
        );

        // Step 1: caps
        let caps =
            match tokio::time::timeout(std::time::Duration::from_secs(15), client.caps()).await {
                Ok(Ok(caps)) => caps,
                Ok(Err(e)) => {
                    last_error = e.to_string();
                    continue; // Try next candidate
                }
                Err(_) => {
                    last_error = "connection timed out".to_string();
                    continue;
                }
            };

        // Step 2: sample search
        let search_result = tokio::time::timeout(
            std::time::Duration::from_secs(15),
            client.search("test", &[]),
        )
        .await;

        let cat_count = caps.categories.len();
        let url_changed = *candidate_url != base_url.trim_end_matches('/');

        // If URL was auto-corrected, update the DB
        if url_changed {
            let _ = sqlx::query("UPDATE indexers SET base_url = $1 WHERE id = $2")
                .bind(candidate_url)
                .bind(id as i32)
                .execute(state.db.pool())
                .await;
            // Re-register in IndexerManager with corrected URL
            if let Ok(Some(updated_row)) = sqlx::query_as::<_, IndexerResponse>(
                "SELECT id, name, indexer_type, base_url, api_key, protocol, categories,
                        enabled, priority, supports_search, supports_rss, config, last_rss_sync
                 FROM indexers WHERE id = $1",
            )
            .bind(id as i32)
            .fetch_optional(state.db.pool())
            .await
            {
                let mut mgr = state.indexer_manager.write().await;
                mgr.remove_indexer(id);
                drop(mgr);
                register_indexer_in_manager(&state, &updated_row).await;
            }
        }

        let correction = if url_changed {
            format!(" (URL auto-corrected to {candidate_url})")
        } else {
            String::new()
        };

        return match search_result {
            Ok(Ok(releases)) => {
                let n = releases.len();
                Json(json!({
                    "success": true,
                    "message": format!("OK — {cat_count} categories, {n} sample results{correction}"),
                    "correctedUrl": if url_changed { Some(candidate_url.clone()) } else { None }
                })).into_response()
            }
            Ok(Err(e)) => Json(json!({
                "success": true,
                "message": format!("OK — {cat_count} categories (search: {e}){correction}"),
                "correctedUrl": if url_changed { Some(candidate_url.clone()) } else { None }
            }))
            .into_response(),
            Err(_) => Json(json!({
                "success": true,
                "message": format!("OK — {cat_count} categories (search timed out){correction}"),
                "correctedUrl": if url_changed { Some(candidate_url.clone()) } else { None }
            }))
            .into_response(),
        };
    }

    // None of the candidates worked
    Json(json!({
        "success": false,
        "message": last_error
    }))
    .into_response()
}

/// Test an indexer configuration without saving it first.
async fn test_indexer_config(
    State(state): State<Arc<AppState>>,
    Json(body): Json<CreateIndexerRequest>,
) -> impl IntoResponse {
    let indexer_type = &body.indexer_type;
    let base_url = body.base_url.trim().to_string();

    if indexer_type.eq_ignore_ascii_case("cardigann") {
        // Get the definition file from config
        let def_file = body
            .config
            .as_ref()
            .and_then(|c| c.get("definitionFile"))
            .and_then(|v| v.as_str())
            .unwrap_or("");

        let def = match state.cardigann_engine.get_definition(def_file) {
            Some(d) => d.clone(),
            None => {
                return Json(json!({
                    "success": false,
                    "message": format!("Cardigann definition '{}' not found", def_file)
                }))
                .into_response();
            }
        };

        let mut idx_config = HashMap::new();
        idx_config.insert("baseUrl".into(), base_url.clone());
        if let Some(ref key) = body.api_key {
            idx_config.insert("apiKey".into(), key.clone());
        }
        if let Some(serde_json::Value::Object(ref map)) = body.config {
            for (k, v) in map {
                if k != "definitionFile" {
                    let val = match v {
                        serde_json::Value::String(s) => s.clone(),
                        serde_json::Value::Number(n) => n.to_string(),
                        serde_json::Value::Bool(b) => b.to_string(),
                        _ => continue,
                    };
                    idx_config.insert(k.clone(), val);
                }
            }
        }

        let indexer = match stackarr_cardigann::search::CardigannIndexer::new(def, idx_config, 0) {
            Ok(i) => i,
            Err(e) => {
                return Json(json!({
                    "success": false,
                    "message": format!("failed to create indexer: {e}")
                }))
                .into_response();
            }
        };

        let sq = stackarr_cardigann::search::SearchQuery {
            query: "test".into(),
            ..Default::default()
        };
        match tokio::time::timeout(std::time::Duration::from_secs(30), indexer.search(&sq)).await {
            Ok(Ok(results)) => {
                return Json(json!({
                    "success": true,
                    "message": format!("search OK — {} results for test query", results.len())
                }))
                .into_response();
            }
            Ok(Err(e)) => {
                return Json(json!({
                    "success": false,
                    "message": format!("search failed: {e}")
                }))
                .into_response();
            }
            Err(_) => {
                return Json(json!({
                    "success": false,
                    "message": "search timed out after 30 seconds"
                }))
                .into_response();
            }
        }
    }

    let api_key_str = body.api_key.as_deref().unwrap_or("");
    let proto = if body.protocol == "torrent" || body.indexer_type == "Torznab" {
        stackarr_indexer::newznab::Protocol::Torrent
    } else {
        stackarr_indexer::newznab::Protocol::Usenet
    };

    // Probe candidate URLs (as-given first, then with API suffix stripped)
    let candidates = stackarr_indexer::newznab::candidate_base_urls(&base_url);
    let mut last_error = String::from("no candidates");

    for candidate_url in &candidates {
        let client = stackarr_indexer::newznab::NewznabClient::new(
            candidate_url,
            api_key_str,
            0,
            "test",
            proto,
        );

        let caps =
            match tokio::time::timeout(std::time::Duration::from_secs(15), client.caps()).await {
                Ok(Ok(caps)) => caps,
                Ok(Err(e)) => {
                    last_error = e.to_string();
                    continue;
                }
                Err(_) => {
                    last_error = "connection timed out".to_string();
                    continue;
                }
            };

        let search_result = tokio::time::timeout(
            std::time::Duration::from_secs(15),
            client.search("test", &[]),
        )
        .await;

        let cat_count = caps.categories.len();
        let url_changed = *candidate_url != base_url.trim_end_matches('/');
        let correction = if url_changed {
            format!(" (URL auto-corrected to {candidate_url})")
        } else {
            String::new()
        };

        return match search_result {
            Ok(Ok(releases)) => {
                let n = releases.len();
                Json(json!({
                    "success": true,
                    "message": format!("OK — {cat_count} categories, {n} sample results{correction}"),
                    "correctedUrl": if url_changed { Some(candidate_url.clone()) } else { None }
                })).into_response()
            }
            Ok(Err(e)) => Json(json!({
                "success": true,
                "message": format!("OK — {cat_count} categories (search: {e}){correction}"),
                "correctedUrl": if url_changed { Some(candidate_url.clone()) } else { None }
            }))
            .into_response(),
            Err(_) => Json(json!({
                "success": true,
                "message": format!("OK — {cat_count} categories (search timed out){correction}"),
                "correctedUrl": if url_changed { Some(candidate_url.clone()) } else { None }
            }))
            .into_response(),
        };
    }

    Json(json!({ "success": false, "message": last_error })).into_response()
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

    available.sort_by_key(|entry| entry.name.to_lowercase());
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

    // Respect the enabled flag from the database — add_indexer/add_cardigann_indexer
    // always sets enabled=true, so disable here if needed.
    if !row.enabled {
        mgr.set_enabled(row.id as i64, false);
    }
}

// ---------------------------------------------------------------------------
// Router
// ---------------------------------------------------------------------------

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/v1/indexer", get(list_indexers).post(create_indexer))
        .route("/api/v1/indexer/available", get(list_available_indexers))
        .route("/api/v1/indexer/available/{id}", get(get_available_indexer))
        .route("/api/v1/indexer/test", post(test_indexer_config))
        .route(
            "/api/v1/indexer/{id}",
            axum::routing::put(update_indexer).delete(delete_indexer),
        )
        .route("/api/v1/indexer/{id}/test", post(test_indexer))
}
