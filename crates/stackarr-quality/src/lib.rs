pub mod custom_formats;

use anyhow::Result;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;

use stackarr_core::models::{CustomFormat, DownloadProtocol, QualityProfile, ReleaseInfo};

// ── Quality profile CRUD ────────────────────────────────────────────────────

#[derive(Clone)]
pub struct QualityProfileService {
    pool: PgPool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateProfileInput {
    pub name: String,
    pub cutoff: i32,
    #[serde(default = "default_true")]
    pub upgrade_allowed: bool,
    #[serde(default)]
    pub min_format_score: i32,
    #[serde(default)]
    pub cutoff_format_score: i32,
    pub items: serde_json::Value,
    pub media_type: Option<String>,
    /// Language preference: -1=Any (default), -2=Original, positive=Radarr language ID.
    #[serde(default = "default_language_any")]
    pub language: i32,
    #[serde(default = "default_min_upgrade")]
    pub min_upgrade_format_score: i32,
    #[serde(default)]
    pub format_items: Option<Vec<ProfileFormatItemInput>>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateProfileInput {
    pub name: Option<String>,
    pub cutoff: Option<i32>,
    pub upgrade_allowed: Option<bool>,
    pub min_format_score: Option<i32>,
    pub cutoff_format_score: Option<i32>,
    pub items: Option<serde_json::Value>,
    pub media_type: Option<String>,
    pub language: Option<i32>,
    pub min_upgrade_format_score: Option<i32>,
    pub format_items: Option<Vec<ProfileFormatItemInput>>,
}

fn default_language_any() -> i32 {
    -1
}

fn default_true() -> bool {
    true
}

fn default_min_upgrade() -> i32 {
    1
}

// ── Profile format items ───────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileFormatItem {
    pub format: i32,
    pub name: String,
    pub score: i32,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileFormatItemInput {
    pub format: i32,
    pub score: i32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QualityProfileResponse {
    #[serde(flatten)]
    pub profile: QualityProfile,
    pub format_items: Vec<ProfileFormatItem>,
}

// ── Custom format CRUD ─────────────────────────────────────────────────────

#[derive(Clone)]
pub struct CustomFormatService {
    pool: PgPool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateCustomFormatInput {
    pub name: String,
    pub specifications: serde_json::Value,
    #[serde(default)]
    pub include_custom_format_when_renaming: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateCustomFormatInput {
    pub name: Option<String>,
    pub specifications: Option<serde_json::Value>,
    pub include_custom_format_when_renaming: Option<bool>,
}

impl CustomFormatService {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn list(&self) -> Result<Vec<CustomFormat>> {
        let rows =
            sqlx::query_as::<_, CustomFormat>("SELECT * FROM custom_formats ORDER BY name")
                .fetch_all(&self.pool)
                .await?;
        Ok(rows)
    }

    pub async fn get(&self, id: i64) -> Result<CustomFormat> {
        let row =
            sqlx::query_as::<_, CustomFormat>("SELECT * FROM custom_formats WHERE id = $1")
                .bind(id)
                .fetch_one(&self.pool)
                .await?;
        Ok(row)
    }

    pub async fn create(&self, input: CreateCustomFormatInput) -> Result<CustomFormat> {
        let row = sqlx::query_as::<_, CustomFormat>(
            "INSERT INTO custom_formats (name, specifications, include_custom_format_when_renaming)
             VALUES ($1, $2, $3) RETURNING *",
        )
        .bind(&input.name)
        .bind(&input.specifications)
        .bind(input.include_custom_format_when_renaming)
        .fetch_one(&self.pool)
        .await?;
        tracing::info!(id = row.id, name = %row.name, "custom format created");
        Ok(row)
    }

    pub async fn update(&self, id: i64, input: UpdateCustomFormatInput) -> Result<CustomFormat> {
        let existing = self.get(id).await?;
        let name = input.name.unwrap_or(existing.name);
        let specs = input.specifications.unwrap_or(existing.specifications);
        let rename = input
            .include_custom_format_when_renaming
            .unwrap_or(existing.include_custom_format_when_renaming);

        let row = sqlx::query_as::<_, CustomFormat>(
            "UPDATE custom_formats SET name=$1, specifications=$2, include_custom_format_when_renaming=$3
             WHERE id=$4 RETURNING *",
        )
        .bind(&name)
        .bind(&specs)
        .bind(rename)
        .bind(id)
        .fetch_one(&self.pool)
        .await?;
        tracing::debug!(id, name = %row.name, "custom format updated");
        Ok(row)
    }

    pub async fn delete(&self, id: i64) -> Result<()> {
        tracing::info!(id, "deleting custom format");
        sqlx::query("DELETE FROM custom_formats WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}

impl QualityProfileService {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn list(&self) -> Result<Vec<QualityProfileResponse>> {
        let mut rows =
            sqlx::query_as::<_, QualityProfile>("SELECT * FROM quality_profiles ORDER BY id")
                .fetch_all(&self.pool)
                .await?;
        for p in &mut rows {
            p.normalize_items();
        }

        // Bulk-load all format scores to avoid N+1
        let all_formats: Vec<(i32, String)> = sqlx::query_as(
            "SELECT id, name FROM custom_formats ORDER BY name",
        )
        .fetch_all(&self.pool)
        .await
        .unwrap_or_default();

        let all_scores: Vec<(i32, i32, i32)> = sqlx::query_as(
            "SELECT profile_id, format_id, score FROM custom_format_scores",
        )
        .fetch_all(&self.pool)
        .await
        .unwrap_or_default();

        let mut score_map: std::collections::HashMap<i32, std::collections::HashMap<i32, i32>> =
            std::collections::HashMap::new();
        for (pid, fid, score) in &all_scores {
            score_map.entry(*pid).or_default().insert(*fid, *score);
        }

        let responses = rows
            .into_iter()
            .map(|profile| {
                let scores = score_map.get(&profile.id);
                let format_items = all_formats
                    .iter()
                    .map(|(fid, fname)| ProfileFormatItem {
                        format: *fid,
                        name: fname.clone(),
                        score: scores
                            .and_then(|s| s.get(fid))
                            .copied()
                            .unwrap_or(0),
                    })
                    .collect();
                QualityProfileResponse {
                    profile,
                    format_items,
                }
            })
            .collect();
        Ok(responses)
    }

    pub async fn get(&self, id: i64) -> Result<QualityProfileResponse> {
        let mut profile =
            sqlx::query_as::<_, QualityProfile>("SELECT * FROM quality_profiles WHERE id = $1")
                .bind(id)
                .fetch_one(&self.pool)
                .await?;
        profile.normalize_items();
        let format_items = self.load_format_items(profile.id).await;
        Ok(QualityProfileResponse {
            profile,
            format_items,
        })
    }

    async fn load_format_items(&self, profile_id: i32) -> Vec<ProfileFormatItem> {
        sqlx::query_as::<_, (i32, String, i32)>(
            "SELECT cf.id, cf.name, COALESCE(cfs.score, 0) as score
             FROM custom_formats cf
             LEFT JOIN custom_format_scores cfs ON cfs.format_id = cf.id AND cfs.profile_id = $1
             ORDER BY cf.name",
        )
        .bind(profile_id)
        .fetch_all(&self.pool)
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|(id, name, score)| ProfileFormatItem {
            format: id,
            name,
            score,
        })
        .collect()
    }

    async fn save_format_items(&self, profile_id: i32, items: &[ProfileFormatItemInput]) -> Result<()> {
        sqlx::query("DELETE FROM custom_format_scores WHERE profile_id = $1")
            .bind(profile_id)
            .execute(&self.pool)
            .await?;
        for item in items {
            if item.score != 0 {
                sqlx::query(
                    "INSERT INTO custom_format_scores (profile_id, format_id, score) VALUES ($1, $2, $3)",
                )
                .bind(profile_id)
                .bind(item.format)
                .bind(item.score)
                .execute(&self.pool)
                .await?;
            }
        }
        Ok(())
    }

    pub async fn create(&self, input: CreateProfileInput) -> Result<QualityProfileResponse> {
        let mut profile = sqlx::query_as::<_, QualityProfile>(
            "INSERT INTO quality_profiles (name, cutoff, upgrade_allowed, min_format_score, cutoff_format_score, items, media_type, language, min_upgrade_format_score)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9) RETURNING *",
        )
        .bind(&input.name)
        .bind(input.cutoff)
        .bind(input.upgrade_allowed)
        .bind(input.min_format_score)
        .bind(input.cutoff_format_score)
        .bind(&input.items)
        .bind(&input.media_type)
        .bind(input.language)
        .bind(input.min_upgrade_format_score)
        .fetch_one(&self.pool)
        .await?;
        profile.normalize_items();
        tracing::info!(id = profile.id, name = %profile.name, "quality profile created");

        if let Some(ref fi) = input.format_items {
            self.save_format_items(profile.id, fi).await?;
        }
        let format_items = self.load_format_items(profile.id).await;
        Ok(QualityProfileResponse {
            profile,
            format_items,
        })
    }

    pub async fn update(&self, id: i64, input: UpdateProfileInput) -> Result<QualityProfileResponse> {
        let existing = self.get_raw(id).await?;
        let name = input.name.unwrap_or(existing.name);
        let cutoff = input.cutoff.unwrap_or(existing.cutoff);
        let upgrade = input.upgrade_allowed.unwrap_or(existing.upgrade_allowed);
        let min_fs = input.min_format_score.unwrap_or(existing.min_format_score);
        let cutoff_fs = input
            .cutoff_format_score
            .unwrap_or(existing.cutoff_format_score);
        let items = input.items.unwrap_or(existing.items);
        let media_type = if input.media_type.is_some() {
            input.media_type
        } else {
            existing.media_type
        };
        let language = input.language.unwrap_or(existing.language);
        let min_upgrade_fs = input
            .min_upgrade_format_score
            .unwrap_or(existing.min_upgrade_format_score);

        let mut profile = sqlx::query_as::<_, QualityProfile>(
            "UPDATE quality_profiles SET name=$1, cutoff=$2, upgrade_allowed=$3, min_format_score=$4, cutoff_format_score=$5, items=$6, media_type=$7, language=$8, min_upgrade_format_score=$9
             WHERE id=$10 RETURNING *",
        )
        .bind(&name)
        .bind(cutoff)
        .bind(upgrade)
        .bind(min_fs)
        .bind(cutoff_fs)
        .bind(&items)
        .bind(&media_type)
        .bind(language)
        .bind(min_upgrade_fs)
        .bind(id)
        .fetch_one(&self.pool)
        .await?;
        profile.normalize_items();
        tracing::debug!(id, name = %profile.name, "quality profile updated");

        if let Some(ref fi) = input.format_items {
            self.save_format_items(profile.id, fi).await?;
        }
        let format_items = self.load_format_items(profile.id).await;
        Ok(QualityProfileResponse {
            profile,
            format_items,
        })
    }

    /// Internal: get raw profile without format items (for update merging).
    async fn get_raw(&self, id: i64) -> Result<QualityProfile> {
        let mut row =
            sqlx::query_as::<_, QualityProfile>("SELECT * FROM quality_profiles WHERE id = $1")
                .bind(id)
                .fetch_one(&self.pool)
                .await?;
        row.normalize_items();
        Ok(row)
    }

    pub async fn delete(&self, id: i64) -> Result<()> {
        tracing::info!(id, "deleting quality profile");
        sqlx::query("DELETE FROM quality_profiles WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}

// ── Decision context ────────────────────────────────────────────────────────

/// Full context passed to each decision specification.
#[derive(Debug, Clone)]
pub struct DecisionContext {
    pub release: ReleaseInfo,
    pub profile: QualityProfile,
    /// Quality discriminant of the existing file on disk, if any.
    pub existing_quality: Option<i32>,
    /// Custom format score of the existing file, if any.
    pub existing_custom_format_score: Option<i32>,
    /// Custom format score computed for this release.
    pub release_custom_format_score: i32,
    /// Custom formats that matched this release (carried through to the decision).
    pub matched_formats: Vec<custom_formats::MatchedFormat>,
    /// Whether this release is already being downloaded.
    pub in_queue: bool,
    /// Whether this release was previously failed/blocklisted.
    pub in_blocklist: bool,
    /// Whether this release (by guid) has already been grabbed and imported.
    pub already_grabbed: bool,
    /// Quality of the highest-quality item in the queue for this media item.
    /// `Some(quality_num)` when an equal-or-better queued item exists.
    pub queued_quality: Option<i32>,
    /// Radarr language ID of the media's original language (for -2/Original profiles).
    pub original_language: Option<i32>,
}

// ── Decision engine ─────────────────────────────────────────────────────────

/// The outcome of a quality decision for a single release.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadDecision {
    pub approved: bool,
    pub release: ReleaseInfo,
    pub rejections: Vec<Rejection>,
    /// Custom format score for this release (used for ranking).
    pub custom_format_score: i32,
    /// Custom formats that matched this release.
    pub matched_formats: Vec<custom_formats::MatchedFormat>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Rejection {
    pub reason: String,
    pub rejection_type: RejectionType,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum RejectionType {
    Permanent,
    Temporary,
}

/// A specification that can accept or reject a release.
pub trait DecisionSpecification: Send + Sync {
    fn is_satisfied(&self, context: &DecisionContext) -> Option<Rejection>;
}

// ── Quality number mapping ──────────────────────────────────────────────────

/// Maps a `stackarr_parser::Quality` variant to the core model's discriminant
/// number used in quality profile items.
pub fn parser_quality_to_num(q: stackarr_parser::Quality) -> i32 {
    match q {
        stackarr_parser::Quality::Unknown => 0,
        stackarr_parser::Quality::SDTV => 1,
        stackarr_parser::Quality::DVD | stackarr_parser::Quality::DVDRip => 2,
        stackarr_parser::Quality::WEBDL480p => 3,
        stackarr_parser::Quality::WEBRip480p => 4,
        // Bluray480p = 5 in core, but parser doesn't have it
        stackarr_parser::Quality::HDTV720p => 6,
        stackarr_parser::Quality::WEBDL720p => 7,
        stackarr_parser::Quality::WEBRip720p => 8,
        stackarr_parser::Quality::Bluray720p => 9,
        stackarr_parser::Quality::HDTV1080p => 10,
        stackarr_parser::Quality::WEBDL1080p => 11,
        stackarr_parser::Quality::WEBRip1080p => 12,
        stackarr_parser::Quality::Bluray1080p => 13,
        stackarr_parser::Quality::Remux1080p => 14,
        stackarr_parser::Quality::HDTV2160p => 15,
        stackarr_parser::Quality::WEBDL2160p => 16,
        stackarr_parser::Quality::WEBRip2160p => 17,
        stackarr_parser::Quality::Bluray2160p => 18,
        stackarr_parser::Quality::Remux2160p => 19,
        stackarr_parser::Quality::Raw => 20,
    }
}

/// Human-readable name for a quality number.
pub fn quality_name(num: i32) -> &'static str {
    match num {
        0 => "Unknown",
        1 => "SDTV",
        2 => "DVD",
        3 => "WEBDL-480p",
        4 => "WEBRip-480p",
        5 => "Bluray-480p",
        6 => "HDTV-720p",
        7 => "WEBDL-720p",
        8 => "WEBRip-720p",
        9 => "Bluray-720p",
        10 => "HDTV-1080p",
        11 => "WEBDL-1080p",
        12 => "WEBRip-1080p",
        13 => "Bluray-1080p",
        14 => "Remux-1080p",
        15 => "HDTV-2160p",
        16 => "WEBDL-2160p",
        17 => "WEBRip-2160p",
        18 => "Bluray-2160p",
        19 => "Remux-2160p",
        20 => "Raw-HD",
        _ => "Unknown",
    }
}

/// Parse a release title into its core-model quality number.
fn parse_quality_num(title: &str) -> i32 {
    let parsed = stackarr_parser::parse_release(title);
    parser_quality_to_num(parsed.quality.quality)
}

// ── Quality item deserialization ────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
struct QualityItem {
    /// Quality ID — accepts bare integer `10`, object `{"id": 10, ...}`, or null.
    #[serde(default, deserialize_with = "deserialize_quality_id")]
    quality: Option<i32>,
    #[serde(default)]
    allowed: bool,
    #[serde(default)]
    items: Vec<QualityItem>,
}

/// Custom deserializer that handles three formats for the quality field:
/// - Bare integer: `10` → `Some(10)`
/// - Object with id: `{"id": 10, "name": "HDTV-1080p", ...}` → `Some(10)`
/// - null / missing → `None`
fn deserialize_quality_id<'de, D>(deserializer: D) -> Result<Option<i32>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value: Option<serde_json::Value> = Option::deserialize(deserializer)?;
    match value {
        None | Some(serde_json::Value::Null) => Ok(None),
        Some(serde_json::Value::Number(n)) => {
            Ok(n.as_i64().and_then(|v| i32::try_from(v).ok()))
        }
        Some(serde_json::Value::Object(map)) => {
            let id = map
                .get("id")
                .and_then(|v| v.as_i64())
                .and_then(|v| i32::try_from(v).ok());
            Ok(id)
        }
        Some(_) => Ok(None),
    }
}

impl QualityItem {
    /// Flatten nested quality groups into a single list of leaf items.
    fn flatten(&self) -> Vec<&QualityItem> {
        if self.items.is_empty() {
            vec![self]
        } else {
            // For a group, propagate the group's `allowed` flag to children
            // that don't override it. In practice each child carries its own
            // allowed flag; the group's flag gates the whole group.
            if self.allowed {
                self.items.iter().flat_map(|child| child.flatten()).collect()
            } else {
                // Group disabled — all children effectively disallowed
                Vec::new()
            }
        }
    }
}

fn parse_profile_items(profile: &QualityProfile) -> Vec<QualityItem> {
    serde_json::from_value(profile.items.clone()).unwrap_or_default()
}

pub fn is_quality_allowed(quality_num: i32, profile: &QualityProfile) -> bool {
    let items: Vec<QualityItem> = parse_profile_items(profile);
    let flat: Vec<&QualityItem> = items.iter().flat_map(|i| i.flatten()).collect();
    flat.iter()
        .any(|item| item.quality == Some(quality_num) && item.allowed)
}

// ── Size limits per quality tier ────────────────────────────────────────────

/// Returns (min_bytes, max_bytes) for a given quality discriminant number.
fn size_limits(quality: i32) -> (i64, i64) {
    match quality {
        1..=5 => (50_000_000, 3_000_000_000),         // SD: 50MB - 3GB
        6..=9 => (100_000_000, 8_000_000_000),         // 720p: 100MB - 8GB
        10..=14 => (200_000_000, 20_000_000_000),      // 1080p: 200MB - 20GB
        15..=19 => (500_000_000, 80_000_000_000),      // 2160p: 500MB - 80GB
        _ => (0, i64::MAX),
    }
}

// ── Specifications ──────────────────────────────────────────────────────────

/// Rejects releases whose quality is not allowed in the profile.
pub struct QualityAllowedSpec;

impl DecisionSpecification for QualityAllowedSpec {
    fn is_satisfied(&self, context: &DecisionContext) -> Option<Rejection> {
        let quality_num = parse_quality_num(&context.release.title);

        if !is_quality_allowed(quality_num, &context.profile) {
            Some(Rejection {
                reason: format!(
                    "{} is not allowed in profile '{}'",
                    quality_name(quality_num),
                    context.profile.name,
                ),
                rejection_type: RejectionType::Permanent,
            })
        } else {
            None
        }
    }
}

/// Rejects releases when the cutoff quality has already been met and no
/// upgrade is possible.
pub struct QualityCutoffSpec;

impl DecisionSpecification for QualityCutoffSpec {
    fn is_satisfied(&self, context: &DecisionContext) -> Option<Rejection> {
        let existing = match context.existing_quality {
            Some(q) => q,
            None => return None, // No existing file — pass
        };

        let release_quality = parse_quality_num(&context.release.title);

        // If upgrades are disabled, reject any release when we already have a file
        if !context.profile.upgrade_allowed {
            return Some(Rejection {
                reason: "upgrades are disabled for this profile".to_string(),
                rejection_type: RejectionType::Permanent,
            });
        }

        // If existing quality already meets or exceeds the cutoff, reject
        if existing >= context.profile.cutoff {
            return Some(Rejection {
                reason: format!(
                    "cutoff already met: existing {} meets cutoff {}",
                    quality_name(existing),
                    quality_name(context.profile.cutoff),
                ),
                rejection_type: RejectionType::Permanent,
            });
        }

        // Release must be an upgrade over existing quality
        if release_quality <= existing {
            return Some(Rejection {
                reason: format!(
                    "{} is not an upgrade over existing {}",
                    quality_name(release_quality),
                    quality_name(existing),
                ),
                rejection_type: RejectionType::Permanent,
            });
        }

        None
    }
}

/// Rejects releases below minimum size thresholds.
pub struct MinimumSizeSpec;

impl DecisionSpecification for MinimumSizeSpec {
    fn is_satisfied(&self, context: &DecisionContext) -> Option<Rejection> {
        let size = context.release.size;
        if size <= 0 {
            return None; // Size unknown — pass
        }

        let quality_num = parse_quality_num(&context.release.title);
        let (min, _max) = size_limits(quality_num);

        if size < min {
            Some(Rejection {
                reason: format!(
                    "release size {} bytes is below minimum {} bytes for {}",
                    size,
                    min,
                    quality_name(quality_num),
                ),
                rejection_type: RejectionType::Permanent,
            })
        } else {
            None
        }
    }
}

/// Rejects releases above maximum size thresholds.
pub struct MaximumSizeSpec;

impl DecisionSpecification for MaximumSizeSpec {
    fn is_satisfied(&self, context: &DecisionContext) -> Option<Rejection> {
        let size = context.release.size;
        if size <= 0 {
            return None; // Size unknown — pass
        }

        let quality_num = parse_quality_num(&context.release.title);
        let (_min, max) = size_limits(quality_num);

        if size > max {
            Some(Rejection {
                reason: format!(
                    "release size {} bytes exceeds maximum {} bytes for {}",
                    size,
                    max,
                    quality_name(quality_num),
                ),
                rejection_type: RejectionType::Permanent,
            })
        } else {
            None
        }
    }
}

/// Rejects releases that are in the blocklist.
pub struct BlocklistSpec;

impl DecisionSpecification for BlocklistSpec {
    fn is_satisfied(&self, context: &DecisionContext) -> Option<Rejection> {
        if context.in_blocklist {
            Some(Rejection {
                reason: "release is in the blocklist".to_string(),
                rejection_type: RejectionType::Permanent,
            })
        } else {
            None
        }
    }
}

/// Rejects releases when:
/// 1. The exact same release (by guid) is already in the download queue, OR
/// 2. The same media item already has a queued download at equal or higher quality.
pub struct QueueConflictSpec;

impl DecisionSpecification for QueueConflictSpec {
    fn is_satisfied(&self, context: &DecisionContext) -> Option<Rejection> {
        // Check 1: exact guid match
        if context.in_queue {
            return Some(Rejection {
                reason: "already in download queue".to_string(),
                rejection_type: RejectionType::Temporary,
            });
        }

        // Check 2: same media item has a queued download at equal/higher quality
        if let Some(queued_q) = context.queued_quality {
            let release_q = parse_quality_num(&context.release.title);
            if queued_q >= release_q {
                let name = quality_name(queued_q);
                return Some(Rejection {
                    reason: format!(
                        "release in queue is of equal or higher preference: {name}",
                    ),
                    rejection_type: RejectionType::Temporary,
                });
            }
        }

        None
    }
}

/// Rejects releases whose detected language doesn't match the profile language.
/// Mirrors Radarr's language filtering for movie profiles.
pub struct LanguageSpec;

impl DecisionSpecification for LanguageSpec {
    fn is_satisfied(&self, context: &DecisionContext) -> Option<Rejection> {
        let profile_lang = context.profile.language;

        // -1 = Any language → always pass
        if profile_lang == -1 {
            return None;
        }

        let release_langs = stackarr_parser::parse_languages(&context.release.title);

        // Resolve the wanted language ID
        let wanted_id = if profile_lang == -2 {
            // -2 = Original → use the media's original language
            match context.original_language {
                Some(id) => id,
                None => return None, // Can't determine original language → pass
            }
        } else {
            profile_lang
        };

        // Convert Radarr language ID to parser Language enum for comparison
        let wanted_parser_lang = radarr_id_to_parser_lang(wanted_id);

        // Check if any detected language matches
        let matched = release_langs.iter().any(|lang| {
            *lang == wanted_parser_lang
                || *lang == stackarr_parser::Language::Multi
                || *lang == stackarr_parser::Language::Unknown
        });

        if matched {
            None
        } else {
            let found_names: Vec<&str> = release_langs
                .iter()
                .map(|l| parser_lang_name(*l))
                .collect();
            let wanted_name = parser_lang_name(wanted_parser_lang);
            Some(Rejection {
                reason: format!(
                    "{wanted_name} is wanted, but found {}",
                    found_names.join(", "),
                ),
                rejection_type: RejectionType::Permanent,
            })
        }
    }
}

/// Map Radarr language ID to the parser's Language enum.
fn radarr_id_to_parser_lang(id: i32) -> stackarr_parser::Language {
    use stackarr_parser::Language;
    match id {
        1 => Language::English,
        2 => Language::French,
        3 => Language::Spanish,
        4 => Language::German,
        5 => Language::Italian,
        6 => Language::Danish,
        7 => Language::Dutch,
        8 => Language::Japanese,
        10 => Language::Chinese,
        11 => Language::Russian,
        12 => Language::Polish,
        14 => Language::Swedish,
        15 => Language::Norwegian,
        16 => Language::Finnish,
        17 => Language::Turkish,
        18 => Language::Portuguese,
        20 => Language::Greek,
        21 => Language::Korean,
        22 => Language::Hungarian,
        23 => Language::Hebrew,
        25 => Language::Czech,
        26 => Language::Hindi,
        27 => Language::Romanian,
        28 => Language::Thai,
        29 => Language::Vietnamese, // Radarr uses 29 for Vietnamese in some versions
        _ => Language::Unknown,
    }
}

/// Human-readable name for a parser Language enum value.
fn parser_lang_name(lang: stackarr_parser::Language) -> &'static str {
    use stackarr_parser::Language;
    match lang {
        Language::English => "English",
        Language::French => "French",
        Language::Spanish => "Spanish",
        Language::German => "German",
        Language::Italian => "Italian",
        Language::Portuguese => "Portuguese",
        Language::Japanese => "Japanese",
        Language::Chinese => "Chinese",
        Language::Korean => "Korean",
        Language::Russian => "Russian",
        Language::Polish => "Polish",
        Language::Dutch => "Dutch",
        Language::Swedish => "Swedish",
        Language::Norwegian => "Norwegian",
        Language::Danish => "Danish",
        Language::Finnish => "Finnish",
        Language::Turkish => "Turkish",
        Language::Arabic => "Arabic",
        Language::Hindi => "Hindi",
        Language::Czech => "Czech",
        Language::Hungarian => "Hungarian",
        Language::Romanian => "Romanian",
        Language::Greek => "Greek",
        Language::Hebrew => "Hebrew",
        Language::Thai => "Thai",
        Language::Vietnamese => "Vietnamese",
        Language::Indonesian => "Indonesian",
        Language::Multi => "Multi",
        Language::Unknown => "Unknown",
    }
}

/// Rejects torrent releases with insufficient seeders.
pub struct MinimumSeedersSpec;

impl DecisionSpecification for MinimumSeedersSpec {
    fn is_satisfied(&self, context: &DecisionContext) -> Option<Rejection> {
        if context.release.protocol != DownloadProtocol::Torrent {
            return None; // Only applies to torrents
        }

        let seeders = context.release.seeders.unwrap_or(0);
        if seeders < 1 {
            Some(Rejection {
                reason: format!("not enough seeders: {} (minimum is 1)", seeders),
                rejection_type: RejectionType::Temporary,
            })
        } else {
            None
        }
    }
}

/// Rejects releases whose custom format score is below the profile minimum.
pub struct CustomFormatScoreSpec;

impl DecisionSpecification for CustomFormatScoreSpec {
    fn is_satisfied(&self, context: &DecisionContext) -> Option<Rejection> {
        let score = context.release_custom_format_score;

        // Reject if score is below the profile minimum (handles both positive
        // minimums and negative scores from LQ custom formats like -10000).
        if score < context.profile.min_format_score {
            Some(Rejection {
                reason: format!(
                    "custom format score {} is below minimum {} for profile '{}'",
                    score, context.profile.min_format_score, context.profile.name,
                ),
                rejection_type: RejectionType::Permanent,
            })
        } else {
            None
        }
    }
}

/// Rejects releases when the existing file's custom format score already meets
/// or exceeds the cutoff format score (mirroring Sonarr/Radarr's
/// "Existing file on disk has equal or higher Custom Format score" rejection).
pub struct CustomFormatCutoffSpec;

impl DecisionSpecification for CustomFormatCutoffSpec {
    fn is_satisfied(&self, context: &DecisionContext) -> Option<Rejection> {
        let existing_cf = match context.existing_custom_format_score {
            Some(s) => s,
            None => return None, // No existing file — pass
        };

        // Only applies when profile has a non-zero cutoff_format_score
        if context.profile.cutoff_format_score <= 0 {
            return None;
        }

        // If existing CF score already meets the cutoff and the release doesn't exceed it
        if existing_cf >= context.profile.cutoff_format_score
            && context.release_custom_format_score <= existing_cf
        {
            return Some(Rejection {
                reason: format!(
                    "existing file has equal or higher custom format score: {}",
                    existing_cf,
                ),
                rejection_type: RejectionType::Permanent,
            });
        }

        None
    }
}

/// Rejects releases that have already been grabbed and imported (by guid).
pub struct AlreadyImportedSpec;

impl DecisionSpecification for AlreadyImportedSpec {
    fn is_satisfied(&self, context: &DecisionContext) -> Option<Rejection> {
        if context.already_grabbed {
            Some(Rejection {
                reason: "release has already been grabbed and imported".to_string(),
                rejection_type: RejectionType::Permanent,
            })
        } else {
            None
        }
    }
}

// ── Decision engine ─────────────────────────────────────────────────────────

/// The decision engine evaluates releases against quality profiles.
pub struct DecisionEngine {
    specs: Vec<Box<dyn DecisionSpecification>>,
}

impl DecisionEngine {
    /// Create a new engine with the default set of specifications.
    pub fn new() -> Self {
        let specs: Vec<Box<dyn DecisionSpecification>> = vec![
            Box::new(AlreadyImportedSpec),
            Box::new(BlocklistSpec),
            Box::new(QueueConflictSpec),
            Box::new(QualityAllowedSpec),
            Box::new(QualityCutoffSpec),
            Box::new(CustomFormatCutoffSpec),
            Box::new(LanguageSpec),
            Box::new(MinimumSizeSpec),
            Box::new(MaximumSizeSpec),
            Box::new(MinimumSeedersSpec),
            Box::new(CustomFormatScoreSpec),
        ];
        Self { specs }
    }

    /// Decide whether a release should be grabbed.
    pub fn decide(&self, context: DecisionContext) -> DownloadDecision {
        let mut rejections = Vec::new();
        for spec in &self.specs {
            if let Some(r) = spec.is_satisfied(&context) {
                rejections.push(r);
            }
        }
        let approved = rejections.is_empty();
        let cf_score = context.release_custom_format_score;
        let matched_formats = context.matched_formats;
        DownloadDecision {
            approved,
            release: context.release,
            rejections,
            custom_format_score: cf_score,
            matched_formats,
        }
    }
}

impl Default for DecisionEngine {
    fn default() -> Self {
        Self::new()
    }
}

// ── Grab Strategy ───────────────────────────────────────────────────────────

/// Controls how releases are ranked when multiple results are available.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum GrabStrategy {
    /// Rank by quality first, use indexer priority as tiebreaker.
    #[default]
    BestQuality,
    /// Rank by indexer priority first, then quality within same priority.
    IndexerPriority,
}

// ── Release ranking ─────────────────────────────────────────────────────────

/// Sort approved decisions by preference according to the chosen strategy.
pub fn rank_releases(
    mut decisions: Vec<DownloadDecision>,
    strategy: GrabStrategy,
) -> Vec<DownloadDecision> {
    decisions.sort_by(|a, b| {
        // Always: approved first
        let cmp = b.approved.cmp(&a.approved);

        match strategy {
            GrabStrategy::BestQuality => {
                cmp
                    // Quality first
                    .then_with(|| {
                        let qa = parse_quality_num(&a.release.title);
                        let qb = parse_quality_num(&b.release.title);
                        qb.cmp(&qa)
                    })
                    // Custom format score (higher is better)
                    .then_with(|| b.custom_format_score.cmp(&a.custom_format_score))
                    // More seeders
                    .then_with(|| {
                        let sa = a.release.seeders.unwrap_or(0);
                        let sb = b.release.seeders.unwrap_or(0);
                        sb.cmp(&sa)
                    })
                    // Newer first
                    .then_with(|| a.release.age_days.cmp(&b.release.age_days))
                    // Indexer priority as tiebreaker
                    .then_with(|| a.release.indexer_priority.cmp(&b.release.indexer_priority))
            }
            GrabStrategy::IndexerPriority => {
                cmp
                    // Indexer priority first (lower = higher priority)
                    .then_with(|| a.release.indexer_priority.cmp(&b.release.indexer_priority))
                    // Then quality
                    .then_with(|| {
                        let qa = parse_quality_num(&a.release.title);
                        let qb = parse_quality_num(&b.release.title);
                        qb.cmp(&qa)
                    })
                    // Custom format score (higher is better)
                    .then_with(|| b.custom_format_score.cmp(&a.custom_format_score))
                    // More seeders
                    .then_with(|| {
                        let sa = a.release.seeders.unwrap_or(0);
                        let sb = b.release.seeders.unwrap_or(0);
                        sb.cmp(&sa)
                    })
                    // Newer first
                    .then_with(|| a.release.age_days.cmp(&b.release.age_days))
            }
        }
    });
    decisions
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn make_release(title: &str) -> ReleaseInfo {
        ReleaseInfo {
            guid: "test-guid".to_string(),
            title: title.to_string(),
            download_url: Some("http://example.com/dl".to_string()),
            info_url: None,
            indexer_id: 1,
            indexer_name: "TestIndexer".to_string(),
            protocol: DownloadProtocol::Usenet,
            size: 1_500_000_000, // 1.5GB
            age_days: 1,
            publish_date: Utc::now(),
            info_hash: None,
            magnet_url: None,
            seeders: None,
            leechers: None,
            nzb_url: None,
            tvdb_id: None,
            imdb_id: None,
            tmdb_id: None,
            categories: vec![],
            indexer_flags: vec![],
            indexer_priority: 25,
        }
    }

    fn make_torrent_release(title: &str, seeders: i32) -> ReleaseInfo {
        let mut r = make_release(title);
        r.protocol = DownloadProtocol::Torrent;
        r.seeders = Some(seeders);
        r
    }

    fn make_profile(items_json: &str) -> QualityProfile {
        QualityProfile {
            id: 1,
            name: "Test Profile".to_string(),
            cutoff: 11, // WEBDL-1080p
            upgrade_allowed: true,
            min_format_score: 0,
            cutoff_format_score: 0,
            items: serde_json::from_str(items_json).unwrap(),
            media_type: None,
            language: -1,
            min_upgrade_format_score: 1,
        }
    }

    fn make_context(release: ReleaseInfo, profile: QualityProfile) -> DecisionContext {
        DecisionContext {
            release,
            profile,
            existing_quality: None,
            existing_custom_format_score: None,
            release_custom_format_score: 0,
            matched_formats: vec![],
            in_queue: false,
            in_blocklist: false,
            already_grabbed: false,
            queued_quality: None,
            original_language: None,
        }
    }

    // ── QualityAllowedSpec ──────────────────────────────────────────────

    #[test]
    fn quality_allowed_passes_when_allowed() {
        let spec = QualityAllowedSpec;
        let release = make_release("Show.S01E01.1080p.WEB-DL.x264-GROUP");
        // Quality 11 = WEBDL-1080p
        let profile = make_profile(r#"[{"quality": 11, "allowed": true}]"#);
        let ctx = make_context(release, profile);
        assert!(spec.is_satisfied(&ctx).is_none());
    }

    #[test]
    fn quality_allowed_rejects_when_not_allowed() {
        let spec = QualityAllowedSpec;
        let release = make_release("Show.S01E01.1080p.WEB-DL.x264-GROUP");
        // Quality 11 present but not allowed
        let profile = make_profile(r#"[{"quality": 11, "allowed": false}]"#);
        let ctx = make_context(release, profile);
        let rejection = spec.is_satisfied(&ctx);
        assert!(rejection.is_some());
        assert_eq!(rejection.unwrap().rejection_type, RejectionType::Permanent);
    }

    #[test]
    fn quality_allowed_rejects_when_quality_missing_from_profile() {
        let spec = QualityAllowedSpec;
        let release = make_release("Show.S01E01.1080p.WEB-DL.x264-GROUP");
        // Only 720p qualities in profile, not 1080p
        let profile = make_profile(r#"[{"quality": 6, "allowed": true}]"#);
        let ctx = make_context(release, profile);
        assert!(spec.is_satisfied(&ctx).is_some());
    }

    #[test]
    fn quality_allowed_handles_nested_groups() {
        let spec = QualityAllowedSpec;
        let release = make_release("Show.S01E01.1080p.WEB-DL.x264-GROUP");
        let profile = make_profile(
            r#"[{"quality": null, "allowed": true, "items": [{"quality": 11, "allowed": true, "items": []}]}]"#,
        );
        let ctx = make_context(release, profile);
        assert!(spec.is_satisfied(&ctx).is_none());
    }

    #[test]
    fn quality_allowed_disabled_group_rejects_children() {
        let spec = QualityAllowedSpec;
        let release = make_release("Show.S01E01.1080p.WEB-DL.x264-GROUP");
        // Group is disabled even though child is allowed
        let profile = make_profile(
            r#"[{"quality": null, "allowed": false, "items": [{"quality": 11, "allowed": true, "items": []}]}]"#,
        );
        let ctx = make_context(release, profile);
        assert!(spec.is_satisfied(&ctx).is_some());
    }

    // ── QualityCutoffSpec ───────────────────────────────────────────────

    #[test]
    fn cutoff_passes_when_no_existing_file() {
        let spec = QualityCutoffSpec;
        let release = make_release("Show.S01E01.1080p.WEB-DL.x264-GROUP");
        let profile = make_profile(r#"[{"quality": 11, "allowed": true}]"#);
        let ctx = make_context(release, profile);
        assert!(spec.is_satisfied(&ctx).is_none());
    }

    #[test]
    fn cutoff_passes_when_upgrade_available() {
        let spec = QualityCutoffSpec;
        let release = make_release("Show.S01E01.1080p.WEB-DL.x264-GROUP"); // quality 11
        let profile = make_profile(r#"[{"quality": 11, "allowed": true}]"#);
        let mut ctx = make_context(release, profile);
        ctx.existing_quality = Some(6); // HDTV-720p, below cutoff of 11
        assert!(spec.is_satisfied(&ctx).is_none());
    }

    #[test]
    fn cutoff_rejects_when_cutoff_met() {
        let spec = QualityCutoffSpec;
        let release = make_release("Show.S01E01.1080p.BluRay.x264-GROUP"); // quality 13
        let mut profile = make_profile(r#"[{"quality": 13, "allowed": true}]"#);
        profile.cutoff = 11; // WEBDL-1080p
        let mut ctx = make_context(release, profile);
        ctx.existing_quality = Some(11); // Already at cutoff
        let rejection = spec.is_satisfied(&ctx);
        assert!(rejection.is_some());
        assert!(rejection.unwrap().reason.contains("cutoff already met"));
    }

    #[test]
    fn cutoff_rejects_when_not_an_upgrade() {
        let spec = QualityCutoffSpec;
        let release = make_release("Show.S01E01.720p.HDTV.x264-GROUP"); // quality 6
        let mut profile = make_profile(r#"[{"quality": 6, "allowed": true}]"#);
        profile.cutoff = 13; // Bluray-1080p
        let mut ctx = make_context(release, profile);
        ctx.existing_quality = Some(7); // WEBDL-720p, release (6) <= existing (7)
        let rejection = spec.is_satisfied(&ctx);
        assert!(rejection.is_some());
        assert!(rejection.unwrap().reason.contains("not an upgrade"));
    }

    #[test]
    fn cutoff_rejects_when_upgrades_disabled() {
        let spec = QualityCutoffSpec;
        let release = make_release("Show.S01E01.1080p.BluRay.x264-GROUP");
        let mut profile = make_profile(r#"[{"quality": 13, "allowed": true}]"#);
        profile.upgrade_allowed = false;
        let mut ctx = make_context(release, profile);
        ctx.existing_quality = Some(6);
        let rejection = spec.is_satisfied(&ctx);
        assert!(rejection.is_some());
        assert!(rejection.unwrap().reason.contains("upgrades are disabled"));
    }

    // ── MinimumSizeSpec ─────────────────────────────────────────────────

    #[test]
    fn minimum_size_passes_within_range() {
        let spec = MinimumSizeSpec;
        let mut release = make_release("Show.S01E01.1080p.WEB-DL.x264-GROUP");
        release.size = 500_000_000; // 500MB, within 1080p range
        let profile = make_profile(r#"[{"quality": 11, "allowed": true}]"#);
        let ctx = make_context(release, profile);
        assert!(spec.is_satisfied(&ctx).is_none());
    }

    #[test]
    fn minimum_size_rejects_too_small() {
        let spec = MinimumSizeSpec;
        let mut release = make_release("Show.S01E01.1080p.WEB-DL.x264-GROUP");
        release.size = 10_000_000; // 10MB, below 200MB minimum for 1080p
        let profile = make_profile(r#"[{"quality": 11, "allowed": true}]"#);
        let ctx = make_context(release, profile);
        let rejection = spec.is_satisfied(&ctx);
        assert!(rejection.is_some());
        assert!(rejection.unwrap().reason.contains("below minimum"));
    }

    #[test]
    fn minimum_size_passes_when_size_unknown() {
        let spec = MinimumSizeSpec;
        let mut release = make_release("Show.S01E01.1080p.WEB-DL.x264-GROUP");
        release.size = 0; // Unknown size
        let profile = make_profile(r#"[{"quality": 11, "allowed": true}]"#);
        let ctx = make_context(release, profile);
        assert!(spec.is_satisfied(&ctx).is_none());
    }

    // ── MaximumSizeSpec ─────────────────────────────────────────────────

    #[test]
    fn maximum_size_passes_within_range() {
        let spec = MaximumSizeSpec;
        let mut release = make_release("Show.S01E01.1080p.WEB-DL.x264-GROUP");
        release.size = 5_000_000_000; // 5GB, within 1080p range
        let profile = make_profile(r#"[{"quality": 11, "allowed": true}]"#);
        let ctx = make_context(release, profile);
        assert!(spec.is_satisfied(&ctx).is_none());
    }

    #[test]
    fn maximum_size_rejects_too_large() {
        let spec = MaximumSizeSpec;
        let mut release = make_release("Show.S01E01.1080p.WEB-DL.x264-GROUP");
        release.size = 25_000_000_000; // 25GB, above 20GB max for 1080p
        let profile = make_profile(r#"[{"quality": 11, "allowed": true}]"#);
        let ctx = make_context(release, profile);
        let rejection = spec.is_satisfied(&ctx);
        assert!(rejection.is_some());
        assert!(rejection.unwrap().reason.contains("exceeds maximum"));
    }

    // ── BlocklistSpec ───────────────────────────────────────────────────

    #[test]
    fn blocklist_passes_when_not_blocklisted() {
        let spec = BlocklistSpec;
        let release = make_release("Show.S01E01.1080p.WEB-DL.x264-GROUP");
        let profile = make_profile(r#"[{"quality": 11, "allowed": true}]"#);
        let ctx = make_context(release, profile);
        assert!(spec.is_satisfied(&ctx).is_none());
    }

    #[test]
    fn blocklist_rejects_when_blocklisted() {
        let spec = BlocklistSpec;
        let release = make_release("Show.S01E01.1080p.WEB-DL.x264-GROUP");
        let profile = make_profile(r#"[{"quality": 11, "allowed": true}]"#);
        let mut ctx = make_context(release, profile);
        ctx.in_blocklist = true;
        let rejection = spec.is_satisfied(&ctx);
        assert!(rejection.is_some());
        assert_eq!(rejection.unwrap().rejection_type, RejectionType::Permanent);
    }

    // ── QueueConflictSpec ───────────────────────────────────────────────

    #[test]
    fn queue_passes_when_not_queued() {
        let spec = QueueConflictSpec;
        let release = make_release("Show.S01E01.1080p.WEB-DL.x264-GROUP");
        let profile = make_profile(r#"[{"quality": 11, "allowed": true}]"#);
        let ctx = make_context(release, profile);
        assert!(spec.is_satisfied(&ctx).is_none());
    }

    #[test]
    fn queue_rejects_when_already_queued() {
        let spec = QueueConflictSpec;
        let release = make_release("Show.S01E01.1080p.WEB-DL.x264-GROUP");
        let profile = make_profile(r#"[{"quality": 11, "allowed": true}]"#);
        let mut ctx = make_context(release, profile);
        ctx.in_queue = true;
        let rejection = spec.is_satisfied(&ctx);
        assert!(rejection.is_some());
        assert_eq!(rejection.unwrap().rejection_type, RejectionType::Temporary);
    }

    // ── MinimumSeedersSpec ──────────────────────────────────────────────

    #[test]
    fn seeders_passes_for_usenet() {
        let spec = MinimumSeedersSpec;
        let release = make_release("Show.S01E01.1080p.WEB-DL.x264-GROUP");
        let profile = make_profile(r#"[{"quality": 11, "allowed": true}]"#);
        let ctx = make_context(release, profile);
        // Usenet — spec doesn't apply
        assert!(spec.is_satisfied(&ctx).is_none());
    }

    #[test]
    fn seeders_passes_with_enough_seeders() {
        let spec = MinimumSeedersSpec;
        let release = make_torrent_release("Show.S01E01.1080p.WEB-DL.x264-GROUP", 5);
        let profile = make_profile(r#"[{"quality": 11, "allowed": true}]"#);
        let ctx = make_context(release, profile);
        assert!(spec.is_satisfied(&ctx).is_none());
    }

    #[test]
    fn seeders_rejects_zero_seeders_torrent() {
        let spec = MinimumSeedersSpec;
        let release = make_torrent_release("Show.S01E01.1080p.WEB-DL.x264-GROUP", 0);
        let profile = make_profile(r#"[{"quality": 11, "allowed": true}]"#);
        let ctx = make_context(release, profile);
        let rejection = spec.is_satisfied(&ctx);
        assert!(rejection.is_some());
        assert_eq!(rejection.unwrap().rejection_type, RejectionType::Temporary);
    }

    #[test]
    fn seeders_rejects_no_seeder_info_torrent() {
        let spec = MinimumSeedersSpec;
        let mut release = make_release("Show.S01E01.1080p.WEB-DL.x264-GROUP");
        release.protocol = DownloadProtocol::Torrent;
        release.seeders = None; // No seeder info
        let profile = make_profile(r#"[{"quality": 11, "allowed": true}]"#);
        let ctx = make_context(release, profile);
        let rejection = spec.is_satisfied(&ctx);
        assert!(rejection.is_some());
    }

    // ── CustomFormatScoreSpec ───────────────────────────────────────────

    #[test]
    fn custom_format_passes_when_minimum_is_zero() {
        let spec = CustomFormatScoreSpec;
        let release = make_release("Show.S01E01.1080p.WEB-DL.x264-GROUP");
        let profile = make_profile(r#"[{"quality": 11, "allowed": true}]"#);
        let ctx = make_context(release, profile);
        assert!(spec.is_satisfied(&ctx).is_none());
    }

    #[test]
    fn custom_format_rejects_when_below_minimum() {
        let spec = CustomFormatScoreSpec;
        let release = make_release("Show.S01E01.1080p.WEB-DL.x264-GROUP");
        let mut profile = make_profile(r#"[{"quality": 11, "allowed": true}]"#);
        profile.min_format_score = 10;
        let ctx = make_context(release, profile);
        let rejection = spec.is_satisfied(&ctx);
        assert!(rejection.is_some());
        assert!(rejection.unwrap().reason.contains("below minimum"));
    }

    // ── DecisionEngine integration ──────────────────────────────────────

    #[test]
    fn engine_approves_good_release() {
        let engine = DecisionEngine::new();
        let release = make_release("Show.S01E01.1080p.WEB-DL.x264-GROUP");
        let profile = make_profile(r#"[{"quality": 11, "allowed": true}]"#);
        let ctx = make_context(release, profile);
        let decision = engine.decide(ctx);
        assert!(decision.approved);
        assert!(decision.rejections.is_empty());
    }

    #[test]
    fn engine_rejects_disallowed_quality() {
        let engine = DecisionEngine::new();
        let release = make_release("Show.S01E01.1080p.WEB-DL.x264-GROUP");
        let profile = make_profile(r#"[{"quality": 6, "allowed": true}]"#);
        let ctx = make_context(release, profile);
        let decision = engine.decide(ctx);
        assert!(!decision.approved);
        assert!(!decision.rejections.is_empty());
    }

    #[test]
    fn engine_collects_multiple_rejections() {
        let engine = DecisionEngine::new();
        let mut release = make_release("Show.S01E01.1080p.WEB-DL.x264-GROUP");
        release.size = 10_000_000; // Too small
        let profile = make_profile(r#"[{"quality": 6, "allowed": true}]"#); // Wrong quality
        let mut ctx = make_context(release, profile);
        ctx.in_blocklist = true;
        let decision = engine.decide(ctx);
        assert!(!decision.approved);
        // Should have at least blocklist + quality + size rejections
        assert!(decision.rejections.len() >= 3);
    }

    // ── rank_releases ───────────────────────────────────────────────────

    #[test]
    fn rank_approved_before_rejected() {
        let approved = DownloadDecision {
            approved: true,
            release: make_release("Show.S01E01.1080p.WEB-DL.x264-GROUP"),
            rejections: vec![],
            custom_format_score: 0,
            matched_formats: vec![],
        };
        let rejected = DownloadDecision {
            approved: false,
            release: make_release("Show.S01E01.720p.HDTV.x264-GROUP"),
            rejections: vec![Rejection {
                reason: "not allowed".to_string(),
                rejection_type: RejectionType::Permanent,
            }],
            custom_format_score: 0,
            matched_formats: vec![],
        };
        let ranked = rank_releases(vec![rejected, approved], GrabStrategy::BestQuality);
        assert!(ranked[0].approved);
        assert!(!ranked[1].approved);
    }

    #[test]
    fn rank_higher_quality_first() {
        let r1080 = DownloadDecision {
            approved: true,
            release: make_release("Show.S01E01.1080p.WEB-DL.x264-GROUP"), // quality 11
            rejections: vec![],
            custom_format_score: 0,
            matched_formats: vec![],
        };
        let r720 = DownloadDecision {
            approved: true,
            release: make_release("Show.S01E01.720p.HDTV.x264-GROUP"), // quality 6
            rejections: vec![],
            custom_format_score: 0,
            matched_formats: vec![],
        };
        let ranked = rank_releases(vec![r720, r1080], GrabStrategy::BestQuality);
        // 1080p should come first
        assert!(ranked[0].release.title.contains("1080p"));
        assert!(ranked[1].release.title.contains("720p"));
    }

    #[test]
    fn rank_more_seeders_first() {
        let r_many = DownloadDecision {
            approved: true,
            release: make_torrent_release("Show.S01E01.1080p.WEB-DL.x264-A", 50),
            rejections: vec![],
            custom_format_score: 0,
            matched_formats: vec![],
        };
        let r_few = DownloadDecision {
            approved: true,
            release: make_torrent_release("Show.S01E01.1080p.WEB-DL.x264-B", 5),
            rejections: vec![],
            custom_format_score: 0,
            matched_formats: vec![],
        };
        let ranked = rank_releases(vec![r_few, r_many], GrabStrategy::BestQuality);
        assert_eq!(ranked[0].release.seeders, Some(50));
        assert_eq!(ranked[1].release.seeders, Some(5));
    }

    #[test]
    fn rank_newer_first_when_equal_quality_and_seeders() {
        let mut r_new = make_release("Show.S01E01.1080p.WEB-DL.x264-A");
        r_new.age_days = 1;
        let mut r_old = make_release("Show.S01E01.1080p.WEB-DL.x264-B");
        r_old.age_days = 10;

        let d_new = DownloadDecision {
            approved: true,
            release: r_new,
            rejections: vec![],
            custom_format_score: 0,
            matched_formats: vec![],
        };
        let d_old = DownloadDecision {
            approved: true,
            release: r_old,
            rejections: vec![],
            custom_format_score: 0,
            matched_formats: vec![],
        };
        let ranked = rank_releases(vec![d_old, d_new], GrabStrategy::BestQuality);
        assert_eq!(ranked[0].release.age_days, 1);
        assert_eq!(ranked[1].release.age_days, 10);
    }

    // ── AlreadyImportedSpec ───────────────────────────────────────────

    #[test]
    fn already_imported_passes_when_not_grabbed() {
        let spec = AlreadyImportedSpec;
        let release = make_release("Show.S01E01.1080p.WEB-DL.x264-GROUP");
        let profile = make_profile(r#"[{"quality": 11, "allowed": true}]"#);
        let ctx = make_context(release, profile);
        assert!(spec.is_satisfied(&ctx).is_none());
    }

    #[test]
    fn already_imported_rejects_when_already_grabbed() {
        let spec = AlreadyImportedSpec;
        let release = make_release("Show.S01E01.1080p.WEB-DL.x264-GROUP");
        let profile = make_profile(r#"[{"quality": 11, "allowed": true}]"#);
        let mut ctx = make_context(release, profile);
        ctx.already_grabbed = true;
        let rejection = spec.is_satisfied(&ctx);
        assert!(rejection.is_some());
        assert_eq!(rejection.unwrap().rejection_type, RejectionType::Permanent);
    }

    // ── CustomFormatScoreSpec with real score ─────────────────────────

    #[test]
    fn custom_format_passes_with_sufficient_score() {
        let spec = CustomFormatScoreSpec;
        let release = make_release("Show.S01E01.1080p.WEB-DL.x264-GROUP");
        let mut profile = make_profile(r#"[{"quality": 11, "allowed": true}]"#);
        profile.min_format_score = 5;
        let mut ctx = make_context(release, profile);
        ctx.release_custom_format_score = 10;
        assert!(spec.is_satisfied(&ctx).is_none());
    }

    #[test]
    fn custom_format_rejects_with_insufficient_score() {
        let spec = CustomFormatScoreSpec;
        let release = make_release("Show.S01E01.1080p.WEB-DL.x264-GROUP");
        let mut profile = make_profile(r#"[{"quality": 11, "allowed": true}]"#);
        profile.min_format_score = 10;
        let mut ctx = make_context(release, profile);
        ctx.release_custom_format_score = 3;
        let rejection = spec.is_satisfied(&ctx);
        assert!(rejection.is_some());
        assert!(rejection.unwrap().reason.contains("below minimum"));
    }

    #[test]
    fn custom_format_passes_at_exact_minimum() {
        let spec = CustomFormatScoreSpec;
        let release = make_release("Show.S01E01.1080p.WEB-DL.x264-GROUP");
        let mut profile = make_profile(r#"[{"quality": 11, "allowed": true}]"#);
        profile.min_format_score = 10;
        let mut ctx = make_context(release, profile);
        ctx.release_custom_format_score = 10;
        assert!(spec.is_satisfied(&ctx).is_none());
    }

    // ── Size limits per quality tier ──────────────────────────────────

    #[test]
    fn minimum_size_sd_within_range() {
        let spec = MinimumSizeSpec;
        // HDTV without resolution parses as SDTV (quality 1)
        let mut release = make_release("Show.S01E01.HDTV.x264-GROUP");
        release.size = 100_000_000; // 100MB, within SD range (50MB-3GB)
        let profile = make_profile(r#"[{"quality": 1, "allowed": true}]"#);
        let ctx = make_context(release, profile);
        assert!(spec.is_satisfied(&ctx).is_none());
    }

    #[test]
    fn minimum_size_sd_below_minimum() {
        let spec = MinimumSizeSpec;
        let mut release = make_release("Show.S01E01.HDTV.x264-GROUP");
        release.size = 10_000_000; // 10MB, below 50MB minimum for SD
        let profile = make_profile(r#"[{"quality": 1, "allowed": true}]"#);
        let ctx = make_context(release, profile);
        assert!(spec.is_satisfied(&ctx).is_some());
    }

    #[test]
    fn maximum_size_sd_exceeds() {
        let spec = MaximumSizeSpec;
        let mut release = make_release("Show.S01E01.HDTV.x264-GROUP");
        release.size = 5_000_000_000; // 5GB, above 3GB max for SD
        let profile = make_profile(r#"[{"quality": 1, "allowed": true}]"#);
        let ctx = make_context(release, profile);
        assert!(spec.is_satisfied(&ctx).is_some());
    }

    #[test]
    fn minimum_size_720p_within_range() {
        let spec = MinimumSizeSpec;
        let mut release = make_release("Show.S01E01.720p.HDTV.x264-GROUP");
        release.size = 500_000_000; // 500MB, within 720p range (100MB-8GB)
        let profile = make_profile(r#"[{"quality": 6, "allowed": true}]"#);
        let ctx = make_context(release, profile);
        assert!(spec.is_satisfied(&ctx).is_none());
    }

    #[test]
    fn minimum_size_720p_below_minimum() {
        let spec = MinimumSizeSpec;
        let mut release = make_release("Show.S01E01.720p.HDTV.x264-GROUP");
        release.size = 50_000_000; // 50MB, below 100MB minimum for 720p
        let profile = make_profile(r#"[{"quality": 6, "allowed": true}]"#);
        let ctx = make_context(release, profile);
        assert!(spec.is_satisfied(&ctx).is_some());
    }

    #[test]
    fn maximum_size_720p_exceeds() {
        let spec = MaximumSizeSpec;
        let mut release = make_release("Show.S01E01.720p.HDTV.x264-GROUP");
        release.size = 10_000_000_000; // 10GB, above 8GB max for 720p
        let profile = make_profile(r#"[{"quality": 6, "allowed": true}]"#);
        let ctx = make_context(release, profile);
        assert!(spec.is_satisfied(&ctx).is_some());
    }

    #[test]
    fn minimum_size_2160p_within_range() {
        let spec = MinimumSizeSpec;
        let mut release = make_release("Show.S01E01.2160p.WEB-DL.x265-GROUP");
        release.size = 10_000_000_000; // 10GB, within 2160p range (500MB-80GB)
        let profile = make_profile(r#"[{"quality": 16, "allowed": true}]"#);
        let ctx = make_context(release, profile);
        assert!(spec.is_satisfied(&ctx).is_none());
    }

    #[test]
    fn maximum_size_2160p_exceeds() {
        let spec = MaximumSizeSpec;
        let mut release = make_release("Show.S01E01.2160p.WEB-DL.x265-GROUP");
        release.size = 90_000_000_000; // 90GB, above 80GB max for 2160p
        let profile = make_profile(r#"[{"quality": 16, "allowed": true}]"#);
        let ctx = make_context(release, profile);
        assert!(spec.is_satisfied(&ctx).is_some());
    }

    #[test]
    fn maximum_size_passes_when_size_unknown() {
        let spec = MaximumSizeSpec;
        let mut release = make_release("Show.S01E01.1080p.WEB-DL.x264-GROUP");
        release.size = 0; // Unknown size
        let profile = make_profile(r#"[{"quality": 11, "allowed": true}]"#);
        let ctx = make_context(release, profile);
        assert!(spec.is_satisfied(&ctx).is_none());
    }

    // ── MinimumSeedersSpec edge cases ─────────────────────────────────

    #[test]
    fn seeders_passes_with_exactly_one_seeder() {
        let spec = MinimumSeedersSpec;
        let release = make_torrent_release("Show.S01E01.1080p.WEB-DL.x264-GROUP", 1);
        let profile = make_profile(r#"[{"quality": 11, "allowed": true}]"#);
        let ctx = make_context(release, profile);
        assert!(spec.is_satisfied(&ctx).is_none());
    }

    #[test]
    fn seeders_passes_with_many_seeders() {
        let spec = MinimumSeedersSpec;
        let release = make_torrent_release("Show.S01E01.1080p.WEB-DL.x264-GROUP", 1000);
        let profile = make_profile(r#"[{"quality": 11, "allowed": true}]"#);
        let ctx = make_context(release, profile);
        assert!(spec.is_satisfied(&ctx).is_none());
    }

    // ── QualityCutoffSpec edge cases ──────────────────────────────────

    #[test]
    fn cutoff_passes_upgrade_below_cutoff() {
        let spec = QualityCutoffSpec;
        let release = make_release("Show.S01E01.1080p.WEB-DL.x264-GROUP"); // quality 11
        let mut profile = make_profile(r#"[{"quality": 11, "allowed": true}]"#);
        profile.cutoff = 13; // Bluray-1080p
        let mut ctx = make_context(release, profile);
        ctx.existing_quality = Some(6); // HDTV-720p, below cutoff and below release
        assert!(spec.is_satisfied(&ctx).is_none());
    }

    #[test]
    fn cutoff_rejects_same_quality_as_existing() {
        let spec = QualityCutoffSpec;
        let release = make_release("Show.S01E01.1080p.WEB-DL.x264-GROUP"); // quality 11
        let mut profile = make_profile(r#"[{"quality": 11, "allowed": true}]"#);
        profile.cutoff = 13;
        let mut ctx = make_context(release, profile);
        ctx.existing_quality = Some(11); // Same quality — not an upgrade
        let rejection = spec.is_satisfied(&ctx);
        assert!(rejection.is_some());
        assert!(rejection.unwrap().reason.contains("not an upgrade"));
    }

    // ── DecisionEngine full integration ───────────────────────────────

    #[test]
    fn engine_approves_torrent_with_seeders() {
        let engine = DecisionEngine::new();
        let release = make_torrent_release("Show.S01E01.1080p.WEB-DL.x264-GROUP", 10);
        let profile = make_profile(r#"[{"quality": 11, "allowed": true}]"#);
        let ctx = make_context(release, profile);
        let decision = engine.decide(ctx);
        assert!(decision.approved);
    }

    #[test]
    fn engine_rejects_torrent_zero_seeders() {
        let engine = DecisionEngine::new();
        let release = make_torrent_release("Show.S01E01.1080p.WEB-DL.x264-GROUP", 0);
        let profile = make_profile(r#"[{"quality": 11, "allowed": true}]"#);
        let ctx = make_context(release, profile);
        let decision = engine.decide(ctx);
        assert!(!decision.approved);
        assert!(decision.rejections.iter().any(|r| r.reason.contains("seeders")));
    }

    #[test]
    fn engine_rejects_already_grabbed() {
        let engine = DecisionEngine::new();
        let release = make_release("Show.S01E01.1080p.WEB-DL.x264-GROUP");
        let profile = make_profile(r#"[{"quality": 11, "allowed": true}]"#);
        let mut ctx = make_context(release, profile);
        ctx.already_grabbed = true;
        let decision = engine.decide(ctx);
        assert!(!decision.approved);
        assert!(decision.rejections.iter().any(|r| r.reason.contains("already been grabbed")));
    }

    #[test]
    fn engine_rejects_blocklisted_and_wrong_quality() {
        let engine = DecisionEngine::new();
        let release = make_release("Show.S01E01.720p.HDTV.x264-GROUP");
        let profile = make_profile(r#"[{"quality": 11, "allowed": true}]"#); // Only 1080p allowed
        let mut ctx = make_context(release, profile);
        ctx.in_blocklist = true;
        let decision = engine.decide(ctx);
        assert!(!decision.approved);
        // Should have both blocklist and quality rejections
        assert!(decision.rejections.iter().any(|r| r.reason.contains("blocklist")));
        assert!(decision.rejections.iter().any(|r| r.reason.contains("not allowed")));
    }

    #[test]
    fn engine_approves_first_file_no_existing() {
        let engine = DecisionEngine::new();
        let release = make_release("Movie.2024.720p.BluRay.x264-GROUP");
        let profile = make_profile(r#"[{"quality": 9, "allowed": true}]"#);
        let ctx = make_context(release, profile);
        let decision = engine.decide(ctx);
        assert!(decision.approved);
    }

    // ── rank_releases additional ──────────────────────────────────────

    #[test]
    fn rank_2160p_before_1080p() {
        let r2160 = DownloadDecision {
            approved: true,
            release: make_release("Movie.2024.2160p.WEB-DL.x265-GROUP"),
            rejections: vec![],
            custom_format_score: 0,
            matched_formats: vec![],
        };
        let r1080 = DownloadDecision {
            approved: true,
            release: make_release("Movie.2024.1080p.WEB-DL.x264-GROUP"),
            rejections: vec![],
            custom_format_score: 0,
            matched_formats: vec![],
        };
        let ranked = rank_releases(vec![r1080, r2160], GrabStrategy::BestQuality);
        assert!(ranked[0].release.title.contains("2160p"));
        assert!(ranked[1].release.title.contains("1080p"));
    }

    #[test]
    fn rank_empty_list() {
        let ranked = rank_releases(vec![], GrabStrategy::BestQuality);
        assert!(ranked.is_empty());
    }

    #[test]
    fn rank_single_item() {
        let d = DownloadDecision {
            approved: true,
            release: make_release("Movie.2024.1080p.WEB-DL.x264-GROUP"),
            rejections: vec![],
            custom_format_score: 0,
            matched_formats: vec![],
        };
        let ranked = rank_releases(vec![d], GrabStrategy::BestQuality);
        assert_eq!(ranked.len(), 1);
        assert!(ranked[0].approved);
    }

    #[test]
    fn rank_multiple_rejected_by_quality() {
        let r_2160 = DownloadDecision {
            approved: false,
            release: make_release("Movie.2024.2160p.WEB-DL.x265-A"),
            rejections: vec![Rejection {
                reason: "test".to_string(),
                rejection_type: RejectionType::Permanent,
            }],
            custom_format_score: 0,
            matched_formats: vec![],
        };
        let r_720 = DownloadDecision {
            approved: false,
            release: make_release("Movie.2024.720p.HDTV.x264-B"),
            rejections: vec![Rejection {
                reason: "test".to_string(),
                rejection_type: RejectionType::Permanent,
            }],
            custom_format_score: 0,
            matched_formats: vec![],
        };
        let ranked = rank_releases(vec![r_720, r_2160], GrabStrategy::BestQuality);
        // Even among rejected, higher quality first
        assert!(ranked[0].release.title.contains("2160p"));
        assert!(ranked[1].release.title.contains("720p"));
    }

    // ── Quality mapping ───────────────────────────────────────────────

    #[test]
    fn quality_name_known_values() {
        assert_eq!(quality_name(0), "Unknown");
        assert_eq!(quality_name(1), "SDTV");
        assert_eq!(quality_name(2), "DVD");
        assert_eq!(quality_name(6), "HDTV-720p");
        assert_eq!(quality_name(11), "WEBDL-1080p");
        assert_eq!(quality_name(13), "Bluray-1080p");
        assert_eq!(quality_name(16), "WEBDL-2160p");
        assert_eq!(quality_name(19), "Remux-2160p");
        assert_eq!(quality_name(20), "Raw-HD");
    }

    #[test]
    fn quality_name_unknown_returns_unknown() {
        assert_eq!(quality_name(99), "Unknown");
        assert_eq!(quality_name(-1), "Unknown");
    }

    #[test]
    fn size_limits_sd() {
        let (min, max) = size_limits(1);
        assert_eq!(min, 50_000_000);
        assert_eq!(max, 3_000_000_000);
    }

    #[test]
    fn size_limits_720p() {
        let (min, max) = size_limits(6);
        assert_eq!(min, 100_000_000);
        assert_eq!(max, 8_000_000_000);
    }

    #[test]
    fn size_limits_1080p() {
        let (min, max) = size_limits(11);
        assert_eq!(min, 200_000_000);
        assert_eq!(max, 20_000_000_000);
    }

    #[test]
    fn size_limits_2160p() {
        let (min, max) = size_limits(16);
        assert_eq!(min, 500_000_000);
        assert_eq!(max, 80_000_000_000);
    }

    #[test]
    fn size_limits_unknown() {
        let (min, max) = size_limits(0);
        assert_eq!(min, 0);
        assert_eq!(max, i64::MAX);
    }

    // ── Quality item deserialization ──────────────────────────────────

    #[test]
    fn quality_allowed_with_multiple_qualities() {
        let profile = make_profile(r#"[
            {"quality": 6, "allowed": true},
            {"quality": 11, "allowed": true},
            {"quality": 13, "allowed": false}
        ]"#);
        assert!(is_quality_allowed(6, &profile));
        assert!(is_quality_allowed(11, &profile));
        assert!(!is_quality_allowed(13, &profile));
        assert!(!is_quality_allowed(16, &profile));
    }

    #[test]
    fn quality_allowed_empty_profile() {
        let profile = make_profile(r#"[]"#);
        assert!(!is_quality_allowed(11, &profile));
    }

    #[test]
    fn quality_allowed_nested_group_enabled() {
        let profile = make_profile(r#"[{
            "quality": null,
            "allowed": true,
            "items": [
                {"quality": 11, "allowed": true, "items": []},
                {"quality": 12, "allowed": true, "items": []}
            ]
        }]"#);
        assert!(is_quality_allowed(11, &profile));
        assert!(is_quality_allowed(12, &profile));
    }

    #[test]
    fn quality_allowed_nested_group_disabled() {
        let profile = make_profile(r#"[{
            "quality": null,
            "allowed": false,
            "items": [
                {"quality": 11, "allowed": true, "items": []},
                {"quality": 12, "allowed": true, "items": []}
            ]
        }]"#);
        assert!(!is_quality_allowed(11, &profile));
        assert!(!is_quality_allowed(12, &profile));
    }

    // ── *arr format quality items ──────────────────────────────────────

    #[test]
    fn quality_allowed_arr_object_format() {
        // Sonarr/Radarr store quality as {"id": N, "name": "..."}
        let profile = make_profile(r#"[
            {"quality": {"id": 11, "name": "WEBDL-1080p"}, "allowed": true, "items": []},
            {"quality": {"id": 6, "name": "HDTV-720p"}, "allowed": false, "items": []}
        ]"#);
        assert!(is_quality_allowed(11, &profile));
        assert!(!is_quality_allowed(6, &profile));
    }

    #[test]
    fn quality_allowed_arr_object_with_extra_fields() {
        // Sonarr includes source/resolution fields in quality objects
        let profile = make_profile(r#"[
            {"quality": {"id": 16, "name": "WEBDL-2160p", "source": "webdl", "resolution": 2160}, "allowed": true, "items": []},
            {"quality": {"id": 11, "name": "WEBDL-1080p", "source": "webdl", "resolution": 1080}, "allowed": false, "items": []}
        ]"#);
        assert!(is_quality_allowed(16, &profile));
        assert!(!is_quality_allowed(11, &profile));
    }

    #[test]
    fn quality_allowed_arr_nested_group_with_objects() {
        // Sonarr quality groups: quality=null with nested items using object format
        let profile = make_profile(r#"[
            {"quality": null, "name": "WEB 2160p", "id": 1003, "allowed": true, "items": [
                {"quality": {"id": 16, "name": "WEBDL-2160p"}, "allowed": true, "items": []},
                {"quality": {"id": 17, "name": "WEBRip-2160p"}, "allowed": true, "items": []}
            ]},
            {"quality": {"id": 11, "name": "WEBDL-1080p"}, "allowed": false, "items": []}
        ]"#);
        assert!(is_quality_allowed(16, &profile));
        assert!(is_quality_allowed(17, &profile));
        assert!(!is_quality_allowed(11, &profile));
    }

    #[test]
    fn quality_allowed_arr_disabled_group_rejects_children() {
        let profile = make_profile(r#"[
            {"quality": null, "allowed": false, "items": [
                {"quality": {"id": 16, "name": "WEBDL-2160p"}, "allowed": true, "items": []},
                {"quality": {"id": 17, "name": "WEBRip-2160p"}, "allowed": true, "items": []}
            ]}
        ]"#);
        assert!(!is_quality_allowed(16, &profile));
        assert!(!is_quality_allowed(17, &profile));
    }

    // ── CustomFormatCutoffSpec ───────────────────────────────────────

    #[test]
    fn cf_cutoff_passes_when_no_existing_file() {
        let spec = CustomFormatCutoffSpec;
        let release = make_release("Show.S01E01.1080p.WEB-DL.x264-GROUP");
        let mut profile = make_profile(r#"[{"quality": 11, "allowed": true}]"#);
        profile.cutoff_format_score = 1000;
        let ctx = make_context(release, profile);
        assert!(spec.is_satisfied(&ctx).is_none());
    }

    #[test]
    fn cf_cutoff_rejects_when_existing_meets_cutoff() {
        let spec = CustomFormatCutoffSpec;
        let release = make_release("Show.S01E01.1080p.WEB-DL.x264-GROUP");
        let mut profile = make_profile(r#"[{"quality": 11, "allowed": true}]"#);
        profile.cutoff_format_score = 1000;
        let mut ctx = make_context(release, profile);
        ctx.existing_custom_format_score = Some(1500);
        ctx.release_custom_format_score = 400;
        let rejection = spec.is_satisfied(&ctx);
        assert!(rejection.is_some());
    }

    #[test]
    fn cf_cutoff_passes_when_release_exceeds_existing() {
        let spec = CustomFormatCutoffSpec;
        let release = make_release("Show.S01E01.1080p.WEB-DL.x264-GROUP");
        let mut profile = make_profile(r#"[{"quality": 11, "allowed": true}]"#);
        profile.cutoff_format_score = 1000;
        let mut ctx = make_context(release, profile);
        ctx.existing_custom_format_score = Some(1500);
        ctx.release_custom_format_score = 2000;
        assert!(spec.is_satisfied(&ctx).is_none());
    }

    // ── parse_quality_num integration ─────────────────────────────────

    #[test]
    fn parse_quality_num_webdl_1080p() {
        assert_eq!(parse_quality_num("Show.S01E01.1080p.WEB-DL.x264-GROUP"), 11);
    }

    #[test]
    fn parse_quality_num_bluray_2160p() {
        assert_eq!(parse_quality_num("Movie.2024.2160p.BluRay.REMUX.HEVC-GROUP"), 19);
    }

    #[test]
    fn parse_quality_num_hdtv_720p() {
        assert_eq!(parse_quality_num("Show.S01E01.720p.HDTV.x264-GROUP"), 6);
    }

    #[test]
    fn parse_quality_num_unknown() {
        assert_eq!(parse_quality_num("random-non-release-text"), 0);
    }

    // ── AlreadyImportedSpec (additional) ──────────────────────────────

    #[test]
    fn already_imported_spec_passes_when_not_grabbed() {
        let spec = AlreadyImportedSpec;
        let release = make_release("Show.S01E01.1080p.WEB-DL.x264-GROUP");
        let profile = make_profile(r#"[{"quality": 11, "allowed": true}]"#);
        let ctx = make_context(release, profile);
        assert!(spec.is_satisfied(&ctx).is_none());
    }

    #[test]
    fn already_imported_spec_rejects_when_grabbed() {
        let spec = AlreadyImportedSpec;
        let release = make_release("Show.S01E01.1080p.WEB-DL.x264-GROUP");
        let profile = make_profile(r#"[{"quality": 11, "allowed": true}]"#);
        let mut ctx = make_context(release, profile);
        ctx.already_grabbed = true;
        let rejection = spec.is_satisfied(&ctx);
        assert!(rejection.is_some());
        assert_eq!(rejection.unwrap().rejection_type, RejectionType::Permanent);
    }

    // ── CustomFormatScoreSpec (additional) ─────────────────────────────

    #[test]
    fn custom_format_score_passes_above_minimum() {
        let spec = CustomFormatScoreSpec;
        let release = make_release("Show.S01E01.1080p.WEB-DL.x264-GROUP");
        let mut profile = make_profile(r#"[{"quality": 11, "allowed": true}]"#);
        profile.min_format_score = 10;
        let mut ctx = make_context(release, profile);
        ctx.release_custom_format_score = 50;
        assert!(spec.is_satisfied(&ctx).is_none());
    }

    #[test]
    fn custom_format_score_passes_at_minimum() {
        let spec = CustomFormatScoreSpec;
        let release = make_release("Show.S01E01.1080p.WEB-DL.x264-GROUP");
        let mut profile = make_profile(r#"[{"quality": 11, "allowed": true}]"#);
        profile.min_format_score = 10;
        let mut ctx = make_context(release, profile);
        ctx.release_custom_format_score = 10;
        assert!(spec.is_satisfied(&ctx).is_none());
    }

    #[test]
    fn custom_format_score_rejects_below_minimum() {
        let spec = CustomFormatScoreSpec;
        let release = make_release("Show.S01E01.1080p.WEB-DL.x264-GROUP");
        let mut profile = make_profile(r#"[{"quality": 11, "allowed": true}]"#);
        profile.min_format_score = 10;
        let mut ctx = make_context(release, profile);
        ctx.release_custom_format_score = 5;
        let rejection = spec.is_satisfied(&ctx);
        assert!(rejection.is_some());
        assert!(rejection.unwrap().reason.contains("below minimum"));
    }

    #[test]
    fn custom_format_score_rejects_negative_score() {
        let spec = CustomFormatScoreSpec;
        let release = make_release("Show.S01E01.1080p.WEB-DL.x264-GROUP");
        let mut profile = make_profile(r#"[{"quality": 11, "allowed": true}]"#);
        profile.min_format_score = 0;
        let mut ctx = make_context(release, profile);
        ctx.release_custom_format_score = -10000;
        let rejection = spec.is_satisfied(&ctx);
        assert!(rejection.is_some());
    }

    // ── CustomFormatCutoffSpec ─────────────────────────────────────────

    #[test]
    fn cf_cutoff_spec_passes_when_no_existing_file() {
        let spec = CustomFormatCutoffSpec;
        let release = make_release("Show.S01E01.1080p.WEB-DL.x264-GROUP");
        let mut profile = make_profile(r#"[{"quality": 11, "allowed": true}]"#);
        profile.cutoff_format_score = 100;
        let ctx = make_context(release, profile);
        assert!(spec.is_satisfied(&ctx).is_none());
    }

    #[test]
    fn cf_cutoff_passes_when_cutoff_score_zero() {
        let spec = CustomFormatCutoffSpec;
        let release = make_release("Show.S01E01.1080p.WEB-DL.x264-GROUP");
        let profile = make_profile(r#"[{"quality": 11, "allowed": true}]"#);
        let mut ctx = make_context(release, profile);
        ctx.existing_custom_format_score = Some(50);
        assert!(spec.is_satisfied(&ctx).is_none());
    }

    #[test]
    fn cf_cutoff_spec_passes_when_release_exceeds_existing() {
        let spec = CustomFormatCutoffSpec;
        let release = make_release("Show.S01E01.1080p.WEB-DL.x264-GROUP");
        let mut profile = make_profile(r#"[{"quality": 11, "allowed": true}]"#);
        profile.cutoff_format_score = 100;
        let mut ctx = make_context(release, profile);
        ctx.existing_custom_format_score = Some(100);
        ctx.release_custom_format_score = 150; // Exceeds existing
        assert!(spec.is_satisfied(&ctx).is_none());
    }

    #[test]
    fn cf_cutoff_spec_rejects_when_existing_meets_cutoff() {
        let spec = CustomFormatCutoffSpec;
        let release = make_release("Show.S01E01.1080p.WEB-DL.x264-GROUP");
        let mut profile = make_profile(r#"[{"quality": 11, "allowed": true}]"#);
        profile.cutoff_format_score = 100;
        let mut ctx = make_context(release, profile);
        ctx.existing_custom_format_score = Some(100);
        ctx.release_custom_format_score = 50;
        let rejection = spec.is_satisfied(&ctx);
        assert!(rejection.is_some());
        assert!(rejection.unwrap().reason.contains("custom format score"));
    }

    // ── LanguageSpec ───────────────────────────────────────────────────

    #[test]
    fn language_passes_with_any_language() {
        let spec = LanguageSpec;
        let release = make_release("Show.S01E01.FRENCH.1080p.WEB-DL.x264-GROUP");
        let mut profile = make_profile(r#"[{"quality": 11, "allowed": true}]"#);
        profile.language = -1; // Any
        let ctx = make_context(release, profile);
        assert!(spec.is_satisfied(&ctx).is_none());
    }

    #[test]
    fn language_passes_when_matching() {
        let spec = LanguageSpec;
        let release = make_release("Show.S01E01.ENGLISH.1080p.WEB-DL.x264-GROUP");
        let mut profile = make_profile(r#"[{"quality": 11, "allowed": true}]"#);
        profile.language = 1; // English
        let ctx = make_context(release, profile);
        assert!(spec.is_satisfied(&ctx).is_none());
    }

    #[test]
    fn language_passes_for_multi_tag() {
        let spec = LanguageSpec;
        let release = make_release("Show.S01E01.MULTi.1080p.WEB-DL.x264-GROUP");
        let mut profile = make_profile(r#"[{"quality": 11, "allowed": true}]"#);
        profile.language = 1; // English required
        let ctx = make_context(release, profile);
        // Multi always matches
        assert!(spec.is_satisfied(&ctx).is_none());
    }

    #[test]
    fn language_passes_for_unknown_tag() {
        let spec = LanguageSpec;
        let release = make_release("Show.S01E01.1080p.WEB-DL.x264-GROUP"); // No language tag
        let mut profile = make_profile(r#"[{"quality": 11, "allowed": true}]"#);
        profile.language = 1; // English required
        let ctx = make_context(release, profile);
        // Unknown language passes (permissive)
        assert!(spec.is_satisfied(&ctx).is_none());
    }

    #[test]
    fn language_rejects_wrong_language() {
        let spec = LanguageSpec;
        let release = make_release("Show.S01E01.FRENCH.1080p.WEB-DL.x264-GROUP");
        let mut profile = make_profile(r#"[{"quality": 11, "allowed": true}]"#);
        profile.language = 1; // English required
        let ctx = make_context(release, profile);
        let rejection = spec.is_satisfied(&ctx);
        assert!(rejection.is_some());
        assert!(rejection.unwrap().reason.contains("English"));
    }

    #[test]
    fn language_original_passes_when_no_original_set() {
        let spec = LanguageSpec;
        let release = make_release("Show.S01E01.1080p.WEB-DL.x264-GROUP");
        let mut profile = make_profile(r#"[{"quality": 11, "allowed": true}]"#);
        profile.language = -2; // Original
        let ctx = make_context(release, profile);
        // No original_language set → pass
        assert!(spec.is_satisfied(&ctx).is_none());
    }

    // ── QueueConflictSpec (additional) ─────────────────────────────────

    #[test]
    fn queue_passes_when_release_quality_higher_than_queued() {
        let spec = QueueConflictSpec;
        let release = make_release("Show.S01E01.1080p.BluRay.x264-GROUP"); // quality 13
        let profile = make_profile(r#"[{"quality": 13, "allowed": true}]"#);
        let mut ctx = make_context(release, profile);
        ctx.queued_quality = Some(6); // 720p queued, release is 1080p
        assert!(spec.is_satisfied(&ctx).is_none());
    }

    #[test]
    fn queue_rejects_when_queued_quality_equal_or_higher() {
        let spec = QueueConflictSpec;
        let release = make_release("Show.S01E01.720p.HDTV.x264-GROUP"); // quality 6
        let profile = make_profile(r#"[{"quality": 6, "allowed": true}]"#);
        let mut ctx = make_context(release, profile);
        ctx.queued_quality = Some(11); // 1080p queued, release is 720p
        let rejection = spec.is_satisfied(&ctx);
        assert!(rejection.is_some());
        assert_eq!(rejection.unwrap().rejection_type, RejectionType::Temporary);
    }

    // ── DecisionEngine integration ─────────────────────────────────────

    #[test]
    fn engine_approves_good_release_v2() {
        let engine = DecisionEngine::new();
        let release = make_release("Show.S01E01.1080p.WEB-DL.x264-GROUP");
        let profile = make_profile(r#"[{"quality": 11, "allowed": true}]"#);
        let ctx = make_context(release, profile);
        let decision = engine.decide(ctx);
        assert!(decision.approved);
        assert!(decision.rejections.is_empty());
    }

    #[test]
    fn engine_rejects_blocklisted_release() {
        let engine = DecisionEngine::new();
        let release = make_release("Show.S01E01.1080p.WEB-DL.x264-GROUP");
        let profile = make_profile(r#"[{"quality": 11, "allowed": true}]"#);
        let mut ctx = make_context(release, profile);
        ctx.in_blocklist = true;
        let decision = engine.decide(ctx);
        assert!(!decision.approved);
        assert!(!decision.rejections.is_empty());
    }

    #[test]
    fn engine_collects_multiple_rejections_v2() {
        let engine = DecisionEngine::new();
        let mut release = make_release("Show.S01E01.1080p.WEB-DL.x264-GROUP");
        release.size = 10; // Way too small
        let profile = make_profile(r#"[{"quality": 6, "allowed": true}]"#); // Only 720p
        let mut ctx = make_context(release, profile);
        ctx.in_blocklist = true;
        let decision = engine.decide(ctx);
        assert!(!decision.approved);
        // Should have at least 2 rejections (blocklist + quality not allowed + minimum size)
        assert!(decision.rejections.len() >= 2);
    }

    // ── Quality number mapping ─────────────────────────────────────────

    #[test]
    fn test_parser_quality_to_num_mapping() {
        assert_eq!(parser_quality_to_num(stackarr_parser::Quality::Unknown), 0);
        assert_eq!(parser_quality_to_num(stackarr_parser::Quality::SDTV), 1);
        assert_eq!(parser_quality_to_num(stackarr_parser::Quality::DVD), 2);
        assert_eq!(parser_quality_to_num(stackarr_parser::Quality::DVDRip), 2);
        assert_eq!(parser_quality_to_num(stackarr_parser::Quality::HDTV720p), 6);
        assert_eq!(parser_quality_to_num(stackarr_parser::Quality::WEBDL1080p), 11);
        assert_eq!(parser_quality_to_num(stackarr_parser::Quality::Remux2160p), 19);
        assert_eq!(parser_quality_to_num(stackarr_parser::Quality::Raw), 20);
    }

    #[test]
    fn test_quality_name_mapping() {
        assert_eq!(quality_name(0), "Unknown");
        assert_eq!(quality_name(1), "SDTV");
        assert_eq!(quality_name(11), "WEBDL-1080p");
        assert_eq!(quality_name(19), "Remux-2160p");
        assert_eq!(quality_name(20), "Raw-HD");
        assert_eq!(quality_name(999), "Unknown");
    }

    // ── is_quality_allowed with various item formats ───────────────────

    #[test]
    fn test_quality_allowed_object_format() {
        let profile = make_profile(r#"[{"quality": {"id": 11, "name": "WEBDL-1080p"}, "allowed": true}]"#);
        assert!(is_quality_allowed(11, &profile));
    }

    #[test]
    fn test_quality_allowed_bare_integer() {
        let profile = make_profile(r#"[{"quality": 11, "allowed": true}]"#);
        assert!(is_quality_allowed(11, &profile));
    }

    #[test]
    fn test_quality_not_allowed() {
        let profile = make_profile(r#"[{"quality": 11, "allowed": false}]"#);
        assert!(!is_quality_allowed(11, &profile));
    }

    #[test]
    fn test_quality_missing_from_profile() {
        let profile = make_profile(r#"[{"quality": 6, "allowed": true}]"#);
        assert!(!is_quality_allowed(11, &profile));
    }

    // ── Size limits ────────────────────────────────────────────────────

    #[test]
    fn sd_size_limits() {
        let (min, max) = size_limits(1);
        assert_eq!(min, 50_000_000);
        assert_eq!(max, 3_000_000_000);
    }

    #[test]
    fn hd720_size_limits() {
        let (min, max) = size_limits(6);
        assert_eq!(min, 100_000_000);
        assert_eq!(max, 8_000_000_000);
    }

    #[test]
    fn hd1080_size_limits() {
        let (min, max) = size_limits(10);
        assert_eq!(min, 200_000_000);
        assert_eq!(max, 20_000_000_000);
    }

    #[test]
    fn uhd_size_limits() {
        let (min, max) = size_limits(15);
        assert_eq!(min, 500_000_000);
        assert_eq!(max, 80_000_000_000);
    }

    #[test]
    fn unknown_quality_size_has_no_limit() {
        let (min, max) = size_limits(0);
        assert_eq!(min, 0);
        assert_eq!(max, i64::MAX);
    }

    // ── Ranking ────────────────────────────────────────────────────────

    #[test]
    fn rank_approved_before_rejected_v2() {
        let approved = DownloadDecision {
            approved: true,
            release: make_release("Show.S01E01.720p.HDTV-GROUP"),
            rejections: vec![],
            custom_format_score: 0,
            matched_formats: vec![],
        };
        let rejected = DownloadDecision {
            approved: false,
            release: make_release("Show.S01E01.1080p.WEB-DL-GROUP"),
            rejections: vec![Rejection {
                reason: "test".to_string(),
                rejection_type: RejectionType::Permanent,
            }],
            custom_format_score: 0,
            matched_formats: vec![],
        };
        let ranked = rank_releases(vec![rejected, approved], GrabStrategy::BestQuality);
        assert!(ranked[0].approved);
        assert!(!ranked[1].approved);
    }

    #[test]
    fn rank_best_quality_higher_quality_first() {
        let lower = DownloadDecision {
            approved: true,
            release: make_release("Show.S01E01.720p.HDTV-GROUP"),
            rejections: vec![],
            custom_format_score: 0,
            matched_formats: vec![],
        };
        let higher = DownloadDecision {
            approved: true,
            release: make_release("Show.S01E01.1080p.WEB-DL-GROUP"),
            rejections: vec![],
            custom_format_score: 0,
            matched_formats: vec![],
        };
        let ranked = rank_releases(vec![lower, higher], GrabStrategy::BestQuality);
        assert!(ranked[0].release.title.contains("1080p"));
    }

    #[test]
    fn rank_indexer_priority_preferred_indexer_first() {
        let mut low_priority_release = make_release("Show.S01E01.1080p.WEB-DL-GROUP1");
        low_priority_release.indexer_priority = 50;
        let mut high_priority_release = make_release("Show.S01E01.1080p.WEB-DL-GROUP2");
        high_priority_release.indexer_priority = 10;

        let low = DownloadDecision {
            approved: true,
            release: low_priority_release,
            rejections: vec![],
            custom_format_score: 0,
            matched_formats: vec![],
        };
        let high = DownloadDecision {
            approved: true,
            release: high_priority_release,
            rejections: vec![],
            custom_format_score: 0,
            matched_formats: vec![],
        };
        let ranked = rank_releases(vec![low, high], GrabStrategy::IndexerPriority);
        assert_eq!(ranked[0].release.indexer_priority, 10);
    }
}
