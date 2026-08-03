// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 The StackArr Authors

use std::path::Path;

use anyhow::{Context, Result};
use chrono::{DateTime, NaiveDate, NaiveDateTime, NaiveTime, Utc};
use rusqlite::{Connection, OpenFlags};
use serde::Deserialize;
use tracing::{debug, warn};

// ---------------------------------------------------------------------------
// Intermediate structs – mirror the Sonarr SQLite schema
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct SonarrSeries {
    pub id: i64,
    pub tvdb_id: Option<i64>,
    pub imdb_id: Option<String>,
    pub title: String,
    pub title_slug: Option<String>,
    pub clean_title: String,
    pub status: i32,
    pub overview: Option<String>,
    pub air_time: Option<String>,
    pub images: Option<String>,
    pub path: String,
    pub monitored: bool,
    pub season_folder: bool,
    pub runtime: Option<i32>,
    pub series_type: i32,
    pub network: Option<String>,
    pub use_scene_numbering: bool,
    pub first_aired: Option<String>,
    pub year: Option<i32>,
    pub seasons: Option<String>,
    pub sort_title: String,
    pub quality_profile_id: i64,
    pub language_profile_id: Option<i64>,
    pub tags: Option<String>,
    pub added: Option<String>,
    pub tvmaze_id: Option<i64>,
    pub tmdb_id: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct SonarrEpisode {
    pub id: i64,
    pub series_id: i64,
    pub season_number: i32,
    pub episode_number: i32,
    pub title: Option<String>,
    pub overview: Option<String>,
    pub episode_file_id: Option<i64>,
    pub absolute_episode_number: Option<i32>,
    pub scene_absolute_episode_number: Option<i32>,
    pub scene_season_number: Option<i32>,
    pub scene_episode_number: Option<i32>,
    pub monitored: bool,
    pub air_date_utc: Option<String>,
    pub air_date: Option<String>,
    pub last_search_time: Option<String>,
    pub tvdb_id: Option<i64>,
    pub runtime: Option<i32>,
    pub finale_type: Option<String>,
}

#[derive(Debug, Clone)]
pub struct SonarrEpisodeFile {
    pub id: i64,
    pub series_id: i64,
    pub quality: Option<String>,
    pub size: i64,
    pub date_added: Option<String>,
    pub season_number: i32,
    pub scene_name: Option<String>,
    pub release_group: Option<String>,
    pub media_info: Option<String>,
    pub relative_path: Option<String>,
    pub languages: Option<String>,
    pub indexer_flags: i32,
    pub release_hash: Option<String>,
}

#[derive(Debug, Clone)]
pub struct SonarrQualityProfile {
    pub id: i64,
    pub name: String,
    pub cutoff: i32,
    pub items: String,
    pub upgrade_allowed: bool,
    pub format_items: Option<String>,
    pub min_format_score: i32,
    pub cutoff_format_score: i32,
    pub min_upgrade_format_score: i32,
}

#[derive(Debug, Clone)]
pub struct SonarrCustomFormat {
    pub id: i64,
    pub name: String,
    pub specifications: String,
    pub include_when_renaming: bool,
}

#[derive(Debug, Clone)]
pub struct SonarrIndexer {
    pub id: i64,
    pub name: String,
    pub implementation: String,
    pub settings: Option<String>,
    pub enable_rss: bool,
    pub enable_automatic_search: bool,
    pub enable_interactive_search: bool,
    pub priority: i32,
    pub tags: Option<String>,
}

#[derive(Debug, Clone)]
pub struct SonarrDownloadClient {
    pub id: i64,
    pub enable: bool,
    pub name: String,
    pub implementation: String,
    pub settings: Option<String>,
    pub priority: i32,
    pub tags: Option<String>,
}

#[derive(Debug, Clone)]
pub struct SonarrTag {
    pub id: i64,
    pub label: String,
}

#[derive(Debug, Clone)]
pub struct SonarrNamingConfig {
    pub multi_episode_style: i32,
    pub rename_episodes: bool,
    pub standard_episode_format: Option<String>,
    pub daily_episode_format: Option<String>,
    pub season_folder_format: Option<String>,
    pub series_folder_format: Option<String>,
    pub anime_episode_format: Option<String>,
    pub colon_replacement_format: i32,
}

#[derive(Debug, Clone)]
pub struct SonarrHistory {
    pub id: i64,
    pub episode_id: i64,
    pub series_id: i64,
    pub source_title: String,
    pub date: String,
    pub quality: Option<String>,
    pub data: Option<String>,
    pub event_type: i32,
    pub download_id: Option<String>,
    pub languages: Option<String>,
}

#[derive(Debug, Clone)]
pub struct SonarrBlocklist {
    pub id: i64,
    pub series_id: i64,
    pub episode_ids: Option<String>,
    pub source_title: String,
    pub quality: Option<String>,
    pub date: Option<String>,
    pub torrent_info_hash: Option<String>,
    pub languages: Option<String>,
    pub indexer_flags: i32,
}

#[derive(Debug, Clone)]
pub struct SonarrLanguageProfile {
    pub id: i64,
    pub name: String,
    pub languages: String,
    pub upgrade_allowed: bool,
    pub cutoff: Option<String>,
}

#[derive(Debug, Clone)]
pub struct SonarrSeason {
    pub season_number: i32,
    pub monitored: bool,
}

/// All data extracted from a Sonarr SQLite database.
#[derive(Debug, Clone)]
pub struct SonarrData {
    pub series: Vec<SonarrSeries>,
    pub episodes: Vec<SonarrEpisode>,
    pub episode_files: Vec<SonarrEpisodeFile>,
    pub quality_profiles: Vec<SonarrQualityProfile>,
    pub custom_formats: Vec<SonarrCustomFormat>,
    pub language_profiles: Vec<SonarrLanguageProfile>,
    pub indexers: Vec<SonarrIndexer>,
    pub download_clients: Vec<SonarrDownloadClient>,
    pub root_folders: Vec<String>,
    pub tags: Vec<SonarrTag>,
    pub naming_config: Option<SonarrNamingConfig>,
    pub history: Vec<SonarrHistory>,
    pub blocklist: Vec<SonarrBlocklist>,
}

// ---------------------------------------------------------------------------
// Settings JSON helpers
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
pub struct IndexerSettings {
    #[serde(alias = "baseUrl", alias = "BaseUrl")]
    pub base_url: Option<String>,
    #[serde(alias = "apiKey", alias = "ApiKey")]
    pub api_key: Option<String>,
    #[serde(alias = "apiPath", alias = "ApiPath")]
    pub api_path: Option<String>,
    #[serde(alias = "categories", alias = "Categories")]
    pub categories: Option<Vec<i32>>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
pub struct DownloadClientSettings {
    #[serde(alias = "host", alias = "Host")]
    pub host: Option<String>,
    #[serde(alias = "port", alias = "Port")]
    pub port: Option<u16>,
    #[serde(alias = "urlBase", alias = "UrlBase")]
    pub url_base: Option<String>,
    #[serde(alias = "username", alias = "Username")]
    pub username: Option<String>,
    #[serde(alias = "password", alias = "Password")]
    pub password: Option<String>,
    #[serde(alias = "apiKey", alias = "ApiKey")]
    pub api_key: Option<String>,
    #[serde(alias = "useSsl", alias = "UseSsl")]
    pub use_ssl: Option<bool>,
    #[serde(alias = "category", alias = "Category")]
    pub category: Option<String>,
}

pub fn parse_indexer_settings(json: &str) -> IndexerSettings {
    serde_json::from_str(json).unwrap_or_default()
}

pub fn parse_download_client_settings(json: &str) -> DownloadClientSettings {
    serde_json::from_str(json).unwrap_or_default()
}

// ---------------------------------------------------------------------------
// Enum mapping helpers
// ---------------------------------------------------------------------------

pub fn map_series_status(status: i32) -> &'static str {
    match status {
        0 => "continuing",
        1 => "ended",
        2 => "upcoming",
        3 => "deleted",
        _ => "continuing",
    }
}

pub fn map_series_type(st: i32) -> &'static str {
    match st {
        0 => "standard",
        1 => "daily",
        2 => "anime",
        _ => "standard",
    }
}

pub fn map_event_type(et: i32) -> &'static str {
    match et {
        1 => "grabbed",
        3 => "imported",
        4 => "download_failed",
        5 => "file_deleted",
        6 => "file_renamed",
        8 => "download_ignored",
        _ => "grabbed",
    }
}

/// Strip " (Prowlarr)" suffix from indexer names imported from Sonarr/Radarr.
pub fn strip_prowlarr_suffix(name: &str) -> String {
    name.trim_end_matches(" (Prowlarr)").to_string()
}

pub fn map_implementation_to_protocol(imp: &str) -> &'static str {
    match imp {
        "Newznab" => "usenet",
        "Torznab" => "torrent",
        _ => "torrent",
    }
}

pub fn map_dl_implementation_to_protocol(imp: &str) -> &'static str {
    match imp {
        "Sabnzbd" | "NzbGet" | "Nzbget" => "usenet",
        "QBittorrent" | "Qbittorrent" | "Transmission" | "Deluge" | "RTorrent" => "torrent",
        _ => "torrent",
    }
}

// ---------------------------------------------------------------------------
// Parse helpers for dates
// ---------------------------------------------------------------------------

pub fn parse_datetime(s: &str) -> Option<DateTime<Utc>> {
    // Sonarr stores dates in several formats, try them in order
    if let Ok(dt) = DateTime::parse_from_rfc3339(s) {
        return Some(dt.with_timezone(&Utc));
    }
    if let Ok(dt) = NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S%.f") {
        return Some(dt.and_utc());
    }
    if let Ok(dt) = NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S") {
        return Some(dt.and_utc());
    }
    if let Ok(dt) = NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S%.f") {
        return Some(dt.and_utc());
    }
    if let Ok(dt) = NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S") {
        return Some(dt.and_utc());
    }
    None
}

pub fn parse_date(s: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(s, "%Y-%m-%d").ok()
}

pub fn parse_time(s: &str) -> Option<NaiveTime> {
    NaiveTime::parse_from_str(s, "%H:%M")
        .ok()
        .or_else(|| NaiveTime::parse_from_str(s, "%H:%M:%S").ok())
}

pub fn parse_seasons_json(json: &str) -> Vec<SonarrSeason> {
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct SeasonEntry {
        season_number: i32,
        monitored: bool,
    }

    serde_json::from_str::<Vec<SeasonEntry>>(json)
        .unwrap_or_default()
        .into_iter()
        .map(|s| SonarrSeason {
            season_number: s.season_number,
            monitored: s.monitored,
        })
        .collect()
}

/// Extract the primary language ID from a Sonarr language profile.
///
/// Sonarr's language profile has a `cutoff` field (JSON object like
/// `{"id":1,"name":"English"}`) indicating the preferred language, and a
/// `languages` field (JSON array of `{"language":{"id":N,"name":"..."},
/// "allowed":bool}` entries). We use the cutoff language ID when available,
/// otherwise fall back to the first allowed language. Returns `None` if the
/// profile cannot be parsed or has no allowed languages.
pub fn language_profile_primary_id(profile: &SonarrLanguageProfile) -> Option<i32> {
    // Try cutoff first — it's the profile's preferred language.
    if let Some(cutoff_json) = &profile.cutoff {
        #[derive(Deserialize)]
        struct Cutoff {
            id: i32,
        }
        if let Ok(c) = serde_json::from_str::<Cutoff>(cutoff_json) {
            // Sonarr uses -1 for "Any" in the cutoff; treat as unknown.
            if c.id > 0 {
                return Some(c.id);
            }
        }
    }

    // Fall back to first allowed language in the Languages array.
    #[derive(Deserialize)]
    struct LangEntry {
        language: LangId,
        allowed: bool,
    }
    #[derive(Deserialize)]
    struct LangId {
        id: i32,
    }

    if let Ok(langs) = serde_json::from_str::<Vec<LangEntry>>(&profile.languages) {
        for entry in &langs {
            if entry.allowed && entry.language.id > 0 {
                return Some(entry.language.id);
            }
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Read the entire Sonarr database
// ---------------------------------------------------------------------------

pub fn read_sonarr(path: &Path) -> Result<SonarrData> {
    let conn = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .with_context(|| format!("failed to open Sonarr DB at {}", path.display()))?;

    debug!("reading Sonarr database from {}", path.display());

    let series = read_series(&conn)?;
    let episodes = read_episodes(&conn)?;
    let episode_files = read_episode_files(&conn)?;
    let quality_profiles = read_quality_profiles(&conn)?;
    let custom_formats = read_custom_formats(&conn)?;
    let language_profiles = read_language_profiles(&conn)?;
    let indexers = read_indexers(&conn)?;
    let download_clients = read_download_clients(&conn)?;
    let root_folders = read_root_folders(&conn)?;
    let tags = read_tags(&conn)?;
    let naming_config = read_naming_config(&conn)?;
    let history = read_history(&conn)?;
    let blocklist = read_blocklist(&conn)?;

    debug!(
        "Sonarr: {} series, {} episodes, {} files, {} profiles, {} custom formats, {} language profiles",
        series.len(),
        episodes.len(),
        episode_files.len(),
        quality_profiles.len(),
        custom_formats.len(),
        language_profiles.len(),
    );

    Ok(SonarrData {
        series,
        episodes,
        episode_files,
        quality_profiles,
        custom_formats,
        language_profiles,
        indexers,
        download_clients,
        root_folders,
        tags,
        naming_config,
        history,
        blocklist,
    })
}

fn read_series(conn: &Connection) -> Result<Vec<SonarrSeries>> {
    // LanguageProfileId exists in Sonarr v3 but was removed in v4. Try to
    // detect it so we can map language preferences into quality profiles.
    let has_language_profile = conn
        .prepare("SELECT LanguageProfileId FROM Series LIMIT 0")
        .is_ok();

    let query = if has_language_profile {
        "SELECT Id, TvdbId, ImdbId, Title, TitleSlug, CleanTitle, Status, Overview,
                AirTime, Images, Path, Monitored, SeasonFolder, Runtime, SeriesType,
                Network, UseSceneNumbering, FirstAired, Year, Seasons, SortTitle,
                QualityProfileId, Tags, Added, TvMazeId, TmdbId, LanguageProfileId
         FROM Series"
    } else {
        "SELECT Id, TvdbId, ImdbId, Title, TitleSlug, CleanTitle, Status, Overview,
                AirTime, Images, Path, Monitored, SeasonFolder, Runtime, SeriesType,
                Network, UseSceneNumbering, FirstAired, Year, Seasons, SortTitle,
                QualityProfileId, Tags, Added, TvMazeId, TmdbId
         FROM Series"
    };

    let mut stmt = conn.prepare(query)?;

    let rows = stmt.query_map([], |row| {
        Ok(SonarrSeries {
            id: row.get(0)?,
            tvdb_id: row.get(1)?,
            imdb_id: row.get(2)?,
            title: row.get(3)?,
            title_slug: row.get(4)?,
            clean_title: row.get(5)?,
            status: row.get(6)?,
            overview: row.get(7)?,
            air_time: row.get(8)?,
            images: row.get(9)?,
            path: row.get(10)?,
            monitored: row.get::<_, i32>(11)? != 0,
            season_folder: row.get::<_, i32>(12)? != 0,
            runtime: row.get(13)?,
            series_type: row.get(14)?,
            network: row.get(15)?,
            use_scene_numbering: row.get::<_, i32>(16)? != 0,
            first_aired: row.get(17)?,
            year: row.get(18)?,
            seasons: row.get(19)?,
            sort_title: row.get(20)?,
            quality_profile_id: row.get(21)?,
            language_profile_id: if has_language_profile {
                row.get::<_, Option<i64>>(26)?
            } else {
                None
            },
            tags: row.get(22)?,
            added: row.get(23)?,
            tvmaze_id: row.get(24)?,
            tmdb_id: row.get(25)?,
        })
    })?;

    let mut result = Vec::new();
    for row in rows {
        match row {
            Ok(s) => result.push(s),
            Err(e) => warn!("skipping malformed Sonarr series row: {e}"),
        }
    }
    Ok(result)
}

fn read_episodes(conn: &Connection) -> Result<Vec<SonarrEpisode>> {
    let mut stmt = conn.prepare(
        "SELECT Id, SeriesId, SeasonNumber, EpisodeNumber, Title, Overview,
                EpisodeFileId, AbsoluteEpisodeNumber, SceneAbsoluteEpisodeNumber,
                SceneSeasonNumber, SceneEpisodeNumber, Monitored, AirDateUtc,
                AirDate, LastSearchTime, TvdbId, Runtime, FinaleType
         FROM Episodes",
    )?;

    let rows = stmt.query_map([], |row| {
        let file_id: Option<i64> = row.get(6)?;
        Ok(SonarrEpisode {
            id: row.get(0)?,
            series_id: row.get(1)?,
            season_number: row.get(2)?,
            episode_number: row.get(3)?,
            title: row.get(4)?,
            overview: row.get(5)?,
            episode_file_id: file_id.filter(|&id| id > 0),
            absolute_episode_number: row.get(7)?,
            scene_absolute_episode_number: row.get(8)?,
            scene_season_number: row.get(9)?,
            scene_episode_number: row.get(10)?,
            monitored: row.get::<_, i32>(11)? != 0,
            air_date_utc: row.get(12)?,
            air_date: row.get(13)?,
            last_search_time: row.get(14)?,
            tvdb_id: row.get(15)?,
            runtime: row.get(16)?,
            finale_type: row.get(17)?,
        })
    })?;

    let mut result = Vec::new();
    for row in rows {
        match row {
            Ok(e) => result.push(e),
            Err(e) => warn!("skipping malformed Sonarr episode row: {e}"),
        }
    }
    Ok(result)
}

fn read_episode_files(conn: &Connection) -> Result<Vec<SonarrEpisodeFile>> {
    let mut stmt = conn.prepare(
        "SELECT Id, SeriesId, Quality, Size, DateAdded, SeasonNumber, SceneName,
                ReleaseGroup, MediaInfo, RelativePath, Languages, IndexerFlags, ReleaseHash
         FROM EpisodeFiles",
    )?;

    let rows = stmt.query_map([], |row| {
        Ok(SonarrEpisodeFile {
            id: row.get(0)?,
            series_id: row.get(1)?,
            quality: row.get(2)?,
            size: row.get(3)?,
            date_added: row.get(4)?,
            season_number: row.get(5)?,
            scene_name: row.get(6)?,
            release_group: row.get(7)?,
            media_info: row.get(8)?,
            relative_path: row.get(9)?,
            languages: row.get(10)?,
            indexer_flags: row.get::<_, i32>(11).unwrap_or(0),
            release_hash: row.get(12)?,
        })
    })?;

    let mut result = Vec::new();
    for row in rows {
        match row {
            Ok(f) => result.push(f),
            Err(e) => warn!("skipping malformed Sonarr episode file row: {e}"),
        }
    }
    Ok(result)
}

fn read_quality_profiles(conn: &Connection) -> Result<Vec<SonarrQualityProfile>> {
    let mut stmt = conn.prepare(
        "SELECT Id, Name, Cutoff, Items, UpgradeAllowed,
                FormatItems, MinFormatScore, CutoffFormatScore, MinUpgradeFormatScore
         FROM QualityProfiles",
    )?;

    let rows = stmt.query_map([], |row| {
        Ok(SonarrQualityProfile {
            id: row.get(0)?,
            name: row.get(1)?,
            cutoff: row.get(2)?,
            items: row.get(3)?,
            upgrade_allowed: row.get::<_, i32>(4)? != 0,
            format_items: row.get(5)?,
            min_format_score: row.get::<_, i32>(6).unwrap_or(0),
            cutoff_format_score: row.get::<_, i32>(7).unwrap_or(0),
            min_upgrade_format_score: row.get::<_, i32>(8).unwrap_or(1),
        })
    })?;

    let mut result = Vec::new();
    for row in rows {
        match row {
            Ok(p) => result.push(p),
            Err(e) => warn!("skipping malformed Sonarr quality profile row: {e}"),
        }
    }
    Ok(result)
}

fn read_custom_formats(conn: &Connection) -> Result<Vec<SonarrCustomFormat>> {
    let mut stmt = conn.prepare(
        "SELECT Id, Name, Specifications, IncludeCustomFormatWhenRenaming
         FROM CustomFormats",
    )?;

    let rows = stmt.query_map([], |row| {
        Ok(SonarrCustomFormat {
            id: row.get(0)?,
            name: row.get(1)?,
            specifications: row.get::<_, String>(2).unwrap_or_else(|_| "[]".to_string()),
            include_when_renaming: row.get::<_, i32>(3).unwrap_or(0) != 0,
        })
    })?;

    let mut result = Vec::new();
    for row in rows {
        match row {
            Ok(cf) => result.push(cf),
            Err(e) => warn!("skipping malformed Sonarr custom format row: {e}"),
        }
    }
    Ok(result)
}

fn read_language_profiles(conn: &Connection) -> Result<Vec<SonarrLanguageProfile>> {
    // The LanguageProfiles table exists in Sonarr v3; v4 removed it.
    let mut stmt = match conn
        .prepare("SELECT Id, Name, Languages, UpgradeAllowed, Cutoff FROM LanguageProfiles")
    {
        Ok(s) => s,
        Err(_) => {
            debug!("LanguageProfiles table not found — Sonarr v4+ or missing");
            return Ok(Vec::new());
        }
    };

    let rows = stmt.query_map([], |row| {
        Ok(SonarrLanguageProfile {
            id: row.get(0)?,
            name: row.get(1)?,
            languages: row.get::<_, String>(2).unwrap_or_else(|_| "[]".to_string()),
            upgrade_allowed: row.get::<_, i32>(3).unwrap_or(0) != 0,
            cutoff: row.get(4)?,
        })
    })?;

    let mut result = Vec::new();
    for row in rows {
        match row {
            Ok(lp) => result.push(lp),
            Err(e) => warn!("skipping malformed Sonarr language profile row: {e}"),
        }
    }
    Ok(result)
}

fn read_indexers(conn: &Connection) -> Result<Vec<SonarrIndexer>> {
    let mut stmt = conn.prepare(
        "SELECT Id, Name, Implementation, Settings, EnableRss,
                EnableAutomaticSearch, EnableInteractiveSearch, Priority, Tags
         FROM Indexers",
    )?;

    let rows = stmt.query_map([], |row| {
        Ok(SonarrIndexer {
            id: row.get(0)?,
            name: row.get(1)?,
            implementation: row.get(2)?,
            settings: row.get(3)?,
            enable_rss: row.get::<_, i32>(4)? != 0,
            enable_automatic_search: row.get::<_, i32>(5)? != 0,
            enable_interactive_search: row.get::<_, i32>(6)? != 0,
            priority: row.get::<_, i32>(7).unwrap_or(25),
            tags: row.get(8)?,
        })
    })?;

    let mut result = Vec::new();
    for row in rows {
        match row {
            Ok(i) => result.push(i),
            Err(e) => warn!("skipping malformed Sonarr indexer row: {e}"),
        }
    }
    Ok(result)
}

fn read_download_clients(conn: &Connection) -> Result<Vec<SonarrDownloadClient>> {
    let mut stmt = conn.prepare(
        "SELECT Id, Enable, Name, Implementation, Settings, Priority, Tags
         FROM DownloadClients",
    )?;

    let rows = stmt.query_map([], |row| {
        Ok(SonarrDownloadClient {
            id: row.get(0)?,
            enable: row.get::<_, i32>(1)? != 0,
            name: row.get(2)?,
            implementation: row.get(3)?,
            settings: row.get(4)?,
            priority: row.get::<_, i32>(5).unwrap_or(1),
            tags: row.get(6)?,
        })
    })?;

    let mut result = Vec::new();
    for row in rows {
        match row {
            Ok(dc) => result.push(dc),
            Err(e) => warn!("skipping malformed Sonarr download client row: {e}"),
        }
    }
    Ok(result)
}

fn read_root_folders(conn: &Connection) -> Result<Vec<String>> {
    let mut stmt = conn.prepare("SELECT Path FROM RootFolders")?;
    let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;

    let mut result = Vec::new();
    for row in rows {
        match row {
            Ok(p) => result.push(p),
            Err(e) => warn!("skipping malformed Sonarr root folder row: {e}"),
        }
    }
    Ok(result)
}

fn read_tags(conn: &Connection) -> Result<Vec<SonarrTag>> {
    let mut stmt = conn.prepare("SELECT Id, Label FROM Tags")?;
    let rows = stmt.query_map([], |row| {
        Ok(SonarrTag {
            id: row.get(0)?,
            label: row.get(1)?,
        })
    })?;

    let mut result = Vec::new();
    for row in rows {
        match row {
            Ok(t) => result.push(t),
            Err(e) => warn!("skipping malformed Sonarr tag row: {e}"),
        }
    }
    Ok(result)
}

fn read_naming_config(conn: &Connection) -> Result<Option<SonarrNamingConfig>> {
    let mut stmt = conn.prepare(
        "SELECT MultiEpisodeStyle, RenameEpisodes, StandardEpisodeFormat,
                DailyEpisodeFormat, SeasonFolderFormat, SeriesFolderFormat,
                AnimeEpisodeFormat, ColonReplacementFormat
         FROM NamingConfig LIMIT 1",
    )?;

    let mut rows = stmt.query_map([], |row| {
        Ok(SonarrNamingConfig {
            multi_episode_style: row.get(0)?,
            rename_episodes: row.get::<_, i32>(1)? != 0,
            standard_episode_format: row.get(2)?,
            daily_episode_format: row.get(3)?,
            season_folder_format: row.get(4)?,
            series_folder_format: row.get(5)?,
            anime_episode_format: row.get(6)?,
            colon_replacement_format: row.get::<_, i32>(7).unwrap_or(4),
        })
    })?;

    match rows.next() {
        Some(Ok(nc)) => Ok(Some(nc)),
        Some(Err(e)) => {
            warn!("failed to read Sonarr naming config: {e}");
            Ok(None)
        }
        None => Ok(None),
    }
}

fn read_history(conn: &Connection) -> Result<Vec<SonarrHistory>> {
    let mut stmt = conn.prepare(
        "SELECT Id, EpisodeId, SeriesId, SourceTitle, Date, Quality,
                Data, EventType, DownloadId, Languages
         FROM History",
    )?;

    let rows = stmt.query_map([], |row| {
        Ok(SonarrHistory {
            id: row.get(0)?,
            episode_id: row.get(1)?,
            series_id: row.get(2)?,
            source_title: row.get(3)?,
            date: row.get(4)?,
            quality: row.get(5)?,
            data: row.get(6)?,
            event_type: row.get(7)?,
            download_id: row.get(8)?,
            languages: row.get(9)?,
        })
    })?;

    let mut result = Vec::new();
    for row in rows {
        match row {
            Ok(h) => result.push(h),
            Err(e) => warn!("skipping malformed Sonarr history row: {e}"),
        }
    }
    Ok(result)
}

fn read_blocklist(conn: &Connection) -> Result<Vec<SonarrBlocklist>> {
    let mut stmt = conn.prepare(
        "SELECT Id, SeriesId, EpisodeIds, SourceTitle, Quality,
                Date, TorrentInfoHash, Languages, IndexerFlags
         FROM Blocklist",
    )?;

    let rows = stmt.query_map([], |row| {
        Ok(SonarrBlocklist {
            id: row.get(0)?,
            series_id: row.get(1)?,
            episode_ids: row.get(2)?,
            source_title: row.get(3)?,
            quality: row.get(4)?,
            date: row.get(5)?,
            torrent_info_hash: row.get(6)?,
            languages: row.get(7)?,
            indexer_flags: row.get::<_, i32>(8).unwrap_or(0),
        })
    })?;

    let mut result = Vec::new();
    for row in rows {
        match row {
            Ok(b) => result.push(b),
            Err(e) => warn!("skipping malformed Sonarr blocklist row: {e}"),
        }
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Datelike, Timelike};

    #[test]
    fn test_map_series_status() {
        assert_eq!(map_series_status(0), "continuing");
        assert_eq!(map_series_status(1), "ended");
        assert_eq!(map_series_status(2), "upcoming");
        assert_eq!(map_series_status(3), "deleted");
        assert_eq!(map_series_status(99), "continuing"); // unknown defaults
    }

    #[test]
    fn test_map_series_type() {
        assert_eq!(map_series_type(0), "standard");
        assert_eq!(map_series_type(1), "daily");
        assert_eq!(map_series_type(2), "anime");
        assert_eq!(map_series_type(99), "standard");
    }

    #[test]
    fn test_map_event_type() {
        assert_eq!(map_event_type(1), "grabbed");
        assert_eq!(map_event_type(3), "imported");
        assert_eq!(map_event_type(4), "download_failed");
        assert_eq!(map_event_type(5), "file_deleted");
        assert_eq!(map_event_type(6), "file_renamed");
        assert_eq!(map_event_type(8), "download_ignored");
        assert_eq!(map_event_type(99), "grabbed");
    }

    #[test]
    fn test_map_implementation_to_protocol() {
        assert_eq!(map_implementation_to_protocol("Newznab"), "usenet");
        assert_eq!(map_implementation_to_protocol("Torznab"), "torrent");
        assert_eq!(map_implementation_to_protocol("Unknown"), "torrent");
    }

    #[test]
    fn test_map_dl_implementation_to_protocol() {
        assert_eq!(map_dl_implementation_to_protocol("Sabnzbd"), "usenet");
        assert_eq!(map_dl_implementation_to_protocol("NzbGet"), "usenet");
        assert_eq!(map_dl_implementation_to_protocol("QBittorrent"), "torrent");
        assert_eq!(map_dl_implementation_to_protocol("Transmission"), "torrent");
        assert_eq!(map_dl_implementation_to_protocol("Deluge"), "torrent");
        assert_eq!(map_dl_implementation_to_protocol("Unknown"), "torrent");
    }

    #[test]
    fn test_parse_datetime_rfc3339() {
        let dt = parse_datetime("2024-06-15T10:30:00Z");
        assert!(dt.is_some());
        let dt = dt.unwrap();
        assert_eq!(dt.year(), 2024);
        assert_eq!(dt.month(), 6);
    }

    #[test]
    fn test_parse_datetime_naive_format() {
        let dt = parse_datetime("2024-01-15 20:30:00");
        assert!(dt.is_some());
        assert_eq!(dt.unwrap().year(), 2024);
    }

    #[test]
    fn test_parse_datetime_invalid() {
        assert!(parse_datetime("not-a-date").is_none());
    }

    #[test]
    fn test_parse_date() {
        let d = parse_date("2024-03-15");
        assert!(d.is_some());
        assert_eq!(d.unwrap().day(), 15);
    }

    #[test]
    fn test_parse_date_invalid() {
        assert!(parse_date("invalid").is_none());
    }

    #[test]
    fn test_parse_time() {
        let t = parse_time("20:00");
        assert!(t.is_some());
        assert_eq!(t.unwrap().hour(), 20);
    }

    #[test]
    fn test_parse_time_with_seconds() {
        let t = parse_time("14:30:00");
        assert!(t.is_some());
        assert_eq!(t.unwrap().minute(), 30);
    }

    #[test]
    fn test_parse_seasons_json() {
        let json = r#"[{"seasonNumber":1,"monitored":true},{"seasonNumber":2,"monitored":false}]"#;
        let seasons = parse_seasons_json(json);
        assert_eq!(seasons.len(), 2);
        assert_eq!(seasons[0].season_number, 1);
        assert!(seasons[0].monitored);
        assert_eq!(seasons[1].season_number, 2);
        assert!(!seasons[1].monitored);
    }

    #[test]
    fn test_parse_seasons_json_invalid() {
        let seasons = parse_seasons_json("not json");
        assert!(seasons.is_empty());
    }

    #[test]
    fn test_indexer_settings_parsing() {
        let json =
            r#"{"baseUrl":"http://nzbgeek.info","apiKey":"abc123","categories":[5030,5040]}"#;
        let settings: IndexerSettings = serde_json::from_str(json).unwrap();
        assert_eq!(settings.base_url.as_deref(), Some("http://nzbgeek.info"));
        assert_eq!(settings.api_key.as_deref(), Some("abc123"));
        assert_eq!(settings.categories, Some(vec![5030, 5040]));
    }

    #[test]
    fn test_download_client_settings_parsing() {
        let json = r#"{"host":"localhost","port":8080,"username":"admin","password":"secret"}"#;
        let settings: DownloadClientSettings = serde_json::from_str(json).unwrap();
        assert_eq!(settings.host.as_deref(), Some("localhost"));
        assert_eq!(settings.port, Some(8080));
    }
}
