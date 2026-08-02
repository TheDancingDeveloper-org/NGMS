pub mod import_lists;

use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::MySqlPool;

use stackarr_core::models::{Episode, Movie, Series, SeriesStatus};

/// Strategy for bulk-setting monitored status across a series.
#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum MonitorStrategy {
    /// Monitor all episodes in all seasons (excluding specials).
    All,
    /// Monitor only episodes in the latest (highest-numbered) season.
    LatestSeason,
    /// Monitor only episodes in season 1.
    FirstSeason,
    /// Monitor only episodes that haven't aired yet (air_date_utc is NULL or in the future).
    Upcoming,
    /// Unmonitor all episodes.
    None,
}

// ── Input types ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateSeriesInput {
    pub title: String,
    #[serde(default)]
    pub path: String,
    #[serde(default)]
    pub quality_profile_id: i32,
    #[serde(default)]
    pub monitored: bool,
    pub tvdb_id: Option<i64>,
    pub tmdb_id: Option<i64>,
    pub imdb_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateSeriesInput {
    pub title: Option<String>,
    pub path: Option<String>,
    pub quality_profile_id: Option<i32>,
    pub monitored: Option<bool>,
    /// When true and `path` differs from the current path, move the on-disk
    /// directory and update all episode file records in the database.
    #[serde(default)]
    pub move_files: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateMovieInput {
    pub title: String,
    #[serde(default)]
    pub path: String,
    #[serde(default)]
    pub quality_profile_id: i32,
    #[serde(default)]
    pub monitored: bool,
    pub tmdb_id: Option<i64>,
    pub imdb_id: Option<String>,
    pub year: Option<i32>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateMovieInput {
    pub title: Option<String>,
    pub path: Option<String>,
    pub quality_profile_id: Option<i32>,
    pub monitored: Option<bool>,
    /// When true and `path` differs from the current path, move the on-disk
    /// directory and update all movie file records in the database.
    #[serde(default)]
    pub move_files: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateEpisodeInput {
    pub series_id: i64,
    pub season_number: i32,
    pub episode_number: i32,
    pub title: Option<String>,
    #[serde(default = "default_true")]
    pub monitored: bool,
}

fn default_true() -> bool {
    true
}

// ── Series service ──────────────────────────────────────────────────────────

#[derive(Clone)]
pub struct SeriesService {
    pool: MySqlPool,
}

impl SeriesService {
    pub fn new(pool: MySqlPool) -> Self {
        Self { pool }
    }

    pub async fn list(&self) -> Result<Vec<Series>> {
        let rows = sqlx::query_as::<_, Series>("SELECT * FROM series ORDER BY sort_title")
            .fetch_all(&self.pool)
            .await?;
        Ok(rows)
    }

    /// List series with optional pagination. Returns (items, total_count).
    pub async fn list_paginated(
        &self,
        limit: Option<i64>,
        offset: Option<i64>,
    ) -> Result<(Vec<Series>, i64)> {
        let total: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM series")
            .fetch_one(&self.pool)
            .await?;

        let rows = match limit {
            Some(lim) => {
                sqlx::query_as::<_, Series>(
                    "SELECT * FROM series ORDER BY sort_title LIMIT ? OFFSET ?",
                )
                .bind(lim)
                .bind(offset.unwrap_or(0))
                .fetch_all(&self.pool)
                .await?
            }
            None => {
                sqlx::query_as::<_, Series>("SELECT * FROM series ORDER BY sort_title")
                    .fetch_all(&self.pool)
                    .await?
            }
        };
        Ok((rows, total.0))
    }

    pub async fn get(&self, id: i64) -> Result<Series> {
        let row = sqlx::query_as::<_, Series>("SELECT * FROM series WHERE id = ?")
            .bind(id)
            .fetch_one(&self.pool)
            .await?;
        Ok(row)
    }

    pub async fn create(&self, input: CreateSeriesInput) -> Result<Series> {
        let clean = stackarr_parser::clean_title(&input.title);
        let sort = clean.clone();
        let result = sqlx::query(
            "INSERT INTO series (title, clean_title, sort_title, path, quality_profile_id, monitored, tvdb_id, tmdb_id, imdb_id)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&input.title)
        .bind(&clean)
        .bind(&sort)
        .bind(&input.path)
        .bind(input.quality_profile_id)
        .bind(input.monitored)
        .bind(input.tvdb_id)
        .bind(input.tmdb_id)
        .bind(&input.imdb_id)
        .execute(&self.pool)
        .await?;
        let row = self.get(result.last_insert_id() as i64).await?;
        tracing::info!(id = row.id, title = %row.title, path = %row.path, "series created");
        Ok(row)
    }

    pub async fn update(&self, id: i64, input: UpdateSeriesInput) -> Result<Series> {
        // Fetch-then-update pattern for partial updates
        let existing = self.get(id).await?;
        let title = input.title.unwrap_or(existing.title.clone());
        let new_path = input.path.unwrap_or(existing.path.clone());
        let qp = input
            .quality_profile_id
            .unwrap_or(existing.quality_profile_id);
        let monitored = input.monitored.unwrap_or(existing.monitored);
        let clean = stackarr_parser::clean_title(&title);

        // Move files on disk and update DB paths if requested and path changed
        if input.move_files && new_path != existing.path {
            move_media_directory(&existing.path, &new_path).await?;
            // Rewrite episode file paths that start with the old path
            sqlx::query(
                "UPDATE episode_files SET path = CONCAT(?, SUBSTRING(path, CHAR_LENGTH(?) + 1))
                 WHERE series_id = ? AND path LIKE CONCAT(?, '%')",
            )
            .bind(&new_path)
            .bind(&existing.path)
            .bind(id)
            .bind(&existing.path)
            .execute(&self.pool)
            .await?;
            tracing::info!(id, old = %existing.path, new = %new_path, "series directory moved");
        }

        sqlx::query(
            "UPDATE series SET title = ?, clean_title = ?, sort_title = ?, path = ?, quality_profile_id = ?, monitored = ?
             WHERE id = ?",
        )
        .bind(&title)
        .bind(&clean)
        .bind(&clean)
        .bind(&new_path)
        .bind(qp)
        .bind(monitored)
        .bind(id)
        .execute(&self.pool)
        .await?;
        let row = self.get(id).await?;
        tracing::debug!(id, title = %row.title, monitored, "series updated");
        Ok(row)
    }

    pub async fn delete(&self, id: i64) -> Result<()> {
        tracing::info!(id, "deleting series");
        sqlx::query("DELETE FROM series WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}

// ── Movie service ───────────────────────────────────────────────────────────

#[derive(Clone)]
pub struct MovieService {
    pool: MySqlPool,
}

impl MovieService {
    pub fn new(pool: MySqlPool) -> Self {
        Self { pool }
    }

    pub async fn list(&self) -> Result<Vec<Movie>> {
        let rows = sqlx::query_as::<_, Movie>("SELECT * FROM movies ORDER BY sort_title")
            .fetch_all(&self.pool)
            .await?;
        Ok(rows)
    }

    /// List movies with optional pagination. Returns (items, total_count).
    pub async fn list_paginated(
        &self,
        limit: Option<i64>,
        offset: Option<i64>,
    ) -> Result<(Vec<Movie>, i64)> {
        let total: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM movies")
            .fetch_one(&self.pool)
            .await?;

        let rows = match limit {
            Some(lim) => {
                sqlx::query_as::<_, Movie>(
                    "SELECT * FROM movies ORDER BY sort_title LIMIT ? OFFSET ?",
                )
                .bind(lim)
                .bind(offset.unwrap_or(0))
                .fetch_all(&self.pool)
                .await?
            }
            None => {
                sqlx::query_as::<_, Movie>("SELECT * FROM movies ORDER BY sort_title")
                    .fetch_all(&self.pool)
                    .await?
            }
        };
        Ok((rows, total.0))
    }

    pub async fn get(&self, id: i64) -> Result<Movie> {
        let row = sqlx::query_as::<_, Movie>("SELECT * FROM movies WHERE id = ?")
            .bind(id)
            .fetch_one(&self.pool)
            .await?;
        Ok(row)
    }

    pub async fn create(&self, input: CreateMovieInput) -> Result<Movie> {
        let clean = stackarr_parser::clean_title(&input.title);
        let sort = clean.clone();
        let result = sqlx::query(
            "INSERT INTO movies (title, clean_title, sort_title, path, quality_profile_id, monitored, tmdb_id, imdb_id, year)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&input.title)
        .bind(&clean)
        .bind(&sort)
        .bind(&input.path)
        .bind(input.quality_profile_id)
        .bind(input.monitored)
        .bind(input.tmdb_id)
        .bind(&input.imdb_id)
        .bind(input.year)
        .execute(&self.pool)
        .await?;
        let row = self.get(result.last_insert_id() as i64).await?;
        tracing::info!(id = row.id, title = %row.title, year = row.year, "movie created");
        Ok(row)
    }

    pub async fn update(&self, id: i64, input: UpdateMovieInput) -> Result<Movie> {
        let existing = self.get(id).await?;
        let title = input.title.unwrap_or(existing.title.clone());
        let new_path = input.path.unwrap_or(existing.path.clone());
        let qp = input
            .quality_profile_id
            .unwrap_or(existing.quality_profile_id);
        let monitored = input.monitored.unwrap_or(existing.monitored);
        let clean = stackarr_parser::clean_title(&title);

        if input.move_files && new_path != existing.path {
            move_media_directory(&existing.path, &new_path).await?;
            sqlx::query(
                "UPDATE movie_files SET path = CONCAT(?, SUBSTRING(path, CHAR_LENGTH(?) + 1))
                 WHERE movie_id = ? AND path LIKE CONCAT(?, '%')",
            )
            .bind(&new_path)
            .bind(&existing.path)
            .bind(id)
            .bind(&existing.path)
            .execute(&self.pool)
            .await?;
            tracing::info!(id, old = %existing.path, new = %new_path, "movie directory moved");
        }

        sqlx::query(
            "UPDATE movies SET title = ?, clean_title = ?, sort_title = ?, path = ?, quality_profile_id = ?, monitored = ?
             WHERE id = ?",
        )
        .bind(&title)
        .bind(&clean)
        .bind(&clean)
        .bind(&new_path)
        .bind(qp)
        .bind(monitored)
        .bind(id)
        .execute(&self.pool)
        .await?;
        let row = self.get(id).await?;
        tracing::debug!(id, title = %row.title, monitored, "movie updated");
        Ok(row)
    }

    pub async fn delete(&self, id: i64) -> Result<()> {
        tracing::info!(id, "deleting movie");
        sqlx::query("DELETE FROM movies WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}

// ── Episode service ─────────────────────────────────────────────────────────

#[derive(Clone)]
pub struct EpisodeService {
    pool: MySqlPool,
}

impl EpisodeService {
    pub fn new(pool: MySqlPool) -> Self {
        Self { pool }
    }

    pub async fn list_by_series(&self, series_id: i64) -> Result<Vec<Episode>> {
        let rows = sqlx::query_as::<_, Episode>(
            "SELECT * FROM episodes WHERE series_id = ? ORDER BY season_number, episode_number",
        )
        .bind(series_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    pub async fn get(&self, id: i64) -> Result<Episode> {
        let row = sqlx::query_as::<_, Episode>("SELECT * FROM episodes WHERE id = ?")
            .bind(id)
            .fetch_one(&self.pool)
            .await?;
        Ok(row)
    }

    pub async fn create(&self, input: CreateEpisodeInput) -> Result<Episode> {
        let result = sqlx::query(
            "INSERT INTO episodes (series_id, season_number, episode_number, title, monitored)
             VALUES (?, ?, ?, ?, ?)",
        )
        .bind(input.series_id)
        .bind(input.season_number)
        .bind(input.episode_number)
        .bind(&input.title)
        .bind(input.monitored)
        .execute(&self.pool)
        .await?;
        let row = self.get(result.last_insert_id() as i64).await?;
        tracing::debug!(
            id = row.id,
            series_id = input.series_id,
            s = input.season_number,
            e = input.episode_number,
            "episode created"
        );
        Ok(row)
    }

    pub async fn set_monitored(&self, id: i64, monitored: bool) -> Result<()> {
        tracing::debug!(id, monitored, "episode monitored changed");
        sqlx::query("UPDATE episodes SET monitored = ? WHERE id = ?")
            .bind(monitored)
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Update episode monitored status and return the updated episode.
    pub async fn update_monitored(&self, id: i64, monitored: bool) -> Result<Episode> {
        sqlx::query("UPDATE episodes SET monitored = ? WHERE id = ?")
            .bind(monitored)
            .bind(id)
            .execute(&self.pool)
            .await?;
        self.get(id).await
    }

    /// Bulk update monitored status for all episodes in a season, and sync the seasons table.
    pub async fn set_season_monitored(
        &self,
        series_id: i64,
        season_number: i32,
        monitored: bool,
    ) -> Result<()> {
        tracing::debug!(
            series_id,
            season_number,
            monitored,
            "season monitored changed"
        );
        sqlx::query("UPDATE episodes SET monitored = ? WHERE series_id = ? AND season_number = ?")
            .bind(monitored)
            .bind(series_id)
            .bind(season_number)
            .execute(&self.pool)
            .await?;

        // Upsert the seasons table to keep it in sync
        sqlx::query(
            "INSERT INTO seasons (series_id, season_number, monitored)
             VALUES (?, ?, ?)
             ON DUPLICATE KEY UPDATE monitored = VALUES(monitored)",
        )
        .bind(series_id)
        .bind(season_number)
        .bind(monitored)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Apply a monitoring strategy across all episodes/seasons for a series.
    pub async fn apply_monitor_strategy(
        &self,
        series_id: i64,
        strategy: MonitorStrategy,
    ) -> Result<()> {
        tracing::info!(series_id, strategy = ?strategy, "applying monitor strategy");
        match strategy {
            MonitorStrategy::All => {
                // Monitor all non-special episodes
                sqlx::query(
                    "UPDATE episodes SET monitored = (season_number > 0) WHERE series_id = ?",
                )
                .bind(series_id)
                .execute(&self.pool)
                .await?;
            }
            MonitorStrategy::LatestSeason => {
                // Find the latest (highest) season number (excluding specials)
                let latest: Option<(i32,)> = sqlx::query_as(
                    "SELECT MAX(season_number) FROM episodes
                     WHERE series_id = ? AND season_number > 0",
                )
                .bind(series_id)
                .fetch_optional(&self.pool)
                .await?;

                if let Some((max_season,)) = latest {
                    sqlx::query(
                        "UPDATE episodes SET monitored = (season_number = ?)
                         WHERE series_id = ? AND season_number > 0",
                    )
                    .bind(max_season)
                    .bind(series_id)
                    .execute(&self.pool)
                    .await?;
                }
            }
            MonitorStrategy::FirstSeason => {
                // Monitor only season 1
                sqlx::query(
                    "UPDATE episodes SET monitored = (season_number = 1)
                     WHERE series_id = ? AND season_number > 0",
                )
                .bind(series_id)
                .execute(&self.pool)
                .await?;
            }
            MonitorStrategy::Upcoming => {
                // Monitor only unaired episodes (air_date_utc is NULL or in the future)
                sqlx::query(
                    "UPDATE episodes SET monitored = (air_date_utc IS NULL OR air_date_utc > NOW())
                     WHERE series_id = ? AND season_number > 0",
                )
                .bind(series_id)
                .execute(&self.pool)
                .await?;
            }
            MonitorStrategy::None => {
                // Unmonitor everything
                sqlx::query("UPDATE episodes SET monitored = false WHERE series_id = ?")
                    .bind(series_id)
                    .execute(&self.pool)
                    .await?;
            }
        }

        // Sync the seasons table: a season is monitored if any of its episodes are monitored
        sqlx::query(
            "INSERT INTO seasons (series_id, season_number, monitored)
             SELECT series_id, season_number, MAX(monitored)
             FROM episodes WHERE series_id = ?
             GROUP BY series_id, season_number
             ON DUPLICATE KEY UPDATE monitored = VALUES(monitored)",
        )
        .bind(series_id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Bulk update monitored status for multiple episode IDs.
    pub async fn set_bulk_monitored(&self, episode_ids: &[i64], monitored: bool) -> Result<()> {
        if episode_ids.is_empty() {
            return Ok(());
        }
        let mut query = sqlx::QueryBuilder::new("UPDATE episodes SET monitored = ");
        query.push_bind(monitored).push(" WHERE id IN (");
        let mut ids = query.separated(", ");
        for id in episode_ids {
            ids.push_bind(id);
        }
        ids.push_unseparated(")");
        query.build().execute(&self.pool).await?;
        Ok(())
    }
}

// ── Calendar service ───────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct CalendarEntry {
    pub episode_id: i64,
    pub series_id: i64,
    pub series_title: String,
    pub season_number: i32,
    pub episode_number: i32,
    pub episode_title: Option<String>,
    pub air_date_utc: Option<DateTime<Utc>>,
    pub has_file: bool,
    pub monitored: bool,
}

#[derive(Clone)]
pub struct CalendarService {
    pool: MySqlPool,
}

impl CalendarService {
    pub fn new(pool: MySqlPool) -> Self {
        Self { pool }
    }

    /// Get upcoming episodes between two dates (inclusive).
    pub async fn get_calendar(&self, start: &str, end: &str) -> Result<Vec<CalendarEntry>> {
        let rows = sqlx::query_as::<_, CalendarEntry>(
            "SELECT e.id AS episode_id, e.series_id, s.title AS series_title,
                    e.season_number, e.episode_number, e.title AS episode_title,
                    e.air_date_utc, (e.episode_file_id IS NOT NULL) AS has_file,
                    e.monitored
             FROM episodes e
             JOIN series s ON e.series_id = s.id
             WHERE e.air_date_utc BETWEEN ? AND ?
               AND s.monitored = true
             ORDER BY e.air_date_utc",
        )
        .bind(start)
        .bind(end)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }
}

// ── Wanted service ─────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WantedPage {
    pub page: i64,
    pub page_size: i64,
    pub total_records: i64,
    pub records: Vec<WantedRecord>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WantedRecord {
    pub id: i64,
    pub media_type: String,
    pub media_id: i64,
    pub title: String,
    pub episode_info: Option<String>,
    pub quality_profile_id: i32,
    pub air_date: Option<String>,
    pub monitored: bool,
}

#[derive(Clone)]
pub struct WantedService {
    pool: MySqlPool,
}

impl WantedService {
    pub fn new(pool: MySqlPool) -> Self {
        Self { pool }
    }

    /// Missing: monitored episodes/movies without a file, aired in the past.
    pub async fn missing(&self, page: i64, page_size: i64) -> Result<WantedPage> {
        let offset = (page - 1).max(0) * page_size;

        // Count total missing episodes
        let episode_count: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM episodes e
             JOIN series s ON e.series_id = s.id
             WHERE e.monitored = true AND s.monitored = true
               AND e.episode_file_id IS NULL
               AND e.air_date_utc < NOW()",
        )
        .fetch_one(&self.pool)
        .await?;

        // Count total missing movies
        let movie_count: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM movies m
             WHERE m.monitored = true AND m.movie_file_id IS NULL",
        )
        .fetch_one(&self.pool)
        .await?;

        let total_records = episode_count.0 + movie_count.0;

        // Fetch combined missing records using a UNION query
        let rows = sqlx::query_as::<_, (i64, String, i64, String, Option<String>, i32, Option<String>, bool)>(
            "SELECT * FROM (
                SELECT e.id, 'series' AS media_type, e.series_id AS media_id,
                       s.title, CONCAT('S', LPAD(e.season_number, 2, '0'), 'E', LPAD(e.episode_number, 2, '0')) AS episode_info,
                       s.quality_profile_id, CAST(e.air_date_utc AS CHAR) AS air_date, e.monitored
                FROM episodes e
                JOIN series s ON e.series_id = s.id
                WHERE e.monitored = true AND s.monitored = true
                  AND e.episode_file_id IS NULL
                  AND e.air_date_utc < NOW()
                UNION ALL
                SELECT m.id, 'movie' AS media_type, m.id AS media_id,
                       m.title, NULL AS episode_info,
                       m.quality_profile_id, CAST(m.physical_release AS CHAR) AS air_date, m.monitored
                FROM movies m
                WHERE m.monitored = true AND m.movie_file_id IS NULL
            ) combined
            ORDER BY air_date IS NULL, air_date DESC
            LIMIT ? OFFSET ?",
        )
        .bind(page_size)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?;

        let records = rows
            .into_iter()
            .map(
                |(
                    id,
                    media_type,
                    media_id,
                    title,
                    episode_info,
                    quality_profile_id,
                    air_date,
                    monitored,
                )| {
                    WantedRecord {
                        id,
                        media_type,
                        media_id,
                        title,
                        episode_info,
                        quality_profile_id,
                        air_date,
                        monitored,
                    }
                },
            )
            .collect();

        Ok(WantedPage {
            page,
            page_size,
            total_records,
            records,
        })
    }

    /// Cutoff unmet: items that have a file but whose file quality is below
    /// the quality profile's cutoff threshold.
    pub async fn cutoff_unmet(&self, page: i64, page_size: i64) -> Result<WantedPage> {
        let offset = (page - 1).max(0) * page_size;

        // Count episodes whose file quality is below the profile cutoff
        let episode_count: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM episodes e
             JOIN series s ON e.series_id = s.id
             JOIN media_files mf ON e.episode_file_id = mf.id
             JOIN quality_profiles qp ON s.quality_profile_id = qp.id
             WHERE e.monitored = true AND s.monitored = true
               AND e.episode_file_id IS NOT NULL
               AND COALESCE(CAST(JSON_UNQUOTE(JSON_EXTRACT(mf.quality, '$.quality')) AS SIGNED), 0) < qp.cutoff",
        )
        .fetch_one(&self.pool)
        .await?;

        // Count movies whose file quality is below the profile cutoff
        let movie_count: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM movies m
             JOIN media_files mf ON m.movie_file_id = mf.id
             JOIN quality_profiles qp ON m.quality_profile_id = qp.id
             WHERE m.monitored = true
               AND m.movie_file_id IS NOT NULL
               AND COALESCE(CAST(JSON_UNQUOTE(JSON_EXTRACT(mf.quality, '$.quality')) AS SIGNED), 0) < qp.cutoff",
        )
        .fetch_one(&self.pool)
        .await?;

        let total_records = episode_count.0 + movie_count.0;

        let rows = sqlx::query_as::<_, (i64, String, i64, String, Option<String>, i32, Option<String>, bool)>(
            "SELECT * FROM (
                SELECT e.id, 'series' AS media_type, e.series_id AS media_id,
                       s.title, CONCAT('S', LPAD(e.season_number, 2, '0'), 'E', LPAD(e.episode_number, 2, '0')) AS episode_info,
                       s.quality_profile_id, CAST(e.air_date_utc AS CHAR) AS air_date, e.monitored
                FROM episodes e
                JOIN series s ON e.series_id = s.id
                JOIN media_files mf ON e.episode_file_id = mf.id
                JOIN quality_profiles qp ON s.quality_profile_id = qp.id
                WHERE e.monitored = true AND s.monitored = true
                  AND e.episode_file_id IS NOT NULL
                  AND COALESCE(CAST(JSON_UNQUOTE(JSON_EXTRACT(mf.quality, '$.quality')) AS SIGNED), 0) < qp.cutoff
                UNION ALL
                SELECT m.id, 'movie' AS media_type, m.id AS media_id,
                       m.title, NULL AS episode_info,
                       m.quality_profile_id, CAST(m.physical_release AS CHAR) AS air_date, m.monitored
                FROM movies m
                JOIN media_files mf ON m.movie_file_id = mf.id
                JOIN quality_profiles qp ON m.quality_profile_id = qp.id
                WHERE m.monitored = true
                  AND m.movie_file_id IS NOT NULL
                  AND COALESCE(CAST(JSON_UNQUOTE(JSON_EXTRACT(mf.quality, '$.quality')) AS SIGNED), 0) < qp.cutoff
            ) combined
            ORDER BY air_date IS NULL, air_date DESC
            LIMIT ? OFFSET ?",
        )
        .bind(page_size)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?;

        let records = rows
            .into_iter()
            .map(
                |(
                    id,
                    media_type,
                    media_id,
                    title,
                    episode_info,
                    quality_profile_id,
                    air_date,
                    monitored,
                )| {
                    WantedRecord {
                        id,
                        media_type,
                        media_id,
                        title,
                        episode_info,
                        quality_profile_id,
                        air_date,
                        monitored,
                    }
                },
            )
            .collect();

        Ok(WantedPage {
            page,
            page_size,
            total_records,
            records,
        })
    }
}

// ── Metadata refresh service ───────────────────────────────────────────────

#[derive(Clone)]
pub struct MetadataRefreshService {
    pool: MySqlPool,
}

impl MetadataRefreshService {
    pub fn new(pool: MySqlPool) -> Self {
        Self { pool }
    }

    /// Find series that haven't been synced in over 12 hours.
    pub async fn find_stale_series(&self) -> Result<Vec<i64>> {
        let rows: Vec<(i64,)> = sqlx::query_as(
            "SELECT id FROM series
             WHERE last_info_sync IS NULL
                OR last_info_sync < NOW() - INTERVAL 12 HOUR",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.into_iter().map(|(id,)| id).collect())
    }

    /// Find movies that haven't been synced in over 12 hours.
    pub async fn find_stale_movies(&self) -> Result<Vec<i64>> {
        let rows: Vec<(i64,)> = sqlx::query_as(
            "SELECT id FROM movies
             WHERE last_info_sync IS NULL
                OR last_info_sync < NOW() - INTERVAL 12 HOUR",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.into_iter().map(|(id,)| id).collect())
    }

    /// Update last_info_sync timestamp for a series.
    pub async fn mark_series_synced(&self, id: i64) -> Result<()> {
        sqlx::query("UPDATE series SET last_info_sync = NOW() WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Update last_info_sync timestamp for a movie.
    pub async fn mark_movie_synced(&self, id: i64) -> Result<()> {
        sqlx::query("UPDATE movies SET last_info_sync = NOW() WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Update series metadata from TMDB data.
    #[allow(clippy::too_many_arguments)]
    pub async fn update_series_metadata(
        &self,
        id: i64,
        overview: Option<&str>,
        status: &str,
        network: Option<&str>,
        runtime: Option<i32>,
        images: Option<&serde_json::Value>,
        genres: Option<&[String]>,
    ) -> Result<()> {
        // Map TMDB status string to our SeriesStatus enum
        let series_status = match status {
            "Returning Series" => SeriesStatus::Continuing,
            "Ended" => SeriesStatus::Ended,
            "Canceled" => SeriesStatus::Ended,
            "In Production" => SeriesStatus::Upcoming,
            "Planned" => SeriesStatus::Upcoming,
            "Pilot" => SeriesStatus::Upcoming,
            _ => {
                tracing::warn!(
                    tmdb_status = status,
                    series_id = id,
                    "unknown TMDB series status, defaulting to Continuing"
                );
                SeriesStatus::Continuing
            }
        };

        sqlx::query(
            "UPDATE series
             SET overview = COALESCE(?, overview),
                 network = COALESCE(?, network),
                 runtime = COALESCE(?, runtime),
                 images = COALESCE(?, images),
                 genres = COALESCE(?, genres),
                 status = ?,
                 last_info_sync = NOW()
             WHERE id = ?",
        )
        .bind(overview)
        .bind(network)
        .bind(runtime)
        .bind(images)
        .bind(genres.map(sqlx::types::Json))
        .bind(series_status)
        .bind(id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Update movie metadata from TMDB data.
    pub async fn update_movie_metadata(
        &self,
        id: i64,
        overview: Option<&str>,
        studio: Option<&str>,
        images: Option<&serde_json::Value>,
        genres: Option<&[String]>,
    ) -> Result<()> {
        sqlx::query(
            "UPDATE movies
             SET overview = COALESCE(?, overview),
                 studio = COALESCE(?, studio),
                 images = COALESCE(?, images),
                 genres = COALESCE(?, genres),
                 last_info_sync = NOW()
             WHERE id = ?",
        )
        .bind(overview)
        .bind(studio)
        .bind(images)
        .bind(genres.map(sqlx::types::Json))
        .bind(id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }
}

/// Move a media directory from `old_path` to `new_path`.
/// Tries an atomic rename first; falls back to recursive copy+delete if the
/// paths span different filesystems.
async fn move_media_directory(old_path: &str, new_path: &str) -> Result<()> {
    use std::path::Path;
    let src = Path::new(old_path);
    let dst = Path::new(new_path);

    // Source doesn't exist — nothing to move, that's fine.
    if !src.exists() {
        if let Some(parent) = dst.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        return Ok(());
    }

    if let Some(parent) = dst.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }

    // Try atomic rename first (same filesystem).
    if tokio::fs::rename(src, dst).await.is_ok() {
        return Ok(());
    }

    // Cross-filesystem: copy recursively then remove source.
    copy_dir_recursive(src, dst).await?;
    tokio::fs::remove_dir_all(src).await?;
    Ok(())
}

async fn copy_dir_recursive(src: &std::path::Path, dst: &std::path::Path) -> Result<()> {
    tokio::fs::create_dir_all(dst).await?;
    let mut entries = tokio::fs::read_dir(src).await?;
    while let Some(entry) = entries.next_entry().await? {
        let file_type = entry.file_type().await?;
        let dest_path = dst.join(entry.file_name());
        if file_type.is_dir() {
            Box::pin(copy_dir_recursive(&entry.path(), &dest_path)).await?;
        } else {
            tokio::fs::copy(entry.path(), dest_path).await?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use stackarr_core::test_helpers::{
        TestDb, seed_episode, seed_media_library_folder, seed_quality_profile, seed_series,
    };

    // ── MonitorStrategy serde ───────────────────────────────────────────

    #[test]
    fn test_monitor_strategy_deserialize_all_variants() {
        assert_eq!(
            serde_json::from_str::<MonitorStrategy>(r#""all""#).unwrap(),
            MonitorStrategy::All
        );
        assert_eq!(
            serde_json::from_str::<MonitorStrategy>(r#""latestSeason""#).unwrap(),
            MonitorStrategy::LatestSeason
        );
        assert_eq!(
            serde_json::from_str::<MonitorStrategy>(r#""firstSeason""#).unwrap(),
            MonitorStrategy::FirstSeason
        );
        assert_eq!(
            serde_json::from_str::<MonitorStrategy>(r#""upcoming""#).unwrap(),
            MonitorStrategy::Upcoming
        );
        assert_eq!(
            serde_json::from_str::<MonitorStrategy>(r#""none""#).unwrap(),
            MonitorStrategy::None
        );
    }

    #[test]
    fn test_monitor_strategy_serialize_roundtrip() {
        let strategies = [
            MonitorStrategy::All,
            MonitorStrategy::LatestSeason,
            MonitorStrategy::FirstSeason,
            MonitorStrategy::Upcoming,
            MonitorStrategy::None,
        ];
        for s in &strategies {
            let json = serde_json::to_string(s).unwrap();
            let back: MonitorStrategy = serde_json::from_str(&json).unwrap();
            assert_eq!(*s, back);
        }
    }

    #[test]
    fn test_monitor_strategy_invalid_variant() {
        assert!(serde_json::from_str::<MonitorStrategy>(r#""invalid""#).is_err());
    }

    // ── CreateSeriesInput serde ─────────────────────────────────────────

    #[test]
    fn test_create_series_input_minimal() {
        let json = r#"{"title": "Breaking Bad"}"#;
        let input: CreateSeriesInput = serde_json::from_str(json).unwrap();
        assert_eq!(input.title, "Breaking Bad");
        assert_eq!(input.path, ""); // default
        assert_eq!(input.quality_profile_id, 0); // default
        assert!(!input.monitored); // default false
        assert!(input.tvdb_id.is_none());
        assert!(input.tmdb_id.is_none());
        assert!(input.imdb_id.is_none());
    }

    #[test]
    fn test_create_series_input_full() {
        let json = r#"{
            "title": "Breaking Bad",
            "path": "/tv/Breaking Bad",
            "qualityProfileId": 5,
            "monitored": true,
            "tvdbId": 81189,
            "tmdbId": 1396,
            "imdbId": "tt0903747"
        }"#;
        let input: CreateSeriesInput = serde_json::from_str(json).unwrap();
        assert_eq!(input.title, "Breaking Bad");
        assert_eq!(input.path, "/tv/Breaking Bad");
        assert_eq!(input.quality_profile_id, 5);
        assert!(input.monitored);
        assert_eq!(input.tvdb_id, Some(81189));
        assert_eq!(input.tmdb_id, Some(1396));
        assert_eq!(input.imdb_id.as_deref(), Some("tt0903747"));
    }

    // ── UpdateSeriesInput serde ─────────────────────────────────────────

    #[test]
    fn test_update_series_input_partial() {
        let json = r#"{"title": "New Title"}"#;
        let input: UpdateSeriesInput = serde_json::from_str(json).unwrap();
        assert_eq!(input.title.as_deref(), Some("New Title"));
        assert!(input.path.is_none());
        assert!(input.quality_profile_id.is_none());
        assert!(input.monitored.is_none());
    }

    #[test]
    fn test_update_series_input_empty() {
        let json = "{}";
        let input: UpdateSeriesInput = serde_json::from_str(json).unwrap();
        assert!(input.title.is_none());
        assert!(input.path.is_none());
        assert!(input.quality_profile_id.is_none());
        assert!(input.monitored.is_none());
    }

    // ── CreateMovieInput serde ──────────────────────────────────────────

    #[test]
    fn test_create_movie_input_minimal() {
        let json = r#"{"title": "Inception"}"#;
        let input: CreateMovieInput = serde_json::from_str(json).unwrap();
        assert_eq!(input.title, "Inception");
        assert_eq!(input.path, ""); // default
        assert_eq!(input.quality_profile_id, 0); // default
        assert!(!input.monitored); // default false
        assert!(input.tmdb_id.is_none());
        assert!(input.imdb_id.is_none());
        assert!(input.year.is_none());
    }

    #[test]
    fn test_create_movie_input_full() {
        let json = r#"{
            "title": "Inception",
            "path": "/movies/Inception (2010)",
            "qualityProfileId": 3,
            "monitored": true,
            "tmdbId": 27205,
            "imdbId": "tt1375666",
            "year": 2010
        }"#;
        let input: CreateMovieInput = serde_json::from_str(json).unwrap();
        assert_eq!(input.title, "Inception");
        assert_eq!(input.path, "/movies/Inception (2010)");
        assert_eq!(input.quality_profile_id, 3);
        assert!(input.monitored);
        assert_eq!(input.tmdb_id, Some(27205));
        assert_eq!(input.imdb_id.as_deref(), Some("tt1375666"));
        assert_eq!(input.year, Some(2010));
    }

    // ── UpdateMovieInput serde ──────────────────────────────────────────

    #[test]
    fn test_update_movie_input_partial() {
        let json = r#"{"monitored": false}"#;
        let input: UpdateMovieInput = serde_json::from_str(json).unwrap();
        assert!(input.title.is_none());
        assert_eq!(input.monitored, Some(false));
    }

    // ── CreateEpisodeInput serde ────────────────────────────────────────

    #[test]
    fn test_create_episode_input_defaults_monitored_true() {
        let json = r#"{"seriesId": 1, "seasonNumber": 1, "episodeNumber": 1}"#;
        let input: CreateEpisodeInput = serde_json::from_str(json).unwrap();
        assert_eq!(input.series_id, 1);
        assert_eq!(input.season_number, 1);
        assert_eq!(input.episode_number, 1);
        assert!(input.monitored); // default_true
        assert!(input.title.is_none());
    }

    #[test]
    fn test_create_episode_input_override_monitored() {
        let json = r#"{"seriesId": 1, "seasonNumber": 1, "episodeNumber": 1, "monitored": false}"#;
        let input: CreateEpisodeInput = serde_json::from_str(json).unwrap();
        assert!(!input.monitored);
    }

    #[test]
    fn test_create_episode_input_with_title() {
        let json = r#"{"seriesId": 1, "seasonNumber": 1, "episodeNumber": 1, "title": "Pilot"}"#;
        let input: CreateEpisodeInput = serde_json::from_str(json).unwrap();
        assert_eq!(input.title.as_deref(), Some("Pilot"));
    }

    // ── SeriesService ───────────────────────────────────────────────────

    #[tokio::test]
    #[ignore = "requires running mariadb"]
    async fn test_series_create_and_get() {
        let db = TestDb::new().await;
        let profile_id = seed_quality_profile(&db.pool).await;

        let svc = SeriesService::new(db.pool.clone());
        let created = svc
            .create(CreateSeriesInput {
                title: "Breaking Bad".into(),
                path: "/tv/Breaking Bad".into(),
                quality_profile_id: profile_id,
                monitored: true,
                tvdb_id: Some(81189),
                tmdb_id: Some(1396),
                imdb_id: Some("tt0903747".into()),
            })
            .await
            .expect("create series");

        assert_eq!(created.title, "Breaking Bad");
        assert_eq!(created.tvdb_id, Some(81189));

        let fetched = svc.get(created.id).await.expect("get series");
        assert_eq!(fetched.title, "Breaking Bad");
        assert_eq!(fetched.quality_profile_id, profile_id);

        db.cleanup().await;
    }

    #[tokio::test]
    #[ignore = "requires running mariadb"]
    async fn test_series_list() {
        let db = TestDb::new().await;
        let profile_id = seed_quality_profile(&db.pool).await;
        let rf = seed_media_library_folder(&db.pool, "/tv", "series").await;
        seed_series(&db.pool, "Alpha", profile_id, rf).await;
        seed_series(&db.pool, "Beta", profile_id, rf).await;
        seed_series(&db.pool, "Gamma", profile_id, rf).await;

        let svc = SeriesService::new(db.pool.clone());
        let list = svc.list().await.expect("list series");
        assert_eq!(list.len(), 3);

        db.cleanup().await;
    }

    #[tokio::test]
    #[ignore = "requires running mariadb"]
    async fn test_series_update_partial() {
        let db = TestDb::new().await;
        let profile_id = seed_quality_profile(&db.pool).await;

        let svc = SeriesService::new(db.pool.clone());
        let created = svc
            .create(CreateSeriesInput {
                title: "Original Title".into(),
                path: "/tv/Original".into(),
                quality_profile_id: profile_id,
                monitored: true,
                tvdb_id: None,
                tmdb_id: None,
                imdb_id: None,
            })
            .await
            .expect("create");

        let updated = svc
            .update(
                created.id,
                UpdateSeriesInput {
                    title: Some("New Title".into()),
                    path: None,
                    quality_profile_id: None,
                    monitored: None,
                    move_files: false,
                },
            )
            .await
            .expect("update");

        assert_eq!(updated.title, "New Title");
        assert_eq!(updated.path, "/tv/Original"); // unchanged

        db.cleanup().await;
    }

    #[tokio::test]
    #[ignore = "requires running mariadb"]
    async fn test_series_delete() {
        let db = TestDb::new().await;
        let profile_id = seed_quality_profile(&db.pool).await;

        let svc = SeriesService::new(db.pool.clone());
        let created = svc
            .create(CreateSeriesInput {
                title: "To Delete".into(),
                path: "/tv/del".into(),
                quality_profile_id: profile_id,
                monitored: false,
                tvdb_id: None,
                tmdb_id: None,
                imdb_id: None,
            })
            .await
            .expect("create");

        svc.delete(created.id).await.expect("delete");
        let result = svc.get(created.id).await;
        assert!(result.is_err());

        db.cleanup().await;
    }

    // ── MovieService ────────────────────────────────────────────────────

    #[tokio::test]
    #[ignore = "requires running mariadb"]
    async fn test_movie_create_and_get() {
        let db = TestDb::new().await;
        let profile_id = seed_quality_profile(&db.pool).await;

        let svc = MovieService::new(db.pool.clone());
        let created = svc
            .create(CreateMovieInput {
                title: "Inception".into(),
                path: "/movies/Inception (2010)".into(),
                quality_profile_id: profile_id,
                monitored: true,
                tmdb_id: Some(27205),
                imdb_id: Some("tt1375666".into()),
                year: Some(2010),
            })
            .await
            .expect("create movie");

        assert_eq!(created.title, "Inception");

        let fetched = svc.get(created.id).await.expect("get movie");
        assert_eq!(fetched.tmdb_id, Some(27205));

        db.cleanup().await;
    }

    #[tokio::test]
    #[ignore = "requires running mariadb"]
    async fn test_movie_delete() {
        let db = TestDb::new().await;
        let profile_id = seed_quality_profile(&db.pool).await;

        let svc = MovieService::new(db.pool.clone());
        let created = svc
            .create(CreateMovieInput {
                title: "To Delete".into(),
                path: "/movies/del".into(),
                quality_profile_id: profile_id,
                monitored: false,
                tmdb_id: None,
                imdb_id: None,
                year: None,
            })
            .await
            .expect("create");

        svc.delete(created.id).await.expect("delete");
        let result = svc.get(created.id).await;
        assert!(result.is_err());

        db.cleanup().await;
    }

    // ── EpisodeService ──────────────────────────────────────────────────

    #[tokio::test]
    #[ignore = "requires running mariadb"]
    async fn test_episode_create_and_list() {
        let db = TestDb::new().await;
        let profile_id = seed_quality_profile(&db.pool).await;
        let rf = seed_media_library_folder(&db.pool, "/tv", "series").await;
        let series_id = seed_series(&db.pool, "Test Series", profile_id, rf).await;
        seed_episode(&db.pool, series_id, 1, 1).await;
        seed_episode(&db.pool, series_id, 1, 2).await;
        seed_episode(&db.pool, series_id, 1, 3).await;

        let svc = EpisodeService::new(db.pool.clone());
        let episodes = svc.list_by_series(series_id).await.expect("list episodes");
        assert_eq!(episodes.len(), 3);

        db.cleanup().await;
    }

    #[tokio::test]
    #[ignore = "requires running mariadb"]
    async fn test_episode_set_monitored() {
        let db = TestDb::new().await;
        let profile_id = seed_quality_profile(&db.pool).await;
        let rf = seed_media_library_folder(&db.pool, "/tv", "series").await;
        let series_id = seed_series(&db.pool, "Test", profile_id, rf).await;
        let ep_id = seed_episode(&db.pool, series_id, 1, 1).await;

        let svc = EpisodeService::new(db.pool.clone());

        // Start monitored, toggle off
        svc.set_monitored(ep_id, false)
            .await
            .expect("set monitored false");
        let ep = svc.get(ep_id).await.expect("get episode");
        assert!(!ep.monitored);

        // Toggle back on
        svc.set_monitored(ep_id, true)
            .await
            .expect("set monitored true");
        let ep = svc.get(ep_id).await.expect("get episode");
        assert!(ep.monitored);

        db.cleanup().await;
    }

    #[tokio::test]
    #[ignore = "requires running mariadb"]
    async fn test_episode_bulk_monitored() {
        let db = TestDb::new().await;
        let profile_id = seed_quality_profile(&db.pool).await;
        let rf = seed_media_library_folder(&db.pool, "/tv", "series").await;
        let series_id = seed_series(&db.pool, "Bulk", profile_id, rf).await;
        let ep1 = seed_episode(&db.pool, series_id, 1, 1).await;
        let ep2 = seed_episode(&db.pool, series_id, 1, 2).await;
        let ep3 = seed_episode(&db.pool, series_id, 1, 3).await;

        let svc = EpisodeService::new(db.pool.clone());
        svc.set_bulk_monitored(&[ep1, ep2, ep3], false)
            .await
            .expect("bulk unmonitor");

        let episodes = svc.list_by_series(series_id).await.expect("list");
        assert!(episodes.iter().all(|e| !e.monitored));

        db.cleanup().await;
    }
}
