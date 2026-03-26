use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;

use crate::config::DatabaseConfig;

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
}
