use std::sync::Arc;

use anyhow::{Context, Result};
use sqlx::PgPool;
use tokio::sync::RwLock;

use stackarr_core::models::{DownloadProtocol, RssFeed, RssItem, RssRule};
use stackarr_download::{DownloadClient, DownloadClientManager};

/// Statistics returned after checking a single feed.
pub struct CheckStats {
    pub new_items: usize,
    pub downloaded: usize,
}

/// Run one RSS sync cycle: check all enabled feeds.
pub async fn rss_sync(
    pool: &PgPool,
    download_manager: &Arc<RwLock<DownloadClientManager>>,
) -> Result<()> {
    let feeds: Vec<RssFeed> = sqlx::query_as(
        "SELECT id, name, url, protocol, poll_interval_secs, category, filter_regex,
                enabled, auto_download, created_at, updated_at
         FROM rss_feeds WHERE enabled = true",
    )
    .fetch_all(pool)
    .await?;

    if feeds.is_empty() {
        tracing::debug!("RSS sync: no enabled feeds");
        return Ok(());
    }

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()?;

    for feed in &feeds {
        match check_single_feed_inner(&client, pool, feed, download_manager).await {
            Ok(stats) => {
                if stats.new_items > 0 || stats.downloaded > 0 {
                    tracing::info!(
                        feed = %feed.name,
                        new_items = stats.new_items,
                        downloaded = stats.downloaded,
                        "RSS feed checked"
                    );
                }
            }
            Err(e) => {
                tracing::error!(feed = %feed.name, error = %e, "failed to check RSS feed");
            }
        }
    }

    // Prune old items (keep last 5000 per feed)
    let _ = sqlx::query(
        "DELETE FROM rss_items WHERE id IN (
            SELECT ri.id FROM rss_items ri
            JOIN (
                SELECT feed_id, id,
                       ROW_NUMBER() OVER (PARTITION BY feed_id ORDER BY first_seen_at DESC) as rn
                FROM rss_items
            ) ranked ON ri.id = ranked.id
            WHERE ranked.rn > 5000
        )",
    )
    .execute(pool)
    .await;

    Ok(())
}

/// Check a single feed — used by both the scheduler and the manual check endpoint.
pub async fn check_single_feed(
    pool: &PgPool,
    feed: &RssFeed,
    download_manager: &Arc<RwLock<DownloadClientManager>>,
) -> Result<CheckStats> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()?;
    check_single_feed_inner(&client, pool, feed, download_manager).await
}

async fn check_single_feed_inner(
    client: &reqwest::Client,
    pool: &PgPool,
    feed: &RssFeed,
    download_manager: &Arc<RwLock<DownloadClientManager>>,
) -> Result<CheckStats> {
    // 1. Fetch and parse
    let response = client
        .get(&feed.url)
        .send()
        .await
        .with_context(|| format!("failed to fetch feed URL: {}", feed.url))?;

    let body = response.bytes().await?;
    let parsed = feed_rs::parser::parse(&body[..])
        .map_err(|e| anyhow::anyhow!("failed to parse feed: {e}"))?;

    // 2. Compile optional feed-level filter
    let filter_re = match &feed.filter_regex {
        Some(re) if !re.is_empty() => Some(
            regex::Regex::new(re).with_context(|| format!("invalid feed filter regex: {re}"))?,
        ),
        _ => None,
    };

    // 3. Collect items
    let mut pending_items: Vec<RssItem> = Vec::new();
    for entry in &parsed.entries {
        let entry_id = entry.id.clone();
        let title = entry
            .title
            .as_ref()
            .map(|t| t.content.clone())
            .unwrap_or_default();

        if title.is_empty() {
            continue;
        }

        let url = extract_download_url(entry, feed.protocol);
        let published_at = entry.published.or(entry.updated);
        let size_bytes = extract_size(entry);

        pending_items.push(RssItem {
            id: entry_id,
            feed_id: feed.id,
            title,
            url,
            published_at,
            first_seen_at: chrono::Utc::now(),
            downloaded: false,
            downloaded_at: None,
            category: feed.category.clone(),
            size_bytes: Some(size_bytes),
        });
    }

    // 4. Batch insert (ON CONFLICT DO NOTHING for dedup)
    let mut new_ids: Vec<String> = Vec::new();
    for item in &pending_items {
        let result = sqlx::query_scalar::<_, String>(
            "INSERT INTO rss_items (id, feed_id, title, url, published_at, first_seen_at, category, size_bytes)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
             ON CONFLICT (id) DO NOTHING
             RETURNING id",
        )
        .bind(&item.id)
        .bind(item.feed_id)
        .bind(&item.title)
        .bind(&item.url)
        .bind(item.published_at)
        .bind(item.first_seen_at)
        .bind(&item.category)
        .bind(item.size_bytes)
        .fetch_optional(pool)
        .await?;

        if let Some(id) = result {
            new_ids.push(id);
        }
    }

    let new_count = new_ids.len();

    // 5. Auto-download new items
    let mut downloaded_count = 0usize;

    if !new_ids.is_empty() {
        // Load rules that apply to this feed
        let rules: Vec<RssRule> = sqlx::query_as(
            "SELECT id, name, feed_ids, category, priority, match_regex, enabled, created_at
             FROM rss_rules WHERE enabled = true AND $1 = ANY(feed_ids)",
        )
        .bind(feed.id)
        .fetch_all(pool)
        .await?;

        // Build regex cache for rules
        let compiled_rules: Vec<(&RssRule, regex::Regex)> = rules
            .iter()
            .filter_map(|r| regex::Regex::new(&r.match_regex).ok().map(|re| (r, re)))
            .collect();

        for item in pending_items.iter().filter(|i| new_ids.contains(&i.id)) {
            // Apply feed-level filter
            if let Some(ref re) = filter_re
                && !re.is_match(&item.title)
            {
                continue;
            }

            // Check rules
            let matched_rule = compiled_rules
                .iter()
                .find(|(_, re)| re.is_match(&item.title));

            let should_download =
                matched_rule.is_some() || (feed.auto_download && filter_re.is_none());

            if !should_download {
                continue;
            }

            let download_url = match &item.url {
                Some(u) if !u.is_empty() => u.clone(),
                _ => continue,
            };

            let (category, _priority) = if let Some((rule, _)) = matched_rule {
                (
                    rule.category.clone().or_else(|| feed.category.clone()),
                    rule.priority,
                )
            } else {
                (feed.category.clone(), 1)
            };

            let protocol = match feed.protocol {
                DownloadProtocol::Torrent => stackarr_download::DownloadProtocol::Torrent,
                DownloadProtocol::Usenet => stackarr_download::DownloadProtocol::Usenet,
            };

            let grab_request = stackarr_download::GrabRequest {
                title: item.title.clone(),
                download_url,
                category: category.clone(),
                protocol,
                password: None, // RSS items don't carry passwords
                torrent_bytes: None,
            };

            // Extract candidates from behind the lock, then drop it before network I/O
            let candidates = {
                let dm = download_manager.read().await;
                dm.grab_candidates(protocol)
            };
            match grab_with_candidates(&candidates, &grab_request).await {
                Ok((client_id, download_id)) => {
                    tracing::info!(
                        feed = %feed.name,
                        item = %item.title,
                        client_id,
                        download_id,
                        "RSS auto-download succeeded"
                    );

                    let _ = sqlx::query(
                        "UPDATE rss_items SET downloaded = true, downloaded_at = NOW(), category = COALESCE($1, category) WHERE id = $2",
                    )
                    .bind(&category)
                    .bind(&item.id)
                    .execute(pool)
                    .await;

                    downloaded_count += 1;
                }
                Err(e) => {
                    tracing::warn!(
                        feed = %feed.name,
                        item = %item.title,
                        error = %e,
                        "RSS auto-download failed"
                    );
                }
            }
        }
    }

    Ok(CheckStats {
        new_items: new_count,
        downloaded: downloaded_count,
    })
}

/// Extract a download URL from a feed entry, choosing based on protocol.
fn extract_download_url(
    entry: &feed_rs::model::Entry,
    protocol: DownloadProtocol,
) -> Option<String> {
    // Check links for protocol-specific URLs
    for link in &entry.links {
        let href = &link.href;
        match protocol {
            DownloadProtocol::Usenet => {
                if href.ends_with(".nzb") || link.media_type.as_deref() == Some("application/x-nzb")
                {
                    return Some(href.clone());
                }
            }
            DownloadProtocol::Torrent => {
                if href.ends_with(".torrent")
                    || href.starts_with("magnet:")
                    || link.media_type.as_deref() == Some("application/x-bittorrent")
                {
                    return Some(href.clone());
                }
            }
        }
    }

    // Check media content
    for content in &entry.media {
        for media_content in &content.content {
            if let Some(ref url) = media_content.url {
                let href = url.as_str();
                match protocol {
                    DownloadProtocol::Usenet => {
                        if href.ends_with(".nzb") {
                            return Some(href.to_string());
                        }
                    }
                    DownloadProtocol::Torrent => {
                        if href.ends_with(".torrent") || href.starts_with("magnet:") {
                            return Some(href.to_string());
                        }
                    }
                }
            }
        }
    }

    // Fallback: first link if available
    entry.links.first().map(|l| l.href.clone())
}

/// Extract size from feed entry media content.
fn extract_size(entry: &feed_rs::model::Entry) -> i64 {
    for content in &entry.media {
        for media_content in &content.content {
            if let Some(size) = media_content.size {
                return size as i64;
            }
        }
    }
    // Check enclosure-style links
    for link in &entry.links {
        if let Some(len) = link.length {
            return len as i64;
        }
    }
    0
}

/// Try to grab a download using pre-extracted candidates (outside the lock).
async fn grab_with_candidates(
    candidates: &[(i64, Arc<dyn DownloadClient>)],
    request: &stackarr_download::GrabRequest,
) -> Result<(i64, String)> {
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
