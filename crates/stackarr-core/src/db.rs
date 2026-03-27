use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;
use uuid::Uuid;

use crate::config::{DatabaseConfig, EnabledModules};

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
