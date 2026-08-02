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
    #[sqlx(json(nullable))]
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
        pool: &sqlx::MySqlPool,
        new: &NewImportCandidate,
    ) -> Result<Option<Self>, sqlx::Error> {
        let episodes = new.parsed_episodes.as_ref().map(sqlx::types::Json);
        let result = sqlx::query(
            r#"
            INSERT INTO import_candidates (
                media_library_folder_id, media_type, match_kind, discovered_path,
                file_count, total_size, parsed_title, parsed_year, parsed_season,
                parsed_episodes, data
            )
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            ON DUPLICATE KEY UPDATE id = LAST_INSERT_ID(id)
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
        .execute(pool)
        .await?;
        Self::get(pool, result.last_insert_id() as i64).await
    }

    pub async fn list_pending(
        pool: &sqlx::MySqlPool,
        media_type: Option<&str>,
        limit: i64,
    ) -> Result<Vec<Self>, sqlx::Error> {
        match media_type {
            Some(mt) => {
                sqlx::query_as::<_, Self>(
                    "SELECT * FROM import_candidates
                     WHERE status = 'pending' AND media_type = ?
                     ORDER BY confidence DESC, discovered_at DESC
                     LIMIT ?",
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
                     LIMIT ?",
                )
                .bind(limit)
                .fetch_all(pool)
                .await
            }
        }
    }

    pub async fn get(pool: &sqlx::MySqlPool, id: i64) -> Result<Option<Self>, sqlx::Error> {
        sqlx::query_as::<_, Self>("SELECT * FROM import_candidates WHERE id = ?")
            .bind(id)
            .fetch_optional(pool)
            .await
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn update_suggestion(
        pool: &sqlx::MySqlPool,
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
             SET suggested_tmdb_id = ?, suggested_title = ?, suggested_year = ?,
                 suggested_poster = ?, suggested_overview = ?, confidence = ?
             WHERE id = ?",
        )
        .bind(tmdb_id)
        .bind(title)
        .bind(year)
        .bind(poster)
        .bind(overview)
        .bind(confidence)
        .bind(id)
        .execute(pool)
        .await?;
        Ok(())
    }

    pub async fn mark_accepted(
        pool: &sqlx::MySqlPool,
        id: i64,
        target_series_id: Option<i64>,
        target_movie_id: Option<i64>,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "UPDATE import_candidates
             SET status = 'accepted', target_series_id = ?, target_movie_id = ?,
                 resolved_at = NOW()
             WHERE id = ?",
        )
        .bind(target_series_id)
        .bind(target_movie_id)
        .bind(id)
        .execute(pool)
        .await?;
        Ok(())
    }

    pub async fn mark_rejected(pool: &sqlx::MySqlPool, id: i64) -> Result<(), sqlx::Error> {
        sqlx::query(
            "UPDATE import_candidates
             SET status = 'rejected', resolved_at = NOW()
             WHERE id = ?",
        )
        .bind(id)
        .execute(pool)
        .await?;
        Ok(())
    }

    pub async fn mark_failed(
        pool: &sqlx::MySqlPool,
        id: i64,
        error: &str,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "UPDATE import_candidates
             SET status = 'failed', error = ?, resolved_at = NOW()
             WHERE id = ?",
        )
        .bind(error)
        .bind(id)
        .execute(pool)
        .await?;
        Ok(())
    }
}
