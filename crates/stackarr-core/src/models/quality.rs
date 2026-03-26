use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Quality {
    Unknown = 0,
    SDTV = 1,
    DVD = 2,
    WEBDL480p = 3,
    WEBRip480p = 4,
    Bluray480p = 5,
    HDTV720p = 6,
    WEBDL720p = 7,
    WEBRip720p = 8,
    Bluray720p = 9,
    HDTV1080p = 10,
    WEBDL1080p = 11,
    WEBRip1080p = 12,
    Bluray1080p = 13,
    Remux1080p = 14,
    HDTV2160p = 15,
    WEBDL2160p = 16,
    WEBRip2160p = 17,
    Bluray2160p = 18,
    Remux2160p = 19,
    Raw = 20,
}

impl Quality {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Unknown => "Unknown",
            Self::SDTV => "SDTV",
            Self::DVD => "DVD",
            Self::WEBDL480p => "WEBDL-480p",
            Self::WEBRip480p => "WEBRip-480p",
            Self::Bluray480p => "Bluray-480p",
            Self::HDTV720p => "HDTV-720p",
            Self::WEBDL720p => "WEBDL-720p",
            Self::WEBRip720p => "WEBRip-720p",
            Self::Bluray720p => "Bluray-720p",
            Self::HDTV1080p => "HDTV-1080p",
            Self::WEBDL1080p => "WEBDL-1080p",
            Self::WEBRip1080p => "WEBRip-1080p",
            Self::Bluray1080p => "Bluray-1080p",
            Self::Remux1080p => "Remux-1080p",
            Self::HDTV2160p => "HDTV-2160p",
            Self::WEBDL2160p => "WEBDL-2160p",
            Self::WEBRip2160p => "WEBRip-2160p",
            Self::Bluray2160p => "Bluray-2160p",
            Self::Remux2160p => "Remux-2160p",
            Self::Raw => "Raw-HD",
        }
    }

    pub fn resolution(&self) -> Option<u16> {
        match self {
            Self::Unknown | Self::Raw => None,
            Self::SDTV | Self::DVD => Some(480),
            Self::WEBDL480p | Self::WEBRip480p | Self::Bluray480p => Some(480),
            Self::HDTV720p | Self::WEBDL720p | Self::WEBRip720p | Self::Bluray720p => Some(720),
            Self::HDTV1080p | Self::WEBDL1080p | Self::WEBRip1080p | Self::Bluray1080p | Self::Remux1080p => Some(1080),
            Self::HDTV2160p | Self::WEBDL2160p | Self::WEBRip2160p | Self::Bluray2160p | Self::Remux2160p => Some(2160),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QualityModel {
    pub quality: Quality,
    pub revision: Revision,
}

impl QualityModel {
    pub fn new(quality: Quality) -> Self {
        Self {
            quality,
            revision: Revision::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Revision {
    pub version: i32,
    pub real: i32,
    pub is_repack: bool,
}

impl Default for Revision {
    fn default() -> Self {
        Self {
            version: 1,
            real: 0,
            is_repack: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct QualityProfile {
    pub id: i32,
    pub name: String,
    pub cutoff: i32,
    pub upgrade_allowed: bool,
    pub min_format_score: i32,
    pub cutoff_format_score: i32,
    pub items: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct CustomFormat {
    pub id: i32,
    pub name: String,
    pub specifications: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QualityProfileItem {
    pub quality: Option<Quality>,
    pub allowed: bool,
    #[serde(default)]
    pub items: Vec<QualityProfileItem>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Language {
    English,
    French,
    Spanish,
    German,
    Italian,
    Portuguese,
    Japanese,
    Korean,
    Chinese,
    Russian,
    Arabic,
    Hindi,
    Dutch,
    Swedish,
    Norwegian,
    Danish,
    Finnish,
    Polish,
    Czech,
    Hungarian,
    Romanian,
    Turkish,
    Thai,
    Vietnamese,
    Indonesian,
    Unknown,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_quality_name_mapping() {
        // Every variant returns a non-empty name.
        let variants = [
            Quality::Unknown, Quality::SDTV, Quality::DVD, Quality::WEBDL480p,
            Quality::WEBRip480p, Quality::Bluray480p, Quality::HDTV720p,
            Quality::WEBDL720p, Quality::WEBRip720p, Quality::Bluray720p,
            Quality::HDTV1080p, Quality::WEBDL1080p, Quality::WEBRip1080p,
            Quality::Bluray1080p, Quality::Remux1080p, Quality::HDTV2160p,
            Quality::WEBDL2160p, Quality::WEBRip2160p, Quality::Bluray2160p,
            Quality::Remux2160p, Quality::Raw,
        ];
        for q in variants {
            assert!(!q.name().is_empty(), "{q:?} returned empty name");
        }
    }

    #[test]
    fn test_quality_resolution() {
        assert_eq!(Quality::Unknown.resolution(), None);
        assert_eq!(Quality::Raw.resolution(), None);
        assert_eq!(Quality::SDTV.resolution(), Some(480));
        assert_eq!(Quality::DVD.resolution(), Some(480));
        assert_eq!(Quality::HDTV720p.resolution(), Some(720));
        assert_eq!(Quality::WEBDL1080p.resolution(), Some(1080));
        assert_eq!(Quality::Bluray2160p.resolution(), Some(2160));
        assert_eq!(Quality::Remux2160p.resolution(), Some(2160));
    }

    #[test]
    fn test_quality_ordering() {
        assert!(Quality::Unknown < Quality::SDTV);
        assert!(Quality::SDTV < Quality::DVD);
        assert!(Quality::HDTV720p < Quality::HDTV1080p);
        assert!(Quality::HDTV1080p < Quality::Bluray1080p);
        assert!(Quality::Bluray1080p < Quality::Remux1080p);
        assert!(Quality::Remux1080p < Quality::Bluray2160p);
        assert!(Quality::Bluray2160p < Quality::Remux2160p);
    }

    #[test]
    fn test_quality_model_default_revision() {
        let model = QualityModel::new(Quality::Bluray1080p);
        assert_eq!(model.quality, Quality::Bluray1080p);
        assert_eq!(model.revision.version, 1);
        assert_eq!(model.revision.real, 0);
        assert!(!model.revision.is_repack);
    }

    #[test]
    fn test_quality_model_serde_roundtrip() {
        let model = QualityModel::new(Quality::WEBDL2160p);
        let json = serde_json::to_string(&model).expect("serialize");
        let parsed: QualityModel = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed.quality, Quality::WEBDL2160p);
        assert_eq!(parsed.revision.version, 1);
    }
}
