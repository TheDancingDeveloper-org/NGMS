use chrono::{DateTime, NaiveDate, NaiveTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

// ---- Enums ----

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "text", rename_all = "snake_case")]
#[serde(rename_all = "camelCase")]
pub enum MediaType {
    Series,
    Movie,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "text", rename_all = "snake_case")]
#[serde(rename_all = "camelCase")]
pub enum SeriesStatus {
    Continuing,
    Ended,
    Upcoming,
    Deleted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "text", rename_all = "snake_case")]
#[serde(rename_all = "camelCase")]
pub enum SeriesType {
    Standard,
    Daily,
    Anime,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "text", rename_all = "snake_case")]
#[serde(rename_all = "camelCase")]
pub enum Availability {
    Announced,
    InCinemas,
    Released,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "text", rename_all = "snake_case")]
#[serde(rename_all = "camelCase")]
pub enum DownloadProtocol {
    Usenet,
    Torrent,
}

// ---- TV ----

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct Series {
    pub id: i64,
    pub title: String,
    pub clean_title: String,
    pub sort_title: String,
    pub overview: Option<String>,
    pub status: SeriesStatus,
    pub series_type: SeriesType,
    pub network: Option<String>,
    pub air_time: Option<NaiveTime>,
    pub first_aired: Option<NaiveDate>,
    pub year: Option<i32>,
    pub runtime: Option<i32>,
    pub path: String,
    pub media_library_folder_id: Option<i32>,
    pub quality_profile_id: i32,
    pub season_folder: bool,
    pub monitored: bool,
    pub use_scene_numbering: bool,
    // External IDs
    pub tvdb_id: Option<i64>,
    pub imdb_id: Option<String>,
    pub tmdb_id: Option<i64>,
    pub tvmaze_id: Option<i64>,
    pub mal_id: Option<i64>,
    pub images: Option<serde_json::Value>,
    pub genres: Option<Vec<String>>,
    pub tags: Option<Vec<i32>>,
    pub added_at: DateTime<Utc>,
    pub last_info_sync: Option<DateTime<Utc>>,
    // Plex integration
    pub plex_rating_key: Option<String>,
    pub plex_rating_key_4k: Option<String>,
    pub media_added_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct Season {
    pub id: i64,
    pub series_id: i64,
    pub season_number: i32,
    pub monitored: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct Episode {
    pub id: i64,
    pub series_id: i64,
    pub season_number: i32,
    pub episode_number: i32,
    pub absolute_number: Option<i32>,
    pub scene_season_number: Option<i32>,
    pub scene_episode_number: Option<i32>,
    pub scene_absolute_number: Option<i32>,
    pub title: Option<String>,
    pub overview: Option<String>,
    pub air_date: Option<NaiveDate>,
    pub air_date_utc: Option<DateTime<Utc>>,
    pub runtime: Option<i32>,
    pub monitored: bool,
    pub episode_file_id: Option<i64>,
    pub last_search_time: Option<DateTime<Utc>>,
}

// ---- Movies ----

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct Movie {
    pub id: i64,
    pub title: String,
    pub clean_title: String,
    pub sort_title: String,
    pub overview: Option<String>,
    pub year: Option<i32>,
    pub studio: Option<String>,
    pub path: String,
    pub media_library_folder_id: Option<i32>,
    pub quality_profile_id: i32,
    pub monitored: bool,
    pub minimum_availability: Availability,
    pub movie_file_id: Option<i64>,
    // External IDs
    pub tmdb_id: Option<i64>,
    pub imdb_id: Option<String>,
    // Dates
    pub in_cinemas: Option<NaiveDate>,
    pub physical_release: Option<NaiveDate>,
    pub digital_release: Option<NaiveDate>,
    pub images: Option<serde_json::Value>,
    pub genres: Option<Vec<String>>,
    pub tags: Option<Vec<i32>>,
    pub collection_tmdb_id: Option<i64>,
    pub added_at: DateTime<Utc>,
    pub last_info_sync: Option<DateTime<Utc>>,
    // Plex integration
    pub plex_rating_key: Option<String>,
    pub plex_rating_key_4k: Option<String>,
    pub media_added_at: Option<DateTime<Utc>>,
}

// ---- Media Files (shared) ----

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct MediaFile {
    pub id: i64,
    pub media_type: MediaType,
    pub relative_path: String,
    pub size: i64,
    pub date_added: DateTime<Utc>,
    pub quality: serde_json::Value,
    pub languages: serde_json::Value,
    pub scene_name: Option<String>,
    pub release_group: Option<String>,
    pub release_hash: Option<String>,
    pub edition: Option<String>,
    pub media_info: Option<serde_json::Value>,
    pub indexer_flags: i32,
}

// ---- Media Library Folders ----

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct MediaLibraryFolder {
    pub id: i64,
    pub path: String,
    pub media_type: MediaType,
    pub free_space: Option<i64>,
    pub last_checked: Option<DateTime<Utc>>,
}

// ---- Alternative Titles ----

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct AlternativeTitle {
    pub id: i64,
    pub media_type: MediaType,
    pub media_id: i64,
    pub title: String,
    pub clean_title: String,
    pub scene_name: bool,
}

// ---- Tags ----

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct Tag {
    pub id: i64,
    pub label: String,
}
