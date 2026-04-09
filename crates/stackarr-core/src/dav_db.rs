//! `PostgresDavDatabase` — implements nzbdav-core's `DavDatabase` trait for PostgreSQL.

use chrono::{DateTime, NaiveDateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use nzbdav_core::database::DavDatabase;
use nzbdav_core::error::{DavError, Result};
use nzbdav_core::models::{
    DavItem, DownloadStatus, HistoryItem, ItemSubType, ItemType, QueueItem,
};

pub struct PostgresDavDatabase {
    pool: PgPool,
}

impl PostgresDavDatabase {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

// Helper to map sqlx errors to DavError
fn db_err(e: sqlx::Error) -> DavError {
    DavError::Database(e.to_string())
}

// Row type for dav_items queries
#[derive(sqlx::FromRow)]
struct DavItemRow {
    id: Uuid,
    id_prefix: String,
    created_at: DateTime<Utc>,
    parent_id: Option<Uuid>,
    name: String,
    file_size: Option<i64>,
    item_type: i32,
    sub_type: i32,
    path: String,
    release_date: Option<DateTime<Utc>>,
    last_health_check: Option<DateTime<Utc>>,
    next_health_check: Option<DateTime<Utc>>,
    history_item_id: Option<Uuid>,
    file_blob_id: Option<Uuid>,
    nzb_blob_id: Option<Uuid>,
}

impl TryFrom<DavItemRow> for DavItem {
    type Error = DavError;
    fn try_from(r: DavItemRow) -> Result<Self> {
        Ok(DavItem {
            id: r.id,
            id_prefix: r.id_prefix,
            created_at: r.created_at.naive_utc(),
            parent_id: r.parent_id,
            name: r.name,
            file_size: r.file_size,
            item_type: ItemType::try_from(r.item_type)
                .map_err(DavError::Other)?,
            sub_type: ItemSubType::try_from(r.sub_type)
                .map_err(DavError::Other)?,
            path: r.path,
            release_date: r.release_date,
            last_health_check: r.last_health_check,
            next_health_check: r.next_health_check,
            history_item_id: r.history_item_id,
            file_blob_id: r.file_blob_id,
            nzb_blob_id: r.nzb_blob_id,
        })
    }
}

#[derive(sqlx::FromRow)]
struct QueueItemRow {
    id: Uuid,
    created_at: DateTime<Utc>,
    file_name: String,
    job_name: String,
    nzb_file_size: i64,
    total_segment_bytes: i64,
    category: String,
    priority: i32,
    post_processing: i32,
    pause_until: Option<DateTime<Utc>>,
}

impl From<QueueItemRow> for QueueItem {
    fn from(r: QueueItemRow) -> Self {
        QueueItem {
            id: r.id,
            created_at: r.created_at.naive_utc(),
            file_name: r.file_name,
            job_name: r.job_name,
            nzb_file_size: r.nzb_file_size,
            total_segment_bytes: r.total_segment_bytes,
            category: r.category,
            priority: r.priority,
            post_processing: r.post_processing,
            pause_until: r.pause_until.map(|dt| dt.naive_utc()),
        }
    }
}

#[derive(sqlx::FromRow)]
struct HistoryItemRow {
    id: Uuid,
    created_at: DateTime<Utc>,
    file_name: String,
    job_name: String,
    category: String,
    download_status: i32,
    total_segment_bytes: i64,
    download_time_seconds: i32,
    fail_message: Option<String>,
    download_dir_id: Option<Uuid>,
    nzb_blob_id: Option<Uuid>,
}

impl TryFrom<HistoryItemRow> for HistoryItem {
    type Error = DavError;
    fn try_from(r: HistoryItemRow) -> Result<Self> {
        Ok(HistoryItem {
            id: r.id,
            created_at: r.created_at.naive_utc(),
            file_name: r.file_name,
            job_name: r.job_name,
            category: r.category,
            download_status: DownloadStatus::try_from(r.download_status)
                .map_err(DavError::Other)?,
            total_segment_bytes: r.total_segment_bytes,
            download_time_seconds: r.download_time_seconds,
            fail_message: r.fail_message,
            download_dir_id: r.download_dir_id,
            nzb_blob_id: r.nzb_blob_id,
        })
    }
}

#[async_trait::async_trait]
impl DavDatabase for PostgresDavDatabase {
    // ── DavItem ────────────────────────────────────────────────────────

    async fn insert_dav_item(&self, item: &DavItem) -> Result<()> {
        sqlx::query(
            "INSERT INTO dav_items (id, id_prefix, created_at, parent_id, name, file_size, \
             item_type, sub_type, path, release_date, last_health_check, next_health_check, \
             history_item_id, file_blob_id, nzb_blob_id) \
             VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15) \
             ON CONFLICT (id) DO UPDATE SET \
             name = EXCLUDED.name, path = EXCLUDED.path, parent_id = EXCLUDED.parent_id, \
             file_size = EXCLUDED.file_size, file_blob_id = EXCLUDED.file_blob_id, \
             nzb_blob_id = EXCLUDED.nzb_blob_id",
        )
        .bind(item.id)
        .bind(&item.id_prefix)
        .bind(DateTime::<Utc>::from_naive_utc_and_offset(item.created_at, Utc))
        .bind(item.parent_id)
        .bind(&item.name)
        .bind(item.file_size)
        .bind(item.item_type as i32)
        .bind(item.sub_type as i32)
        .bind(&item.path)
        .bind(item.release_date)
        .bind(item.last_health_check)
        .bind(item.next_health_check)
        .bind(item.history_item_id)
        .bind(item.file_blob_id)
        .bind(item.nzb_blob_id)
        .execute(&self.pool)
        .await
        .map_err(db_err)?;
        Ok(())
    }

    async fn get_dav_item_by_id(&self, id: Uuid) -> Result<Option<DavItem>> {
        let row: Option<DavItemRow> =
            sqlx::query_as("SELECT * FROM dav_items WHERE id = $1")
                .bind(id)
                .fetch_optional(&self.pool)
                .await
                .map_err(db_err)?;
        row.map(DavItem::try_from).transpose()
    }

    async fn get_dav_item_by_path(&self, path: &str) -> Result<Option<DavItem>> {
        let row: Option<DavItemRow> =
            sqlx::query_as("SELECT * FROM dav_items WHERE path = $1")
                .bind(path)
                .fetch_optional(&self.pool)
                .await
                .map_err(db_err)?;
        row.map(DavItem::try_from).transpose()
    }

    async fn get_dav_children(&self, parent_id: Uuid) -> Result<Vec<DavItem>> {
        let rows: Vec<DavItemRow> =
            sqlx::query_as("SELECT * FROM dav_items WHERE parent_id = $1")
                .bind(parent_id)
                .fetch_all(&self.pool)
                .await
                .map_err(db_err)?;
        rows.into_iter().map(DavItem::try_from).collect()
    }

    async fn get_dav_children_by_path(&self, parent_path: &str) -> Result<Vec<DavItem>> {
        let rows: Vec<DavItemRow> = sqlx::query_as(
            "SELECT c.* FROM dav_items c \
             INNER JOIN dav_items p ON c.parent_id = p.id \
             WHERE p.path = $1",
        )
        .bind(parent_path)
        .fetch_all(&self.pool)
        .await
        .map_err(db_err)?;
        rows.into_iter().map(DavItem::try_from).collect()
    }

    async fn delete_dav_item(&self, id: Uuid) -> Result<()> {
        sqlx::query("DELETE FROM dav_items WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(db_err)?;
        Ok(())
    }

    async fn delete_dav_items_by_history(&self, history_item_id: Uuid) -> Result<()> {
        sqlx::query("DELETE FROM dav_items WHERE history_item_id = $1")
            .bind(history_item_id)
            .execute(&self.pool)
            .await
            .map_err(db_err)?;
        Ok(())
    }

    async fn move_dav_item(
        &self,
        id: Uuid,
        new_name: &str,
        new_path: &str,
        new_parent_id: Uuid,
    ) -> Result<()> {
        sqlx::query(
            "UPDATE dav_items SET name = $1, path = $2, parent_id = $3 WHERE id = $4",
        )
        .bind(new_name)
        .bind(new_path)
        .bind(new_parent_id)
        .bind(id)
        .execute(&self.pool)
        .await
        .map_err(db_err)?;
        Ok(())
    }

    async fn update_dav_health_check(
        &self,
        id: Uuid,
        last: DateTime<Utc>,
        next: DateTime<Utc>,
    ) -> Result<()> {
        let result = sqlx::query(
            "UPDATE dav_items SET last_health_check = $1, next_health_check = $2 WHERE id = $3",
        )
        .bind(last)
        .bind(next)
        .bind(id)
        .execute(&self.pool)
        .await
        .map_err(db_err)?;
        if result.rows_affected() == 0 {
            return Err(DavError::ItemNotFound(id.to_string()));
        }
        Ok(())
    }

    // ── Blobs ──────────────────────────────────────────────────────────

    async fn get_file_blob(&self, id: Uuid) -> Result<Vec<u8>> {
        let row: Option<(Vec<u8>,)> =
            sqlx::query_as("SELECT data FROM dav_blobs WHERE id = $1")
                .bind(id)
                .fetch_optional(&self.pool)
                .await
                .map_err(db_err)?;
        row.map(|r| r.0)
            .ok_or_else(|| DavError::BlobNotFound(id.to_string()))
    }

    async fn put_file_blob(&self, id: Uuid, data: &[u8]) -> Result<()> {
        sqlx::query(
            "INSERT INTO dav_blobs (id, data) VALUES ($1, $2) \
             ON CONFLICT (id) DO UPDATE SET data = EXCLUDED.data",
        )
        .bind(id)
        .bind(data)
        .execute(&self.pool)
        .await
        .map_err(db_err)?;
        Ok(())
    }

    async fn get_nzb_blob(&self, id: Uuid) -> Result<Vec<u8>> {
        let row: Option<(Vec<u8>,)> =
            sqlx::query_as("SELECT data FROM dav_nzb_blobs WHERE id = $1")
                .bind(id)
                .fetch_optional(&self.pool)
                .await
                .map_err(db_err)?;
        row.map(|r| r.0)
            .ok_or_else(|| DavError::BlobNotFound(id.to_string()))
    }

    async fn put_nzb_blob(&self, id: Uuid, data: &[u8]) -> Result<()> {
        sqlx::query(
            "INSERT INTO dav_nzb_blobs (id, data) VALUES ($1, $2) \
             ON CONFLICT (id) DO UPDATE SET data = EXCLUDED.data",
        )
        .bind(id)
        .bind(data)
        .execute(&self.pool)
        .await
        .map_err(db_err)?;
        Ok(())
    }

    async fn delete_nzb_blob(&self, id: Uuid) -> Result<()> {
        sqlx::query("DELETE FROM dav_nzb_blobs WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(db_err)?;
        Ok(())
    }

    // ── Queue ──────────────────────────────────────────────────────────

    async fn list_queue_items(&self) -> Result<Vec<QueueItem>> {
        let rows: Vec<QueueItemRow> = sqlx::query_as(
            "SELECT * FROM dav_queue_items ORDER BY priority DESC, created_at ASC",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(db_err)?;
        Ok(rows.into_iter().map(QueueItem::from).collect())
    }

    async fn get_next_queue_item(&self, exclude_ids: &[Uuid]) -> Result<Option<QueueItem>> {
        let row: Option<QueueItemRow> = if exclude_ids.is_empty() {
            sqlx::query_as(
                "SELECT * FROM dav_queue_items \
                 WHERE pause_until IS NULL OR pause_until <= NOW() \
                 ORDER BY priority DESC, created_at ASC LIMIT 1",
            )
            .fetch_optional(&self.pool)
            .await
            .map_err(db_err)?
        } else {
            sqlx::query_as(
                "SELECT * FROM dav_queue_items \
                 WHERE (pause_until IS NULL OR pause_until <= NOW()) \
                 AND id != ALL($1) \
                 ORDER BY priority DESC, created_at ASC LIMIT 1",
            )
            .bind(exclude_ids)
            .fetch_optional(&self.pool)
            .await
            .map_err(db_err)?
        };
        Ok(row.map(QueueItem::from))
    }

    async fn insert_queue_item(&self, item: &QueueItem) -> Result<()> {
        sqlx::query(
            "INSERT INTO dav_queue_items (id, created_at, file_name, job_name, nzb_file_size, \
             total_segment_bytes, category, priority, post_processing, pause_until) \
             VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10)",
        )
        .bind(item.id)
        .bind(DateTime::<Utc>::from_naive_utc_and_offset(item.created_at, Utc))
        .bind(&item.file_name)
        .bind(&item.job_name)
        .bind(item.nzb_file_size)
        .bind(item.total_segment_bytes)
        .bind(&item.category)
        .bind(item.priority)
        .bind(item.post_processing)
        .bind(item.pause_until.map(|dt| DateTime::<Utc>::from_naive_utc_and_offset(dt, Utc)))
        .execute(&self.pool)
        .await
        .map_err(db_err)?;
        Ok(())
    }

    async fn delete_queue_item(&self, id: Uuid) -> Result<()> {
        sqlx::query("DELETE FROM dav_queue_items WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(db_err)?;
        Ok(())
    }

    async fn update_queue_pause_until(
        &self,
        id: Uuid,
        pause_until: Option<NaiveDateTime>,
    ) -> Result<()> {
        sqlx::query("UPDATE dav_queue_items SET pause_until = $1 WHERE id = $2")
            .bind(pause_until.map(|dt| DateTime::<Utc>::from_naive_utc_and_offset(dt, Utc)))
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(db_err)?;
        Ok(())
    }

    async fn count_queue_items(&self) -> Result<i64> {
        let (count,): (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM dav_queue_items")
                .fetch_one(&self.pool)
                .await
                .map_err(db_err)?;
        Ok(count)
    }

    // ── History ────────────────────────────────────────────────────────

    async fn insert_history_item(&self, item: &HistoryItem) -> Result<()> {
        sqlx::query(
            "INSERT INTO dav_history_items (id, created_at, file_name, job_name, category, \
             download_status, total_segment_bytes, download_time_seconds, fail_message, \
             download_dir_id, nzb_blob_id) \
             VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11)",
        )
        .bind(item.id)
        .bind(DateTime::<Utc>::from_naive_utc_and_offset(item.created_at, Utc))
        .bind(&item.file_name)
        .bind(&item.job_name)
        .bind(&item.category)
        .bind(item.download_status as i32)
        .bind(item.total_segment_bytes)
        .bind(item.download_time_seconds)
        .bind(&item.fail_message)
        .bind(item.download_dir_id)
        .bind(item.nzb_blob_id)
        .execute(&self.pool)
        .await
        .map_err(db_err)?;
        Ok(())
    }

    async fn list_history_items(&self, offset: i64, limit: i64) -> Result<Vec<HistoryItem>> {
        let rows: Vec<HistoryItemRow> = sqlx::query_as(
            "SELECT * FROM dav_history_items ORDER BY created_at DESC LIMIT $1 OFFSET $2",
        )
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await
        .map_err(db_err)?;
        rows.into_iter().map(HistoryItem::try_from).collect()
    }

    async fn delete_history_item(&self, id: Uuid) -> Result<()> {
        sqlx::query("DELETE FROM dav_history_items WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(db_err)?;
        Ok(())
    }

    async fn delete_all_history_items(&self) -> Result<()> {
        sqlx::query("DELETE FROM dav_history_items")
            .execute(&self.pool)
            .await
            .map_err(db_err)?;
        Ok(())
    }

    async fn count_history_items(&self) -> Result<i64> {
        let (count,): (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM dav_history_items")
                .fetch_one(&self.pool)
                .await
                .map_err(db_err)?;
        Ok(count)
    }

    // ── Config ─────────────────────────────────────────────────────────

    async fn load_config_items(&self) -> Result<Vec<(String, String)>> {
        let rows: Vec<(String, String)> =
            sqlx::query_as("SELECT key, value FROM dav_config")
                .fetch_all(&self.pool)
                .await
                .map_err(db_err)?;
        Ok(rows)
    }

    async fn set_config_item(&self, key: &str, value: &str) -> Result<()> {
        sqlx::query(
            "INSERT INTO dav_config (key, value) VALUES ($1, $2) \
             ON CONFLICT (key) DO UPDATE SET value = EXCLUDED.value",
        )
        .bind(key)
        .bind(value)
        .execute(&self.pool)
        .await
        .map_err(db_err)?;
        Ok(())
    }
}
