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
    pub media_library_folder_id: Option<i64>,
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
    pub media_library_folder_id: Option<i64>,
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
    pub media_library_folder_id: Option<Option<i64>>,
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
        let rows = sqlx::query_as::<_, ImportList>("SELECT * FROM import_lists ORDER BY id")
            .fetch_all(&self.pool)
            .await?;
        Ok(rows)
    }

    pub async fn get(&self, id: i64) -> Result<ImportList> {
        let row = sqlx::query_as::<_, ImportList>("SELECT * FROM import_lists WHERE id = $1")
            .bind(id)
            .fetch_one(&self.pool)
            .await?;
        Ok(row)
    }

    pub async fn create(&self, input: CreateImportListInput) -> Result<ImportList> {
        let row = sqlx::query_as::<_, ImportList>(
            "INSERT INTO import_lists (name, list_type, media_type, config, quality_profile_id, media_library_folder_id, monitored, enabled, poll_interval_secs)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
             RETURNING *",
        )
        .bind(&input.name)
        .bind(&input.list_type)
        .bind(&input.media_type)
        .bind(&input.config)
        .bind(input.quality_profile_id)
        .bind(input.media_library_folder_id)
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
        let media_library_folder_id = input
            .media_library_folder_id
            .unwrap_or(existing.media_library_folder_id);
        let monitored = input.monitored.unwrap_or(existing.monitored);
        let enabled = input.enabled.unwrap_or(existing.enabled);
        let poll_interval_secs = input
            .poll_interval_secs
            .unwrap_or(existing.poll_interval_secs);

        let row = sqlx::query_as::<_, ImportList>(
            "UPDATE import_lists
             SET name = $1, list_type = $2, media_type = $3, config = $4,
                 quality_profile_id = $5, media_library_folder_id = $6, monitored = $7,
                 enabled = $8, poll_interval_secs = $9
             WHERE id = $10
             RETURNING *",
        )
        .bind(&name)
        .bind(&list_type)
        .bind(&media_type)
        .bind(&config)
        .bind(quality_profile_id)
        .bind(media_library_folder_id)
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
    pub async fn sync(&self, id: i64, tmdb_client: &TmdbClient) -> Result<ImportListSyncResult> {
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

        // Resolve media library folder path for building media paths
        let media_library_folder_path = if let Some(rf_id) = list.media_library_folder_id {
            let row: Option<(String,)> =
                sqlx::query_as("SELECT path FROM media_library_folders WHERE id = $1")
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
                    let exists: Option<(i64,)> =
                        sqlx::query_as("SELECT id FROM movies WHERE tmdb_id = $1")
                            .bind(item.tmdb_id)
                            .fetch_optional(&self.pool)
                            .await?;

                    if exists.is_some() {
                        result.items_existing += 1;
                        continue;
                    }

                    let clean = stackarr_parser::clean_title(&item.title);
                    let path = match &media_library_folder_path {
                        Some(rf) => format!(
                            "{rf}/{} ({year})",
                            item.title,
                            year = item.year.unwrap_or(0)
                        ),
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
                    let exists: Option<(i64,)> =
                        sqlx::query_as("SELECT id FROM series WHERE tmdb_id = $1")
                            .bind(item.tmdb_id)
                            .fetch_optional(&self.pool)
                            .await?;

                    if exists.is_some() {
                        result.items_existing += 1;
                        continue;
                    }

                    let clean = stackarr_parser::clean_title(&item.title);
                    let path = match &media_library_folder_path {
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
    pub async fn sync_all(&self, tmdb_client: &TmdbClient) -> Result<Vec<ImportListSyncResult>> {
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

async fn fetch_list_items(list: &ImportList, tmdb_client: &TmdbClient) -> Result<Vec<FetchedItem>> {
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
        _ => anyhow::bail!(
            "Unsupported media_type for tmdb_popular: {}",
            list.media_type
        ),
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
        _ => anyhow::bail!(
            "Unsupported media_type for tmdb_trending: {}",
            list.media_type
        ),
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

// ── Trakt Watchlist ────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct TraktWatchlistItem {
    movie: Option<TraktMedia>,
    show: Option<TraktMedia>,
}

#[derive(Debug, Deserialize)]
struct TraktMedia {
    title: Option<String>,
    year: Option<i32>,
    ids: Option<TraktIds>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct TraktIds {
    trakt: Option<i64>,
    tmdb: Option<i64>,
    imdb: Option<String>,
}

async fn fetch_trakt_watchlist(list: &ImportList) -> Result<Vec<FetchedItem>> {
    let client_id = list
        .config
        .get("client_id")
        .and_then(|v| v.as_str())
        .context("Trakt import list requires 'client_id' in config")?;

    let access_token = list
        .config
        .get("access_token")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    let username = list
        .config
        .get("username")
        .and_then(|v| v.as_str())
        .context("Trakt import list requires 'username' in config")?;

    let url = format!("https://api.trakt.tv/users/{username}/watchlist");

    let http = reqwest::Client::new();
    let resp = http
        .get(&url)
        .header("Content-Type", "application/json")
        .header("trakt-api-version", "2")
        .header("trakt-api-key", client_id)
        .header("Authorization", format!("Bearer {access_token}"))
        .send()
        .await
        .context("Trakt API request failed")?
        .error_for_status()
        .context("Trakt API returned error")?;

    let items: Vec<TraktWatchlistItem> = resp
        .json()
        .await
        .context("failed to parse Trakt watchlist response")?;

    let is_movie = list.media_type == "movie";
    let mut results = Vec::new();

    for item in items {
        let media = if is_movie {
            item.movie.as_ref()
        } else {
            item.show.as_ref()
        };
        let Some(media) = media else { continue };
        let Some(ids) = &media.ids else { continue };
        let Some(tmdb_id) = ids.tmdb else { continue };

        results.push(FetchedItem {
            title: media.title.clone().unwrap_or_default(),
            tmdb_id,
            year: media.year,
            overview: None,
        });
    }

    tracing::info!(
        list_name = %list.name,
        items = results.len(),
        "Trakt watchlist fetched"
    );

    Ok(results)
}

// ── IMDB List ─────────────────────────────────────────────────────────────

async fn fetch_imdb_list(list: &ImportList) -> Result<Vec<FetchedItem>> {
    let list_id = list
        .config
        .get("list_id")
        .and_then(|v| v.as_str())
        .context(
            "IMDB import list requires 'list_id' in config (e.g. 'ls012345678' or 'ur12345678')",
        )?;

    // IMDB exposes list data as RSS/XML or via export CSV.
    // The RSS feed at /list/{id}/export is the most reliable public endpoint.
    let url = format!("https://www.imdb.com/list/{list_id}/export");

    let http = reqwest::Client::builder()
        .user_agent("Mozilla/5.0 (compatible; StackArr/1.0)")
        .build()?;
    let csv_text = http
        .get(&url)
        .send()
        .await
        .context("IMDB list export request failed")?
        .error_for_status()
        .context("IMDB list export returned error (is the list public?)")?
        .text()
        .await
        .context("failed to read IMDB CSV response")?;

    // Parse CSV: columns are Position, Const, Created, Modified, Description,
    // Title, URL, Title Type, IMDb Rating, Runtime, Year, Genres, Num Votes, ...
    // "Const" is the IMDB ID (tt1234567)
    let mut results = Vec::new();
    let tmdb_api_key = std::env::var("STACKARR_TMDB_API_KEY")
        .context("STACKARR_TMDB_API_KEY not set — cannot resolve IMDB IDs to TMDB")?;

    let is_movie = list.media_type == "movie";
    let external_source = "imdb_id";

    for line in csv_text.lines().skip(1) {
        // Basic CSV parse — IMDB export uses simple comma separation
        let fields: Vec<&str> = line.splitn(8, ',').collect();
        if fields.len() < 7 {
            continue;
        }
        let imdb_id = fields[1].trim();
        if !imdb_id.starts_with("tt") {
            continue;
        }
        let title = fields[5].trim().trim_matches('"');
        let year: Option<i32> = fields.get(10).and_then(|y| y.trim().parse().ok());

        // Resolve IMDB ID → TMDB ID via TMDB /find endpoint
        let find_url = format!(
            "https://api.themoviedb.org/3/find/{imdb_id}?api_key={tmdb_api_key}&external_source={external_source}"
        );
        let tmdb_id = match reqwest::get(&find_url).await {
            Ok(resp) => {
                let body: serde_json::Value = resp.json().await.unwrap_or_default();
                let results_key = if is_movie {
                    "movie_results"
                } else {
                    "tv_results"
                };
                body[results_key]
                    .as_array()
                    .and_then(|arr| arr.first())
                    .and_then(|r| r["id"].as_i64())
            }
            Err(_) => None,
        };

        let Some(tmdb_id) = tmdb_id else {
            tracing::debug!(
                imdb_id,
                title,
                "could not resolve IMDB ID to TMDB — skipping"
            );
            continue;
        };

        results.push(FetchedItem {
            title: title.to_string(),
            tmdb_id,
            year,
            overview: None,
        });
    }

    tracing::info!(
        list_name = %list.name,
        list_id,
        items = results.len(),
        "IMDB list fetched and resolved to TMDB IDs"
    );

    Ok(results)
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
