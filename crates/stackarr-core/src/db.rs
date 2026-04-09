use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::PgPool;
use sqlx::postgres::PgPoolOptions;
use uuid::Uuid;

use crate::config::{DatabaseConfig, EnabledModules};
use crate::models::user::{
    ContinueWatchingItem, Invite, MediaRequest, PushSubscription, SystemActivity, User, UserDevice,
    UserNotification, UserRating, UserSession, UserWatchlistItem, WatchProgress,
};

#[derive(Clone)]
pub struct Database {
    pool: PgPool,
}

impl Database {
    pub async fn connect(config: &DatabaseConfig) -> crate::Result<Self> {
        let pool = PgPoolOptions::new()
            .max_connections(config.max_connections)
            .connect(&config.url)
            .await?;
        Ok(Self { pool })
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    /// Create a Database wrapper from an existing pool.
    pub fn from_pool(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn run_migrations(&self) -> crate::Result<()> {
        sqlx::migrate!("../../migrations")
            .run(&self.pool)
            .await
            .map_err(|e| crate::Error::Config(format!("migration failed: {e}")))?;
        Ok(())
    }

    pub async fn is_first_boot(&self) -> crate::Result<bool> {
        let result: (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM enabled_modules WHERE enabled = true")
                .fetch_one(&self.pool)
                .await?;
        Ok(result.0 == 0)
    }

    pub async fn load_enabled_modules(&self) -> crate::Result<EnabledModules> {
        let rows: Vec<(String, bool)> =
            sqlx::query_as("SELECT module, enabled FROM enabled_modules")
                .fetch_all(&self.pool)
                .await?;

        let mut modules = EnabledModules::default();
        for (module, enabled) in rows {
            match module.as_str() {
                "tv_management" => modules.tv_management = enabled,
                "movie_management" => modules.movie_management = enabled,
                "torrent_embedded" => modules.torrent_embedded = enabled,
                "usenet_embedded" => modules.usenet_embedded = enabled,
                "torrent_external" => modules.torrent_external = enabled,
                "usenet_external" => modules.usenet_external = enabled,
                "indexarr_sidecar" => modules.indexarr_sidecar = enabled,
                "external_indexers" => modules.external_indexers = enabled,
                "plex_integration" => modules.plex_integration = enabled,
                "notifications" => modules.notifications = enabled,
                "streaming" => modules.streaming = enabled,
                "remote_access" => modules.remote_access = enabled,
                "stremio_addon" => modules.stremio_addon = enabled,
                _ => {}
            }
        }
        Ok(modules)
    }

    pub async fn save_enabled_modules(&self, modules: &EnabledModules) -> crate::Result<()> {
        let module_list = [
            ("tv_management", modules.tv_management),
            ("movie_management", modules.movie_management),
            ("torrent_embedded", modules.torrent_embedded),
            ("usenet_embedded", modules.usenet_embedded),
            ("torrent_external", modules.torrent_external),
            ("usenet_external", modules.usenet_external),
            ("indexarr_sidecar", modules.indexarr_sidecar),
            ("external_indexers", modules.external_indexers),
            ("plex_integration", modules.plex_integration),
            ("notifications", modules.notifications),
            ("streaming", modules.streaming),
            ("remote_access", modules.remote_access),
            ("stremio_addon", modules.stremio_addon),
        ];

        for (name, enabled) in module_list {
            sqlx::query(
                "INSERT INTO enabled_modules (module, enabled) VALUES ($1, $2)
                 ON CONFLICT (module) DO UPDATE SET enabled = $2",
            )
            .bind(name)
            .bind(enabled)
            .execute(&self.pool)
            .await?;
        }
        Ok(())
    }

    // ── Server identity ─────────────────────────────────────────────────

    /// Load or generate the stable server identity UUID.
    pub async fn ensure_server_id(&self) -> crate::Result<Uuid> {
        let existing: Option<serde_json::Value> =
            sqlx::query_scalar("SELECT value FROM app_config WHERE key = 'server_id'")
                .fetch_optional(&self.pool)
                .await?;

        match existing {
            Some(val) => {
                let id_str = val
                    .as_str()
                    .ok_or_else(|| crate::Error::Config("server_id is not a string".into()))?;
                Uuid::parse_str(id_str)
                    .map_err(|e| crate::Error::Config(format!("invalid server_id: {e}")))
            }
            None => {
                let new_id = Uuid::new_v4();
                sqlx::query("INSERT INTO app_config (key, value) VALUES ('server_id', $1)")
                    .bind(serde_json::Value::String(new_id.to_string()))
                    .execute(&self.pool)
                    .await?;
                Ok(new_id)
            }
        }
    }

    // ── Remote clients ──────────────────────────────────────────────────

    pub async fn create_remote_client(&self, client_token: Uuid) -> crate::Result<i32> {
        let row: (i32,) =
            sqlx::query_as("INSERT INTO remote_clients (client_token) VALUES ($1) RETURNING id")
                .bind(client_token)
                .fetch_one(&self.pool)
                .await?;
        Ok(row.0)
    }

    pub async fn set_remote_client_name(
        &self,
        client_token: Uuid,
        name: &str,
    ) -> crate::Result<bool> {
        let result = sqlx::query(
            "UPDATE remote_clients SET client_name = $1, last_seen = NOW() \
             WHERE client_token = $2 AND revoked = false",
        )
        .bind(name)
        .bind(client_token)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn validate_remote_client(&self, client_token: Uuid) -> crate::Result<bool> {
        let row: Option<(bool,)> =
            sqlx::query_as("SELECT revoked FROM remote_clients WHERE client_token = $1")
                .bind(client_token)
                .fetch_optional(&self.pool)
                .await?;
        match row {
            Some((revoked,)) => Ok(!revoked),
            None => Ok(false),
        }
    }

    pub async fn touch_remote_client(&self, client_token: Uuid) -> crate::Result<()> {
        sqlx::query("UPDATE remote_clients SET last_seen = NOW() WHERE client_token = $1")
            .bind(client_token)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn list_remote_clients(&self) -> crate::Result<Vec<RemoteClient>> {
        let rows = sqlx::query_as::<_, RemoteClient>(
            "SELECT id, client_token, client_name, created_at, last_seen, revoked \
             FROM remote_clients ORDER BY created_at DESC",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    pub async fn revoke_remote_client(&self, id: i32) -> crate::Result<bool> {
        let result = sqlx::query("UPDATE remote_clients SET revoked = true WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn delete_remote_client(&self, id: i32) -> crate::Result<bool> {
        let result = sqlx::query("DELETE FROM remote_clients WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }

    // ── Users ────────────────────────────────────────────────────────────

    pub async fn create_user(
        &self,
        username: &str,
        display_name: &str,
        password_hash: &str,
        role: &str,
    ) -> crate::Result<User> {
        let user = sqlx::query_as::<_, User>(
            "INSERT INTO users (username, display_name, password_hash, role) \
             VALUES ($1, $2, $3, $4) RETURNING *",
        )
        .bind(username)
        .bind(display_name)
        .bind(password_hash)
        .bind(role)
        .fetch_one(&self.pool)
        .await?;
        Ok(user)
    }

    pub async fn get_user_by_id(&self, id: i64) -> crate::Result<Option<User>> {
        let user = sqlx::query_as::<_, User>("SELECT * FROM users WHERE id = $1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await?;
        Ok(user)
    }

    pub async fn get_user_by_username(&self, username: &str) -> crate::Result<Option<User>> {
        let user = sqlx::query_as::<_, User>("SELECT * FROM users WHERE username = $1")
            .bind(username)
            .fetch_optional(&self.pool)
            .await?;
        Ok(user)
    }

    pub async fn list_users(&self) -> crate::Result<Vec<User>> {
        let users = sqlx::query_as::<_, User>("SELECT * FROM users ORDER BY created_at DESC")
            .fetch_all(&self.pool)
            .await?;
        Ok(users)
    }

    pub async fn update_user(
        &self,
        id: i64,
        display_name: &str,
        role: &str,
        enabled: bool,
        avatar_url: Option<&str>,
    ) -> crate::Result<Option<User>> {
        let user = sqlx::query_as::<_, User>(
            "UPDATE users SET display_name = $1, role = $2, enabled = $3, avatar_url = $4, \
             updated_at = NOW() WHERE id = $5 RETURNING *",
        )
        .bind(display_name)
        .bind(role)
        .bind(enabled)
        .bind(avatar_url)
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(user)
    }

    pub async fn update_user_password(&self, id: i64, password_hash: &str) -> crate::Result<bool> {
        let result =
            sqlx::query("UPDATE users SET password_hash = $1, updated_at = NOW() WHERE id = $2")
                .bind(password_hash)
                .bind(id)
                .execute(&self.pool)
                .await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn delete_user(&self, id: i64) -> crate::Result<bool> {
        let result = sqlx::query("DELETE FROM users WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn count_users(&self) -> crate::Result<i64> {
        let row: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM users")
            .fetch_one(&self.pool)
            .await?;
        Ok(row.0)
    }

    // ── Sessions ─────────────────────────────────────────────────────────

    pub async fn create_session(
        &self,
        user_id: i64,
        token_hash: &str,
        user_agent: Option<&str>,
        ip_address: Option<&str>,
        expires_at: DateTime<Utc>,
    ) -> crate::Result<UserSession> {
        let session = sqlx::query_as::<_, UserSession>(
            "INSERT INTO user_sessions (user_id, token_hash, user_agent, ip_address, expires_at) \
             VALUES ($1, $2, $3, $4::INET, $5) RETURNING id, user_id, token_hash, user_agent, \
             ip_address::TEXT, created_at, expires_at, last_active",
        )
        .bind(user_id)
        .bind(token_hash)
        .bind(user_agent)
        .bind(ip_address)
        .bind(expires_at)
        .fetch_one(&self.pool)
        .await?;
        Ok(session)
    }

    /// Validate a session token hash and return the associated user if valid.
    /// Checks that the session is not expired and the user is enabled.
    pub async fn validate_session(&self, token_hash: &str) -> crate::Result<Option<User>> {
        let user = sqlx::query_as::<_, User>(
            "SELECT u.* FROM users u \
             INNER JOIN user_sessions s ON s.user_id = u.id \
             WHERE s.token_hash = $1 AND s.expires_at > NOW() AND u.enabled = true",
        )
        .bind(token_hash)
        .fetch_optional(&self.pool)
        .await?;
        Ok(user)
    }

    pub async fn touch_session(&self, token_hash: &str) -> crate::Result<()> {
        sqlx::query("UPDATE user_sessions SET last_active = NOW() WHERE token_hash = $1")
            .bind(token_hash)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn delete_session(&self, token_hash: &str) -> crate::Result<bool> {
        let result = sqlx::query("DELETE FROM user_sessions WHERE token_hash = $1")
            .bind(token_hash)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn delete_all_sessions(&self, user_id: i64) -> crate::Result<u64> {
        let result = sqlx::query("DELETE FROM user_sessions WHERE user_id = $1")
            .bind(user_id)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected())
    }

    pub async fn list_sessions(&self, user_id: i64) -> crate::Result<Vec<UserSession>> {
        let sessions = sqlx::query_as::<_, UserSession>(
            "SELECT id, user_id, token_hash, user_agent, ip_address::TEXT, \
             created_at, expires_at, last_active \
             FROM user_sessions WHERE user_id = $1 ORDER BY last_active DESC",
        )
        .bind(user_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(sessions)
    }

    pub async fn cleanup_expired_sessions(&self) -> crate::Result<u64> {
        let result = sqlx::query("DELETE FROM user_sessions WHERE expires_at <= NOW()")
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected())
    }

    // ── User devices ─────────────────────────────────────────────────────

    pub async fn create_user_device(
        &self,
        user_id: i64,
        device_token: Uuid,
        device_name: Option<&str>,
        device_type: Option<&str>,
    ) -> crate::Result<UserDevice> {
        let device = sqlx::query_as::<_, UserDevice>(
            "INSERT INTO user_devices (user_id, device_token, device_name, device_type) \
             VALUES ($1, $2, $3, $4) RETURNING *",
        )
        .bind(user_id)
        .bind(device_token)
        .bind(device_name)
        .bind(device_type)
        .fetch_one(&self.pool)
        .await?;
        Ok(device)
    }

    /// Validate a device token and return the associated user if valid.
    pub async fn validate_user_device(&self, device_token: Uuid) -> crate::Result<Option<User>> {
        let user = sqlx::query_as::<_, User>(
            "SELECT u.* FROM users u \
             INNER JOIN user_devices d ON d.user_id = u.id \
             WHERE d.device_token = $1 AND d.revoked = false AND u.enabled = true",
        )
        .bind(device_token)
        .fetch_optional(&self.pool)
        .await?;
        Ok(user)
    }

    pub async fn touch_user_device(&self, device_token: Uuid) -> crate::Result<()> {
        sqlx::query("UPDATE user_devices SET last_seen = NOW() WHERE device_token = $1")
            .bind(device_token)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn list_user_devices(&self, user_id: i64) -> crate::Result<Vec<UserDevice>> {
        let devices = sqlx::query_as::<_, UserDevice>(
            "SELECT * FROM user_devices WHERE user_id = $1 ORDER BY created_at DESC",
        )
        .bind(user_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(devices)
    }

    pub async fn revoke_user_device(&self, id: i32) -> crate::Result<bool> {
        let result = sqlx::query("UPDATE user_devices SET revoked = true WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn delete_user_device(&self, id: i32) -> crate::Result<bool> {
        let result = sqlx::query("DELETE FROM user_devices WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn link_device_to_user(
        &self,
        device_token: Uuid,
        user_id: i64,
    ) -> crate::Result<bool> {
        let result = sqlx::query("UPDATE user_devices SET user_id = $1 WHERE device_token = $2")
            .bind(user_id)
            .bind(device_token)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }

    // ── Invites ──────────────────────────────────────────────────────────

    pub async fn create_invite(
        &self,
        code: &str,
        created_by: i64,
        role: &str,
        expires_at: Option<DateTime<Utc>>,
    ) -> crate::Result<Invite> {
        let invite = sqlx::query_as::<_, Invite>(
            "INSERT INTO invites (code, created_by, role, expires_at) \
             VALUES ($1, $2, $3, $4) RETURNING *",
        )
        .bind(code)
        .bind(created_by)
        .bind(role)
        .bind(expires_at)
        .fetch_one(&self.pool)
        .await?;
        Ok(invite)
    }

    /// Validate an invite code. Returns the invite if it's unclaimed and not expired.
    pub async fn validate_invite(&self, code: &str) -> crate::Result<Option<Invite>> {
        let invite = sqlx::query_as::<_, Invite>(
            "SELECT * FROM invites WHERE code = $1 AND claimed_by IS NULL \
             AND (expires_at IS NULL OR expires_at > NOW())",
        )
        .bind(code)
        .fetch_optional(&self.pool)
        .await?;
        Ok(invite)
    }

    pub async fn claim_invite(&self, code: &str, user_id: i64) -> crate::Result<bool> {
        let result = sqlx::query(
            "UPDATE invites SET claimed_by = $1 WHERE code = $2 AND claimed_by IS NULL",
        )
        .bind(user_id)
        .bind(code)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn list_invites(&self) -> crate::Result<Vec<Invite>> {
        let invites = sqlx::query_as::<_, Invite>("SELECT * FROM invites ORDER BY created_at DESC")
            .fetch_all(&self.pool)
            .await?;
        Ok(invites)
    }

    pub async fn delete_invite(&self, id: i32) -> crate::Result<bool> {
        let result = sqlx::query("DELETE FROM invites WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }

    // ── Migration helpers ────────────────────────────────────────────────

    // ── Watch progress ─────────────────────────────────────────────────

    #[allow(clippy::too_many_arguments)]
    pub async fn upsert_watch_progress(
        &self,
        user_id: i64,
        media_file_id: i64,
        media_type: &str,
        media_id: i64,
        episode_id: Option<i64>,
        position_secs: f32,
        duration_secs: f32,
        completed: bool,
    ) -> crate::Result<WatchProgress> {
        let row = sqlx::query_as::<_, WatchProgress>(
            "INSERT INTO watch_progress (user_id, media_file_id, media_type, media_id, episode_id, \
             position_secs, duration_secs, completed, updated_at) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, NOW()) \
             ON CONFLICT (user_id, media_file_id) DO UPDATE SET \
             position_secs = $6, duration_secs = $7, completed = $8, updated_at = NOW() \
             RETURNING *",
        )
        .bind(user_id)
        .bind(media_file_id)
        .bind(media_type)
        .bind(media_id)
        .bind(episode_id)
        .bind(position_secs)
        .bind(duration_secs)
        .bind(completed)
        .fetch_one(&self.pool)
        .await?;
        Ok(row)
    }

    pub async fn get_watch_progress(
        &self,
        user_id: i64,
        media_file_id: i64,
    ) -> crate::Result<Option<WatchProgress>> {
        let row = sqlx::query_as::<_, WatchProgress>(
            "SELECT * FROM watch_progress WHERE user_id = $1 AND media_file_id = $2",
        )
        .bind(user_id)
        .bind(media_file_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row)
    }

    pub async fn get_continue_watching(
        &self,
        user_id: i64,
        limit: i64,
    ) -> crate::Result<Vec<WatchProgress>> {
        let rows = sqlx::query_as::<_, WatchProgress>(
            "SELECT * FROM watch_progress \
             WHERE user_id = $1 AND completed = false AND position_secs > 0 \
             ORDER BY updated_at DESC LIMIT $2",
        )
        .bind(user_id)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    pub async fn get_series_progress(
        &self,
        user_id: i64,
        series_id: i64,
    ) -> crate::Result<Vec<WatchProgress>> {
        let rows = sqlx::query_as::<_, WatchProgress>(
            "SELECT * FROM watch_progress \
             WHERE user_id = $1 AND media_type = 'series' AND media_id = $2 \
             ORDER BY updated_at DESC",
        )
        .bind(user_id)
        .bind(series_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    pub async fn get_movie_progress(
        &self,
        user_id: i64,
        movie_id: i64,
    ) -> crate::Result<Option<WatchProgress>> {
        let row = sqlx::query_as::<_, WatchProgress>(
            "SELECT * FROM watch_progress \
             WHERE user_id = $1 AND media_type = 'movie' AND media_id = $2 \
             ORDER BY updated_at DESC LIMIT 1",
        )
        .bind(user_id)
        .bind(movie_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row)
    }

    pub async fn delete_watch_progress(
        &self,
        user_id: i64,
        media_file_id: i64,
    ) -> crate::Result<bool> {
        let result =
            sqlx::query("DELETE FROM watch_progress WHERE user_id = $1 AND media_file_id = $2")
                .bind(user_id)
                .bind(media_file_id)
                .execute(&self.pool)
                .await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn mark_series_watched(&self, user_id: i64, series_id: i64) -> crate::Result<u64> {
        let result = sqlx::query(
            "UPDATE watch_progress SET completed = true, updated_at = NOW() \
             WHERE user_id = $1 AND media_type = 'series' AND media_id = $2",
        )
        .bind(user_id)
        .bind(series_id)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected())
    }

    /// Resolve a media_file_id to its media_type, media_id, and episode_id.
    pub async fn resolve_media_file(
        &self,
        media_file_id: i64,
    ) -> crate::Result<Option<(String, i64, Option<i64>)>> {
        let row: Option<(String, i64, Option<i64>)> = sqlx::query_as(
            "SELECT \
               CASE WHEN ef.episode_id IS NOT NULL THEN 'series' ELSE 'movie' END AS media_type, \
               COALESCE(e.series_id, mf_movie.movie_id, 0) AS media_id, \
               ef.episode_id \
             FROM media_files mf \
             LEFT JOIN episode_files ef ON ef.media_file_id = mf.id \
             LEFT JOIN episodes e ON e.id = ef.episode_id \
             LEFT JOIN ( \
               SELECT m.id AS movie_id, m.movie_file_id FROM movies m WHERE m.movie_file_id IS NOT NULL \
             ) mf_movie ON mf_movie.movie_file_id = mf.id \
             WHERE mf.id = $1 \
             LIMIT 1",
        )
        .bind(media_file_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row)
    }

    /// Get enriched continue-watching items with media metadata via JOINs.
    pub async fn get_continue_watching_enriched(
        &self,
        user_id: i64,
        limit: i64,
    ) -> crate::Result<Vec<ContinueWatchingItem>> {
        // Fetch base progress rows plus joined metadata.
        // LEFT JOIN to series, movies, and episodes to enrich.
        let rows: Vec<ContinueWatchingItem> = sqlx::query_as::<_, ContinueWatchingRow>(
            "SELECT wp.id, wp.user_id, wp.media_file_id, wp.media_type, wp.media_id, \
                    wp.episode_id, wp.position_secs, wp.duration_secs, wp.completed, wp.updated_at, \
                    COALESCE(s.title, m.title) AS title, \
                    s.images AS series_images, \
                    m.images AS movie_images, \
                    e.title AS episode_title, \
                    e.season_number, \
                    e.episode_number, \
                    COALESCE(s.year, m.year) AS year \
             FROM watch_progress wp \
             LEFT JOIN series s ON wp.media_type = 'series' AND s.id = wp.media_id \
             LEFT JOIN movies m ON wp.media_type = 'movie' AND m.id = wp.media_id \
             LEFT JOIN episodes e ON e.id = wp.episode_id \
             WHERE wp.user_id = $1 AND wp.completed = false AND wp.position_secs > 0 \
             ORDER BY wp.updated_at DESC LIMIT $2",
        )
        .bind(user_id)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?
        .into_iter()
        .map(|r| {
            let images = r.series_images.as_ref().or(r.movie_images.as_ref());
            let poster_url = extract_image_url_from_json(images, "poster");
            let backdrop_url = extract_image_url_from_json(images, "fanart");
            ContinueWatchingItem {
                id: r.id,
                user_id: r.user_id,
                media_file_id: r.media_file_id,
                media_type: r.media_type,
                media_id: r.media_id,
                episode_id: r.episode_id,
                position_secs: r.position_secs,
                duration_secs: r.duration_secs,
                completed: r.completed,
                updated_at: r.updated_at,
                title: r.title,
                poster_url,
                backdrop_url,
                episode_title: r.episode_title,
                season_number: r.season_number,
                episode_number: r.episode_number,
                year: r.year,
            }
        })
        .collect();
        Ok(rows)
    }

    // ── Media requests ─────────────────────────────────────────────

    #[allow(clippy::too_many_arguments)]
    pub async fn create_media_request(
        &self,
        user_id: i64,
        media_type: &str,
        tmdb_id: i64,
        title: &str,
        year: Option<i32>,
        poster_url: Option<&str>,
        overview: Option<&str>,
    ) -> crate::Result<MediaRequest> {
        let row = sqlx::query_as::<_, MediaRequest>(
            "INSERT INTO media_requests (user_id, media_type, tmdb_id, title, year, poster_url, overview) \
             VALUES ($1, $2, $3, $4, $5, $6, $7) RETURNING *",
        )
        .bind(user_id)
        .bind(media_type)
        .bind(tmdb_id)
        .bind(title)
        .bind(year)
        .bind(poster_url)
        .bind(overview)
        .fetch_one(&self.pool)
        .await?;
        Ok(row)
    }

    pub async fn get_media_request(&self, id: i64) -> crate::Result<Option<MediaRequest>> {
        let row = sqlx::query_as::<_, MediaRequest>("SELECT * FROM media_requests WHERE id = $1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await?;
        Ok(row)
    }

    pub async fn list_media_requests(
        &self,
        status: Option<&str>,
        user_id: Option<i64>,
    ) -> crate::Result<Vec<MediaRequest>> {
        let rows = match (status, user_id) {
            (Some(s), Some(uid)) => {
                sqlx::query_as::<_, MediaRequest>(
                    "SELECT * FROM media_requests WHERE status = $1 AND user_id = $2 ORDER BY created_at DESC",
                )
                .bind(s)
                .bind(uid)
                .fetch_all(&self.pool)
                .await?
            }
            (Some(s), None) => {
                sqlx::query_as::<_, MediaRequest>(
                    "SELECT * FROM media_requests WHERE status = $1 ORDER BY created_at DESC",
                )
                .bind(s)
                .fetch_all(&self.pool)
                .await?
            }
            (None, Some(uid)) => {
                sqlx::query_as::<_, MediaRequest>(
                    "SELECT * FROM media_requests WHERE user_id = $1 ORDER BY created_at DESC",
                )
                .bind(uid)
                .fetch_all(&self.pool)
                .await?
            }
            (None, None) => {
                sqlx::query_as::<_, MediaRequest>(
                    "SELECT * FROM media_requests ORDER BY created_at DESC",
                )
                .fetch_all(&self.pool)
                .await?
            }
        };
        Ok(rows)
    }

    pub async fn update_request_status(
        &self,
        id: i64,
        status: &str,
        approved_by: Option<i64>,
        admin_note: Option<&str>,
    ) -> crate::Result<bool> {
        let result = sqlx::query(
            "UPDATE media_requests SET status = $1, approved_by = $2, admin_note = $3, \
             updated_at = NOW() WHERE id = $4",
        )
        .bind(status)
        .bind(approved_by)
        .bind(admin_note)
        .bind(id)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn delete_media_request(&self, id: i64) -> crate::Result<bool> {
        let result = sqlx::query("DELETE FROM media_requests WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn check_request_exists(
        &self,
        tmdb_id: i64,
        media_type: &str,
    ) -> crate::Result<Option<MediaRequest>> {
        let row = sqlx::query_as::<_, MediaRequest>(
            "SELECT * FROM media_requests WHERE tmdb_id = $1 AND media_type = $2",
        )
        .bind(tmdb_id)
        .bind(media_type)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row)
    }

    pub async fn mark_request_available(
        &self,
        tmdb_id: i64,
        media_type: &str,
    ) -> crate::Result<bool> {
        let result = sqlx::query(
            "UPDATE media_requests SET status = 'available', updated_at = NOW() \
             WHERE tmdb_id = $1 AND media_type = $2 AND status != 'available'",
        )
        .bind(tmdb_id)
        .bind(media_type)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn count_pending_requests(&self) -> crate::Result<i64> {
        let row: (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM media_requests WHERE status = 'pending'")
                .fetch_one(&self.pool)
                .await?;
        Ok(row.0)
    }

    // ── Watchlist ─────────────────────────────────────────────────

    pub async fn add_to_watchlist(
        &self,
        user_id: i64,
        media_type: &str,
        media_id: i64,
        tmdb_id: i64,
    ) -> crate::Result<UserWatchlistItem> {
        let row = sqlx::query_as::<_, UserWatchlistItem>(
            "INSERT INTO user_watchlist (user_id, media_type, media_id, tmdb_id) \
             VALUES ($1, $2, $3, $4) \
             ON CONFLICT (user_id, media_type, media_id) DO UPDATE SET added_at = NOW() \
             RETURNING *",
        )
        .bind(user_id)
        .bind(media_type)
        .bind(media_id)
        .bind(tmdb_id)
        .fetch_one(&self.pool)
        .await?;
        Ok(row)
    }

    pub async fn remove_from_watchlist(
        &self,
        user_id: i64,
        media_type: &str,
        media_id: i64,
    ) -> crate::Result<bool> {
        let result = sqlx::query(
            "DELETE FROM user_watchlist WHERE user_id = $1 AND media_type = $2 AND media_id = $3",
        )
        .bind(user_id)
        .bind(media_type)
        .bind(media_id)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn get_watchlist(
        &self,
        user_id: i64,
        media_type: Option<&str>,
    ) -> crate::Result<Vec<UserWatchlistItem>> {
        let rows = match media_type {
            Some(mt) => {
                sqlx::query_as::<_, UserWatchlistItem>(
                    "SELECT * FROM user_watchlist WHERE user_id = $1 AND media_type = $2 \
                     ORDER BY added_at DESC",
                )
                .bind(user_id)
                .bind(mt)
                .fetch_all(&self.pool)
                .await?
            }
            None => {
                sqlx::query_as::<_, UserWatchlistItem>(
                    "SELECT * FROM user_watchlist WHERE user_id = $1 ORDER BY added_at DESC",
                )
                .bind(user_id)
                .fetch_all(&self.pool)
                .await?
            }
        };
        Ok(rows)
    }

    pub async fn is_on_watchlist(
        &self,
        user_id: i64,
        media_type: &str,
        media_id: i64,
    ) -> crate::Result<bool> {
        let row: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM user_watchlist \
             WHERE user_id = $1 AND media_type = $2 AND media_id = $3",
        )
        .bind(user_id)
        .bind(media_type)
        .bind(media_id)
        .fetch_one(&self.pool)
        .await?;
        Ok(row.0 > 0)
    }

    // ── Ratings ─────────────────────────────────────────────────────

    pub async fn set_rating(
        &self,
        user_id: i64,
        media_type: &str,
        media_id: i64,
        rating: i16,
    ) -> crate::Result<UserRating> {
        let row = sqlx::query_as::<_, UserRating>(
            "INSERT INTO user_ratings (user_id, media_type, media_id, rating) \
             VALUES ($1, $2, $3, $4) \
             ON CONFLICT (user_id, media_type, media_id) DO UPDATE SET \
             rating = $4, updated_at = NOW() \
             RETURNING *",
        )
        .bind(user_id)
        .bind(media_type)
        .bind(media_id)
        .bind(rating)
        .fetch_one(&self.pool)
        .await?;
        Ok(row)
    }

    pub async fn get_rating(
        &self,
        user_id: i64,
        media_type: &str,
        media_id: i64,
    ) -> crate::Result<Option<UserRating>> {
        let row = sqlx::query_as::<_, UserRating>(
            "SELECT * FROM user_ratings WHERE user_id = $1 AND media_type = $2 AND media_id = $3",
        )
        .bind(user_id)
        .bind(media_type)
        .bind(media_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row)
    }

    pub async fn delete_rating(
        &self,
        user_id: i64,
        media_type: &str,
        media_id: i64,
    ) -> crate::Result<bool> {
        let result = sqlx::query(
            "DELETE FROM user_ratings WHERE user_id = $1 AND media_type = $2 AND media_id = $3",
        )
        .bind(user_id)
        .bind(media_type)
        .bind(media_id)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn get_user_ratings(
        &self,
        user_id: i64,
        media_type: Option<&str>,
    ) -> crate::Result<Vec<UserRating>> {
        let rows = match media_type {
            Some(mt) => {
                sqlx::query_as::<_, UserRating>(
                    "SELECT * FROM user_ratings WHERE user_id = $1 AND media_type = $2 \
                     ORDER BY updated_at DESC",
                )
                .bind(user_id)
                .bind(mt)
                .fetch_all(&self.pool)
                .await?
            }
            None => {
                sqlx::query_as::<_, UserRating>(
                    "SELECT * FROM user_ratings WHERE user_id = $1 ORDER BY updated_at DESC",
                )
                .bind(user_id)
                .fetch_all(&self.pool)
                .await?
            }
        };
        Ok(rows)
    }

    pub async fn get_average_rating(
        &self,
        media_type: &str,
        media_id: i64,
    ) -> crate::Result<(f64, i64)> {
        let row: (Option<f64>, i64) = sqlx::query_as(
            "SELECT AVG(rating::DOUBLE PRECISION), COUNT(*) FROM user_ratings \
             WHERE media_type = $1 AND media_id = $2",
        )
        .bind(media_type)
        .bind(media_id)
        .fetch_one(&self.pool)
        .await?;
        Ok((row.0.unwrap_or(0.0), row.1))
    }

    // ── Notifications ─────────────────────────────────────────────

    pub async fn create_notification(
        &self,
        user_id: i64,
        notification_type: &str,
        title: &str,
        body: Option<&str>,
        data: Option<serde_json::Value>,
    ) -> crate::Result<UserNotification> {
        let row = sqlx::query_as::<_, UserNotification>(
            "INSERT INTO user_notifications (user_id, notification_type, title, body, data) \
             VALUES ($1, $2, $3, $4, $5) RETURNING *",
        )
        .bind(user_id)
        .bind(notification_type)
        .bind(title)
        .bind(body)
        .bind(data)
        .fetch_one(&self.pool)
        .await?;
        Ok(row)
    }

    pub async fn create_notification_for_all_users(
        &self,
        notification_type: &str,
        title: &str,
        body: Option<&str>,
        data: Option<serde_json::Value>,
    ) -> crate::Result<u64> {
        let result = sqlx::query(
            "INSERT INTO user_notifications (user_id, notification_type, title, body, data) \
             SELECT id, $1, $2, $3, $4 FROM users WHERE enabled = true",
        )
        .bind(notification_type)
        .bind(title)
        .bind(body)
        .bind(data)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected())
    }

    pub async fn list_notifications(
        &self,
        user_id: i64,
        unread_only: bool,
        limit: i64,
        offset: i64,
    ) -> crate::Result<Vec<UserNotification>> {
        let rows = if unread_only {
            sqlx::query_as::<_, UserNotification>(
                "SELECT * FROM user_notifications \
                 WHERE user_id = $1 AND read = false \
                 ORDER BY created_at DESC LIMIT $2 OFFSET $3",
            )
            .bind(user_id)
            .bind(limit)
            .bind(offset)
            .fetch_all(&self.pool)
            .await?
        } else {
            sqlx::query_as::<_, UserNotification>(
                "SELECT * FROM user_notifications \
                 WHERE user_id = $1 \
                 ORDER BY created_at DESC LIMIT $2 OFFSET $3",
            )
            .bind(user_id)
            .bind(limit)
            .bind(offset)
            .fetch_all(&self.pool)
            .await?
        };
        Ok(rows)
    }

    pub async fn unread_notification_count(&self, user_id: i64) -> crate::Result<i64> {
        let row: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM user_notifications WHERE user_id = $1 AND read = false",
        )
        .bind(user_id)
        .fetch_one(&self.pool)
        .await?;
        Ok(row.0)
    }

    pub async fn mark_notification_read(&self, id: i64, user_id: i64) -> crate::Result<bool> {
        let result =
            sqlx::query("UPDATE user_notifications SET read = true WHERE id = $1 AND user_id = $2")
                .bind(id)
                .bind(user_id)
                .execute(&self.pool)
                .await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn mark_all_notifications_read(&self, user_id: i64) -> crate::Result<u64> {
        let result = sqlx::query(
            "UPDATE user_notifications SET read = true WHERE user_id = $1 AND read = false",
        )
        .bind(user_id)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected())
    }

    pub async fn delete_old_notifications(&self, days: i32) -> crate::Result<u64> {
        let result = sqlx::query(
            "DELETE FROM user_notifications WHERE created_at < NOW() - make_interval(days => $1)",
        )
        .bind(days)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected())
    }

    // ── Push subscriptions ────────────────────────────────────────

    pub async fn save_push_subscription(
        &self,
        user_id: i64,
        endpoint: &str,
        p256dh: &str,
        auth: &str,
        user_agent: Option<&str>,
    ) -> crate::Result<PushSubscription> {
        let row = sqlx::query_as::<_, PushSubscription>(
            "INSERT INTO push_subscriptions (user_id, endpoint, p256dh, auth, user_agent) \
             VALUES ($1, $2, $3, $4, $5) \
             ON CONFLICT (endpoint) DO UPDATE SET \
             user_id = $1, p256dh = $3, auth = $4, user_agent = $5 \
             RETURNING *",
        )
        .bind(user_id)
        .bind(endpoint)
        .bind(p256dh)
        .bind(auth)
        .bind(user_agent)
        .fetch_one(&self.pool)
        .await?;
        Ok(row)
    }

    pub async fn get_push_subscriptions(
        &self,
        user_id: i64,
    ) -> crate::Result<Vec<PushSubscription>> {
        let rows = sqlx::query_as::<_, PushSubscription>(
            "SELECT * FROM push_subscriptions WHERE user_id = $1 ORDER BY created_at DESC",
        )
        .bind(user_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    pub async fn delete_push_subscription(
        &self,
        endpoint: &str,
        user_id: i64,
    ) -> crate::Result<bool> {
        let result =
            sqlx::query("DELETE FROM push_subscriptions WHERE endpoint = $1 AND user_id = $2")
                .bind(endpoint)
                .bind(user_id)
                .execute(&self.pool)
                .await?;
        Ok(result.rows_affected() > 0)
    }

    // ── System Activities ────────────────────────────────────────────────────

    pub async fn create_activity(
        &self,
        activity_type: &str,
        title: &str,
        detail: Option<&str>,
    ) -> crate::Result<SystemActivity> {
        let row = sqlx::query_as::<_, SystemActivity>(
            "INSERT INTO system_activities (activity_type, title, detail) \
             VALUES ($1, $2, $3) RETURNING *",
        )
        .bind(activity_type)
        .bind(title)
        .bind(detail)
        .fetch_one(&self.pool)
        .await?;
        Ok(row)
    }

    pub async fn update_activity_progress(
        &self,
        id: i64,
        detail: Option<&str>,
        progress: Option<serde_json::Value>,
    ) -> crate::Result<bool> {
        let result = sqlx::query(
            "UPDATE system_activities \
             SET detail = COALESCE($2, detail), progress = COALESCE($3, progress), updated_at = NOW() \
             WHERE id = $1",
        )
        .bind(id)
        .bind(detail)
        .bind(progress)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn complete_activity(
        &self,
        id: i64,
        status: &str,
        detail: Option<&str>,
        result: Option<serde_json::Value>,
        error: Option<&str>,
    ) -> crate::Result<bool> {
        let res = sqlx::query(
            "UPDATE system_activities \
             SET status = $2, detail = COALESCE($3, detail), result = $4, error = $5, \
             completed_at = NOW(), updated_at = NOW() \
             WHERE id = $1",
        )
        .bind(id)
        .bind(status)
        .bind(detail)
        .bind(result)
        .bind(error)
        .execute(&self.pool)
        .await?;
        Ok(res.rows_affected() > 0)
    }

    /// Mark all stale "running" activities as failed (e.g. after server restart).
    pub async fn cleanup_stale_activities(&self) -> crate::Result<u64> {
        let result = sqlx::query(
            "UPDATE system_activities \
             SET status = 'failed', error = 'Interrupted by server restart', \
             completed_at = NOW(), updated_at = NOW() \
             WHERE status = 'running'",
        )
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected())
    }

    pub async fn list_activities(
        &self,
        limit: i64,
        include_completed: bool,
    ) -> crate::Result<Vec<SystemActivity>> {
        let rows = if include_completed {
            sqlx::query_as::<_, SystemActivity>(
                "SELECT * FROM system_activities \
                 ORDER BY CASE WHEN status = 'running' THEN 0 ELSE 1 END, started_at DESC \
                 LIMIT $1",
            )
            .bind(limit)
            .fetch_all(&self.pool)
            .await?
        } else {
            sqlx::query_as::<_, SystemActivity>(
                "SELECT * FROM system_activities WHERE status = 'running' \
                 ORDER BY started_at DESC LIMIT $1",
            )
            .bind(limit)
            .fetch_all(&self.pool)
            .await?
        };
        Ok(rows)
    }

    pub async fn get_running_activity_count(&self) -> crate::Result<i64> {
        let (count,): (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM system_activities WHERE status = 'running'")
                .fetch_one(&self.pool)
                .await?;
        Ok(count)
    }

    pub async fn get_running_activity_by_type(
        &self,
        activity_type: &str,
    ) -> crate::Result<Option<SystemActivity>> {
        // Auto-complete stale activities (running > 30 minutes) so they don't
        // permanently block new searches after a crash, restart, or panic.
        sqlx::query(
            "UPDATE system_activities \
             SET status = 'failed', error = 'stale: timed out', \
                 completed_at = NOW(), updated_at = NOW() \
             WHERE activity_type = $1 AND status = 'running' \
               AND started_at < NOW() - INTERVAL '30 minutes'",
        )
        .bind(activity_type)
        .execute(&self.pool)
        .await
        .ok();

        let row = sqlx::query_as::<_, SystemActivity>(
            "SELECT * FROM system_activities \
             WHERE activity_type = $1 AND status = 'running' \
             ORDER BY started_at DESC LIMIT 1",
        )
        .bind(activity_type)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row)
    }

    pub async fn delete_old_activities(&self, days: i32) -> crate::Result<u64> {
        let result = sqlx::query(
            "DELETE FROM system_activities WHERE started_at < NOW() - make_interval(days => $1)",
        )
        .bind(days)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected())
    }

    /// Migrate existing remote_clients to user_devices for a given user.
    pub async fn migrate_remote_clients_to_user_devices(&self, user_id: i64) -> crate::Result<u64> {
        let result = sqlx::query(
            "INSERT INTO user_devices (user_id, device_token, device_name, created_at, last_seen, revoked) \
             SELECT $1, client_token, client_name, created_at, last_seen, revoked \
             FROM remote_clients \
             ON CONFLICT (device_token) DO NOTHING",
        )
        .bind(user_id)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected())
    }
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct RemoteClient {
    pub id: i32,
    pub client_token: Uuid,
    pub client_name: Option<String>,
    pub created_at: DateTime<Utc>,
    pub last_seen: Option<DateTime<Utc>>,
    pub revoked: bool,
}

/// Internal row type for the enriched continue-watching query.
#[derive(Debug, Clone, sqlx::FromRow)]
struct ContinueWatchingRow {
    id: i64,
    user_id: i64,
    media_file_id: i64,
    media_type: String,
    media_id: i64,
    episode_id: Option<i64>,
    position_secs: f32,
    duration_secs: f32,
    completed: bool,
    updated_at: DateTime<Utc>,
    title: Option<String>,
    series_images: Option<serde_json::Value>,
    movie_images: Option<serde_json::Value>,
    episode_title: Option<String>,
    season_number: Option<i32>,
    episode_number: Option<i32>,
    year: Option<i32>,
}

/// Extract a proxied image URL from a JSONB images value by cover type.
fn extract_image_url_from_json(
    images: Option<&serde_json::Value>,
    cover_type: &str,
) -> Option<String> {
    images?.as_array()?.iter().find_map(|img| {
        if img.get("coverType")?.as_str()? == cover_type {
            img.get("remoteUrl")
                .or_else(|| img.get("url"))
                .and_then(|v| v.as_str())
                .map(|url| format!("/api/v1/images/{url}"))
        } else {
            None
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::TestDb;

    #[tokio::test]
    #[ignore = "requires running postgres"]
    async fn test_connect_and_migrate() {
        let db = TestDb::new().await;
        // If we get here, connect + migrations succeeded
        // Verify a table exists by running a simple query
        let _: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM enabled_modules")
            .fetch_one(&db.pool)
            .await
            .expect("enabled_modules table should exist");
        db.cleanup().await;
    }

    #[tokio::test]
    #[ignore = "requires running postgres"]
    async fn test_is_first_boot_true() {
        let db = TestDb::new().await;
        let database = Database {
            pool: db.pool.clone(),
        };
        let first = database.is_first_boot().await.expect("is_first_boot");
        assert!(first, "fresh DB should be first boot");
        db.cleanup().await;
    }

    #[tokio::test]
    #[ignore = "requires running postgres"]
    async fn test_enabled_modules_round_trip() {
        let db = TestDb::new().await;
        let database = Database {
            pool: db.pool.clone(),
        };

        let modules = EnabledModules {
            tv_management: true,
            movie_management: true,
            torrent_embedded: false,
            usenet_embedded: true,
            torrent_external: false,
            usenet_external: false,
            indexarr_sidecar: true,
            external_indexers: false,
            plex_integration: true,
            notifications: false,
            streaming: false,
            remote_access: false,
            stremio_addon: false,
        };
        database.save_enabled_modules(&modules).await.expect("save");

        let loaded = database.load_enabled_modules().await.expect("load");
        assert_eq!(loaded.tv_management, true);
        assert_eq!(loaded.movie_management, true);
        assert_eq!(loaded.torrent_embedded, false);
        assert_eq!(loaded.usenet_embedded, true);
        assert_eq!(loaded.indexarr_sidecar, true);
        assert_eq!(loaded.plex_integration, true);
        assert_eq!(loaded.notifications, false);

        // After saving modules, no longer first boot
        let first = database.is_first_boot().await.expect("is_first_boot");
        assert!(!first);

        db.cleanup().await;
    }
}
