use anyhow::Result;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;

use stackarr_core::models::{QualityProfile, ReleaseInfo};

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
    fn is_satisfied(&self, release: &ReleaseInfo, profile: &QualityProfile) -> Option<Rejection>;
}

// ── Stub specifications ─────────────────────────────────────────────────────

/// Rejects releases whose quality is not allowed in the profile.
pub struct QualityAllowedSpec;

impl DecisionSpecification for QualityAllowedSpec {
    fn is_satisfied(&self, _release: &ReleaseInfo, _profile: &QualityProfile) -> Option<Rejection> {
        // TODO: parse quality from release title and check profile items
        None
    }
}

/// Rejects releases below minimum size thresholds.
pub struct MinimumSizeSpec;

impl DecisionSpecification for MinimumSizeSpec {
    fn is_satisfied(&self, _release: &ReleaseInfo, _profile: &QualityProfile) -> Option<Rejection> {
        // TODO: implement minimum size check
        None
    }
}

/// Rejects releases above maximum size thresholds.
pub struct MaximumSizeSpec;

impl DecisionSpecification for MaximumSizeSpec {
    fn is_satisfied(&self, _release: &ReleaseInfo, _profile: &QualityProfile) -> Option<Rejection> {
        // TODO: implement maximum size check
        None
    }
}

/// The decision engine evaluates releases against quality profiles.
pub struct DecisionEngine {
    specs: Vec<Box<dyn DecisionSpecification>>,
}

impl DecisionEngine {
    /// Create a new engine with the default set of specifications.
    pub fn new() -> Self {
        let specs: Vec<Box<dyn DecisionSpecification>> = vec![
            Box::new(QualityAllowedSpec),
            Box::new(MinimumSizeSpec),
            Box::new(MaximumSizeSpec),
        ];
        Self { specs }
    }

    /// Decide whether a release should be grabbed.
    pub fn decide(
        &self,
        release: ReleaseInfo,
        profile: &QualityProfile,
    ) -> DownloadDecision {
        let mut rejections = Vec::new();
        for spec in &self.specs {
            if let Some(r) = spec.is_satisfied(&release, profile) {
                rejections.push(r);
            }
        }
        let approved = rejections.is_empty();
        DownloadDecision {
            approved,
            release,
            rejections,
        }
    }
}

impl Default for DecisionEngine {
    fn default() -> Self {
        Self::new()
    }
}
