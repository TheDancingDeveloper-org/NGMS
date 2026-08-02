use std::path::PathBuf;

use anyhow::Result;
use sqlx::MySqlPool;

use stackarr_core::models::quality::QualityProfile;
use stackarr_quality::{is_quality_allowed, parser_quality_to_num, quality_name};

// ── Types ───────────────────────────────────────────────────────────────────

/// The outcome of an upgrade check for an incoming file.
pub enum UpgradeCheckResult {
    /// No existing file on disk — import freely.
    NoExistingFile,
    /// The new file is a quality upgrade — proceed with replacement.
    Upgrade {
        existing_file_id: i64,
        existing_path: PathBuf,
        existing_quality: serde_json::Value,
    },
    /// The new file is NOT an upgrade — skip import.
    NotAnUpgrade { reason: String },
}

// ── Upgrade check ─────────────────────────────────────────���─────────────────

/// Check whether importing a file with `new_quality_num` for the given media
/// is an upgrade over the existing file (if any).
pub async fn check_upgrade(
    pool: &MySqlPool,
    media_type: &str,
    media_id: i64,
    episode_id: Option<i64>,
    new_quality_num: i32,
) -> Result<UpgradeCheckResult> {
    // 1. Look up existing file
    let existing = match media_type {
        "series" | "tv" => lookup_series_file(pool, episode_id).await?,
        "movie" => lookup_movie_file(pool, media_id).await?,
        _ => None,
    };

    let (file_id, relative_path, quality_json, media_path) = match existing {
        None => return Ok(UpgradeCheckResult::NoExistingFile),
        Some(e) => e,
    };

    // 2. Extract existing quality number from JSONB
    let existing_quality_num = extract_quality_num(&quality_json);

    // 3. Load quality profile
    let profile = load_quality_profile(pool, media_type, media_id).await?;

    // 4. Apply upgrade rules
    if !profile.upgrade_allowed {
        return Ok(UpgradeCheckResult::NotAnUpgrade {
            reason: "upgrades are disabled for this quality profile".to_string(),
        });
    }

    if existing_quality_num >= profile.cutoff && profile.cutoff > 0 {
        return Ok(UpgradeCheckResult::NotAnUpgrade {
            reason: format!(
                "cutoff already met: existing {} meets cutoff {}",
                quality_name(existing_quality_num),
                quality_name(profile.cutoff),
            ),
        });
    }

    if new_quality_num <= existing_quality_num {
        return Ok(UpgradeCheckResult::NotAnUpgrade {
            reason: format!(
                "{} is not an upgrade over existing {}",
                quality_name(new_quality_num),
                quality_name(existing_quality_num),
            ),
        });
    }

    if !is_quality_allowed(new_quality_num, &profile) {
        return Ok(UpgradeCheckResult::NotAnUpgrade {
            reason: format!(
                "{} is not allowed in quality profile '{}'",
                quality_name(new_quality_num),
                profile.name,
            ),
        });
    }

    // 5. Resolve absolute path
    let absolute_path = PathBuf::from(&media_path).join(&relative_path);

    Ok(UpgradeCheckResult::Upgrade {
        existing_file_id: file_id,
        existing_path: absolute_path,
        existing_quality: quality_json,
    })
}

// ── DB lookups ──────────────────────────────────────────────────────────────

/// Returns (media_file_id, relative_path, quality_json, series_path) for an
/// episode's existing file, or None if no file is linked.
async fn lookup_series_file(
    pool: &MySqlPool,
    episode_id: Option<i64>,
) -> Result<Option<(i64, String, serde_json::Value, String)>> {
    let ep_id = match episode_id {
        Some(id) => id,
        None => return Ok(None),
    };

    let row: Option<(i64, String, serde_json::Value, String)> = sqlx::query_as(
        "SELECT mf.id, mf.relative_path, mf.quality, s.path \
         FROM episodes e \
         JOIN media_files mf ON e.episode_file_id = mf.id \
         JOIN series s ON e.series_id = s.id \
         WHERE e.id = ? AND e.episode_file_id IS NOT NULL",
    )
    .bind(ep_id)
    .fetch_optional(pool)
    .await?;

    Ok(row)
}

/// Returns (media_file_id, relative_path, quality_json, movie_path) for a
/// movie's existing file, or None if no file is linked.
async fn lookup_movie_file(
    pool: &MySqlPool,
    movie_id: i64,
) -> Result<Option<(i64, String, serde_json::Value, String)>> {
    let row: Option<(i64, String, serde_json::Value, String)> = sqlx::query_as(
        "SELECT mf.id, mf.relative_path, mf.quality, m.path \
         FROM movies m \
         JOIN media_files mf ON m.movie_file_id = mf.id \
         WHERE m.id = ? AND m.movie_file_id IS NOT NULL",
    )
    .bind(movie_id)
    .fetch_optional(pool)
    .await?;

    Ok(row)
}

async fn load_quality_profile(
    pool: &MySqlPool,
    media_type: &str,
    media_id: i64,
) -> Result<QualityProfile> {
    let profile_id: (i32,) = match media_type {
        "series" | "tv" => {
            sqlx::query_as("SELECT quality_profile_id FROM series WHERE id = ?")
                .bind(media_id)
                .fetch_one(pool)
                .await?
        }
        "movie" => {
            sqlx::query_as("SELECT quality_profile_id FROM movies WHERE id = ?")
                .bind(media_id)
                .fetch_one(pool)
                .await?
        }
        _ => anyhow::bail!("unknown media type: {media_type}"),
    };

    let profile =
        sqlx::query_as::<_, QualityProfile>("SELECT * FROM quality_profiles WHERE id = ?")
            .bind(profile_id.0)
            .fetch_one(pool)
            .await?;

    Ok(profile)
}

/// Extract the quality discriminant number from the JSONB stored in
/// `media_files.quality`. Handles both integer IDs (`{"quality": 11}`)
/// and legacy string names (`{"quality": "WEBDL1080p"}`).
fn extract_quality_num(quality_json: &serde_json::Value) -> i32 {
    if let Some(quality_val) = quality_json.get("quality") {
        // Integer format (normalized)
        if let Some(n) = quality_val.as_i64() {
            return n as i32;
        }
        // Legacy string enum format
        if let Ok(q) = serde_json::from_value::<stackarr_parser::Quality>(quality_val.clone()) {
            return parser_quality_to_num(q);
        }
    }
    0 // Unknown
}
