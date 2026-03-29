use std::sync::Arc;

use rusqlite::Connection;
use tokio::sync::Mutex;

pub struct BootstrapDb {
    conn: Arc<Mutex<Connection>>,
}

pub enum UpsertResult {
    Created,
    Updated,
    Conflict(String),
}

#[allow(dead_code)]
pub struct ClaimRow {
    pub code: String,
    pub server_id: String,
    pub claim_type: String,
    pub invite_code: Option<String>,
}

impl BootstrapDb {
    pub fn new(path: &str) -> anyhow::Result<Self> {
        let conn = Connection::open(path)?;

        // Enable WAL mode for better concurrent read performance
        conn.execute_batch("PRAGMA journal_mode=WAL;")?;

        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS server_names (
                server_name TEXT NOT NULL PRIMARY KEY COLLATE NOCASE,
                server_id TEXT NOT NULL UNIQUE,
                recovery_key_hash TEXT,
                registered_at TEXT NOT NULL DEFAULT (datetime('now')),
                last_heartbeat TEXT NOT NULL DEFAULT (datetime('now'))
            );

            CREATE TABLE IF NOT EXISTS pending_claims (
                code TEXT NOT NULL PRIMARY KEY,
                server_id TEXT NOT NULL,
                claim_type TEXT NOT NULL DEFAULT 'device',
                invite_code TEXT,
                expires_at TEXT NOT NULL
            );",
        )?;

        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    pub async fn upsert_server_name(
        &self,
        server_name: &str,
        server_id: &str,
    ) -> anyhow::Result<UpsertResult> {
        let conn = self.conn.clone();
        let server_name = server_name.to_string();
        let server_id = server_id.to_string();

        tokio::task::spawn_blocking(move || {
            let conn = conn.blocking_lock();

            // Check if the name already exists
            let existing: Option<String> = conn
                .query_row(
                    "SELECT server_id FROM server_names WHERE server_name = ?1",
                    [&server_name],
                    |row| row.get(0),
                )
                .ok();

            match existing {
                Some(ref existing_id) if existing_id == &server_id => {
                    // Same server, same name — update heartbeat
                    conn.execute(
                        "UPDATE server_names SET last_heartbeat = datetime('now') WHERE server_name = ?1",
                        [&server_name],
                    )?;
                    Ok(UpsertResult::Updated)
                }
                Some(existing_id) => {
                    // Different server owns this name
                    Ok(UpsertResult::Conflict(existing_id))
                }
                None => {
                    // Check if this server_id already has a different name registered
                    let old_name: Option<String> = conn
                        .query_row(
                            "SELECT server_name FROM server_names WHERE server_id = ?1",
                            [&server_id],
                            |row| row.get(0),
                        )
                        .ok();

                    if let Some(old) = old_name {
                        // Server renamed — update the existing row
                        conn.execute(
                            "UPDATE server_names SET server_name = ?1, last_heartbeat = datetime('now') WHERE server_id = ?2",
                            [&server_name, &server_id],
                        )?;
                        tracing::info!(old_name = %old, new_name = %server_name, "server renamed");
                        Ok(UpsertResult::Updated)
                    } else {
                        // Truly new registration
                        conn.execute(
                            "INSERT INTO server_names (server_name, server_id) VALUES (?1, ?2)",
                            [&server_name, &server_id],
                        )?;
                        Ok(UpsertResult::Created)
                    }
                }
            }
        })
        .await?
    }

    #[allow(dead_code)]
    pub async fn update_heartbeat(&self, server_id: &str) -> anyhow::Result<()> {
        let conn = self.conn.clone();
        let server_id = server_id.to_string();

        tokio::task::spawn_blocking(move || {
            let conn = conn.blocking_lock();
            conn.execute(
                "UPDATE server_names SET last_heartbeat = datetime('now') WHERE server_id = ?1",
                [&server_id],
            )?;
            Ok(())
        })
        .await?
    }

    #[allow(dead_code)]
    pub async fn lookup_by_name(&self, name: &str) -> anyhow::Result<Option<String>> {
        let conn = self.conn.clone();
        let name = name.to_string();

        tokio::task::spawn_blocking(move || {
            let conn = conn.blocking_lock();
            let result = conn
                .query_row(
                    "SELECT server_id FROM server_names WHERE server_name = ?1",
                    [&name],
                    |row| row.get(0),
                )
                .ok();
            Ok(result)
        })
        .await?
    }

    pub async fn insert_claim(
        &self,
        code: &str,
        server_id: &str,
        claim_type: &str,
        invite_code: Option<&str>,
        expires_at: &str,
    ) -> anyhow::Result<()> {
        let conn = self.conn.clone();
        let code = code.to_string();
        let server_id = server_id.to_string();
        let claim_type = claim_type.to_string();
        let invite_code = invite_code.map(String::from);
        let expires_at = expires_at.to_string();

        tokio::task::spawn_blocking(move || {
            let conn = conn.blocking_lock();
            conn.execute(
                "INSERT INTO pending_claims (code, server_id, claim_type, invite_code, expires_at) VALUES (?1, ?2, ?3, ?4, ?5)",
                rusqlite::params![code, server_id, claim_type, invite_code, expires_at],
            )?;
            Ok(())
        })
        .await?
    }

    pub async fn take_claim(&self, code: &str) -> anyhow::Result<Option<ClaimRow>> {
        let conn = self.conn.clone();
        let code = code.to_string();

        tokio::task::spawn_blocking(move || {
            let conn = conn.blocking_lock();

            let row = conn
                .query_row(
                    "SELECT code, server_id, claim_type, invite_code FROM pending_claims WHERE code = ?1",
                    [&code],
                    |row| {
                        Ok(ClaimRow {
                            code: row.get(0)?,
                            server_id: row.get(1)?,
                            claim_type: row.get(2)?,
                            invite_code: row.get(3)?,
                        })
                    },
                )
                .ok();

            if row.is_some() {
                conn.execute("DELETE FROM pending_claims WHERE code = ?1", [&code])?;
            }

            Ok(row)
        })
        .await?
    }

    pub async fn sweep_expired_claims(&self) -> anyhow::Result<usize> {
        let conn = self.conn.clone();

        tokio::task::spawn_blocking(move || {
            let conn = conn.blocking_lock();
            let count = conn.execute(
                "DELETE FROM pending_claims WHERE expires_at < datetime('now')",
                [],
            )?;
            Ok(count)
        })
        .await?
    }

    #[allow(dead_code)]
    pub async fn set_recovery_key_hash(
        &self,
        server_name: &str,
        hash: &str,
    ) -> anyhow::Result<()> {
        let conn = self.conn.clone();
        let server_name = server_name.to_string();
        let hash = hash.to_string();

        tokio::task::spawn_blocking(move || {
            let conn = conn.blocking_lock();
            conn.execute(
                "UPDATE server_names SET recovery_key_hash = ?1 WHERE server_name = ?2",
                [&hash, &server_name],
            )?;
            Ok(())
        })
        .await?
    }

    #[allow(dead_code)]
    pub async fn get_recovery_key_hash(
        &self,
        server_name: &str,
    ) -> anyhow::Result<Option<String>> {
        let conn = self.conn.clone();
        let server_name = server_name.to_string();

        tokio::task::spawn_blocking(move || {
            let conn = conn.blocking_lock();
            let result = conn
                .query_row(
                    "SELECT recovery_key_hash FROM server_names WHERE server_name = ?1",
                    [&server_name],
                    |row| row.get(0),
                )
                .ok();
            Ok(result)
        })
        .await?
    }

    #[allow(dead_code)]
    pub async fn transfer_server_name(
        &self,
        server_name: &str,
        new_server_id: &str,
    ) -> anyhow::Result<()> {
        let conn = self.conn.clone();
        let server_name = server_name.to_string();
        let new_server_id = new_server_id.to_string();

        tokio::task::spawn_blocking(move || {
            let conn = conn.blocking_lock();
            conn.execute(
                "UPDATE server_names SET server_id = ?1, last_heartbeat = datetime('now') WHERE server_name = ?2",
                [&new_server_id, &server_name],
            )?;
            Ok(())
        })
        .await?
    }
}
