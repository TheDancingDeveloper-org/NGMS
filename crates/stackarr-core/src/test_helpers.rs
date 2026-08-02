//! Test utilities for database-backed integration tests.
//!
//! Requires a running MariaDB instance (e.g. `docker compose -f docker/docker-compose.dev.yml up -d`).
//! Set `TEST_DATABASE_URL` to override the default connection string.

use sqlx::mysql::MySqlPoolOptions;
use sqlx::{Executor, MySqlPool};

/// Default MariaDB URL matching docker-compose.dev.yml.
const DEFAULT_TEST_URL: &str = "mysql://stackarr:stackarr@127.0.0.1:3306/mysql";

fn base_url() -> String {
    std::env::var("TEST_DATABASE_URL").unwrap_or_else(|_| DEFAULT_TEST_URL.to_string())
}

/// A guard that owns a test database and drops it when the guard goes out of scope.
pub struct TestDb {
    pub pool: MySqlPool,
    pub name: String,
    base_url: String,
}

impl TestDb {
    /// Create a fresh database with a random name, run all migrations, and return
    /// a pool connected to it.
    pub async fn new() -> Self {
        let base = base_url();
        let name = format!("stackarr_test_{}", uuid::Uuid::new_v4().simple());

        // Connect to the maintenance database to create a new test database.
        let admin_pool = MySqlPoolOptions::new()
            .max_connections(2)
            .connect(&base)
            .await
            .expect("failed to connect to MariaDB admin database — is docker compose running?");

        admin_pool
            .execute(
                format!(
                    "CREATE DATABASE `{name}` CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci"
                )
                .as_str(),
            )
            .await
            .expect("failed to create test database");

        admin_pool.close().await;

        // Build connection URL for the new test database.
        let test_url = if let Some(pos) = base.rfind('/') {
            format!("{}/{name}", &base[..pos])
        } else {
            panic!("TEST_DATABASE_URL must include a database name component");
        };

        let pool = MySqlPoolOptions::new()
            .max_connections(5)
            .connect(&test_url)
            .await
            .expect("failed to connect to test database");

        // Run migrations.
        sqlx::migrate!("../../migrations")
            .run(&pool)
            .await
            .expect("migration failed on test database");

        Self {
            pool,
            name,
            base_url: base,
        }
    }
}

impl TestDb {
    /// Explicitly drop the test database. Call this at the end of your test
    /// instead of relying on `Drop` for reliable cleanup.
    pub async fn cleanup(self) {
        let base = self.base_url.clone();
        let name = self.name.clone();
        self.pool.close().await;
        if let Ok(admin) = MySqlPoolOptions::new()
            .max_connections(2)
            .connect(&base)
            .await
        {
            let _ = admin
                .execute(format!("DROP DATABASE IF EXISTS `{name}`").as_str())
                .await;
            admin.close().await;
        }
    }
}

// ---------------------------------------------------------------------------
// Seed helpers — insert minimal rows so foreign-key dependent tests work.
// ---------------------------------------------------------------------------

/// Insert a minimal quality profile and return its id.
pub async fn seed_quality_profile(pool: &MySqlPool) -> i32 {
    let result = sqlx::query(
        "INSERT INTO quality_profiles (name, cutoff, upgrade_allowed, min_format_score, cutoff_format_score, items)
         VALUES ('Test Profile', 6, true, 0, 0, JSON_ARRAY())",
    )
    .execute(pool)
    .await
    .expect("seed quality profile");
    result.last_insert_id() as i32
}

/// Insert a media library folder and return its id.
pub async fn seed_media_library_folder(pool: &MySqlPool, path: &str, media_type: &str) -> i32 {
    let result = sqlx::query("INSERT INTO media_library_folders (path, media_type) VALUES (?, ?)")
        .bind(path)
        .bind(media_type)
        .execute(pool)
        .await
        .expect("seed media library folder");
    result.last_insert_id() as i32
}

/// Insert a test series and return its id.
pub async fn seed_series(
    pool: &MySqlPool,
    title: &str,
    profile_id: i32,
    media_library_folder_id: i32,
) -> i64 {
    let clean = title.to_lowercase().replace(' ', "");
    let result = sqlx::query(
        "INSERT INTO series (title, clean_title, sort_title, path, quality_profile_id, media_library_folder_id, monitored)
         VALUES (?, ?, ?, ?, ?, ?, true)",
    )
    .bind(title)
    .bind(&clean)
    .bind(&clean)
    .bind(format!("/tv/{title}"))
    .bind(profile_id)
    .bind(media_library_folder_id)
    .execute(pool)
    .await
    .expect("seed series");
    result.last_insert_id() as i64
}

/// Insert a test movie and return its id.
pub async fn seed_movie(
    pool: &MySqlPool,
    title: &str,
    profile_id: i32,
    media_library_folder_id: i32,
) -> i64 {
    let clean = title.to_lowercase().replace(' ', "");
    let result = sqlx::query(
        "INSERT INTO movies (title, clean_title, sort_title, path, quality_profile_id, media_library_folder_id, monitored, minimum_availability)
         VALUES (?, ?, ?, ?, ?, ?, true, 'released')",
    )
    .bind(title)
    .bind(&clean)
    .bind(&clean)
    .bind(format!("/movies/{title}"))
    .bind(profile_id)
    .bind(media_library_folder_id)
    .execute(pool)
    .await
    .expect("seed movie");
    result.last_insert_id() as i64
}

/// Insert a test episode and return its id.
pub async fn seed_episode(pool: &MySqlPool, series_id: i64, season: i32, episode: i32) -> i64 {
    let result = sqlx::query(
        "INSERT INTO episodes (series_id, season_number, episode_number, title, monitored)
         VALUES (?, ?, ?, ?, true)",
    )
    .bind(series_id)
    .bind(season)
    .bind(episode)
    .bind(format!("Episode {episode}"))
    .execute(pool)
    .await
    .expect("seed episode");
    result.last_insert_id() as i64
}
