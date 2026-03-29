use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct User {
    pub id: i64,
    pub username: String,
    pub display_name: String,
    #[serde(skip_serializing)]
    pub password_hash: String,
    pub role: String,
    pub avatar_url: Option<String>,
    pub enabled: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct UserSession {
    pub id: Uuid,
    pub user_id: i64,
    pub token_hash: String,
    pub user_agent: Option<String>,
    pub ip_address: Option<String>,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub last_active: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct UserDevice {
    pub id: i32,
    pub user_id: i64,
    pub device_token: Uuid,
    pub device_name: Option<String>,
    pub device_type: Option<String>,
    pub created_at: DateTime<Utc>,
    pub last_seen: Option<DateTime<Utc>>,
    pub revoked: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct Invite {
    pub id: i32,
    pub code: String,
    pub created_by: i64,
    pub claimed_by: Option<i64>,
    pub role: String,
    pub expires_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct WatchProgress {
    pub id: i64,
    pub user_id: i64,
    pub media_file_id: i64,
    pub media_type: String,
    pub media_id: i64,
    pub episode_id: Option<i64>,
    pub position_secs: f32,
    pub duration_secs: f32,
    pub completed: bool,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct MediaRequest {
    pub id: i64,
    pub user_id: i64,
    pub media_type: String,
    pub tmdb_id: i64,
    pub title: String,
    pub year: Option<i32>,
    pub poster_url: Option<String>,
    pub overview: Option<String>,
    pub status: String,
    pub admin_note: Option<String>,
    pub approved_by: Option<i64>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct UserWatchlistItem {
    pub id: i64,
    pub user_id: i64,
    pub media_type: String,
    pub media_id: i64,
    pub tmdb_id: i64,
    pub added_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct UserRating {
    pub id: i64,
    pub user_id: i64,
    pub media_type: String,
    pub media_id: i64,
    pub rating: i16,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct UserNotification {
    pub id: i64,
    pub user_id: i64,
    pub notification_type: String,
    pub title: String,
    pub body: Option<String>,
    pub data: Option<serde_json::Value>,
    pub read: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct PushSubscription {
    pub id: i64,
    pub user_id: i64,
    pub endpoint: String,
    pub p256dh: String,
    pub auth: String,
    pub user_agent: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct SystemActivity {
    pub id: i64,
    pub activity_type: String,
    pub status: String,
    pub title: String,
    pub detail: Option<String>,
    pub progress: Option<serde_json::Value>,
    pub result: Option<serde_json::Value>,
    pub error: Option<String>,
    pub started_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}

/// Enriched continue-watching item with joined media metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContinueWatchingItem {
    pub id: i64,
    pub user_id: i64,
    pub media_file_id: i64,
    pub media_type: String,
    pub media_id: i64,
    pub episode_id: Option<i64>,
    pub position_secs: f32,
    pub duration_secs: f32,
    pub completed: bool,
    pub updated_at: DateTime<Utc>,
    pub title: Option<String>,
    pub poster_url: Option<String>,
    pub backdrop_url: Option<String>,
    pub episode_title: Option<String>,
    pub season_number: Option<i32>,
    pub episode_number: Option<i32>,
    pub year: Option<i32>,
}
