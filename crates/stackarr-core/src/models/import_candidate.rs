use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct ImportCandidate {
    pub id: i64,
    pub media_library_folder_id: Option<i32>,
    pub media_type: String,
    pub match_kind: String,
    pub discovered_path: String,
    pub file_count: i32,
    pub total_size: i64,
    pub parsed_title: Option<String>,
    pub parsed_year: Option<i32>,
    pub parsed_season: Option<i32>,
    pub parsed_episodes: Option<Vec<i32>>,
    pub suggested_tmdb_id: Option<i32>,
    pub suggested_title: Option<String>,
    pub suggested_year: Option<i32>,
    pub suggested_poster: Option<String>,
    pub suggested_overview: Option<String>,
    pub confidence: f32,
    pub status: String,
    pub target_series_id: Option<i64>,
    pub target_movie_id: Option<i64>,
    pub error: Option<String>,
    pub data: serde_json::Value,
    pub discovered_at: DateTime<Utc>,
    pub resolved_at: Option<DateTime<Utc>>,
}

/// Parameters for inserting a new import candidate.
///
/// Most fields are optional because the scanner may only know the path +
/// parsed title at the time it creates the row; the TMDB match pass fills
/// in `suggested_*` later.
#[derive(Debug, Clone, Default)]
pub struct NewImportCandidate {
    pub media_library_folder_id: Option<i32>,
    pub media_type: String,
    pub match_kind: String,
    pub discovered_path: String,
    pub file_count: i32,
    pub total_size: i64,
    pub parsed_title: Option<String>,
    pub parsed_year: Option<i32>,
    pub parsed_season: Option<i32>,
    pub parsed_episodes: Option<Vec<i32>>,
    pub data: serde_json::Value,
}

impl ImportCandidate {
    /// Insert a new pending candidate. Uses the partial unique index on
    /// `(discovered_path) WHERE status = 'pending'` to dedupe across scans.
    /// Returns `Ok(None)` if a pending row already exists for that path.
    pub async fn insert_pending(
        pool: &sqlx::PgPool,
        new: &NewImportCandidate,
    ) -> Result<Option<Self>, sqlx::Error> {
        let episodes: Option<Vec<i32>> = new.parsed_episodes.clone();
        let row: Option<Self> = sqlx::query_as(
            r#"
            INSERT INTO import_candidates (
                media_library_folder_id, media_type, match_kind, discovered_path,
                file_count, total_size, parsed_title, parsed_year, parsed_season,
                parsed_episodes, data
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
            ON CONFLICT (discovered_path) WHERE status = 'pending' DO NOTHING
            RETURNING *
            "#,
        )
        .bind(new.media_library_folder_id)
        .bind(&new.media_type)
        .bind(&new.match_kind)
        .bind(&new.discovered_path)
        .bind(new.file_count)
        .bind(new.total_size)
        .bind(&new.parsed_title)
        .bind(new.parsed_year)
        .bind(new.parsed_season)
        .bind(episodes)
        .bind(&new.data)
        .fetch_optional(pool)
        .await?;
        Ok(row)
    }

    pub async fn list_pending(
        pool: &sqlx::PgPool,
        media_type: Option<&str>,
        limit: i64,
    ) -> Result<Vec<Self>, sqlx::Error> {
        match media_type {
            Some(mt) => {
                sqlx::query_as::<_, Self>(
                    "SELECT * FROM import_candidates
                     WHERE status = 'pending' AND media_type = $1
                     ORDER BY confidence DESC, discovered_at DESC
                     LIMIT $2",
                )
                .bind(mt)
                .bind(limit)
                .fetch_all(pool)
                .await
            }
            None => {
                sqlx::query_as::<_, Self>(
                    "SELECT * FROM import_candidates
                     WHERE status = 'pending'
                     ORDER BY confidence DESC, discovered_at DESC
                     LIMIT $1",
                )
                .bind(limit)
                .fetch_all(pool)
                .await
            }
        }
    }

    pub async fn get(pool: &sqlx::PgPool, id: i64) -> Result<Option<Self>, sqlx::Error> {
        sqlx::query_as::<_, Self>("SELECT * FROM import_candidates WHERE id = $1")
            .bind(id)
            .fetch_optional(pool)
            .await
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn update_suggestion(
        pool: &sqlx::PgPool,
        id: i64,
        tmdb_id: Option<i32>,
        title: Option<&str>,
        year: Option<i32>,
        poster: Option<&str>,
        overview: Option<&str>,
        confidence: f32,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "UPDATE import_candidates
             SET suggested_tmdb_id = $2, suggested_title = $3, suggested_year = $4,
                 suggested_poster = $5, suggested_overview = $6, confidence = $7
             WHERE id = $1",
        )
        .bind(id)
        .bind(tmdb_id)
        .bind(title)
        .bind(year)
        .bind(poster)
        .bind(overview)
        .bind(confidence)
        .execute(pool)
        .await?;
        Ok(())
    }

    pub async fn mark_accepted(
        pool: &sqlx::PgPool,
        id: i64,
        target_series_id: Option<i64>,
        target_movie_id: Option<i64>,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "UPDATE import_candidates
             SET status = 'accepted', target_series_id = $2, target_movie_id = $3,
                 resolved_at = NOW()
             WHERE id = $1",
        )
        .bind(id)
        .bind(target_series_id)
        .bind(target_movie_id)
        .execute(pool)
        .await?;
        Ok(())
    }

    pub async fn mark_rejected(pool: &sqlx::PgPool, id: i64) -> Result<(), sqlx::Error> {
        sqlx::query(
            "UPDATE import_candidates
             SET status = 'rejected', resolved_at = NOW()
             WHERE id = $1",
        )
        .bind(id)
        .execute(pool)
        .await?;
        Ok(())
    }

    pub async fn mark_failed(pool: &sqlx::PgPool, id: i64, error: &str) -> Result<(), sqlx::Error> {
        sqlx::query(
            "UPDATE import_candidates
             SET status = 'failed', error = $2, resolved_at = NOW()
             WHERE id = $1",
        )
        .bind(id)
        .bind(error)
        .execute(pool)
        .await?;
        Ok(())
    }
}
