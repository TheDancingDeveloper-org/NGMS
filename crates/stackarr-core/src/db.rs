use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;
use uuid::Uuid;

use crate::config::{DatabaseConfig, EnabledModules};
use crate::models::user::{Invite, User, UserDevice, UserSession};

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

    /// Create a Database wrapper from an existing pool (useful in tests).
    #[cfg(any(test, feature = "testing"))]
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
        let existing: Option<serde_json::Value> = sqlx::query_scalar(
            "SELECT value FROM app_config WHERE key = 'server_id'",
        )
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
                sqlx::query(
                    "INSERT INTO app_config (key, value) VALUES ('server_id', $1)",
                )
                .bind(serde_json::Value::String(new_id.to_string()))
                .execute(&self.pool)
                .await?;
                Ok(new_id)
            }
        }
    }

    // ── Remote clients ──────────────────────────────────────────────────

    pub async fn create_remote_client(&self, client_token: Uuid) -> crate::Result<i32> {
        let row: (i32,) = sqlx::query_as(
            "INSERT INTO remote_clients (client_token) VALUES ($1) RETURNING id",
        )
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
        let row: Option<(bool,)> = sqlx::query_as(
            "SELECT revoked FROM remote_clients WHERE client_token = $1",
        )
        .bind(client_token)
        .fetch_optional(&self.pool)
        .await?;
        match row {
            Some((revoked,)) => Ok(!revoked),
            None => Ok(false),
        }
    }

    pub async fn touch_remote_client(&self, client_token: Uuid) -> crate::Result<()> {
        sqlx::query(
            "UPDATE remote_clients SET last_seen = NOW() WHERE client_token = $1",
        )
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
        let result = sqlx::query(
            "UPDATE remote_clients SET revoked = true WHERE id = $1",
        )
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
        let user =
            sqlx::query_as::<_, User>("SELECT * FROM users WHERE username = $1")
                .bind(username)
                .fetch_optional(&self.pool)
                .await?;
        Ok(user)
    }

    pub async fn list_users(&self) -> crate::Result<Vec<User>> {
        let users =
            sqlx::query_as::<_, User>("SELECT * FROM users ORDER BY created_at DESC")
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

    pub async fn update_user_password(
        &self,
        id: i64,
        password_hash: &str,
    ) -> crate::Result<bool> {
        let result = sqlx::query(
            "UPDATE users SET password_hash = $1, updated_at = NOW() WHERE id = $2",
        )
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
        sqlx::query(
            "UPDATE user_sessions SET last_active = NOW() WHERE token_hash = $1",
        )
        .bind(token_hash)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn delete_session(&self, token_hash: &str) -> crate::Result<bool> {
        let result =
            sqlx::query("DELETE FROM user_sessions WHERE token_hash = $1")
                .bind(token_hash)
                .execute(&self.pool)
                .await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn delete_all_sessions(&self, user_id: i64) -> crate::Result<u64> {
        let result =
            sqlx::query("DELETE FROM user_sessions WHERE user_id = $1")
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
        let result =
            sqlx::query("DELETE FROM user_sessions WHERE expires_at <= NOW()")
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
        sqlx::query(
            "UPDATE user_devices SET last_seen = NOW() WHERE device_token = $1",
        )
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
        let result = sqlx::query(
            "UPDATE user_devices SET revoked = true WHERE id = $1",
        )
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
        let result = sqlx::query(
            "UPDATE user_devices SET user_id = $1 WHERE device_token = $2",
        )
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
        let invites = sqlx::query_as::<_, Invite>(
            "SELECT * FROM invites ORDER BY created_at DESC",
        )
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

    /// Migrate existing remote_clients to user_devices for a given user.
    pub async fn migrate_remote_clients_to_user_devices(
        &self,
        user_id: i64,
    ) -> crate::Result<u64> {
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
        let database = Database { pool: db.pool.clone() };
        let first = database.is_first_boot().await.expect("is_first_boot");
        assert!(first, "fresh DB should be first boot");
        db.cleanup().await;
    }

    #[tokio::test]
    #[ignore = "requires running postgres"]
    async fn test_enabled_modules_round_trip() {
        let db = TestDb::new().await;
        let database = Database { pool: db.pool.clone() };

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
