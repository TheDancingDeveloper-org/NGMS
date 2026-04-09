use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::json;

use stackarr_core::models::{
    CreateRssFeed, CreateRssRule, RssFeed, RssItem, RssRule, UpdateRssFeed, UpdateRssRule,
};
use stackarr_download::DownloadClient;

use crate::AppState;

// ── Feed handlers ─────────���────────────────────────────────────────────────

async fn list_feeds(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let pool = state.db.pool();

    match sqlx::query_as::<_, RssFeed>(
        "SELECT id, name, url, protocol, poll_interval_secs, category, filter_regex,
                enabled, auto_download, created_at, updated_at
         FROM rss_feeds ORDER BY name",
    )
    .fetch_all(pool)
    .await
    {
        Ok(feeds) => Json(serde_json::to_value(&feeds).unwrap_or_default()).into_response(),
        Err(e) => {
            tracing::error!(error = %e, "failed to list RSS feeds");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "internal server error"})),
            )
                .into_response()
        }
    }
}

async fn create_feed(
    State(state): State<Arc<AppState>>,
    Json(body): Json<CreateRssFeed>,
) -> impl IntoResponse {
    let pool = state.db.pool();

    if body.name.trim().is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "name cannot be empty"})),
        )
            .into_response();
    }
    if body.url.trim().is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "url cannot be empty"})),
        )
            .into_response();
    }

    let enabled = body.enabled.unwrap_or(true);
    let auto_download = body.auto_download.unwrap_or(false);
    let poll_interval = body.poll_interval_secs.unwrap_or(900);

    match sqlx::query_as::<_, RssFeed>(
        "INSERT INTO rss_feeds (name, url, protocol, poll_interval_secs, category, filter_regex, enabled, auto_download)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
         RETURNING id, name, url, protocol, poll_interval_secs, category, filter_regex,
                   enabled, auto_download, created_at, updated_at",
    )
    .bind(body.name.trim())
    .bind(body.url.trim())
    .bind(body.protocol)
    .bind(poll_interval)
    .bind(&body.category)
    .bind(&body.filter_regex)
    .bind(enabled)
    .bind(auto_download)
    .fetch_one(pool)
    .await
    {
        Ok(feed) => (StatusCode::CREATED, Json(serde_json::to_value(&feed).unwrap_or_default())).into_response(),
        Err(e) => {
            tracing::error!(error = %e, "failed to create RSS feed");
            if e.to_string().contains("duplicate key") {
                (StatusCode::CONFLICT, Json(json!({"error": "a feed with that name already exists"}))).into_response()
            } else {
                (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": "internal server error"}))).into_response()
            }
        }
    }
}

async fn update_feed(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    Json(body): Json<UpdateRssFeed>,
) -> impl IntoResponse {
    let pool = state.db.pool();

    match sqlx::query_as::<_, RssFeed>(
        "UPDATE rss_feeds SET
            name = COALESCE($1, name),
            url = COALESCE($2, url),
            protocol = COALESCE($3, protocol),
            poll_interval_secs = COALESCE($4, poll_interval_secs),
            category = COALESCE($5, category),
            filter_regex = COALESCE($6, filter_regex),
            enabled = COALESCE($7, enabled),
            auto_download = COALESCE($8, auto_download),
            updated_at = NOW()
         WHERE id = $9
         RETURNING id, name, url, protocol, poll_interval_secs, category, filter_regex,
                   enabled, auto_download, created_at, updated_at",
    )
    .bind(body.name.as_deref().map(str::trim))
    .bind(body.url.as_deref().map(str::trim))
    .bind(body.protocol)
    .bind(body.poll_interval_secs)
    .bind(&body.category)
    .bind(&body.filter_regex)
    .bind(body.enabled)
    .bind(body.auto_download)
    .bind(id)
    .fetch_optional(pool)
    .await
    {
        Ok(Some(feed)) => Json(serde_json::to_value(&feed).unwrap_or_default()).into_response(),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "feed not found"})),
        )
            .into_response(),
        Err(e) => {
            tracing::error!(error = %e, "failed to update RSS feed");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "internal server error"})),
            )
                .into_response()
        }
    }
}

async fn delete_feed(State(state): State<Arc<AppState>>, Path(id): Path<i64>) -> impl IntoResponse {
    let pool = state.db.pool();

    match sqlx::query("DELETE FROM rss_feeds WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await
    {
        Ok(result) => {
            if result.rows_affected() == 0 {
                (
                    StatusCode::NOT_FOUND,
                    Json(json!({"error": "feed not found"})),
                )
                    .into_response()
            } else {
                StatusCode::NO_CONTENT.into_response()
            }
        }
        Err(e) => {
            tracing::error!(error = %e, "failed to delete RSS feed");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "internal server error"})),
            )
                .into_response()
        }
    }
}

async fn check_feed(State(state): State<Arc<AppState>>, Path(id): Path<i64>) -> impl IntoResponse {
    let pool = state.db.pool();

    // Load the feed
    let feed = match sqlx::query_as::<_, RssFeed>(
        "SELECT id, name, url, protocol, poll_interval_secs, category, filter_regex,
                enabled, auto_download, created_at, updated_at
         FROM rss_feeds WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(pool)
    .await
    {
        Ok(Some(f)) => f,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({"error": "feed not found"})),
            )
                .into_response();
        }
        Err(e) => {
            tracing::error!(error = %e, "failed to load RSS feed");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "internal server error"})),
            )
                .into_response();
        }
    };

    // Run the feed check
    let dm = state.download_manager.clone();
    match stackarr_scheduler::rss::check_single_feed(pool, &feed, &dm).await {
        Ok(stats) => Json(json!({
            "newItems": stats.new_items,
            "downloaded": stats.downloaded,
        }))
        .into_response(),
        Err(e) => {
            tracing::error!(error = %e, feed_id = id, "manual RSS feed check failed");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("feed check failed: {e}")})),
            )
                .into_response()
        }
    }
}

// ── Item handlers ���──────────────────────────────────────���──────────────────

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ItemQuery {
    feed_id: Option<i64>,
    limit: Option<i64>,
}

async fn list_items(
    State(state): State<Arc<AppState>>,
    Query(params): Query<ItemQuery>,
) -> impl IntoResponse {
    let pool = state.db.pool();
    let limit = params.limit.unwrap_or(500).min(5000);

    let items = if let Some(feed_id) = params.feed_id {
        sqlx::query_as::<_, RssItem>(
            "SELECT id, feed_id, title, url, published_at, first_seen_at,
                    downloaded, downloaded_at, category, size_bytes
             FROM rss_items WHERE feed_id = $1
             ORDER BY first_seen_at DESC LIMIT $2",
        )
        .bind(feed_id)
        .bind(limit)
        .fetch_all(pool)
        .await
    } else {
        sqlx::query_as::<_, RssItem>(
            "SELECT id, feed_id, title, url, published_at, first_seen_at,
                    downloaded, downloaded_at, category, size_bytes
             FROM rss_items ORDER BY first_seen_at DESC LIMIT $1",
        )
        .bind(limit)
        .fetch_all(pool)
        .await
    };

    match items {
        Ok(items) => Json(serde_json::to_value(&items).unwrap_or_default()).into_response(),
        Err(e) => {
            tracing::error!(error = %e, "failed to list RSS items");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "internal server error"})),
            )
                .into_response()
        }
    }
}

async fn download_item(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let pool = state.db.pool();

    // Load the item
    let item = match sqlx::query_as::<_, RssItem>(
        "SELECT id, feed_id, title, url, published_at, first_seen_at,
                downloaded, downloaded_at, category, size_bytes
         FROM rss_items WHERE id = $1",
    )
    .bind(&id)
    .fetch_optional(pool)
    .await
    {
        Ok(Some(item)) => item,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({"error": "item not found"})),
            )
                .into_response();
        }
        Err(e) => {
            tracing::error!(error = %e, "failed to load RSS item");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "internal server error"})),
            )
                .into_response();
        }
    };

    let download_url = match &item.url {
        Some(u) if !u.is_empty() => u.clone(),
        _ => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({"error": "item has no download URL"})),
            )
                .into_response();
        }
    };

    // Get the feed's protocol
    let feed = match sqlx::query_as::<_, RssFeed>(
        "SELECT id, name, url, protocol, poll_interval_secs, category, filter_regex,
                enabled, auto_download, created_at, updated_at
         FROM rss_feeds WHERE id = $1",
    )
    .bind(item.feed_id)
    .fetch_optional(pool)
    .await
    {
        Ok(Some(f)) => f,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({"error": "parent feed not found"})),
            )
                .into_response();
        }
        Err(e) => {
            tracing::error!(error = %e, "failed to load parent feed");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "internal server error"})),
            )
                .into_response();
        }
    };

    // Convert to download protocol
    let protocol = match feed.protocol {
        stackarr_core::models::DownloadProtocol::Torrent => {
            stackarr_download::DownloadProtocol::Torrent
        }
        stackarr_core::models::DownloadProtocol::Usenet => {
            stackarr_download::DownloadProtocol::Usenet
        }
    };

    let category = item.category.clone().or_else(|| feed.category.clone());

    let grab_request = stackarr_download::GrabRequest {
        title: item.title.clone(),
        download_url,
        category: category.clone(),
        protocol,
        password: None,
    };

    // Extract candidates from behind the lock, then drop it before network I/O
    let candidates = {
        let dm = state.download_manager.read().await;
        dm.grab_candidates(protocol)
    };
    match grab_with_candidates(&candidates, &grab_request).await {
        Ok((client_id, download_id)) => {
            tracing::info!(
                item_id = %id,
                client_id,
                download_id,
                "RSS item grabbed successfully"
            );

            // Mark as downloaded
            let _ = sqlx::query(
                "UPDATE rss_items SET downloaded = true, downloaded_at = NOW(), category = COALESCE($1, category) WHERE id = $2",
            )
            .bind(&category)
            .bind(&id)
            .execute(pool)
            .await;

            Json(json!({
                "success": true,
                "clientId": client_id,
                "downloadId": download_id,
            }))
            .into_response()
        }
        Err(e) => {
            tracing::error!(error = %e, item_id = %id, "failed to grab RSS item");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("grab failed: {e}")})),
            )
                .into_response()
        }
    }
}

// ── Rule handlers ──────────────────────────────────────────────────────────

async fn list_rules(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let pool = state.db.pool();

    match sqlx::query_as::<_, RssRule>(
        "SELECT id, name, feed_ids, category, priority, match_regex, enabled, created_at
         FROM rss_rules ORDER BY name",
    )
    .fetch_all(pool)
    .await
    {
        Ok(rules) => Json(serde_json::to_value(&rules).unwrap_or_default()).into_response(),
        Err(e) => {
            tracing::error!(error = %e, "failed to list RSS rules");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "internal server error"})),
            )
                .into_response()
        }
    }
}

async fn create_rule(
    State(state): State<Arc<AppState>>,
    Json(body): Json<CreateRssRule>,
) -> impl IntoResponse {
    let pool = state.db.pool();

    if body.name.trim().is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "name cannot be empty"})),
        )
            .into_response();
    }
    if body.match_regex.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "match_regex cannot be empty"})),
        )
            .into_response();
    }

    // Validate regex
    if let Err(e) = regex::Regex::new(&body.match_regex) {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": format!("invalid regex: {e}")})),
        )
            .into_response();
    }

    let priority = body.priority.unwrap_or(1);
    let enabled = body.enabled.unwrap_or(true);

    match sqlx::query_as::<_, RssRule>(
        "INSERT INTO rss_rules (name, feed_ids, category, priority, match_regex, enabled)
         VALUES ($1, $2, $3, $4, $5, $6)
         RETURNING id, name, feed_ids, category, priority, match_regex, enabled, created_at",
    )
    .bind(body.name.trim())
    .bind(&body.feed_ids)
    .bind(&body.category)
    .bind(priority)
    .bind(&body.match_regex)
    .bind(enabled)
    .fetch_one(pool)
    .await
    {
        Ok(rule) => (
            StatusCode::CREATED,
            Json(serde_json::to_value(&rule).unwrap_or_default()),
        )
            .into_response(),
        Err(e) => {
            tracing::error!(error = %e, "failed to create RSS rule");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "internal server error"})),
            )
                .into_response()
        }
    }
}

async fn update_rule(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    Json(body): Json<UpdateRssRule>,
) -> impl IntoResponse {
    let pool = state.db.pool();

    // Validate regex if provided
    if let Some(ref re) = body.match_regex
        && let Err(e) = regex::Regex::new(re)
    {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": format!("invalid regex: {e}")})),
        )
            .into_response();
    }

    match sqlx::query_as::<_, RssRule>(
        "UPDATE rss_rules SET
            name = COALESCE($1, name),
            feed_ids = COALESCE($2, feed_ids),
            category = COALESCE($3, category),
            priority = COALESCE($4, priority),
            match_regex = COALESCE($5, match_regex),
            enabled = COALESCE($6, enabled)
         WHERE id = $7
         RETURNING id, name, feed_ids, category, priority, match_regex, enabled, created_at",
    )
    .bind(body.name.as_deref().map(str::trim))
    .bind(&body.feed_ids)
    .bind(&body.category)
    .bind(body.priority)
    .bind(&body.match_regex)
    .bind(body.enabled)
    .bind(id)
    .fetch_optional(pool)
    .await
    {
        Ok(Some(rule)) => Json(serde_json::to_value(&rule).unwrap_or_default()).into_response(),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "rule not found"})),
        )
            .into_response(),
        Err(e) => {
            tracing::error!(error = %e, "failed to update RSS rule");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "internal server error"})),
            )
                .into_response()
        }
    }
}

async fn delete_rule(State(state): State<Arc<AppState>>, Path(id): Path<i64>) -> impl IntoResponse {
    let pool = state.db.pool();

    match sqlx::query("DELETE FROM rss_rules WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await
    {
        Ok(result) => {
            if result.rows_affected() == 0 {
                (
                    StatusCode::NOT_FOUND,
                    Json(json!({"error": "rule not found"})),
                )
                    .into_response()
            } else {
                StatusCode::NO_CONTENT.into_response()
            }
        }
        Err(e) => {
            tracing::error!(error = %e, "failed to delete RSS rule");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "internal server error"})),
            )
                .into_response()
        }
    }
}

/// Try to grab a download using pre-extracted candidates (outside the lock).
async fn grab_with_candidates(
    candidates: &[(i64, Arc<dyn DownloadClient>)],
    request: &stackarr_download::GrabRequest,
) -> anyhow::Result<(i64, String)> {
    for (id, client) in candidates {
        match client.add(request).await {
            Ok(download_id) => {
                tracing::info!(
                    client = client.name(),
                    title = %request.title,
                    download_id = %download_id,
                    "download grabbed successfully"
                );
                return Ok((*id, download_id));
            }
            Err(e) => {
                tracing::warn!(client = client.name(), error = %e, "download client failed, trying next");
            }
        }
    }
    anyhow::bail!("no {} download client available", request.protocol);
}

// ── Router ──────────────���─────────────────────────��────────────────────────

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/v1/rss/feed", get(list_feeds).post(create_feed))
        .route(
            "/api/v1/rss/feed/{id}",
            axum::routing::put(update_feed).delete(delete_feed),
        )
        .route("/api/v1/rss/feed/{id}/check", post(check_feed))
        .route("/api/v1/rss/item", get(list_items))
        .route("/api/v1/rss/item/{id}/download", post(download_item))
        .route("/api/v1/rss/rule", get(list_rules).post(create_rule))
        .route(
            "/api/v1/rss/rule/{id}",
            axum::routing::put(update_rule).delete(delete_rule),
        )
}
