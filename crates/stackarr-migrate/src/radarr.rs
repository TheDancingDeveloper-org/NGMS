use std::path::Path;

use anyhow::{Context, Result};
use rusqlite::{Connection, OpenFlags};
use tracing::{debug, warn};

// ---------------------------------------------------------------------------
// Intermediate structs – mirror the Radarr SQLite schema
// ---------------------------------------------------------------------------

/// Movie joined with MovieMetadata.
#[derive(Debug, Clone)]
pub struct RadarrMovie {
    // From Movies table
    pub id: i64,
    pub path: String,
    pub monitored: bool,
    pub quality_profile_id: i64,
    pub added: Option<String>,
    pub tags: Option<String>,
    pub movie_file_id: Option<i64>,
    pub minimum_availability: i32,
    // From MovieMetadata table (joined)
    pub tmdb_id: Option<i64>,
    pub imdb_id: Option<String>,
    pub images: Option<String>,
    pub genres: Option<String>,
    pub title: String,
    pub sort_title: String,
    pub clean_title: String,
    pub original_title: Option<String>,
    pub status: i32,
    pub runtime: Option<i32>,
    pub in_cinemas: Option<String>,
    pub physical_release: Option<String>,
    pub digital_release: Option<String>,
    pub year: Option<i32>,
    pub ratings: Option<String>,
    pub certification: Option<String>,
    pub studio: Option<String>,
    pub overview: Option<String>,
    pub collection_tmdb_id: Option<i64>,
    pub collection_title: Option<String>,
    /// Radarr language ID for the movie's original language.
    pub original_language: Option<i32>,
}

#[derive(Debug, Clone)]
pub struct RadarrMovieFile {
    pub id: i64,
    pub movie_id: i64,
    pub quality: Option<String>,
    pub size: i64,
    pub date_added: Option<String>,
    pub scene_name: Option<String>,
    pub media_info: Option<String>,
    pub release_group: Option<String>,
    pub relative_path: Option<String>,
    pub edition: Option<String>,
    pub languages: Option<String>,
    pub indexer_flags: i32,
}

#[derive(Debug, Clone)]
pub struct RadarrQualityProfile {
    pub id: i64,
    pub name: String,
    pub cutoff: i32,
    pub items: String,
    pub upgrade_allowed: bool,
    pub format_items: Option<String>,
    pub min_format_score: i32,
    pub cutoff_format_score: i32,
    pub min_upgrade_format_score: i32,
    /// Radarr language ID: -1=Any, -2=Original, 1=English, 2=French, etc.
    pub language: i32,
}

#[derive(Debug, Clone)]
pub struct RadarrCustomFormat {
    pub id: i64,
    pub name: String,
    pub specifications: String,
    pub include_when_renaming: bool,
}

#[derive(Debug, Clone)]
pub struct RadarrIndexer {
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
pub struct RadarrDownloadClient {
    pub id: i64,
    pub enable: bool,
    pub name: String,
    pub implementation: String,
    pub settings: Option<String>,
    pub priority: i32,
    pub tags: Option<String>,
}

#[derive(Debug, Clone)]
pub struct RadarrTag {
    pub id: i64,
    pub label: String,
}

#[derive(Debug, Clone)]
pub struct RadarrNamingConfig {
    pub replace_illegal_characters: bool,
    pub standard_movie_format: Option<String>,
    pub movie_folder_format: Option<String>,
    pub colon_replacement_format: i32,
    pub rename_movies: bool,
}

#[derive(Debug, Clone)]
pub struct RadarrHistory {
    pub id: i64,
    pub movie_id: i64,
    pub source_title: String,
    pub date: String,
    pub quality: Option<String>,
    pub data: Option<String>,
    pub event_type: i32,
    pub download_id: Option<String>,
    pub languages: Option<String>,
}

#[derive(Debug, Clone)]
pub struct RadarrBlocklist {
    pub id: i64,
    pub movie_id: i64,
    pub source_title: String,
    pub quality: Option<String>,
    pub date: Option<String>,
    pub torrent_info_hash: Option<String>,
    pub languages: Option<String>,
    pub indexer_flags: i32,
}

/// All data extracted from a Radarr SQLite database.
#[derive(Debug, Clone)]
pub struct RadarrData {
    pub movies: Vec<RadarrMovie>,
    pub movie_files: Vec<RadarrMovieFile>,
    pub quality_profiles: Vec<RadarrQualityProfile>,
    pub custom_formats: Vec<RadarrCustomFormat>,
    pub indexers: Vec<RadarrIndexer>,
    pub download_clients: Vec<RadarrDownloadClient>,
    pub root_folders: Vec<String>,
    pub tags: Vec<RadarrTag>,
    pub naming_config: Option<RadarrNamingConfig>,
    pub history: Vec<RadarrHistory>,
    pub blocklist: Vec<RadarrBlocklist>,
}

// ---------------------------------------------------------------------------
// Enum mapping helpers
// ---------------------------------------------------------------------------

pub fn map_minimum_availability(val: i32) -> &'static str {
    match val {
        1 => "announced",
        3 => "in_cinemas",
        4 => "released",
        _ => "released",
    }
}

// ---------------------------------------------------------------------------
// Read the entire Radarr database
// ---------------------------------------------------------------------------

pub fn read_radarr(path: &Path) -> Result<RadarrData> {
    let conn = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .with_context(|| format!("failed to open Radarr DB at {}", path.display()))?;

    debug!("reading Radarr database from {}", path.display());

    let movies = read_movies(&conn)?;
    let movie_files = read_movie_files(&conn)?;
    let quality_profiles = read_quality_profiles(&conn)?;
    let custom_formats = read_custom_formats(&conn)?;
    let indexers = read_indexers(&conn)?;
    let download_clients = read_download_clients(&conn)?;
    let root_folders = read_root_folders(&conn)?;
    let tags = read_tags(&conn)?;
    let naming_config = read_naming_config(&conn)?;
    let history = read_history(&conn)?;
    let blocklist = read_blocklist(&conn)?;

    debug!(
        "Radarr: {} movies, {} files, {} profiles, {} custom formats",
        movies.len(),
        movie_files.len(),
        quality_profiles.len(),
        custom_formats.len(),
    );

    Ok(RadarrData {
        movies,
        movie_files,
        quality_profiles,
        custom_formats,
        indexers,
        download_clients,
        root_folders,
        tags,
        naming_config,
        history,
        blocklist,
    })
}

fn read_movies(conn: &Connection) -> Result<Vec<RadarrMovie>> {
    let mut stmt = conn.prepare(
        "SELECT m.Id, m.Path, m.Monitored, m.QualityProfileId, m.Added, m.Tags,
                m.MovieFileId, m.MinimumAvailability,
                mm.TmdbId, mm.ImdbId, mm.Images, mm.Genres, mm.Title, mm.SortTitle,
                mm.CleanTitle, mm.OriginalTitle, mm.Status, mm.Runtime,
                mm.InCinemas, mm.PhysicalRelease, mm.DigitalRelease, mm.Year,
                mm.Ratings, mm.Certification, mm.Studio, mm.Overview,
                mm.CollectionTmdbId, mm.CollectionTitle, mm.OriginalLanguage
         FROM Movies m
         JOIN MovieMetadata mm ON m.MovieMetadataId = mm.Id",
    )?;

    let rows = stmt.query_map([], |row| {
        let file_id: Option<i64> = row.get(6)?;
        Ok(RadarrMovie {
            id: row.get(0)?,
            path: row.get(1)?,
            monitored: row.get::<_, i32>(2)? != 0,
            quality_profile_id: row.get(3)?,
            added: row.get(4)?,
            tags: row.get(5)?,
            movie_file_id: file_id.filter(|&id| id > 0),
            minimum_availability: row.get(7)?,
            tmdb_id: row.get(8)?,
            imdb_id: row.get(9)?,
            images: row.get(10)?,
            genres: row.get(11)?,
            title: row.get(12)?,
            sort_title: row.get(13)?,
            clean_title: row.get(14)?,
            original_title: row.get(15)?,
            status: row.get(16)?,
            runtime: row.get(17)?,
            in_cinemas: row.get(18)?,
            physical_release: row.get(19)?,
            digital_release: row.get(20)?,
            year: row.get(21)?,
            ratings: row.get(22)?,
            certification: row.get(23)?,
            studio: row.get(24)?,
            overview: row.get(25)?,
            collection_tmdb_id: row.get(26)?,
            collection_title: row.get(27)?,
            original_language: row.get::<_, Option<i32>>(28).unwrap_or(None),
        })
    })?;

    let mut result = Vec::new();
    for row in rows {
        match row {
            Ok(m) => result.push(m),
            Err(e) => warn!("skipping malformed Radarr movie row: {e}"),
        }
    }
    Ok(result)
}

fn read_movie_files(conn: &Connection) -> Result<Vec<RadarrMovieFile>> {
    let mut stmt = conn.prepare(
        "SELECT Id, MovieId, Quality, Size, DateAdded, SceneName,
                MediaInfo, ReleaseGroup, RelativePath, Edition, Languages, IndexerFlags
         FROM MovieFiles",
    )?;

    let rows = stmt.query_map([], |row| {
        Ok(RadarrMovieFile {
            id: row.get(0)?,
            movie_id: row.get(1)?,
            quality: row.get(2)?,
            size: row.get(3)?,
            date_added: row.get(4)?,
            scene_name: row.get(5)?,
            media_info: row.get(6)?,
            release_group: row.get(7)?,
            relative_path: row.get(8)?,
            edition: row.get(9)?,
            languages: row.get(10)?,
            indexer_flags: row.get::<_, i32>(11).unwrap_or(0),
        })
    })?;

    let mut result = Vec::new();
    for row in rows {
        match row {
            Ok(f) => result.push(f),
            Err(e) => warn!("skipping malformed Radarr movie file row: {e}"),
        }
    }
    Ok(result)
}

fn read_quality_profiles(conn: &Connection) -> Result<Vec<RadarrQualityProfile>> {
    let mut stmt = conn.prepare(
        "SELECT Id, Name, Cutoff, Items, UpgradeAllowed,
                FormatItems, MinFormatScore, CutoffFormatScore, Language,
                MinUpgradeFormatScore
         FROM QualityProfiles",
    )?;

    let rows = stmt.query_map([], |row| {
        Ok(RadarrQualityProfile {
            id: row.get(0)?,
            name: row.get(1)?,
            cutoff: row.get(2)?,
            items: row.get(3)?,
            upgrade_allowed: row.get::<_, i32>(4)? != 0,
            format_items: row.get(5)?,
            min_format_score: row.get::<_, i32>(6).unwrap_or(0),
            cutoff_format_score: row.get::<_, i32>(7).unwrap_or(0),
            // Radarr treats NULL language as English (1), not Any (-1)
            language: row.get::<_, i32>(8).unwrap_or(1),
            min_upgrade_format_score: row.get::<_, i32>(9).unwrap_or(1),
        })
    })?;

    let mut result = Vec::new();
    for row in rows {
        match row {
            Ok(p) => result.push(p),
            Err(e) => warn!("skipping malformed Radarr quality profile row: {e}"),
        }
    }
    Ok(result)
}

fn read_custom_formats(conn: &Connection) -> Result<Vec<RadarrCustomFormat>> {
    let mut stmt = conn.prepare(
        "SELECT Id, Name, Specifications, IncludeCustomFormatWhenRenaming
         FROM CustomFormats",
    )?;

    let rows = stmt.query_map([], |row| {
        Ok(RadarrCustomFormat {
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
            Err(e) => warn!("skipping malformed Radarr custom format row: {e}"),
        }
    }
    Ok(result)
}

fn read_indexers(conn: &Connection) -> Result<Vec<RadarrIndexer>> {
    let mut stmt = conn.prepare(
        "SELECT Id, Name, Implementation, Settings, EnableRss,
                EnableAutomaticSearch, EnableInteractiveSearch, Priority, Tags
         FROM Indexers",
    )?;

    let rows = stmt.query_map([], |row| {
        Ok(RadarrIndexer {
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
            Err(e) => warn!("skipping malformed Radarr indexer row: {e}"),
        }
    }
    Ok(result)
}

fn read_download_clients(conn: &Connection) -> Result<Vec<RadarrDownloadClient>> {
    let mut stmt = conn.prepare(
        "SELECT Id, Enable, Name, Implementation, Settings, Priority, Tags
         FROM DownloadClients",
    )?;

    let rows = stmt.query_map([], |row| {
        Ok(RadarrDownloadClient {
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
            Err(e) => warn!("skipping malformed Radarr download client row: {e}"),
        }
    }
    Ok(result)
}

fn read_root_folders(conn: &Connection) -> Result<Vec<String>> {
    // Radarr may or may not have a RootFolders table; extract from movie paths if missing
    let has_root_folders = conn
        .prepare("SELECT 1 FROM sqlite_master WHERE type='table' AND name='RootFolders'")
        .and_then(|mut s| s.query_row([], |_| Ok(())))
        .is_ok();

    if has_root_folders {
        let mut stmt = conn.prepare("SELECT Path FROM RootFolders")?;
        let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
        let mut result = Vec::new();
        for row in rows {
            match row {
                Ok(p) => result.push(p),
                Err(e) => warn!("skipping malformed Radarr root folder row: {e}"),
            }
        }
        Ok(result)
    } else {
        Ok(Vec::new())
    }
}

fn read_tags(conn: &Connection) -> Result<Vec<RadarrTag>> {
    let mut stmt = conn.prepare("SELECT Id, Label FROM Tags")?;
    let rows = stmt.query_map([], |row| {
        Ok(RadarrTag {
            id: row.get(0)?,
            label: row.get(1)?,
        })
    })?;

    let mut result = Vec::new();
    for row in rows {
        match row {
            Ok(t) => result.push(t),
            Err(e) => warn!("skipping malformed Radarr tag row: {e}"),
        }
    }
    Ok(result)
}

fn read_naming_config(conn: &Connection) -> Result<Option<RadarrNamingConfig>> {
    let mut stmt = conn.prepare(
        "SELECT ReplaceIllegalCharacters, StandardMovieFormat, MovieFolderFormat,
                ColonReplacementFormat, RenameMovies
         FROM NamingConfig LIMIT 1",
    )?;

    let mut rows = stmt.query_map([], |row| {
        Ok(RadarrNamingConfig {
            replace_illegal_characters: row.get::<_, i32>(0)? != 0,
            standard_movie_format: row.get(1)?,
            movie_folder_format: row.get(2)?,
            colon_replacement_format: row.get::<_, i32>(3).unwrap_or(4),
            rename_movies: row.get::<_, i32>(4)? != 0,
        })
    })?;

    match rows.next() {
        Some(Ok(nc)) => Ok(Some(nc)),
        Some(Err(e)) => {
            warn!("failed to read Radarr naming config: {e}");
            Ok(None)
        }
        None => Ok(None),
    }
}

fn read_history(conn: &Connection) -> Result<Vec<RadarrHistory>> {
    // Check if History table exists -- some Radarr installs may not have it
    let has_history = conn
        .prepare("SELECT 1 FROM sqlite_master WHERE type='table' AND name='History'")
        .and_then(|mut s| s.query_row([], |_| Ok(())))
        .is_ok();

    if !has_history {
        return Ok(Vec::new());
    }

    let mut stmt = conn.prepare(
        "SELECT Id, MovieId, SourceTitle, Date, Quality,
                Data, EventType, DownloadId, Languages
         FROM History",
    )?;

    let rows = stmt.query_map([], |row| {
        Ok(RadarrHistory {
            id: row.get(0)?,
            movie_id: row.get(1)?,
            source_title: row.get(2)?,
            date: row.get(3)?,
            quality: row.get(4)?,
            data: row.get(5)?,
            event_type: row.get(6)?,
            download_id: row.get(7)?,
            languages: row.get(8)?,
        })
    })?;

    let mut result = Vec::new();
    for row in rows {
        match row {
            Ok(h) => result.push(h),
            Err(e) => warn!("skipping malformed Radarr history row: {e}"),
        }
    }
    Ok(result)
}

fn read_blocklist(conn: &Connection) -> Result<Vec<RadarrBlocklist>> {
    let has_blocklist = conn
        .prepare("SELECT 1 FROM sqlite_master WHERE type='table' AND name='Blocklist'")
        .and_then(|mut s| s.query_row([], |_| Ok(())))
        .is_ok();

    if !has_blocklist {
        return Ok(Vec::new());
    }

    let mut stmt = conn.prepare(
        "SELECT Id, MovieId, SourceTitle, Quality,
                Date, TorrentInfoHash, Languages, IndexerFlags
         FROM Blocklist",
    )?;

    let rows = stmt.query_map([], |row| {
        Ok(RadarrBlocklist {
            id: row.get(0)?,
            movie_id: row.get(1)?,
            source_title: row.get(2)?,
            quality: row.get(3)?,
            date: row.get(4)?,
            torrent_info_hash: row.get(5)?,
            languages: row.get(6)?,
            indexer_flags: row.get::<_, i32>(7).unwrap_or(0),
        })
    })?;

    let mut result = Vec::new();
    for row in rows {
        match row {
            Ok(b) => result.push(b),
            Err(e) => warn!("skipping malformed Radarr blocklist row: {e}"),
        }
    }
    Ok(result)
}
