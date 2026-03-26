pub mod custom_formats;

use anyhow::Result;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;

use stackarr_core::models::{DownloadProtocol, QualityProfile, ReleaseInfo};

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
}

fn default_true() -> bool {
    true
}

impl QualityProfileService {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn list(&self) -> Result<Vec<QualityProfile>> {
        let rows =
            sqlx::query_as::<_, QualityProfile>("SELECT * FROM quality_profiles ORDER BY id")
                .fetch_all(&self.pool)
                .await?;
        Ok(rows)
    }

    pub async fn get(&self, id: i64) -> Result<QualityProfile> {
        let row =
            sqlx::query_as::<_, QualityProfile>("SELECT * FROM quality_profiles WHERE id = $1")
                .bind(id)
                .fetch_one(&self.pool)
                .await?;
        Ok(row)
    }

    pub async fn create(&self, input: CreateProfileInput) -> Result<QualityProfile> {
        let row = sqlx::query_as::<_, QualityProfile>(
            "INSERT INTO quality_profiles (name, cutoff, upgrade_allowed, min_format_score, cutoff_format_score, items)
             VALUES ($1, $2, $3, $4, $5, $6) RETURNING *",
        )
        .bind(&input.name)
        .bind(input.cutoff)
        .bind(input.upgrade_allowed)
        .bind(input.min_format_score)
        .bind(input.cutoff_format_score)
        .bind(&input.items)
        .fetch_one(&self.pool)
        .await?;
        Ok(row)
    }

    pub async fn update(&self, id: i64, input: UpdateProfileInput) -> Result<QualityProfile> {
        let existing = self.get(id).await?;
        let name = input.name.unwrap_or(existing.name);
        let cutoff = input.cutoff.unwrap_or(existing.cutoff);
        let upgrade = input.upgrade_allowed.unwrap_or(existing.upgrade_allowed);
        let min_fs = input.min_format_score.unwrap_or(existing.min_format_score);
        let cutoff_fs = input
            .cutoff_format_score
            .unwrap_or(existing.cutoff_format_score);
        let items = input.items.unwrap_or(existing.items);

        let row = sqlx::query_as::<_, QualityProfile>(
            "UPDATE quality_profiles SET name=$1, cutoff=$2, upgrade_allowed=$3, min_format_score=$4, cutoff_format_score=$5, items=$6
             WHERE id=$7 RETURNING *",
        )
        .bind(&name)
        .bind(cutoff)
        .bind(upgrade)
        .bind(min_fs)
        .bind(cutoff_fs)
        .bind(&items)
        .bind(id)
        .fetch_one(&self.pool)
        .await?;
        Ok(row)
    }

    pub async fn delete(&self, id: i64) -> Result<()> {
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
    /// Whether this release is already being downloaded.
    pub in_queue: bool,
    /// Whether this release was previously failed/blocklisted.
    pub in_blocklist: bool,
    /// Whether this release (by guid) has already been grabbed and imported.
    pub already_grabbed: bool,
}

// ── Decision engine ─────────────────────────────────────────────────────────

/// The outcome of a quality decision for a single release.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadDecision {
    pub approved: bool,
    pub release: ReleaseInfo,
    pub rejections: Vec<Rejection>,
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
fn parser_quality_to_num(q: stackarr_parser::Quality) -> i32 {
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
fn quality_name(num: i32) -> &'static str {
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
    quality: Option<i32>,
    allowed: bool,
    #[serde(default)]
    items: Vec<QualityItem>,
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

fn is_quality_allowed(quality_num: i32, profile: &QualityProfile) -> bool {
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

/// Rejects releases that are already in the download queue.
pub struct QueueConflictSpec;

impl DecisionSpecification for QueueConflictSpec {
    fn is_satisfied(&self, context: &DecisionContext) -> Option<Rejection> {
        if context.in_queue {
            Some(Rejection {
                reason: "already in download queue".to_string(),
                rejection_type: RejectionType::Temporary,
            })
        } else {
            None
        }
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

        if context.profile.min_format_score > 0 && score < context.profile.min_format_score {
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
        DownloadDecision {
            approved,
            release: context.release,
            rejections,
        }
    }
}

impl Default for DecisionEngine {
    fn default() -> Self {
        Self::new()
    }
}

// ── Release ranking ─────────────────────────────────────────────────────────

/// Sort approved decisions by preference: approved first, then higher quality,
/// more seeders, newer age, and higher priority indexer.
pub fn rank_releases(mut decisions: Vec<DownloadDecision>) -> Vec<DownloadDecision> {
    decisions.sort_by(|a, b| {
        // 1. Approved first
        b.approved
            .cmp(&a.approved)
            // 2. Higher quality first
            .then_with(|| {
                let qa = parse_quality_num(&a.release.title);
                let qb = parse_quality_num(&b.release.title);
                qb.cmp(&qa)
            })
            // 3. More seeders first (torrents)
            .then_with(|| {
                let sa = a.release.seeders.unwrap_or(0);
                let sb = b.release.seeders.unwrap_or(0);
                sb.cmp(&sa)
            })
            // 4. Smaller age first (newer)
            .then_with(|| a.release.age_days.cmp(&b.release.age_days))
            // 5. Higher priority indexer first
            .then_with(|| {
                // Lower priority number = higher priority
                // TODO: indexer priority from config
                0.cmp(&0)
            })
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
        }
    }

    fn make_context(release: ReleaseInfo, profile: QualityProfile) -> DecisionContext {
        DecisionContext {
            release,
            profile,
            existing_quality: None,
            existing_custom_format_score: None,
            release_custom_format_score: 0,
            in_queue: false,
            in_blocklist: false,
            already_grabbed: false,
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
        };
        let rejected = DownloadDecision {
            approved: false,
            release: make_release("Show.S01E01.720p.HDTV.x264-GROUP"),
            rejections: vec![Rejection {
                reason: "not allowed".to_string(),
                rejection_type: RejectionType::Permanent,
            }],
        };
        let ranked = rank_releases(vec![rejected, approved]);
        assert!(ranked[0].approved);
        assert!(!ranked[1].approved);
    }

    #[test]
    fn rank_higher_quality_first() {
        let r1080 = DownloadDecision {
            approved: true,
            release: make_release("Show.S01E01.1080p.WEB-DL.x264-GROUP"), // quality 11
            rejections: vec![],
        };
        let r720 = DownloadDecision {
            approved: true,
            release: make_release("Show.S01E01.720p.HDTV.x264-GROUP"), // quality 6
            rejections: vec![],
        };
        let ranked = rank_releases(vec![r720, r1080]);
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
        };
        let r_few = DownloadDecision {
            approved: true,
            release: make_torrent_release("Show.S01E01.1080p.WEB-DL.x264-B", 5),
            rejections: vec![],
        };
        let ranked = rank_releases(vec![r_few, r_many]);
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
        };
        let d_old = DownloadDecision {
            approved: true,
            release: r_old,
            rejections: vec![],
        };
        let ranked = rank_releases(vec![d_old, d_new]);
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
}
