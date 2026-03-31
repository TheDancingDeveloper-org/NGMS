use std::collections::HashMap;

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde_json::Value as JsonValue;
use sqlx::PgPool;
use tracing::{debug, info, warn};

use crate::prowlarr::{
    ProwlarrData, map_prowlarr_indexer_type, map_prowlarr_protocol,
    parse_prowlarr_settings,
};
use crate::radarr::{RadarrData, map_minimum_availability};

/// Radarr uses different quality IDs from Sonarr/StackArr.
/// This maps Radarr quality IDs to StackArr IDs (which match Sonarr).
fn radarr_quality_id_to_stackarr(radarr_id: i64) -> i64 {
    match radarr_id {
        0 => 0,   // Unknown
        1 => 1,   // SDTV
        2 => 2,   // DVD
        3 => 11,  // WEBDL-1080p (Radarr=3, Sonarr=11)
        4 => 6,   // HDTV-720p (Radarr=4, Sonarr=6)
        5 => 7,   // WEBDL-720p (Radarr=5, Sonarr=7)
        6 => 9,   // Bluray-720p (Radarr=6, Sonarr=9)
        7 => 13,  // Bluray-1080p (Radarr=7, Sonarr=13)
        8 => 3,   // WEBDL-480p (Radarr=8, Sonarr=3)
        9 => 10,  // HDTV-1080p (Radarr=9, Sonarr=10)
        10 => 20, // Raw-HD (Radarr=10, Sonarr=20)
        12 => 4,  // WEBRip-480p (Radarr=12, Sonarr=4)
        14 => 8,  // WEBRip-720p (Radarr=14, Sonarr=8)
        15 => 12, // WEBRip-1080p (Radarr=15, Sonarr=12)
        16 => 15, // HDTV-2160p (Radarr=16, Sonarr=15)
        17 => 17, // WEBRip-2160p (same in both)
        18 => 16, // WEBDL-2160p (Radarr=18, Sonarr=16)
        19 => 18, // Bluray-2160p (Radarr=19, Sonarr=18)
        20 => 5,  // Bluray-480p (Radarr=20, Sonarr=5)
        30 => 14, // Remux-1080p (Radarr=30, Sonarr=14)
        31 => 19, // Remux-2160p (Radarr=31, Sonarr=19)
        other => other, // Unknown/Radarr-only qualities pass through
    }
}

/// Recursively normalize Radarr quality IDs in profile items JSON to StackArr IDs.
fn normalize_radarr_quality_ids(items: &JsonValue) -> JsonValue {
    match items {
        JsonValue::Array(arr) => {
            JsonValue::Array(arr.iter().map(normalize_radarr_item).collect())
        }
        other => other.clone(),
    }
}

fn normalize_radarr_item(item: &JsonValue) -> JsonValue {
    let Some(obj) = item.as_object() else {
        return item.clone();
    };
    let mut out = obj.clone();

    // Remap the quality ID
    if let Some(q) = obj.get("quality") {
        match q {
            JsonValue::Number(n) => {
                if let Some(id) = n.as_i64() {
                    let mapped = radarr_quality_id_to_stackarr(id);
                    out.insert("quality".to_string(), serde_json::json!(mapped));
                }
            }
            JsonValue::Object(qobj) => {
                if let Some(id) = qobj.get("id").and_then(|v| v.as_i64()) {
                    let mapped = radarr_quality_id_to_stackarr(id);
                    let mut qobj = qobj.clone();
                    qobj.insert("id".to_string(), serde_json::json!(mapped));
                    out.insert("quality".to_string(), JsonValue::Object(qobj));
                }
            }
            _ => {}
        }
    }

    // Recurse into nested items (quality groups)
    if let Some(nested) = obj.get("items") {
        out.insert("items".to_string(), normalize_radarr_quality_ids(nested));
    }

    JsonValue::Object(out)
}
use crate::sonarr::{
    SonarrData, map_dl_implementation_to_protocol, map_event_type,
    map_implementation_to_protocol, map_series_status, map_series_type, parse_datetime,
    parse_date, parse_download_client_settings, parse_indexer_settings, parse_seasons_json,
    parse_time,
};

// ---------------------------------------------------------------------------
// Insert structs – what we write to Postgres
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct QualityProfileInsert {
    pub name: String,
    pub cutoff: i32,
    pub upgrade_allowed: bool,
    pub min_format_score: i32,
    pub cutoff_format_score: i32,
    pub min_upgrade_format_score: i32,
    pub items: JsonValue,
    /// The source *arr old ID, used for mapping.
    pub old_id: i64,
    /// Media type: "series", "movie", or None for any.
    pub media_type: Option<String>,
    /// Radarr language preference: -1=Any, -2=Original, positive=specific.
    pub language: i32,
    /// Format scores from the source *arr FormatItems JSON.
    /// Each entry is (old_custom_format_id, score).
    pub format_scores: Vec<(i64, i32)>,
}

#[derive(Debug, Clone)]
pub struct CustomFormatInsert {
    pub name: String,
    pub specifications: JsonValue,
    pub include_when_renaming: bool,
    /// Old IDs from each source that map to this merged format.
    /// (source, old_id) where source is "sonarr" or "radarr".
    pub old_ids: Vec<(String, i64)>,
}

#[derive(Debug, Clone)]
pub struct MediaLibraryFolderInsert {
    pub path: String,
    pub media_type: String,
}

#[derive(Debug, Clone)]
pub struct NamingConfigInsert {
    pub media_type: String,
    pub rename_files: bool,
    pub standard_format: Option<String>,
    pub daily_format: Option<String>,
    pub anime_format: Option<String>,
    pub season_folder_format: Option<String>,
    pub movie_format: Option<String>,
    pub movie_folder_format: Option<String>,
    pub colon_replacement: String,
}

#[derive(Debug, Clone)]
pub struct IndexerInsert {
    pub name: String,
    pub indexer_type: String,
    pub base_url: String,
    pub api_key: Option<String>,
    pub protocol: String,
    pub categories: Option<Vec<i32>>,
    pub enabled: bool,
    pub priority: i32,
    pub supports_search: bool,
    pub supports_rss: bool,
    pub config: Option<JsonValue>,
}

#[derive(Debug, Clone)]
pub struct DownloadClientInsert {
    pub name: String,
    pub client_type: String,
    pub protocol: String,
    pub config: JsonValue,
    pub enabled: bool,
    pub priority: i32,
    /// Dedup key: name + host (from config).
    pub dedup_key: String,
}

#[derive(Debug, Clone)]
pub struct SeriesInsert {
    pub old_id: i64,
    pub title: String,
    pub clean_title: String,
    pub sort_title: String,
    pub overview: Option<String>,
    pub status: String,
    pub series_type: String,
    pub network: Option<String>,
    pub air_time: Option<chrono::NaiveTime>,
    pub first_aired: Option<chrono::NaiveDate>,
    pub year: Option<i32>,
    pub runtime: Option<i32>,
    pub path: String,
    pub quality_profile_old_id: i64,
    pub season_folder: bool,
    pub monitored: bool,
    pub use_scene_numbering: bool,
    pub tvdb_id: Option<i64>,
    pub imdb_id: Option<String>,
    pub tmdb_id: Option<i64>,
    pub tvmaze_id: Option<i64>,
    pub images: Option<JsonValue>,
    pub tags: Option<Vec<i32>>,
    pub added_at: DateTime<Utc>,
    pub seasons_json: Option<String>,
}

#[derive(Debug, Clone)]
pub struct EpisodeInsert {
    pub old_id: i64,
    pub old_series_id: i64,
    pub season_number: i32,
    pub episode_number: i32,
    pub absolute_number: Option<i32>,
    pub scene_season_number: Option<i32>,
    pub scene_episode_number: Option<i32>,
    pub scene_absolute_number: Option<i32>,
    pub title: Option<String>,
    pub overview: Option<String>,
    pub air_date: Option<chrono::NaiveDate>,
    pub air_date_utc: Option<DateTime<Utc>>,
    pub runtime: Option<i32>,
    pub monitored: bool,
    pub old_episode_file_id: Option<i64>,
    pub last_search_time: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone)]
pub struct MediaFileInsert {
    pub old_id: i64,
    pub media_type: String,
    pub relative_path: String,
    pub size: i64,
    pub date_added: DateTime<Utc>,
    pub quality: JsonValue,
    pub languages: JsonValue,
    pub scene_name: Option<String>,
    pub release_group: Option<String>,
    pub release_hash: Option<String>,
    pub edition: Option<String>,
    pub media_info: Option<JsonValue>,
    pub indexer_flags: i32,
}

#[derive(Debug, Clone)]
pub struct MovieInsert {
    pub old_id: i64,
    pub title: String,
    pub clean_title: String,
    pub sort_title: String,
    pub overview: Option<String>,
    pub year: Option<i32>,
    pub studio: Option<String>,
    pub path: String,
    pub quality_profile_old_id: i64,
    pub monitored: bool,
    pub minimum_availability: String,
    pub old_movie_file_id: Option<i64>,
    pub tmdb_id: Option<i64>,
    pub imdb_id: Option<String>,
    pub in_cinemas: Option<chrono::NaiveDate>,
    pub physical_release: Option<chrono::NaiveDate>,
    pub digital_release: Option<chrono::NaiveDate>,
    pub images: Option<JsonValue>,
    pub genres: Option<Vec<String>>,
    pub tags: Option<Vec<i32>>,
    pub collection_tmdb_id: Option<i64>,
    pub added_at: DateTime<Utc>,
    pub original_language: Option<i32>,
}

#[derive(Debug, Clone)]
pub struct HistoryInsert {
    pub media_type: String,
    /// Old media ID (series_id or movie_id from source).
    pub old_media_id: i64,
    /// Old episode ID (Sonarr only).
    pub old_episode_id: Option<i64>,
    pub event_type: String,
    pub quality: JsonValue,
    pub languages: Option<JsonValue>,
    pub source_title: String,
    pub download_id: Option<String>,
    pub data: Option<JsonValue>,
    pub occurred_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct BlocklistInsert {
    pub media_type: String,
    pub old_media_id: i64,
    pub source_title: String,
    pub quality: JsonValue,
    pub languages: Option<JsonValue>,
    pub info_hash: Option<String>,
    pub added_at: DateTime<Utc>,
}

// ---------------------------------------------------------------------------
// Merged migration data
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct MigrationData {
    pub quality_profiles: Vec<QualityProfileInsert>,
    pub custom_formats: Vec<CustomFormatInsert>,
    pub media_library_folders: Vec<MediaLibraryFolderInsert>,
    pub tags: Vec<String>,
    /// Maps old source tag IDs (Sonarr/Radarr) to their label (lowercase).
    /// Used during write to re-map old integer tag IDs on series/movies to
    /// the new PostgreSQL tag IDs via label lookup.
    pub old_tag_id_to_label: HashMap<i64, String>,
    pub naming_series: Option<NamingConfigInsert>,
    pub naming_movie: Option<NamingConfigInsert>,
    pub indexers: Vec<IndexerInsert>,
    pub download_clients: Vec<DownloadClientInsert>,
    pub series: Vec<SeriesInsert>,
    pub episodes: Vec<EpisodeInsert>,
    pub media_files: Vec<MediaFileInsert>,
    pub movies: Vec<MovieInsert>,
    pub history: Vec<HistoryInsert>,
    pub blocklist: Vec<BlocklistInsert>,
}

// ---------------------------------------------------------------------------
// Migration report
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MigrationReport {
    pub series_imported: usize,
    pub movies_imported: usize,
    pub episodes_imported: usize,
    pub media_files_imported: usize,
    pub quality_profiles_imported: usize,
    pub custom_formats_imported: usize,
    pub format_scores_imported: usize,
    pub indexers_imported: usize,
    pub download_clients_imported: usize,
    pub history_events_imported: usize,
    pub blocklist_entries_imported: usize,
    pub warnings: Vec<String>,
    pub dry_run: bool,
}

impl std::fmt::Display for MigrationReport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Migration Report (dry_run={})", self.dry_run)?;
        writeln!(f, "  Quality profiles: {}", self.quality_profiles_imported)?;
        writeln!(f, "  Custom formats:   {}", self.custom_formats_imported)?;
        writeln!(f, "  Format scores:    {}", self.format_scores_imported)?;
        writeln!(f, "  Indexers:          {}", self.indexers_imported)?;
        writeln!(f, "  Download clients:  {}", self.download_clients_imported)?;
        writeln!(f, "  Series:            {}", self.series_imported)?;
        writeln!(f, "  Episodes:          {}", self.episodes_imported)?;
        writeln!(f, "  Movies:            {}", self.movies_imported)?;
        writeln!(f, "  Media files:       {}", self.media_files_imported)?;
        writeln!(f, "  History events:    {}", self.history_events_imported)?;
        writeln!(f, "  Blocklist entries: {}", self.blocklist_entries_imported)?;
        if !self.warnings.is_empty() {
            writeln!(f, "  Warnings:")?;
            for w in &self.warnings {
                writeln!(f, "    - {w}")?;
            }
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Colon replacement mapping
// ---------------------------------------------------------------------------

fn colon_replacement_str(val: i32) -> &'static str {
    match val {
        0 => "delete",
        1 => "dash",
        2 => "space_dash",
        3 => "space_dash_space",
        4 => "smart",
        _ => "smart",
    }
}

// ---------------------------------------------------------------------------
// Build MigrationData from source databases
// ---------------------------------------------------------------------------

pub fn build_migration_data(
    sonarr: Option<&SonarrData>,
    radarr: Option<&RadarrData>,
    prowlarr: Option<&ProwlarrData>,
) -> (MigrationData, Vec<String>) {
    let mut warnings: Vec<String> = Vec::new();

    // --- Tags (merge by label, case-insensitive) ---
    let mut tag_set: Vec<String> = Vec::new();
    let mut seen_tags: HashMap<String, usize> = HashMap::new();
    // Map old source tag IDs → lowercase label for re-mapping during write.
    let mut old_tag_id_to_label: HashMap<i64, String> = HashMap::new();

    let mut add_tags = |labels: &[(i64, String)], id_map: &mut HashMap<i64, String>| {
        for (old_id, label) in labels {
            let lower = label.to_lowercase();
            id_map.insert(*old_id, lower.clone());
            if !seen_tags.contains_key(&lower) {
                seen_tags.insert(lower, tag_set.len());
                tag_set.push(label.clone());
            }
        }
    };

    if let Some(s) = sonarr {
        let pairs: Vec<_> = s.tags.iter().map(|t| (t.id, t.label.clone())).collect();
        add_tags(&pairs, &mut old_tag_id_to_label);
    }
    if let Some(r) = radarr {
        let pairs: Vec<_> = r.tags.iter().map(|t| (t.id, t.label.clone())).collect();
        add_tags(&pairs, &mut old_tag_id_to_label);
    }
    if let Some(p) = prowlarr {
        let pairs: Vec<_> = p.tags.iter().map(|t| (t.id, t.label.clone())).collect();
        add_tags(&pairs, &mut old_tag_id_to_label);
    }

    // --- Custom formats (merge Sonarr + Radarr by name) ---
    // Both Sonarr and Radarr often have identical custom formats (e.g. from
    // TRaSH guides). We merge by name and track all old IDs for score remapping.
    let mut custom_formats: Vec<CustomFormatInsert> = Vec::new();
    // Map (source, old_id) → index in custom_formats vec
    let mut cf_name_to_idx: HashMap<String, usize> = HashMap::new();
    // Map (source, old_id) → index in custom_formats vec (for score remapping)
    let mut cf_old_id_to_idx: HashMap<(String, i64), usize> = HashMap::new();

    if let Some(s) = sonarr {
        for cf in &s.custom_formats {
            let lower = cf.name.to_lowercase();
            let specs: JsonValue =
                serde_json::from_str(&cf.specifications).unwrap_or(JsonValue::Array(vec![]));
            if let Some(&idx) = cf_name_to_idx.get(&lower) {
                // Already exists — just add the old ID mapping
                custom_formats[idx].old_ids.push(("sonarr".to_string(), cf.id));
            } else {
                let idx = custom_formats.len();
                cf_name_to_idx.insert(lower, idx);
                custom_formats.push(CustomFormatInsert {
                    name: cf.name.clone(),
                    specifications: specs,
                    include_when_renaming: cf.include_when_renaming,
                    old_ids: vec![("sonarr".to_string(), cf.id)],
                });
            }
            cf_old_id_to_idx.insert(("sonarr".to_string(), cf.id), cf_name_to_idx[&cf.name.to_lowercase()]);
        }
    }

    if let Some(r) = radarr {
        for cf in &r.custom_formats {
            let lower = cf.name.to_lowercase();
            let specs: JsonValue =
                serde_json::from_str(&cf.specifications).unwrap_or(JsonValue::Array(vec![]));
            if let Some(&idx) = cf_name_to_idx.get(&lower) {
                // Already exists from Sonarr — just add the Radarr old ID
                custom_formats[idx].old_ids.push(("radarr".to_string(), cf.id));
            } else {
                let idx = custom_formats.len();
                cf_name_to_idx.insert(lower, idx);
                custom_formats.push(CustomFormatInsert {
                    name: cf.name.clone(),
                    specifications: specs,
                    include_when_renaming: cf.include_when_renaming,
                    old_ids: vec![("radarr".to_string(), cf.id)],
                });
            }
            cf_old_id_to_idx.insert(("radarr".to_string(), cf.id), cf_name_to_idx[&cf.name.to_lowercase()]);
        }
    }

    // Helper: parse FormatItems JSON → vec of (old_cf_id, score), skipping score=0
    fn parse_format_scores(json: &str) -> Vec<(i64, i32)> {
        #[derive(serde::Deserialize)]
        struct FormatItem {
            format: i64,
            score: i32,
        }
        serde_json::from_str::<Vec<FormatItem>>(json)
            .unwrap_or_default()
            .into_iter()
            .filter(|fi| fi.score != 0)
            .map(|fi| (fi.format, fi.score))
            .collect()
    }

    // --- Quality profiles (Sonarr + Radarr imported separately) ---
    // Sonarr and Radarr may share profile names but have different items
    // (e.g., Remux-2160p enabled in one but not the other). Import both
    // so each media type uses the correct profile version.
    let mut profiles: Vec<QualityProfileInsert> = Vec::new();
    let mut sonarr_profile_names: HashMap<String, usize> = HashMap::new();

    if let Some(s) = sonarr {
        for p in &s.quality_profiles {
            let items: JsonValue =
                serde_json::from_str(&p.items).unwrap_or(JsonValue::Array(vec![]));
            let format_scores = p.format_items.as_deref()
                .map(parse_format_scores)
                .unwrap_or_default()
                .into_iter()
                .filter_map(|(old_id, score)| {
                    cf_old_id_to_idx.get(&("sonarr".to_string(), old_id))
                        .map(|&idx| (idx as i64, score))
                })
                .collect();
            profiles.push(QualityProfileInsert {
                name: p.name.clone(),
                cutoff: p.cutoff,
                upgrade_allowed: p.upgrade_allowed,
                min_format_score: p.min_format_score,
                cutoff_format_score: p.cutoff_format_score,
                min_upgrade_format_score: p.min_upgrade_format_score,
                items,
                old_id: p.id,
                media_type: Some("series".to_string()),
                language: -1, // Sonarr v4 has no profile-level language filter
                format_scores,
            });
            sonarr_profile_names.insert(p.name.to_lowercase(), profiles.len() - 1);
        }
    }

    // Offset Radarr old_ids to avoid collisions with Sonarr old_ids
    // in the profile_id_map (both sources can have id=1, id=2, etc.).
    const RADARR_PROFILE_OFFSET: i64 = 100_000;

    if let Some(r) = radarr {
        for p in &r.quality_profiles {
            let raw_items: JsonValue =
                serde_json::from_str(&p.items).unwrap_or(JsonValue::Array(vec![]));
            // Remap Radarr quality IDs to StackArr/Sonarr numbering
            let items = normalize_radarr_quality_ids(&raw_items);
            let cutoff = radarr_quality_id_to_stackarr(p.cutoff as i64) as i32;
            let lower = p.name.to_lowercase();
            // If a Sonarr profile has the same name, import the Radarr
            // version with a " (Movie)" suffix so movies get the correct items.
            let name = if sonarr_profile_names.contains_key(&lower) {
                format!("{} (Movie)", p.name)
            } else {
                p.name.clone()
            };
            let format_scores = p.format_items.as_deref()
                .map(parse_format_scores)
                .unwrap_or_default()
                .into_iter()
                .filter_map(|(old_id, score)| {
                    cf_old_id_to_idx.get(&("radarr".to_string(), old_id))
                        .map(|&idx| (idx as i64, score))
                })
                .collect();
            profiles.push(QualityProfileInsert {
                name,
                cutoff,
                upgrade_allowed: p.upgrade_allowed,
                min_format_score: p.min_format_score,
                cutoff_format_score: p.cutoff_format_score,
                min_upgrade_format_score: p.min_upgrade_format_score,
                items,
                old_id: p.id + RADARR_PROFILE_OFFSET,
                media_type: Some("movie".to_string()),
                language: p.language,
                format_scores,
            });
        }
    }

    // --- Media library folders ---
    let mut media_library_folders: Vec<MediaLibraryFolderInsert> = Vec::new();
    let mut seen_root_paths: HashMap<String, ()> = HashMap::new();

    if let Some(s) = sonarr {
        for path in &s.root_folders {
            if !seen_root_paths.contains_key(path) {
                seen_root_paths.insert(path.clone(), ());
                media_library_folders.push(MediaLibraryFolderInsert {
                    path: path.clone(),
                    media_type: "series".to_string(),
                });
            }
        }
    }
    if let Some(r) = radarr {
        for path in &r.root_folders {
            if !seen_root_paths.contains_key(path) {
                seen_root_paths.insert(path.clone(), ());
                media_library_folders.push(MediaLibraryFolderInsert {
                    path: path.clone(),
                    media_type: "movie".to_string(),
                });
            }
        }
    }

    // --- Naming config ---
    let naming_series = sonarr.and_then(|s| {
        s.naming_config.as_ref().map(|nc| NamingConfigInsert {
            media_type: "series".to_string(),
            rename_files: nc.rename_episodes,
            standard_format: nc.standard_episode_format.clone(),
            daily_format: nc.daily_episode_format.clone(),
            anime_format: nc.anime_episode_format.clone(),
            season_folder_format: nc.season_folder_format.clone(),
            movie_format: None,
            movie_folder_format: None,
            colon_replacement: colon_replacement_str(nc.colon_replacement_format).to_string(),
        })
    });

    let naming_movie = radarr.and_then(|r| {
        r.naming_config.as_ref().map(|nc| NamingConfigInsert {
            media_type: "movie".to_string(),
            rename_files: nc.rename_movies,
            standard_format: None,
            daily_format: None,
            anime_format: None,
            season_folder_format: None,
            movie_format: nc.standard_movie_format.clone(),
            movie_folder_format: nc.movie_folder_format.clone(),
            colon_replacement: colon_replacement_str(nc.colon_replacement_format).to_string(),
        })
    });

    // --- Indexers (Prowlarr takes priority, dedup by base_url) ---
    let mut indexers: Vec<IndexerInsert> = Vec::new();
    let mut seen_indexer_urls: HashMap<String, usize> = HashMap::new();

    if let Some(p) = prowlarr {
        for idx in &p.indexers {
            let settings = idx
                .settings
                .as_deref()
                .map(parse_prowlarr_settings)
                .unwrap_or_default();
            let base_url = settings.base_url.clone().unwrap_or_default();
            if base_url.is_empty() {
                warnings.push(format!(
                    "Prowlarr indexer '{}' has no base URL, skipping",
                    idx.name
                ));
                continue;
            }

            let protocol = map_prowlarr_protocol(&idx.implementation);
            let indexer_type = map_prowlarr_indexer_type(&idx.implementation);

            seen_indexer_urls.insert(base_url.clone(), indexers.len());
            indexers.push(IndexerInsert {
                name: idx.name.clone(),
                indexer_type: indexer_type.to_string(),
                base_url,
                api_key: settings.api_key,
                protocol: protocol.to_string(),
                categories: settings.categories,
                enabled: idx.enable,
                priority: idx.priority,
                supports_search: true,
                supports_rss: true,
                config: idx
                    .settings
                    .as_deref()
                    .and_then(|s| serde_json::from_str(s).ok()),
            });
        }
    }

    // Sonarr indexers (dedup against Prowlarr by base_url)
    if let Some(s) = sonarr {
        for idx in &s.indexers {
            let settings = idx
                .settings
                .as_deref()
                .map(parse_indexer_settings)
                .unwrap_or_default();
            let base_url = settings.base_url.clone().unwrap_or_default();
            if base_url.is_empty() {
                warnings.push(format!(
                    "Sonarr indexer '{}' has no base URL, skipping",
                    idx.name
                ));
                continue;
            }
            if seen_indexer_urls.contains_key(&base_url) {
                continue; // Already imported from Prowlarr
            }

            let protocol = map_implementation_to_protocol(&idx.implementation);

            seen_indexer_urls.insert(base_url.clone(), indexers.len());
            indexers.push(IndexerInsert {
                name: idx.name.clone(),
                indexer_type: idx.implementation.clone(),
                base_url,
                api_key: settings.api_key,
                protocol: protocol.to_string(),
                categories: settings.categories,
                enabled: idx.enable_rss || idx.enable_automatic_search,
                priority: idx.priority,
                supports_search: idx.enable_automatic_search || idx.enable_interactive_search,
                supports_rss: idx.enable_rss,
                config: idx
                    .settings
                    .as_deref()
                    .and_then(|s| serde_json::from_str(s).ok()),
            });
        }
    }

    // Radarr indexers (dedup against everything)
    if let Some(r) = radarr {
        for idx in &r.indexers {
            let settings = idx
                .settings
                .as_deref()
                .map(parse_indexer_settings)
                .unwrap_or_default();
            let base_url = settings.base_url.clone().unwrap_or_default();
            if base_url.is_empty() || seen_indexer_urls.contains_key(&base_url) {
                continue;
            }

            let protocol = map_implementation_to_protocol(&idx.implementation);

            seen_indexer_urls.insert(base_url.clone(), indexers.len());
            indexers.push(IndexerInsert {
                name: idx.name.clone(),
                indexer_type: idx.implementation.clone(),
                base_url,
                api_key: settings.api_key,
                protocol: protocol.to_string(),
                categories: settings.categories,
                enabled: idx.enable_rss || idx.enable_automatic_search,
                priority: idx.priority,
                supports_search: idx.enable_automatic_search || idx.enable_interactive_search,
                supports_rss: idx.enable_rss,
                config: idx
                    .settings
                    .as_deref()
                    .and_then(|s| serde_json::from_str(s).ok()),
            });
        }
    }

    // --- Download clients (dedup by name+host) ---
    let mut download_clients: Vec<DownloadClientInsert> = Vec::new();
    let mut seen_dl_keys: HashMap<String, usize> = HashMap::new();

    let mut add_download_client =
        |name: &str, implementation: &str, settings_json: Option<&str>, enabled: bool, priority: i32| {
            let settings = settings_json
                .map(parse_download_client_settings)
                .unwrap_or_default();
            let host = settings.host.clone().unwrap_or_default();
            let port = settings.port.unwrap_or(0);
            // Dedup by implementation+host+port so the same physical client with
            // different names in Sonarr vs Radarr doesn't create duplicates.
            let dedup_key = format!("{}:{}:{}", implementation.to_lowercase(), host.to_lowercase(), port);

            if seen_dl_keys.contains_key(&dedup_key) {
                return;
            }

            let protocol = map_dl_implementation_to_protocol(implementation);
            let config: JsonValue = settings_json
                .and_then(|s| serde_json::from_str(s).ok())
                .unwrap_or(JsonValue::Object(serde_json::Map::new()));

            seen_dl_keys.insert(dedup_key.clone(), download_clients.len());
            download_clients.push(DownloadClientInsert {
                name: name.to_string(),
                client_type: implementation.to_string(),
                protocol: protocol.to_string(),
                config,
                enabled,
                priority,
                dedup_key,
            });
        };

    if let Some(s) = sonarr {
        for dc in &s.download_clients {
            add_download_client(
                &dc.name,
                &dc.implementation,
                dc.settings.as_deref(),
                dc.enable,
                dc.priority,
            );
        }
    }
    if let Some(r) = radarr {
        for dc in &r.download_clients {
            add_download_client(
                &dc.name,
                &dc.implementation,
                dc.settings.as_deref(),
                dc.enable,
                dc.priority,
            );
        }
    }

    // --- Series ---
    let mut series_inserts: Vec<SeriesInsert> = Vec::new();
    if let Some(s) = sonarr {
        for sr in &s.series {
            let images: Option<JsonValue> = sr
                .images
                .as_deref()
                .and_then(|s| serde_json::from_str(s).ok());
            let tags: Option<Vec<i32>> = sr
                .tags
                .as_deref()
                .and_then(|s| serde_json::from_str(s).ok());
            let added_at = sr
                .added
                .as_deref()
                .and_then(parse_datetime)
                .unwrap_or_else(Utc::now);
            let first_aired = sr.first_aired.as_deref().and_then(|s| {
                parse_date(s).or_else(|| parse_datetime(s).map(|dt| dt.date_naive()))
            });

            series_inserts.push(SeriesInsert {
                old_id: sr.id,
                title: sr.title.clone(),
                clean_title: sr.clean_title.clone(),
                sort_title: sr.sort_title.clone(),
                overview: sr.overview.clone(),
                status: map_series_status(sr.status).to_string(),
                series_type: map_series_type(sr.series_type).to_string(),
                network: sr.network.clone(),
                air_time: sr.air_time.as_deref().and_then(parse_time),
                first_aired,
                year: sr.year,
                runtime: sr.runtime,
                path: sr.path.clone(),
                quality_profile_old_id: sr.quality_profile_id,
                season_folder: sr.season_folder,
                monitored: sr.monitored,
                use_scene_numbering: sr.use_scene_numbering,
                tvdb_id: sr.tvdb_id,
                imdb_id: sr.imdb_id.clone(),
                tmdb_id: sr.tmdb_id,
                tvmaze_id: sr.tvmaze_id,
                images,
                tags,
                added_at,
                seasons_json: sr.seasons.clone(),
            });
        }
    }

    // --- Episodes ---
    let mut episode_inserts: Vec<EpisodeInsert> = Vec::new();
    if let Some(s) = sonarr {
        for ep in &s.episodes {
            episode_inserts.push(EpisodeInsert {
                old_id: ep.id,
                old_series_id: ep.series_id,
                season_number: ep.season_number,
                episode_number: ep.episode_number,
                absolute_number: ep.absolute_episode_number,
                scene_season_number: ep.scene_season_number,
                scene_episode_number: ep.scene_episode_number,
                scene_absolute_number: ep.scene_absolute_episode_number,
                title: ep.title.clone(),
                overview: ep.overview.clone(),
                air_date: ep.air_date.as_deref().and_then(parse_date),
                air_date_utc: ep.air_date_utc.as_deref().and_then(parse_datetime),
                runtime: ep.runtime,
                monitored: ep.monitored,
                old_episode_file_id: ep.episode_file_id,
                last_search_time: ep.last_search_time.as_deref().and_then(parse_datetime),
            });
        }
    }

    // --- Media files (episodes + movies) ---
    let mut media_file_inserts: Vec<MediaFileInsert> = Vec::new();

    if let Some(s) = sonarr {
        for ef in &s.episode_files {
            let quality: JsonValue = ef
                .quality
                .as_deref()
                .and_then(|s| serde_json::from_str(s).ok())
                .unwrap_or(JsonValue::Object(serde_json::Map::new()));
            let languages: JsonValue = ef
                .languages
                .as_deref()
                .and_then(|s| serde_json::from_str(s).ok())
                .unwrap_or(JsonValue::Array(vec![]));
            let media_info: Option<JsonValue> = ef
                .media_info
                .as_deref()
                .and_then(|s| serde_json::from_str(s).ok());
            let date_added = ef
                .date_added
                .as_deref()
                .and_then(parse_datetime)
                .unwrap_or_else(Utc::now);

            media_file_inserts.push(MediaFileInsert {
                old_id: ef.id,
                media_type: "series".to_string(),
                relative_path: ef.relative_path.clone().unwrap_or_default(),
                size: ef.size,
                date_added,
                quality,
                languages,
                scene_name: ef.scene_name.clone(),
                release_group: ef.release_group.clone(),
                release_hash: ef.release_hash.clone(),
                edition: None,
                media_info,
                indexer_flags: ef.indexer_flags,
            });
        }
    }

    if let Some(r) = radarr {
        for mf in &r.movie_files {
            let quality: JsonValue = mf
                .quality
                .as_deref()
                .and_then(|s| serde_json::from_str(s).ok())
                .unwrap_or(JsonValue::Object(serde_json::Map::new()));
            let languages: JsonValue = mf
                .languages
                .as_deref()
                .and_then(|s| serde_json::from_str(s).ok())
                .unwrap_or(JsonValue::Array(vec![]));
            let media_info: Option<JsonValue> = mf
                .media_info
                .as_deref()
                .and_then(|s| serde_json::from_str(s).ok());
            let date_added = mf
                .date_added
                .as_deref()
                .and_then(parse_datetime)
                .unwrap_or_else(Utc::now);

            media_file_inserts.push(MediaFileInsert {
                old_id: mf.id,
                media_type: "movie".to_string(),
                relative_path: mf.relative_path.clone().unwrap_or_default(),
                size: mf.size,
                date_added,
                quality,
                languages,
                scene_name: mf.scene_name.clone(),
                release_group: mf.release_group.clone(),
                release_hash: None,
                edition: mf.edition.clone(),
                media_info,
                indexer_flags: mf.indexer_flags,
            });
        }
    }

    // --- Movies ---
    let mut movie_inserts: Vec<MovieInsert> = Vec::new();
    if let Some(r) = radarr {
        for mv in &r.movies {
            let images: Option<JsonValue> = mv
                .images
                .as_deref()
                .and_then(|s| serde_json::from_str(s).ok());
            let genres: Option<Vec<String>> = mv
                .genres
                .as_deref()
                .and_then(|s| serde_json::from_str(s).ok());
            let tags: Option<Vec<i32>> = mv
                .tags
                .as_deref()
                .and_then(|s| serde_json::from_str(s).ok());
            let added_at = mv
                .added
                .as_deref()
                .and_then(parse_datetime)
                .unwrap_or_else(Utc::now);
            let in_cinemas = mv.in_cinemas.as_deref().and_then(|s| {
                parse_date(s).or_else(|| parse_datetime(s).map(|dt| dt.date_naive()))
            });
            let physical_release = mv.physical_release.as_deref().and_then(|s| {
                parse_date(s).or_else(|| parse_datetime(s).map(|dt| dt.date_naive()))
            });
            let digital_release = mv.digital_release.as_deref().and_then(|s| {
                parse_date(s).or_else(|| parse_datetime(s).map(|dt| dt.date_naive()))
            });

            movie_inserts.push(MovieInsert {
                old_id: mv.id,
                title: mv.title.clone(),
                clean_title: mv.clean_title.clone(),
                sort_title: mv.sort_title.clone(),
                overview: mv.overview.clone(),
                year: mv.year,
                studio: mv.studio.clone(),
                path: mv.path.clone(),
                quality_profile_old_id: mv.quality_profile_id + RADARR_PROFILE_OFFSET,
                monitored: mv.monitored,
                minimum_availability: map_minimum_availability(mv.minimum_availability)
                    .to_string(),
                old_movie_file_id: mv.movie_file_id,
                tmdb_id: mv.tmdb_id,
                imdb_id: mv.imdb_id.clone(),
                in_cinemas,
                physical_release,
                digital_release,
                images,
                genres,
                tags,
                collection_tmdb_id: mv.collection_tmdb_id,
                added_at,
                original_language: mv.original_language,
            });
        }
    }

    // --- History ---
    let mut history_inserts: Vec<HistoryInsert> = Vec::new();

    if let Some(s) = sonarr {
        for h in &s.history {
            let quality: JsonValue = h
                .quality
                .as_deref()
                .and_then(|s| serde_json::from_str(s).ok())
                .unwrap_or(JsonValue::Object(serde_json::Map::new()));
            let languages: Option<JsonValue> = h
                .languages
                .as_deref()
                .and_then(|s| serde_json::from_str(s).ok());
            let data: Option<JsonValue> = h
                .data
                .as_deref()
                .and_then(|s| serde_json::from_str(s).ok());
            let occurred_at = parse_datetime(&h.date).unwrap_or_else(Utc::now);

            history_inserts.push(HistoryInsert {
                media_type: "series".to_string(),
                old_media_id: h.series_id,
                old_episode_id: Some(h.episode_id),
                event_type: map_event_type(h.event_type).to_string(),
                quality,
                languages,
                source_title: h.source_title.clone(),
                download_id: h.download_id.clone(),
                data,
                occurred_at,
            });
        }
    }

    if let Some(r) = radarr {
        for h in &r.history {
            let quality: JsonValue = h
                .quality
                .as_deref()
                .and_then(|s| serde_json::from_str(s).ok())
                .unwrap_or(JsonValue::Object(serde_json::Map::new()));
            let languages: Option<JsonValue> = h
                .languages
                .as_deref()
                .and_then(|s| serde_json::from_str(s).ok());
            let data: Option<JsonValue> = h
                .data
                .as_deref()
                .and_then(|s| serde_json::from_str(s).ok());
            let occurred_at = parse_datetime(&h.date).unwrap_or_else(Utc::now);

            history_inserts.push(HistoryInsert {
                media_type: "movie".to_string(),
                old_media_id: h.movie_id,
                old_episode_id: None,
                event_type: map_event_type(h.event_type).to_string(),
                quality,
                languages,
                source_title: h.source_title.clone(),
                download_id: h.download_id.clone(),
                data,
                occurred_at,
            });
        }
    }

    // --- Blocklist ---
    let mut blocklist_inserts: Vec<BlocklistInsert> = Vec::new();

    if let Some(s) = sonarr {
        for b in &s.blocklist {
            let quality: JsonValue = b
                .quality
                .as_deref()
                .and_then(|s| serde_json::from_str(s).ok())
                .unwrap_or(JsonValue::Object(serde_json::Map::new()));
            let languages: Option<JsonValue> = b
                .languages
                .as_deref()
                .and_then(|s| serde_json::from_str(s).ok());
            let added_at = b
                .date
                .as_deref()
                .and_then(parse_datetime)
                .unwrap_or_else(Utc::now);

            blocklist_inserts.push(BlocklistInsert {
                media_type: "series".to_string(),
                old_media_id: b.series_id,
                source_title: b.source_title.clone(),
                quality,
                languages,
                info_hash: b.torrent_info_hash.clone(),
                added_at,
            });
        }
    }

    if let Some(r) = radarr {
        for b in &r.blocklist {
            let quality: JsonValue = b
                .quality
                .as_deref()
                .and_then(|s| serde_json::from_str(s).ok())
                .unwrap_or(JsonValue::Object(serde_json::Map::new()));
            let languages: Option<JsonValue> = b
                .languages
                .as_deref()
                .and_then(|s| serde_json::from_str(s).ok());
            let added_at = b
                .date
                .as_deref()
                .and_then(parse_datetime)
                .unwrap_or_else(Utc::now);

            blocklist_inserts.push(BlocklistInsert {
                media_type: "movie".to_string(),
                old_media_id: b.movie_id,
                source_title: b.source_title.clone(),
                quality,
                languages,
                info_hash: b.torrent_info_hash.clone(),
                added_at,
            });
        }
    }

    let data = MigrationData {
        quality_profiles: profiles,
        custom_formats,
        media_library_folders,
        tags: tag_set,
        old_tag_id_to_label,
        naming_series,
        naming_movie,
        indexers,
        download_clients,
        series: series_inserts,
        episodes: episode_inserts,
        media_files: media_file_inserts,
        movies: movie_inserts,
        history: history_inserts,
        blocklist: blocklist_inserts,
    };

    (data, warnings)
}

// ---------------------------------------------------------------------------
// Migration writer
// ---------------------------------------------------------------------------

pub struct MigrationWriter {
    pool: PgPool,
}

impl MigrationWriter {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn write_all(&self, data: MigrationData) -> Result<MigrationReport> {
        let mut report = MigrationReport {
            series_imported: 0,
            movies_imported: 0,
            episodes_imported: 0,
            media_files_imported: 0,
            quality_profiles_imported: 0,
            custom_formats_imported: 0,
            format_scores_imported: 0,
            indexers_imported: 0,
            download_clients_imported: 0,
            history_events_imported: 0,
            blocklist_entries_imported: 0,
            warnings: Vec::new(),
            dry_run: false,
        };

        // All writes happen in one transaction for atomicity.
        let mut tx = self.pool.begin().await.context("begin transaction")?;

        // 1. Tags
        let tag_id_map = self.write_tags(&mut tx, &data.tags).await?;
        debug!("wrote {} tags", tag_id_map.len());

        // 2. Custom formats (must come before quality profiles so we have new IDs for scores)
        let cf_id_map = self
            .write_custom_formats(&mut tx, &data.custom_formats)
            .await?;
        report.custom_formats_imported = cf_id_map.len();
        debug!("wrote {} custom formats", cf_id_map.len());

        // 3. Quality profiles (map old_id -> new_id)
        let profile_id_map = self
            .write_quality_profiles(&mut tx, &data.quality_profiles)
            .await?;
        report.quality_profiles_imported = profile_id_map.len();
        debug!("wrote {} quality profiles", profile_id_map.len());

        // 3b. Format scores (link profiles to custom formats)
        let scores_count = self
            .write_format_scores(&mut tx, &data.quality_profiles, &profile_id_map, &cf_id_map)
            .await?;
        report.format_scores_imported = scores_count;
        debug!("wrote {} format scores", scores_count);

        // Build a name->new_id map for Radarr profiles that were deduped by name.
        let profile_name_map: HashMap<String, i64> = {
            let mut m = HashMap::new();
            for p in &data.quality_profiles {
                if let Some(&new_id) = profile_id_map.get(&p.old_id) {
                    m.insert(p.name.to_lowercase(), new_id);
                }
            }
            m
        };

        // 3. Media library folders
        let media_library_folder_id_map = self.write_media_library_folders(&mut tx, &data.media_library_folders).await?;
        debug!("wrote {} media library folders", media_library_folder_id_map.len());

        // 4. Naming config
        if let Some(ref nc) = data.naming_series {
            self.write_naming_config(&mut tx, nc).await?;
        }
        if let Some(ref nc) = data.naming_movie {
            self.write_naming_config(&mut tx, nc).await?;
        }

        // 5. Indexers
        let indexer_count = self.write_indexers(&mut tx, &data.indexers).await?;
        report.indexers_imported = indexer_count;

        // 6. Download clients
        let dl_count = self
            .write_download_clients(&mut tx, &data.download_clients)
            .await?;
        report.download_clients_imported = dl_count;

        // 7. Series
        let series_id_map = self
            .write_series(
                &mut tx,
                &data.series,
                &profile_id_map,
                &profile_name_map,
                &media_library_folder_id_map,
                &tag_id_map,
                &data.old_tag_id_to_label,
            )
            .await?;
        report.series_imported = series_id_map.len();
        debug!("wrote {} series", series_id_map.len());

        // 8. Media files (before episodes, because episodes reference file IDs)
        // We need separate maps for series files vs movie files because old_ids
        // can collide between Sonarr EpisodeFiles and Radarr MovieFiles.
        let (series_file_id_map, movie_file_id_map) = self
            .write_media_files(&mut tx, &data.media_files)
            .await?;
        report.media_files_imported = series_file_id_map.len() + movie_file_id_map.len();
        debug!(
            "wrote {} media files ({} series, {} movie)",
            report.media_files_imported,
            series_file_id_map.len(),
            movie_file_id_map.len()
        );

        // 9. Episodes (with mapped series_id and episode_file_id)
        let episode_id_map = self
            .write_episodes(
                &mut tx,
                &data.episodes,
                &series_id_map,
                &series_file_id_map,
            )
            .await?;
        report.episodes_imported = episode_id_map.len();

        // 10. Seasons (extracted from series JSON)
        self.write_seasons(&mut tx, &data.series, &series_id_map)
            .await?;

        // 11. Movies
        let movie_id_map = self
            .write_movies(
                &mut tx,
                &data.movies,
                &profile_id_map,
                &profile_name_map,
                &media_library_folder_id_map,
                &movie_file_id_map,
                &tag_id_map,
                &data.old_tag_id_to_label,
            )
            .await?;
        report.movies_imported = movie_id_map.len();

        // 12. History — skipped: imported history from Sonarr/Radarr is
        //     confusing to the end user (events they never triggered in StackArr).
        report.history_events_imported = 0;

        // 13. Blocklist
        let blocklist_count = self
            .write_blocklist(&mut tx, &data.blocklist, &series_id_map, &movie_id_map)
            .await?;
        report.blocklist_entries_imported = blocklist_count;

        tx.commit().await.context("commit transaction")?;
        info!("migration committed successfully");

        Ok(report)
    }

    // -- Tag writer --

    async fn write_tags(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        tags: &[String],
    ) -> Result<HashMap<String, i64>> {
        let mut map = HashMap::new();
        for label in tags {
            let row: (i32,) = sqlx::query_as(
                "INSERT INTO tags (label) VALUES ($1)
                 ON CONFLICT (label) DO UPDATE SET label = tags.label
                 RETURNING id",
            )
            .bind(label)
            .fetch_one(&mut **tx)
            .await
            .with_context(|| format!("insert tag '{label}'"))?;
            map.insert(label.to_lowercase(), row.0 as i64);
        }
        Ok(map)
    }

    // -- Quality profile writer --

    async fn write_quality_profiles(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        profiles: &[QualityProfileInsert],
    ) -> Result<HashMap<i64, i64>> {
        let mut map = HashMap::new();
        for p in profiles {
            let row: (i32,) = sqlx::query_as(
                "INSERT INTO quality_profiles (name, cutoff, upgrade_allowed, min_format_score, cutoff_format_score, min_upgrade_format_score, items, media_type, language)
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
                 RETURNING id",
            )
            .bind(&p.name)
            .bind(p.cutoff)
            .bind(p.upgrade_allowed)
            .bind(p.min_format_score)
            .bind(p.cutoff_format_score)
            .bind(p.min_upgrade_format_score)
            .bind(&p.items)
            .bind(&p.media_type)
            .bind(p.language)
            .fetch_one(&mut **tx)
            .await
            .with_context(|| format!("insert quality profile '{}'", p.name))?;
            map.insert(p.old_id, row.0 as i64);
        }
        Ok(map)
    }

    // -- Custom format writer --

    async fn write_custom_formats(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        formats: &[CustomFormatInsert],
    ) -> Result<HashMap<usize, i64>> {
        let mut map = HashMap::new();
        for (idx, cf) in formats.iter().enumerate() {
            let row: (i32,) = sqlx::query_as(
                "INSERT INTO custom_formats (name, specifications, include_custom_format_when_renaming)
                 VALUES ($1, $2, $3)
                 RETURNING id",
            )
            .bind(&cf.name)
            .bind(&cf.specifications)
            .bind(cf.include_when_renaming)
            .fetch_one(&mut **tx)
            .await
            .with_context(|| format!("insert custom format '{}'", cf.name))?;
            map.insert(idx, row.0 as i64);
        }
        Ok(map)
    }

    // -- Format score writer --

    /// Write custom_format_scores rows linking quality profiles to custom formats.
    /// Profile format_scores contain (cf_insert_idx, score) — we remap both
    /// the profile old_id and cf_insert_idx to their new Postgres IDs.
    async fn write_format_scores(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        profiles: &[QualityProfileInsert],
        profile_id_map: &HashMap<i64, i64>,
        cf_id_map: &HashMap<usize, i64>,
    ) -> Result<usize> {
        let mut count = 0;
        for p in profiles {
            let Some(&new_profile_id) = profile_id_map.get(&p.old_id) else {
                continue;
            };
            for &(cf_idx, score) in &p.format_scores {
                let Some(&new_cf_id) = cf_id_map.get(&(cf_idx as usize)) else {
                    continue;
                };
                sqlx::query(
                    "INSERT INTO custom_format_scores (profile_id, format_id, score)
                     VALUES ($1, $2, $3)
                     ON CONFLICT (profile_id, format_id) DO UPDATE SET score = $3",
                )
                .bind(new_profile_id as i32)
                .bind(new_cf_id as i32)
                .bind(score)
                .execute(&mut **tx)
                .await
                .with_context(|| {
                    format!(
                        "insert format score for profile {} format {}",
                        new_profile_id, new_cf_id
                    )
                })?;
                count += 1;
            }
        }
        Ok(count)
    }

    // -- Media library folder writer --

    async fn write_media_library_folders(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        folders: &[MediaLibraryFolderInsert],
    ) -> Result<HashMap<String, i64>> {
        let mut map = HashMap::new();
        for f in folders {
            let row: (i32,) = sqlx::query_as(
                "INSERT INTO media_library_folders (path, media_type)
                 VALUES ($1, $2)
                 ON CONFLICT (path) DO UPDATE SET media_type = media_library_folders.media_type
                 RETURNING id",
            )
            .bind(&f.path)
            .bind(&f.media_type)
            .fetch_one(&mut **tx)
            .await
            .with_context(|| format!("insert media library folder '{}'", f.path))?;
            map.insert(f.path.clone(), row.0 as i64);
        }
        Ok(map)
    }

    // -- Naming config writer --

    async fn write_naming_config(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        nc: &NamingConfigInsert,
    ) -> Result<()> {
        sqlx::query(
            "INSERT INTO naming_config (media_type, rename_files, standard_format, daily_format, anime_format, season_folder_format, movie_format, movie_folder_format, colon_replacement)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
             ON CONFLICT (media_type) DO UPDATE SET
                rename_files = $2,
                standard_format = $3,
                daily_format = $4,
                anime_format = $5,
                season_folder_format = $6,
                movie_format = $7,
                movie_folder_format = $8,
                colon_replacement = $9",
        )
        .bind(&nc.media_type)
        .bind(nc.rename_files)
        .bind(&nc.standard_format)
        .bind(&nc.daily_format)
        .bind(&nc.anime_format)
        .bind(&nc.season_folder_format)
        .bind(&nc.movie_format)
        .bind(&nc.movie_folder_format)
        .bind(&nc.colon_replacement)
        .execute(&mut **tx)
        .await
        .with_context(|| format!("upsert naming config for '{}'", nc.media_type))?;
        Ok(())
    }

    // -- Indexer writer --

    async fn write_indexers(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        indexers: &[IndexerInsert],
    ) -> Result<usize> {
        let mut count = 0;
        for idx in indexers {
            sqlx::query(
                "INSERT INTO indexers (name, indexer_type, base_url, api_key, protocol, categories, enabled, priority, supports_search, supports_rss, config)
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)",
            )
            .bind(&idx.name)
            .bind(&idx.indexer_type)
            .bind(&idx.base_url)
            .bind(&idx.api_key)
            .bind(&idx.protocol)
            .bind(&idx.categories)
            .bind(idx.enabled)
            .bind(idx.priority)
            .bind(idx.supports_search)
            .bind(idx.supports_rss)
            .bind(&idx.config)
            .execute(&mut **tx)
            .await
            .with_context(|| format!("insert indexer '{}'", idx.name))?;
            count += 1;
        }
        Ok(count)
    }

    // -- Download client writer --

    async fn write_download_clients(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        clients: &[DownloadClientInsert],
    ) -> Result<usize> {
        let mut count = 0;
        for dc in clients {
            sqlx::query(
                "INSERT INTO download_clients (name, client_type, protocol, config, enabled, priority)
                 VALUES ($1, $2, $3, $4, $5, $6)",
            )
            .bind(&dc.name)
            .bind(&dc.client_type)
            .bind(&dc.protocol)
            .bind(&dc.config)
            .bind(dc.enabled)
            .bind(dc.priority)
            .execute(&mut **tx)
            .await
            .with_context(|| format!("insert download client '{}'", dc.name))?;
            count += 1;
        }
        Ok(count)
    }

    // -- Series writer --

    async fn write_series(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        series: &[SeriesInsert],
        profile_id_map: &HashMap<i64, i64>,
        _profile_name_map: &HashMap<String, i64>,
        media_library_folder_map: &HashMap<String, i64>,
        tag_id_map: &HashMap<String, i64>,
        old_tag_id_to_label: &HashMap<i64, String>,
    ) -> Result<HashMap<i64, i64>> {
        let mut map = HashMap::new();
        for s in series {
            let quality_profile_id = profile_id_map
                .get(&s.quality_profile_old_id)
                .copied()
                .unwrap_or(1); // fallback to first profile

            // Resolve media_library_folder_id from the series path
            let media_library_folder_id = media_library_folder_map
                .iter()
                .find(|(path, _)| s.path.starts_with(path.as_str()))
                .map(|(_, &id)| id);

            // Map old tag IDs to new tag IDs via label lookup
            let mapped_tags: Option<Vec<i32>> = s.tags.as_ref().map(|old_tags| {
                old_tags
                    .iter()
                    .filter_map(|old_id| {
                        let label = old_tag_id_to_label.get(&(*old_id as i64))?;
                        let new_id = tag_id_map.get(label)?;
                        Some(*new_id as i32)
                    })
                    .collect()
            });

            let row: (i64,) = sqlx::query_as(
                "INSERT INTO series (title, clean_title, sort_title, overview, status, series_type,
                    network, air_time, first_aired, year, runtime, path, media_library_folder_id,
                    quality_profile_id, season_folder, monitored, use_scene_numbering,
                    tvdb_id, imdb_id, tmdb_id, tvmaze_id, images, genres, tags, added_at)
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15,
                         $16, $17, $18, $19, $20, $21, $22, $23, $24, $25)
                 RETURNING id",
            )
            .bind(&s.title)
            .bind(&s.clean_title)
            .bind(&s.sort_title)
            .bind(&s.overview)
            .bind(&s.status)
            .bind(&s.series_type)
            .bind(&s.network)
            .bind(s.air_time)
            .bind(s.first_aired)
            .bind(s.year)
            .bind(s.runtime)
            .bind(&s.path)
            .bind(media_library_folder_id)
            .bind(quality_profile_id)
            .bind(s.season_folder)
            .bind(s.monitored)
            .bind(s.use_scene_numbering)
            .bind(s.tvdb_id)
            .bind(&s.imdb_id)
            .bind(s.tmdb_id)
            .bind(s.tvmaze_id)
            .bind(&s.images)
            .bind::<Option<&[String]>>(None) // genres - Sonarr doesn't store them on Series
            .bind(mapped_tags.as_deref())
            .bind(s.added_at)
            .fetch_one(&mut **tx)
            .await
            .with_context(|| format!("insert series '{}'", s.title))?;

            map.insert(s.old_id, row.0);
        }
        Ok(map)
    }

    // -- Seasons writer --

    async fn write_seasons(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        series: &[SeriesInsert],
        series_id_map: &HashMap<i64, i64>,
    ) -> Result<()> {
        for s in series {
            let Some(&new_series_id) = series_id_map.get(&s.old_id) else {
                continue;
            };
            if let Some(ref json) = s.seasons_json {
                let seasons = parse_seasons_json(json);
                for season in seasons {
                    let result = sqlx::query(
                        "INSERT INTO seasons (series_id, season_number, monitored)
                         VALUES ($1, $2, $3)
                         ON CONFLICT (series_id, season_number) DO NOTHING",
                    )
                    .bind(new_series_id)
                    .bind(season.season_number)
                    .bind(season.monitored)
                    .execute(&mut **tx)
                    .await;

                    if let Err(e) = result {
                        warn!("failed to insert season {}/{}: {e}", new_series_id, season.season_number);
                    }
                }
            }
        }
        Ok(())
    }

    // -- Media files writer --

    async fn write_media_files(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        files: &[MediaFileInsert],
    ) -> Result<(HashMap<i64, i64>, HashMap<i64, i64>)> {
        let mut series_map = HashMap::new();
        let mut movie_map = HashMap::new();

        for f in files {
            let row: (i64,) = sqlx::query_as(
                "INSERT INTO media_files (media_type, relative_path, size, date_added, quality,
                    languages, scene_name, release_group, release_hash, edition, media_info, indexer_flags)
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
                 RETURNING id",
            )
            .bind(&f.media_type)
            .bind(&f.relative_path)
            .bind(f.size)
            .bind(f.date_added)
            .bind(&f.quality)
            .bind(&f.languages)
            .bind(&f.scene_name)
            .bind(&f.release_group)
            .bind(&f.release_hash)
            .bind(&f.edition)
            .bind(&f.media_info)
            .bind(f.indexer_flags)
            .fetch_one(&mut **tx)
            .await
            .with_context(|| format!("insert media file '{}'", f.relative_path))?;

            match f.media_type.as_str() {
                "series" => {
                    series_map.insert(f.old_id, row.0);
                }
                "movie" => {
                    movie_map.insert(f.old_id, row.0);
                }
                _ => {}
            }
        }

        Ok((series_map, movie_map))
    }

    // -- Episodes writer --

    async fn write_episodes(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        episodes: &[EpisodeInsert],
        series_id_map: &HashMap<i64, i64>,
        file_id_map: &HashMap<i64, i64>,
    ) -> Result<HashMap<i64, i64>> {
        let mut map = HashMap::new();

        for ep in episodes {
            let Some(&new_series_id) = series_id_map.get(&ep.old_series_id) else {
                warn!(
                    "episode {} references unknown series {}, skipping",
                    ep.old_id, ep.old_series_id
                );
                continue;
            };

            let episode_file_id = ep
                .old_episode_file_id
                .and_then(|old_id| file_id_map.get(&old_id).copied());

            let result = sqlx::query_as::<_, (i64,)>(
                "INSERT INTO episodes (series_id, season_number, episode_number, absolute_number,
                    scene_season_number, scene_episode_number, scene_absolute_number,
                    title, overview, air_date, air_date_utc, runtime, monitored,
                    episode_file_id, last_search_time)
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15)
                 ON CONFLICT (series_id, season_number, episode_number) DO NOTHING
                 RETURNING id",
            )
            .bind(new_series_id)
            .bind(ep.season_number)
            .bind(ep.episode_number)
            .bind(ep.absolute_number)
            .bind(ep.scene_season_number)
            .bind(ep.scene_episode_number)
            .bind(ep.scene_absolute_number)
            .bind(&ep.title)
            .bind(&ep.overview)
            .bind(ep.air_date)
            .bind(ep.air_date_utc)
            .bind(ep.runtime)
            .bind(ep.monitored)
            .bind(episode_file_id)
            .bind(ep.last_search_time)
            .fetch_optional(&mut **tx)
            .await
            .with_context(|| {
                format!(
                    "insert episode S{:02}E{:02} for series {}",
                    ep.season_number, ep.episode_number, new_series_id
                )
            })?;

            if let Some((new_id,)) = result {
                map.insert(ep.old_id, new_id);

                // Also insert into episode_files join table if there is a file
                if let Some(new_file_id) = episode_file_id {
                    let _ = sqlx::query(
                        "INSERT INTO episode_files (episode_id, media_file_id)
                         VALUES ($1, $2) ON CONFLICT DO NOTHING",
                    )
                    .bind(new_id)
                    .bind(new_file_id)
                    .execute(&mut **tx)
                    .await;
                }
            }
        }

        Ok(map)
    }

    // -- Movies writer --

    #[allow(clippy::too_many_arguments)]
    async fn write_movies(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        movies: &[MovieInsert],
        profile_id_map: &HashMap<i64, i64>,
        profile_name_map: &HashMap<String, i64>,
        media_library_folder_map: &HashMap<String, i64>,
        file_id_map: &HashMap<i64, i64>,
        tag_id_map: &HashMap<String, i64>,
        old_tag_id_to_label: &HashMap<i64, String>,
    ) -> Result<HashMap<i64, i64>> {
        let mut map = HashMap::new();

        for m in movies {
            // Resolve quality profile via direct ID map (Radarr old_ids are offset
            // by RADARR_PROFILE_OFFSET to avoid collisions with Sonarr).
            let quality_profile_id = profile_id_map
                .get(&m.quality_profile_old_id)
                .copied()
                .or_else(|| profile_name_map.values().next().copied())
                .unwrap_or(1);

            let media_library_folder_id = media_library_folder_map
                .iter()
                .find(|(path, _)| m.path.starts_with(path.as_str()))
                .map(|(_, &id)| id);

            let movie_file_id = m
                .old_movie_file_id
                .and_then(|old_id| file_id_map.get(&old_id).copied());

            // Map old tag IDs to new tag IDs via label lookup
            let mapped_tags: Option<Vec<i32>> = m.tags.as_ref().map(|old_tags| {
                old_tags
                    .iter()
                    .filter_map(|old_id| {
                        let label = old_tag_id_to_label.get(&(*old_id as i64))?;
                        let new_id = tag_id_map.get(label)?;
                        Some(*new_id as i32)
                    })
                    .collect()
            });

            let row: (i64,) = sqlx::query_as(
                "INSERT INTO movies (title, clean_title, sort_title, overview, year, studio,
                    path, media_library_folder_id, quality_profile_id, monitored, minimum_availability,
                    movie_file_id, tmdb_id, imdb_id, in_cinemas, physical_release,
                    digital_release, images, genres, tags, collection_tmdb_id, added_at, original_language)
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14,
                         $15, $16, $17, $18, $19, $20, $21, $22, $23)
                 RETURNING id",
            )
            .bind(&m.title)
            .bind(&m.clean_title)
            .bind(&m.sort_title)
            .bind(&m.overview)
            .bind(m.year)
            .bind(&m.studio)
            .bind(&m.path)
            .bind(media_library_folder_id)
            .bind(quality_profile_id)
            .bind(m.monitored)
            .bind(&m.minimum_availability)
            .bind(movie_file_id)
            .bind(m.tmdb_id)
            .bind(&m.imdb_id)
            .bind(m.in_cinemas)
            .bind(m.physical_release)
            .bind(m.digital_release)
            .bind(&m.images)
            .bind(m.genres.as_deref())
            .bind(mapped_tags.as_deref())
            .bind(m.collection_tmdb_id)
            .bind(m.added_at)
            .bind(m.original_language)
            .fetch_one(&mut **tx)
            .await
            .with_context(|| format!("insert movie '{}'", m.title))?;

            map.insert(m.old_id, row.0);
        }

        Ok(map)
    }

    // -- History writer (currently unused — import skipped to avoid confusing UX) --

    #[allow(dead_code)]
    async fn write_history(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        history: &[HistoryInsert],
        series_id_map: &HashMap<i64, i64>,
        movie_id_map: &HashMap<i64, i64>,
        episode_id_map: &HashMap<i64, i64>,
    ) -> Result<usize> {
        let mut count = 0;

        for h in history {
            let media_id = match h.media_type.as_str() {
                "series" => series_id_map.get(&h.old_media_id).copied(),
                "movie" => movie_id_map.get(&h.old_media_id).copied(),
                _ => None,
            };

            let Some(media_id) = media_id else {
                // Referenced media was not imported; skip this history event.
                continue;
            };

            let episode_id = h
                .old_episode_id
                .and_then(|old_id| episode_id_map.get(&old_id).copied());

            sqlx::query(
                "INSERT INTO history (media_type, media_id, episode_id, event_type, quality,
                    languages, source_title, download_id, data, occurred_at)
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)",
            )
            .bind(&h.media_type)
            .bind(media_id)
            .bind(episode_id)
            .bind(&h.event_type)
            .bind(&h.quality)
            .bind(&h.languages)
            .bind(&h.source_title)
            .bind(&h.download_id)
            .bind(&h.data)
            .bind(h.occurred_at)
            .execute(&mut **tx)
            .await
            .with_context(|| {
                format!(
                    "insert history event '{}' for {} {}",
                    h.event_type, h.media_type, media_id
                )
            })?;

            count += 1;
        }

        Ok(count)
    }

    // -- Blocklist writer --

    async fn write_blocklist(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        blocklist: &[BlocklistInsert],
        series_id_map: &HashMap<i64, i64>,
        movie_id_map: &HashMap<i64, i64>,
    ) -> Result<usize> {
        let mut count = 0;

        for b in blocklist {
            let media_id = match b.media_type.as_str() {
                "series" => series_id_map.get(&b.old_media_id).copied(),
                "movie" => movie_id_map.get(&b.old_media_id).copied(),
                _ => None,
            };

            let Some(media_id) = media_id else {
                continue;
            };

            sqlx::query(
                "INSERT INTO blocklist (media_type, media_id, source_title, quality, languages,
                    info_hash, added_at)
                 VALUES ($1, $2, $3, $4, $5, $6, $7)",
            )
            .bind(&b.media_type)
            .bind(media_id)
            .bind(&b.source_title)
            .bind(&b.quality)
            .bind(&b.languages)
            .bind(&b.info_hash)
            .bind(b.added_at)
            .execute(&mut **tx)
            .await
            .with_context(|| {
                format!(
                    "insert blocklist entry '{}' for {} {}",
                    b.source_title, b.media_type, media_id
                )
            })?;

            count += 1;
        }

        Ok(count)
    }
}
