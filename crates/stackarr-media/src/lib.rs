use anyhow::Result;
use serde::Deserialize;
use sqlx::PgPool;

use stackarr_core::models::{Episode, Movie, Series};

// ── Input types ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateSeriesInput {
    pub title: String,
    pub path: String,
    pub quality_profile_id: i64,
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
    pub quality_profile_id: Option<i64>,
    pub monitored: Option<bool>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateMovieInput {
    pub title: String,
    pub path: String,
    pub quality_profile_id: i64,
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
    pub quality_profile_id: Option<i64>,
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
}
