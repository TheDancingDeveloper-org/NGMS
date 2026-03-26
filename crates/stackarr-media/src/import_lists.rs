use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;

use stackarr_metadata::TmdbClient;

// ── DB row type ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct ImportList {
    pub id: i64,
    pub name: String,
    pub list_type: String,
    pub media_type: String,
    pub config: serde_json::Value,
    pub quality_profile_id: Option<i64>,
    pub root_folder_id: Option<i64>,
    pub monitored: bool,
    pub enabled: bool,
    pub poll_interval_secs: i32,
}

// ── Input types ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateImportListInput {
    pub name: String,
    pub list_type: String,
    pub media_type: String,
    pub config: serde_json::Value,
    pub quality_profile_id: Option<i64>,
    pub root_folder_id: Option<i64>,
    #[serde(default = "default_true")]
    pub monitored: bool,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_poll_interval")]
    pub poll_interval_secs: i32,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateImportListInput {
    pub name: Option<String>,
    pub list_type: Option<String>,
    pub media_type: Option<String>,
    pub config: Option<serde_json::Value>,
    pub quality_profile_id: Option<Option<i64>>,
    pub root_folder_id: Option<Option<i64>>,
    pub monitored: Option<bool>,
    pub enabled: Option<bool>,
    pub poll_interval_secs: Option<i32>,
}

fn default_true() -> bool {
    true
}

fn default_poll_interval() -> i32 {
    3600
}

// ── Sync result ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportListSyncResult {
    pub list_id: i64,
    pub list_name: String,
    pub items_found: usize,
    pub items_added: usize,
    pub items_existing: usize,
    pub errors: Vec<String>,
}

// ── Internal item from a list source ───────────────────────────────────────

struct FetchedItem {
    title: String,
    tmdb_id: i64,
    year: Option<i32>,
    overview: Option<String>,
}

// ── Service ────────────────────────────────────────────────────────────────

#[derive(Clone)]
pub struct ImportListService {
    pool: PgPool,
}

impl ImportListService {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn list(&self) -> Result<Vec<ImportList>> {
        let rows = sqlx::query_as::<_, ImportList>(
            "SELECT * FROM import_lists ORDER BY id",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    pub async fn get(&self, id: i64) -> Result<ImportList> {
        let row = sqlx::query_as::<_, ImportList>(
            "SELECT * FROM import_lists WHERE id = $1",
        )
        .bind(id)
        .fetch_one(&self.pool)
        .await?;
        Ok(row)
    }

    pub async fn create(&self, input: CreateImportListInput) -> Result<ImportList> {
        let row = sqlx::query_as::<_, ImportList>(
            "INSERT INTO import_lists (name, list_type, media_type, config, quality_profile_id, root_folder_id, monitored, enabled, poll_interval_secs)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
             RETURNING *",
        )
        .bind(&input.name)
        .bind(&input.list_type)
        .bind(&input.media_type)
        .bind(&input.config)
        .bind(input.quality_profile_id)
        .bind(input.root_folder_id)
        .bind(input.monitored)
        .bind(input.enabled)
        .bind(input.poll_interval_secs)
        .fetch_one(&self.pool)
        .await?;
        Ok(row)
    }

    pub async fn update(&self, id: i64, input: UpdateImportListInput) -> Result<ImportList> {
        let existing = self.get(id).await?;
        let name = input.name.unwrap_or(existing.name);
        let list_type = input.list_type.unwrap_or(existing.list_type);
        let media_type = input.media_type.unwrap_or(existing.media_type);
        let config = input.config.unwrap_or(existing.config);
        let quality_profile_id = input
            .quality_profile_id
            .unwrap_or(existing.quality_profile_id);
        let root_folder_id = input.root_folder_id.unwrap_or(existing.root_folder_id);
        let monitored = input.monitored.unwrap_or(existing.monitored);
        let enabled = input.enabled.unwrap_or(existing.enabled);
        let poll_interval_secs = input
            .poll_interval_secs
            .unwrap_or(existing.poll_interval_secs);

        let row = sqlx::query_as::<_, ImportList>(
            "UPDATE import_lists
             SET name = $1, list_type = $2, media_type = $3, config = $4,
                 quality_profile_id = $5, root_folder_id = $6, monitored = $7,
                 enabled = $8, poll_interval_secs = $9
             WHERE id = $10
             RETURNING *",
        )
        .bind(&name)
        .bind(&list_type)
        .bind(&media_type)
        .bind(&config)
        .bind(quality_profile_id)
        .bind(root_folder_id)
        .bind(monitored)
        .bind(enabled)
        .bind(poll_interval_secs)
        .bind(id)
        .fetch_one(&self.pool)
        .await?;
        Ok(row)
    }

    pub async fn delete(&self, id: i64) -> Result<()> {
        sqlx::query("DELETE FROM import_lists WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Sync a single list -- fetch items from source, add missing media to library.
    pub async fn sync(
        &self,
        id: i64,
        tmdb_client: &TmdbClient,
    ) -> Result<ImportListSyncResult> {
        let list = self.get(id).await?;
        let mut result = ImportListSyncResult {
            list_id: list.id,
            list_name: list.name.clone(),
            items_found: 0,
            items_added: 0,
            items_existing: 0,
            errors: Vec::new(),
        };

        // Fetch items from the external source
        let items = match fetch_list_items(&list, tmdb_client).await {
            Ok(items) => items,
            Err(e) => {
                result.errors.push(format!("Failed to fetch list: {e}"));
                return Ok(result);
            }
        };

        result.items_found = items.len();

        // Resolve root folder path for building media paths
        let root_folder_path = if let Some(rf_id) = list.root_folder_id {
            let row: Option<(String,)> =
                sqlx::query_as("SELECT path FROM root_folders WHERE id = $1")
                    .bind(rf_id)
                    .fetch_optional(&self.pool)
                    .await?;
            row.map(|(p,)| p)
        } else {
            None
        };

        let quality_profile_id = list.quality_profile_id.unwrap_or(1);

        for item in &items {
            match list.media_type.as_str() {
                "movie" => {
                    // Check if already exists by tmdb_id
                    let exists: Option<(i64,)> = sqlx::query_as(
                        "SELECT id FROM movies WHERE tmdb_id = $1",
                    )
                    .bind(item.tmdb_id)
                    .fetch_optional(&self.pool)
                    .await?;

                    if exists.is_some() {
                        result.items_existing += 1;
                        continue;
                    }

                    let clean = stackarr_parser::clean_title(&item.title);
                    let path = match &root_folder_path {
                        Some(rf) => format!("{rf}/{} ({year})", item.title, year = item.year.unwrap_or(0)),
                        None => item.title.clone(),
                    };

                    match sqlx::query(
                        "INSERT INTO movies (title, clean_title, sort_title, path, quality_profile_id, monitored, tmdb_id, year, overview)
                         VALUES ($1, $2, $2, $3, $4, $5, $6, $7, $8)",
                    )
                    .bind(&item.title)
                    .bind(&clean)
                    .bind(&path)
                    .bind(quality_profile_id)
                    .bind(list.monitored)
                    .bind(item.tmdb_id)
                    .bind(item.year)
                    .bind(&item.overview)
                    .execute(&self.pool)
                    .await
                    {
                        Ok(_) => result.items_added += 1,
                        Err(e) => result.errors.push(format!(
                            "Failed to insert movie '{}': {e}",
                            item.title
                        )),
                    }
                }
                "series" => {
                    // Check if already exists by tmdb_id
                    let exists: Option<(i64,)> = sqlx::query_as(
                        "SELECT id FROM series WHERE tmdb_id = $1",
                    )
                    .bind(item.tmdb_id)
                    .fetch_optional(&self.pool)
                    .await?;

                    if exists.is_some() {
                        result.items_existing += 1;
                        continue;
                    }

                    let clean = stackarr_parser::clean_title(&item.title);
                    let path = match &root_folder_path {
                        Some(rf) => format!("{rf}/{}", item.title),
                        None => item.title.clone(),
                    };

                    match sqlx::query(
                        "INSERT INTO series (title, clean_title, sort_title, path, quality_profile_id, monitored, tmdb_id, overview)
                         VALUES ($1, $2, $2, $3, $4, $5, $6, $7)",
                    )
                    .bind(&item.title)
                    .bind(&clean)
                    .bind(&path)
                    .bind(quality_profile_id)
                    .bind(list.monitored)
                    .bind(item.tmdb_id)
                    .bind(&item.overview)
                    .execute(&self.pool)
                    .await
                    {
                        Ok(_) => result.items_added += 1,
                        Err(e) => result.errors.push(format!(
                            "Failed to insert series '{}': {e}",
                            item.title
                        )),
                    }
                }
                other => {
                    result
                        .errors
                        .push(format!("Unsupported media_type: {other}"));
                    break;
                }
            }
        }

        Ok(result)
    }

    /// Sync all enabled lists.
    pub async fn sync_all(
        &self,
        tmdb_client: &TmdbClient,
    ) -> Result<Vec<ImportListSyncResult>> {
        let lists = sqlx::query_as::<_, ImportList>(
            "SELECT * FROM import_lists WHERE enabled = true ORDER BY id",
        )
        .fetch_all(&self.pool)
        .await?;

        let mut results = Vec::with_capacity(lists.len());
        for list in &lists {
            match self.sync(list.id, tmdb_client).await {
                Ok(r) => results.push(r),
                Err(e) => results.push(ImportListSyncResult {
                    list_id: list.id,
                    list_name: list.name.clone(),
                    items_found: 0,
                    items_added: 0,
                    items_existing: 0,
                    errors: vec![format!("Sync failed: {e}")],
                }),
            }
        }
        Ok(results)
    }
}

// ── Fetch items from an external source ────────────────────────────────────

async fn fetch_list_items(
    list: &ImportList,
    tmdb_client: &TmdbClient,
) -> Result<Vec<FetchedItem>> {
    match list.list_type.as_str() {
        "tmdb_popular" => fetch_tmdb_popular(list, tmdb_client).await,
        "tmdb_trending" => fetch_tmdb_trending(list, tmdb_client).await,
        "trakt_watchlist" => fetch_trakt_watchlist(list).await,
        "imdb_list" => fetch_imdb_list(list).await,
        other => anyhow::bail!("Unknown list type: {other}"),
    }
}

// ── TMDB Popular ───────────────────────────────────────────────────────────

/// Response shape for TMDB paginated results (movies and TV).
#[derive(Debug, Deserialize)]
struct TmdbPagedResponse {
    results: Vec<TmdbListItem>,
}

#[derive(Debug, Deserialize)]
struct TmdbListItem {
    id: i64,
    // Movie uses "title", TV uses "name"
    #[serde(alias = "name")]
    title: Option<String>,
    overview: Option<String>,
    // Movie uses "release_date", TV uses "first_air_date"
    release_date: Option<String>,
    first_air_date: Option<String>,
    #[serde(default)]
    vote_average: f64,
    #[serde(default)]
    vote_count: i64,
}

async fn fetch_tmdb_popular(
    list: &ImportList,
    tmdb_client: &TmdbClient,
) -> Result<Vec<FetchedItem>> {
    let min_vote_avg = list
        .config
        .get("min_vote_average")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);
    let min_vote_count = list
        .config
        .get("min_vote_count")
        .and_then(|v| v.as_i64())
        .unwrap_or(0);

    let endpoint = match list.media_type.as_str() {
        "movie" => "movie/popular",
        "series" => "tv/popular",
        _ => anyhow::bail!("Unsupported media_type for tmdb_popular: {}", list.media_type),
    };

    // Fetch first page
    let url = format!(
        "https://api.themoviedb.org/3/{endpoint}?api_key={key}&page=1",
        key = tmdb_api_key(tmdb_client)?,
    );
    let resp: TmdbPagedResponse = reqwest::get(&url)
        .await
        .context("TMDB popular request failed")?
        .error_for_status()
        .context("TMDB popular returned error")?
        .json()
        .await
        .context("Failed to parse TMDB popular response")?;

    let items = resp
        .results
        .into_iter()
        .filter(|r| r.vote_average >= min_vote_avg && r.vote_count >= min_vote_count)
        .map(|r| tmdb_item_to_fetched(&r, &list.media_type))
        .collect();

    Ok(items)
}

// ── TMDB Trending ──────────────────────────────────────────────────────────

async fn fetch_tmdb_trending(
    list: &ImportList,
    tmdb_client: &TmdbClient,
) -> Result<Vec<FetchedItem>> {
    let time_window = list
        .config
        .get("time_window")
        .and_then(|v| v.as_str())
        .unwrap_or("week");

    let media = match list.media_type.as_str() {
        "movie" => "movie",
        "series" => "tv",
        _ => anyhow::bail!("Unsupported media_type for tmdb_trending: {}", list.media_type),
    };

    let url = format!(
        "https://api.themoviedb.org/3/trending/{media}/{time_window}?api_key={key}",
        key = tmdb_api_key(tmdb_client)?,
    );
    let resp: TmdbPagedResponse = reqwest::get(&url)
        .await
        .context("TMDB trending request failed")?
        .error_for_status()
        .context("TMDB trending returned error")?
        .json()
        .await
        .context("Failed to parse TMDB trending response")?;

    let items = resp
        .results
        .into_iter()
        .map(|r| tmdb_item_to_fetched(&r, &list.media_type))
        .collect();

    Ok(items)
}

// ── Trakt Watchlist (stub) ─────────────────────────────────────────────────

async fn fetch_trakt_watchlist(_list: &ImportList) -> Result<Vec<FetchedItem>> {
    tracing::info!("Trakt watchlist import is a stub -- returning empty results");
    Ok(Vec::new())
}

// ── IMDB List (stub) ───────────────────────────────────────────────────────

async fn fetch_imdb_list(_list: &ImportList) -> Result<Vec<FetchedItem>> {
    tracing::info!("IMDB list import is a stub -- returning empty results");
    Ok(Vec::new())
}

// ── Helpers ────────────────────────────────────────────────────────────────

/// Extract the API key from the TmdbClient by reading the env var
/// (the client itself doesn't expose the key, so we read the same env).
fn tmdb_api_key(_client: &TmdbClient) -> Result<String> {
    std::env::var("STACKARR_TMDB_API_KEY")
        .context("STACKARR_TMDB_API_KEY not set -- cannot fetch import lists from TMDB")
}

fn tmdb_item_to_fetched(item: &TmdbListItem, media_type: &str) -> FetchedItem {
    let title = item.title.clone().unwrap_or_default();
    let date_str = match media_type {
        "series" => item.first_air_date.as_deref(),
        _ => item.release_date.as_deref(),
    };
    let year = date_str
        .and_then(|d| d.get(..4))
        .and_then(|y| y.parse::<i32>().ok());

    FetchedItem {
        title,
        tmdb_id: item.id,
        year,
        overview: item.overview.clone(),
    }
}
