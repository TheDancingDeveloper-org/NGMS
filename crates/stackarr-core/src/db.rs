use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;

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
                "notifications" => modules.notifications = enabled,
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
            ("notifications", modules.notifications),
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
}
