pub mod import_lists;

use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;

use stackarr_core::models::{Episode, Movie, Series};

// ── Input types ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateSeriesInput {
    pub title: String,
    pub path: String,
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
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateMovieInput {
    pub title: String,
    pub path: String,
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
    pool: PgPool,
}

impl SeriesService {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn list(&self) -> Result<Vec<Series>> {
        let rows = sqlx::query_as::<_, Series>("SELECT * FROM series ORDER BY sort_title")
            .fetch_all(&self.pool)
            .await?;
        Ok(rows)
    }

    pub async fn get(&self, id: i64) -> Result<Series> {
        let row = sqlx::query_as::<_, Series>("SELECT * FROM series WHERE id = $1")
            .bind(id)
            .fetch_one(&self.pool)
            .await?;
        Ok(row)
    }

    pub async fn create(&self, input: CreateSeriesInput) -> Result<Series> {
        let clean = stackarr_parser::clean_title(&input.title);
        let sort = clean.clone();
        let row = sqlx::query_as::<_, Series>(
            "INSERT INTO series (title, clean_title, sort_title, path, quality_profile_id, monitored, tvdb_id, tmdb_id, imdb_id)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
             RETURNING *",
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
        .fetch_one(&self.pool)
        .await?;
        Ok(row)
    }

    pub async fn update(&self, id: i64, input: UpdateSeriesInput) -> Result<Series> {
        // Fetch-then-update pattern for partial updates
        let existing = self.get(id).await?;
        let title = input.title.unwrap_or(existing.title);
        let path = input.path.unwrap_or(existing.path);
        let qp = input.quality_profile_id.unwrap_or(existing.quality_profile_id);
        let monitored = input.monitored.unwrap_or(existing.monitored);
        let clean = stackarr_parser::clean_title(&title);

        let row = sqlx::query_as::<_, Series>(
            "UPDATE series SET title = $1, clean_title = $2, sort_title = $2, path = $3, quality_profile_id = $4, monitored = $5
             WHERE id = $6 RETURNING *",
        )
        .bind(&title)
        .bind(&clean)
        .bind(&path)
        .bind(qp)
        .bind(monitored)
        .bind(id)
        .fetch_one(&self.pool)
        .await?;
        Ok(row)
    }

    pub async fn delete(&self, id: i64) -> Result<()> {
        sqlx::query("DELETE FROM series WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}

// ── Movie service ───────────────────────────────────────────────────────────

#[derive(Clone)]
pub struct MovieService {
    pool: PgPool,
}

impl MovieService {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn list(&self) -> Result<Vec<Movie>> {
        let rows = sqlx::query_as::<_, Movie>("SELECT * FROM movies ORDER BY sort_title")
            .fetch_all(&self.pool)
            .await?;
        Ok(rows)
    }

    pub async fn get(&self, id: i64) -> Result<Movie> {
        let row = sqlx::query_as::<_, Movie>("SELECT * FROM movies WHERE id = $1")
            .bind(id)
            .fetch_one(&self.pool)
            .await?;
        Ok(row)
    }

    pub async fn create(&self, input: CreateMovieInput) -> Result<Movie> {
        let clean = stackarr_parser::clean_title(&input.title);
        let sort = clean.clone();
        let row = sqlx::query_as::<_, Movie>(
            "INSERT INTO movies (title, clean_title, sort_title, path, quality_profile_id, monitored, tmdb_id, imdb_id, year)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
             RETURNING *",
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
        .fetch_one(&self.pool)
        .await?;
        Ok(row)
    }

    pub async fn update(&self, id: i64, input: UpdateMovieInput) -> Result<Movie> {
        let existing = self.get(id).await?;
        let title = input.title.unwrap_or(existing.title);
        let path = input.path.unwrap_or(existing.path);
        let qp = input.quality_profile_id.unwrap_or(existing.quality_profile_id);
        let monitored = input.monitored.unwrap_or(existing.monitored);
        let clean = stackarr_parser::clean_title(&title);

        let row = sqlx::query_as::<_, Movie>(
            "UPDATE movies SET title = $1, clean_title = $2, sort_title = $2, path = $3, quality_profile_id = $4, monitored = $5
             WHERE id = $6 RETURNING *",
        )
        .bind(&title)
        .bind(&clean)
        .bind(&path)
        .bind(qp)
        .bind(monitored)
        .bind(id)
        .fetch_one(&self.pool)
        .await?;
        Ok(row)
    }

    pub async fn delete(&self, id: i64) -> Result<()> {
        sqlx::query("DELETE FROM movies WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}

// ── Episode service ─────────────────────────────────────────────────────────

#[derive(Clone)]
pub struct EpisodeService {
    pool: PgPool,
}

impl EpisodeService {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn list_by_series(&self, series_id: i64) -> Result<Vec<Episode>> {
        let rows = sqlx::query_as::<_, Episode>(
            "SELECT * FROM episodes WHERE series_id = $1 ORDER BY season_number, episode_number",
        )
        .bind(series_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    pub async fn get(&self, id: i64) -> Result<Episode> {
        let row = sqlx::query_as::<_, Episode>("SELECT * FROM episodes WHERE id = $1")
            .bind(id)
            .fetch_one(&self.pool)
            .await?;
        Ok(row)
    }

    pub async fn create(&self, input: CreateEpisodeInput) -> Result<Episode> {
        let row = sqlx::query_as::<_, Episode>(
            "INSERT INTO episodes (series_id, season_number, episode_number, title, monitored)
             VALUES ($1, $2, $3, $4, $5) RETURNING *",
        )
        .bind(input.series_id)
        .bind(input.season_number)
        .bind(input.episode_number)
        .bind(&input.title)
        .bind(input.monitored)
        .fetch_one(&self.pool)
        .await?;
        Ok(row)
    }

    pub async fn set_monitored(&self, id: i64, monitored: bool) -> Result<()> {
        sqlx::query("UPDATE episodes SET monitored = $1 WHERE id = $2")
            .bind(monitored)
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Update episode monitored status and return the updated episode.
    pub async fn update_monitored(&self, id: i64, monitored: bool) -> Result<Episode> {
        let row = sqlx::query_as::<_, Episode>(
            "UPDATE episodes SET monitored = $1 WHERE id = $2 RETURNING *",
        )
        .bind(monitored)
        .bind(id)
        .fetch_one(&self.pool)
        .await?;
        Ok(row)
    }

    /// Bulk update monitored status for all episodes in a season.
    pub async fn set_season_monitored(
        &self,
        series_id: i64,
        season_number: i32,
        monitored: bool,
    ) -> Result<()> {
        sqlx::query(
            "UPDATE episodes SET monitored = $1 WHERE series_id = $2 AND season_number = $3",
        )
        .bind(monitored)
        .bind(series_id)
        .bind(season_number)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Bulk update monitored status for multiple episode IDs.
    pub async fn set_bulk_monitored(&self, episode_ids: &[i64], monitored: bool) -> Result<()> {
        if episode_ids.is_empty() {
            return Ok(());
        }
        sqlx::query("UPDATE episodes SET monitored = $1 WHERE id = ANY($2)")
            .bind(monitored)
            .bind(episode_ids)
            .execute(&self.pool)
            .await?;
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
    pool: PgPool,
}

impl CalendarService {
    pub fn new(pool: PgPool) -> Self {
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
             WHERE e.air_date_utc BETWEEN $1::timestamptz AND $2::timestamptz
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
    pool: PgPool,
}

impl WantedService {
    pub fn new(pool: PgPool) -> Self {
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
                SELECT e.id, 'series'::text AS media_type, e.series_id AS media_id,
                       s.title, CONCAT('S', LPAD(e.season_number::text, 2, '0'), 'E', LPAD(e.episode_number::text, 2, '0')) AS episode_info,
                       s.quality_profile_id, e.air_date_utc::text AS air_date, e.monitored
                FROM episodes e
                JOIN series s ON e.series_id = s.id
                WHERE e.monitored = true AND s.monitored = true
                  AND e.episode_file_id IS NULL
                  AND e.air_date_utc < NOW()
                UNION ALL
                SELECT m.id, 'movie'::text AS media_type, m.id AS media_id,
                       m.title, NULL AS episode_info,
                       m.quality_profile_id, m.physical_release::text AS air_date, m.monitored
                FROM movies m
                WHERE m.monitored = true AND m.movie_file_id IS NULL
            ) combined
            ORDER BY air_date DESC NULLS LAST
            LIMIT $1 OFFSET $2",
        )
        .bind(page_size)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?;

        let records = rows
            .into_iter()
            .map(|(id, media_type, media_id, title, episode_info, quality_profile_id, air_date, monitored)| {
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
            })
            .collect();

        Ok(WantedPage {
            page,
            page_size,
            total_records,
            records,
        })
    }

    /// Cutoff unmet: items that have a file but below quality profile cutoff.
    /// Simplified stub — real cutoff comparison will come in Phase 4 with the
    /// decision engine. For now returns items with files as candidates.
    pub async fn cutoff_unmet(&self, page: i64, page_size: i64) -> Result<WantedPage> {
        let offset = (page - 1).max(0) * page_size;

        // Count episodes with files (candidates for cutoff check)
        let episode_count: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM episodes e
             JOIN series s ON e.series_id = s.id
             WHERE e.monitored = true AND s.monitored = true
               AND e.episode_file_id IS NOT NULL",
        )
        .fetch_one(&self.pool)
        .await?;

        // Count movies with files
        let movie_count: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM movies m
             WHERE m.monitored = true AND m.movie_file_id IS NOT NULL",
        )
        .fetch_one(&self.pool)
        .await?;

        let total_records = episode_count.0 + movie_count.0;

        let rows = sqlx::query_as::<_, (i64, String, i64, String, Option<String>, i32, Option<String>, bool)>(
            "SELECT * FROM (
                SELECT e.id, 'series'::text AS media_type, e.series_id AS media_id,
                       s.title, CONCAT('S', LPAD(e.season_number::text, 2, '0'), 'E', LPAD(e.episode_number::text, 2, '0')) AS episode_info,
                       s.quality_profile_id, e.air_date_utc::text AS air_date, e.monitored
                FROM episodes e
                JOIN series s ON e.series_id = s.id
                WHERE e.monitored = true AND s.monitored = true
                  AND e.episode_file_id IS NOT NULL
                UNION ALL
                SELECT m.id, 'movie'::text AS media_type, m.id AS media_id,
                       m.title, NULL AS episode_info,
                       m.quality_profile_id, m.physical_release::text AS air_date, m.monitored
                FROM movies m
                WHERE m.monitored = true AND m.movie_file_id IS NOT NULL
            ) combined
            ORDER BY air_date DESC NULLS LAST
            LIMIT $1 OFFSET $2",
        )
        .bind(page_size)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?;

        let records = rows
            .into_iter()
            .map(|(id, media_type, media_id, title, episode_info, quality_profile_id, air_date, monitored)| {
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
            })
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
    pool: PgPool,
}

impl MetadataRefreshService {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Find series that haven't been synced in over 12 hours.
    pub async fn find_stale_series(&self) -> Result<Vec<i64>> {
        let rows: Vec<(i64,)> = sqlx::query_as(
            "SELECT id FROM series
             WHERE last_info_sync IS NULL
                OR last_info_sync < NOW() - INTERVAL '12 hours'",
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
                OR last_info_sync < NOW() - INTERVAL '12 hours'",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.into_iter().map(|(id,)| id).collect())
    }

    /// Update last_info_sync timestamp for a series.
    pub async fn mark_series_synced(&self, id: i64) -> Result<()> {
        sqlx::query("UPDATE series SET last_info_sync = NOW() WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Update last_info_sync timestamp for a movie.
    pub async fn mark_movie_synced(&self, id: i64) -> Result<()> {
        sqlx::query("UPDATE movies SET last_info_sync = NOW() WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Update series metadata from TMDB data.
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
        sqlx::query(
            "UPDATE series
             SET overview = COALESCE($1, overview),
                 network = COALESCE($2, network),
                 runtime = COALESCE($3, runtime),
                 images = COALESCE($4, images),
                 genres = COALESCE($5, genres),
                 last_info_sync = NOW()
             WHERE id = $6",
        )
        .bind(overview)
        .bind(network)
        .bind(runtime)
        .bind(images)
        .bind(genres)
        .bind(id)
        .execute(&self.pool)
        .await?;
        // Note: status is not updated here because the DB column uses the
        // SeriesStatus enum type — mapping the TMDB string to that enum is
        // deferred to a later phase.
        let _ = status;
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
             SET overview = COALESCE($1, overview),
                 studio = COALESCE($2, studio),
                 images = COALESCE($3, images),
                 genres = COALESCE($4, genres),
                 last_info_sync = NOW()
             WHERE id = $5",
        )
        .bind(overview)
        .bind(studio)
        .bind(images)
        .bind(genres)
        .bind(id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use stackarr_core::test_helpers::{TestDb, seed_quality_profile, seed_root_folder, seed_series, seed_episode};

    // ── SeriesService ───────────────────────────────────────────────────

    #[tokio::test]
    async fn test_series_create_and_get() {
        let db = TestDb::new().await;
        let profile_id = seed_quality_profile(&db.pool).await;

        let svc = SeriesService::new(db.pool.clone());
        let created = svc.create(CreateSeriesInput {
            title: "Breaking Bad".into(),
            path: "/tv/Breaking Bad".into(),
            quality_profile_id: profile_id,
            monitored: true,
            tvdb_id: Some(81189),
            tmdb_id: Some(1396),
            imdb_id: Some("tt0903747".into()),
        }).await.expect("create series");

        assert_eq!(created.title, "Breaking Bad");
        assert_eq!(created.tvdb_id, Some(81189));

        let fetched = svc.get(created.id).await.expect("get series");
        assert_eq!(fetched.title, "Breaking Bad");
        assert_eq!(fetched.quality_profile_id, profile_id);

        db.cleanup().await;
    }

    #[tokio::test]
    async fn test_series_list() {
        let db = TestDb::new().await;
        let profile_id = seed_quality_profile(&db.pool).await;
        let rf = seed_root_folder(&db.pool, "/tv", "series").await;
        seed_series(&db.pool, "Alpha", profile_id, rf).await;
        seed_series(&db.pool, "Beta", profile_id, rf).await;
        seed_series(&db.pool, "Gamma", profile_id, rf).await;

        let svc = SeriesService::new(db.pool.clone());
        let list = svc.list().await.expect("list series");
        assert_eq!(list.len(), 3);

        db.cleanup().await;
    }

    #[tokio::test]
    async fn test_series_update_partial() {
        let db = TestDb::new().await;
        let profile_id = seed_quality_profile(&db.pool).await;

        let svc = SeriesService::new(db.pool.clone());
        let created = svc.create(CreateSeriesInput {
            title: "Original Title".into(),
            path: "/tv/Original".into(),
            quality_profile_id: profile_id,
            monitored: true,
            tvdb_id: None,
            tmdb_id: None,
            imdb_id: None,
        }).await.expect("create");

        let updated = svc.update(created.id, UpdateSeriesInput {
            title: Some("New Title".into()),
            path: None,
            quality_profile_id: None,
            monitored: None,
        }).await.expect("update");

        assert_eq!(updated.title, "New Title");
        assert_eq!(updated.path, "/tv/Original"); // unchanged

        db.cleanup().await;
    }

    #[tokio::test]
    async fn test_series_delete() {
        let db = TestDb::new().await;
        let profile_id = seed_quality_profile(&db.pool).await;

        let svc = SeriesService::new(db.pool.clone());
        let created = svc.create(CreateSeriesInput {
            title: "To Delete".into(),
            path: "/tv/del".into(),
            quality_profile_id: profile_id,
            monitored: false,
            tvdb_id: None,
            tmdb_id: None,
            imdb_id: None,
        }).await.expect("create");

        svc.delete(created.id).await.expect("delete");
        let result = svc.get(created.id).await;
        assert!(result.is_err());

        db.cleanup().await;
    }

    // ── MovieService ────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_movie_create_and_get() {
        let db = TestDb::new().await;
        let profile_id = seed_quality_profile(&db.pool).await;

        let svc = MovieService::new(db.pool.clone());
        let created = svc.create(CreateMovieInput {
            title: "Inception".into(),
            path: "/movies/Inception (2010)".into(),
            quality_profile_id: profile_id,
            monitored: true,
            tmdb_id: Some(27205),
            imdb_id: Some("tt1375666".into()),
            year: Some(2010),
        }).await.expect("create movie");

        assert_eq!(created.title, "Inception");

        let fetched = svc.get(created.id).await.expect("get movie");
        assert_eq!(fetched.tmdb_id, Some(27205));

        db.cleanup().await;
    }

    #[tokio::test]
    async fn test_movie_delete() {
        let db = TestDb::new().await;
        let profile_id = seed_quality_profile(&db.pool).await;

        let svc = MovieService::new(db.pool.clone());
        let created = svc.create(CreateMovieInput {
            title: "To Delete".into(),
            path: "/movies/del".into(),
            quality_profile_id: profile_id,
            monitored: false,
            tmdb_id: None,
            imdb_id: None,
            year: None,
        }).await.expect("create");

        svc.delete(created.id).await.expect("delete");
        let result = svc.get(created.id).await;
        assert!(result.is_err());

        db.cleanup().await;
    }

    // ── EpisodeService ──────────────────────────────────────────────────

    #[tokio::test]
    async fn test_episode_create_and_list() {
        let db = TestDb::new().await;
        let profile_id = seed_quality_profile(&db.pool).await;
        let rf = seed_root_folder(&db.pool, "/tv", "series").await;
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
    async fn test_episode_set_monitored() {
        let db = TestDb::new().await;
        let profile_id = seed_quality_profile(&db.pool).await;
        let rf = seed_root_folder(&db.pool, "/tv", "series").await;
        let series_id = seed_series(&db.pool, "Test", profile_id, rf).await;
        let ep_id = seed_episode(&db.pool, series_id, 1, 1).await;

        let svc = EpisodeService::new(db.pool.clone());

        // Start monitored, toggle off
        svc.set_monitored(ep_id, false).await.expect("set monitored false");
        let ep = svc.get(ep_id).await.expect("get episode");
        assert!(!ep.monitored);

        // Toggle back on
        svc.set_monitored(ep_id, true).await.expect("set monitored true");
        let ep = svc.get(ep_id).await.expect("get episode");
        assert!(ep.monitored);

        db.cleanup().await;
    }

    #[tokio::test]
    async fn test_episode_bulk_monitored() {
        let db = TestDb::new().await;
        let profile_id = seed_quality_profile(&db.pool).await;
        let rf = seed_root_folder(&db.pool, "/tv", "series").await;
        let series_id = seed_series(&db.pool, "Bulk", profile_id, rf).await;
        let ep1 = seed_episode(&db.pool, series_id, 1, 1).await;
        let ep2 = seed_episode(&db.pool, series_id, 1, 2).await;
        let ep3 = seed_episode(&db.pool, series_id, 1, 3).await;

        let svc = EpisodeService::new(db.pool.clone());
        svc.set_bulk_monitored(&[ep1, ep2, ep3], false).await.expect("bulk unmonitor");

        let episodes = svc.list_by_series(series_id).await.expect("list");
        assert!(episodes.iter().all(|e| !e.monitored));

        db.cleanup().await;
    }
}
