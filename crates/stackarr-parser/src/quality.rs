use once_cell::sync::Lazy;
use regex::Regex;
use serde::{Deserialize, Serialize};

/// Video quality level detected from a release name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Quality {
    Unknown,
    SDTV,
    DVD,
    WEBDL480p,
    HDTV720p,
    HDTV1080p,
    Raw,
    WEBDL720p,
    Bluray720p,
    WEBDL1080p,
    Bluray1080p,
    HDTV2160p,
    WEBDL2160p,
    Bluray2160p,
    DVDRip,
    WEBRip480p,
    WEBRip720p,
    WEBRip1080p,
    WEBRip2160p,
    Remux1080p,
    Remux2160p,
}

/// Revision information for a release (PROPER, REPACK, etc.).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Revision {
    pub version: i32,
    pub real: i32,
}

impl Default for Revision {
    fn default() -> Self {
        Self {
            version: 1,
            real: 0,
        }
    }
}

/// Complete quality model combining the quality level and revision.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QualityModel {
    pub quality: Quality,
    pub revision: Revision,
}

// ── Regex patterns ──────────────────────────────────────────────────────────

static RE_RESOLUTION: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?i)\b(2160|1080|720|480)[pi]\b").unwrap());

static RE_SOURCE_REMUX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?i)\b[Rr][Ee][Mm][Uu][Xx]\b").unwrap());

static RE_SOURCE_BLURAY: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?i)\b(BluRay|BDRip|BRRip|BD[Rr]ip)\b").unwrap());

static RE_SOURCE_WEBDL: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?i)\bWEB[.\-_ ]?DL\b|\bWEB\b").unwrap());

static RE_SOURCE_WEBRIP: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?i)\bWEB[Rr]ip\b").unwrap());

static RE_SOURCE_HDTV: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?i)\b(HDTV|PDTV|DSR)\b").unwrap());

static RE_SOURCE_DVDRIP: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?i)\bDVD[Rr]ip\b").unwrap());

static RE_SOURCE_DVD: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?i)\bDVD(?:R|9|5)?\b").unwrap());

static RE_SOURCE_RAW: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?i)\b[Rr][Aa][Ww]\b").unwrap());

static RE_PROPER: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?i)\bPROPER\b").unwrap());

static RE_REPACK: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?i)\bREPACK\b").unwrap());

static RE_REAL: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?i)\bREAL\b").unwrap());

/// Parse a release name into a [`QualityModel`].
pub fn parse_quality(name: &str) -> QualityModel {
    let revision = parse_revision(name);

    // Determine resolution
    let resolution: Option<u32> = RE_RESOLUTION
        .captures(name)
        .and_then(|c| c.get(1))
        .and_then(|m| m.as_str().parse().ok());

    // Determine source
    let is_remux = RE_SOURCE_REMUX.is_match(name);
    let is_bluray = RE_SOURCE_BLURAY.is_match(name);
    let is_webdl = RE_SOURCE_WEBDL.is_match(name);
    let is_webrip = RE_SOURCE_WEBRIP.is_match(name);
    let is_hdtv = RE_SOURCE_HDTV.is_match(name);
    let is_dvdrip = RE_SOURCE_DVDRIP.is_match(name);
    let is_dvd = RE_SOURCE_DVD.is_match(name) && !is_dvdrip;
    let is_raw = RE_SOURCE_RAW.is_match(name);

    let quality = match (
        resolution, is_remux, is_bluray, is_webdl, is_webrip, is_hdtv, is_dvdrip, is_dvd, is_raw,
    ) {
        // Raw
        (_, _, _, _, _, _, _, _, true) => Quality::Raw,

        // Remux
        (Some(2160), true, _, _, _, _, _, _, _) | (None, true, _, _, _, _, _, _, _)
            if resolution.unwrap_or(2160) >= 2160 =>
        {
            Quality::Remux2160p
        }
        (Some(1080), true, _, _, _, _, _, _, _) => Quality::Remux1080p,
        (_, true, _, _, _, _, _, _, _) => Quality::Remux1080p,

        // BluRay (non-remux)
        (Some(2160), _, true, _, _, _, _, _, _) => Quality::Bluray2160p,
        (Some(1080), _, true, _, _, _, _, _, _) => Quality::Bluray1080p,
        (Some(720), _, true, _, _, _, _, _, _) => Quality::Bluray720p,
        (_, _, true, _, _, _, _, _, _) => Quality::Bluray1080p,

        // WEB-DL
        (Some(2160), _, _, true, _, _, _, _, _) => Quality::WEBDL2160p,
        (Some(1080), _, _, true, _, _, _, _, _) => Quality::WEBDL1080p,
        (Some(720), _, _, true, _, _, _, _, _) => Quality::WEBDL720p,
        (Some(480), _, _, true, _, _, _, _, _) => Quality::WEBDL480p,
        (_, _, _, true, _, _, _, _, _) => Quality::WEBDL720p,

        // WEBRip
        (Some(2160), _, _, _, true, _, _, _, _) => Quality::WEBRip2160p,
        (Some(1080), _, _, _, true, _, _, _, _) => Quality::WEBRip1080p,
        (Some(720), _, _, _, true, _, _, _, _) => Quality::WEBRip720p,
        (Some(480), _, _, _, true, _, _, _, _) => Quality::WEBRip480p,
        (_, _, _, _, true, _, _, _, _) => Quality::WEBRip720p,

        // HDTV
        (Some(2160), _, _, _, _, true, _, _, _) => Quality::HDTV2160p,
        (Some(1080), _, _, _, _, true, _, _, _) => Quality::HDTV1080p,
        (Some(720), _, _, _, _, true, _, _, _) => Quality::HDTV720p,
        (_, _, _, _, _, true, _, _, _) => Quality::SDTV,

        // DVDRip
        (_, _, _, _, _, _, true, _, _) => Quality::DVDRip,

        // DVD
        (_, _, _, _, _, _, _, true, _) => Quality::DVD,

        // Resolution only (no source)
        (Some(2160), _, _, _, _, _, _, _, _) => Quality::HDTV2160p,
        (Some(1080), _, _, _, _, _, _, _, _) => Quality::HDTV1080p,
        (Some(720), _, _, _, _, _, _, _, _) => Quality::HDTV720p,
        (Some(480), _, _, _, _, _, _, _, _) => Quality::SDTV,

        _ => Quality::Unknown,
    };

    QualityModel { quality, revision }
}

fn parse_revision(name: &str) -> Revision {
    let mut version = 1i32;
    let mut real = 0i32;

    if RE_PROPER.is_match(name) {
        version = 2;
    }
    if RE_REPACK.is_match(name) {
        version = 2;
    }
    if RE_REAL.is_match(name) {
        real = 1;
    }

    Revision { version, real }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hdtv_720p() {
        let q = parse_quality("Show.Name.S01E01.720p.HDTV.x264-GROUP");
        assert_eq!(q.quality, Quality::HDTV720p);
        assert_eq!(q.revision.version, 1);
    }

    #[test]
    fn test_hdtv_1080p() {
        let q = parse_quality("Show.Name.S01E01.1080p.HDTV.x264-GROUP");
        assert_eq!(q.quality, Quality::HDTV1080p);
    }

    #[test]
    fn test_webdl_1080p() {
        let q = parse_quality("Show.Name.S01E01.1080p.WEB-DL.DD5.1.H264-GROUP");
        assert_eq!(q.quality, Quality::WEBDL1080p);
    }

    #[test]
    fn test_webdl_2160p() {
        let q = parse_quality("Show.Name.S01E01.2160p.WEB.DL.DDP5.1.H265-GROUP");
        assert_eq!(q.quality, Quality::WEBDL2160p);
    }

    #[test]
    fn test_webrip_1080p() {
        let q = parse_quality("Show.Name.S01E01.1080p.WEBRip.x264-GROUP");
        assert_eq!(q.quality, Quality::WEBRip1080p);
    }

    #[test]
    fn test_bluray_1080p() {
        let q = parse_quality("Show.Name.S01E01.1080p.BluRay.x264-GROUP");
        assert_eq!(q.quality, Quality::Bluray1080p);
    }

    #[test]
    fn test_bluray_2160p() {
        let q = parse_quality("Movie.Name.2024.2160p.BluRay.REMUX.HEVC.DTS-HD-GROUP");
        assert_eq!(q.quality, Quality::Remux2160p);
    }

    #[test]
    fn test_remux_1080p() {
        let q = parse_quality("Movie.Name.2024.1080p.Remux.AVC.DTS-HD-GROUP");
        assert_eq!(q.quality, Quality::Remux1080p);
    }

    #[test]
    fn test_dvdrip() {
        let q = parse_quality("Show.Name.S01E01.DVDRip.x264-GROUP");
        assert_eq!(q.quality, Quality::DVDRip);
    }

    #[test]
    fn test_sdtv_hdtv_no_resolution() {
        let q = parse_quality("Show.Name.S01E01.HDTV.x264-GROUP");
        assert_eq!(q.quality, Quality::SDTV);
    }

    #[test]
    fn test_proper() {
        let q = parse_quality("Show.Name.S01E01.720p.HDTV.PROPER.x264-GROUP");
        assert_eq!(q.quality, Quality::HDTV720p);
        assert_eq!(q.revision.version, 2);
    }

    #[test]
    fn test_repack() {
        let q = parse_quality("Show.Name.S01E01.720p.HDTV.REPACK.x264-GROUP");
        assert_eq!(q.quality, Quality::HDTV720p);
        assert_eq!(q.revision.version, 2);
    }

    #[test]
    fn test_real() {
        let q = parse_quality("Show.Name.S01E01.720p.HDTV.REAL.PROPER.x264-GROUP");
        assert_eq!(q.quality, Quality::HDTV720p);
        assert_eq!(q.revision.version, 2);
        assert_eq!(q.revision.real, 1);
    }

    #[test]
    fn test_unknown() {
        let q = parse_quality("something_random");
        assert_eq!(q.quality, Quality::Unknown);
    }

    #[test]
    fn test_bdrip() {
        let q = parse_quality("Movie.2020.1080p.BDRip.x264-GROUP");
        assert_eq!(q.quality, Quality::Bluray1080p);
    }

    #[test]
    fn test_brrip() {
        let q = parse_quality("Movie.2020.720p.BRRip.x264-GROUP");
        assert_eq!(q.quality, Quality::Bluray720p);
    }

    #[test]
    fn test_webrip_480p() {
        let q = parse_quality("Show.S01E01.480p.WEBRip.x264-GROUP");
        assert_eq!(q.quality, Quality::WEBRip480p);
    }

    #[test]
    fn test_webdl_480p() {
        let q = parse_quality("Show.S01E01.480p.WEB-DL.x264-GROUP");
        assert_eq!(q.quality, Quality::WEBDL480p);
    }

    #[test]
    fn test_raw() {
        let q = parse_quality("Show.Name.S01E01.Raw.TS");
        assert_eq!(q.quality, Quality::Raw);
    }

    #[test]
    fn test_dvd5() {
        let q = parse_quality("Movie.2020.DVD5.x264-GROUP");
        assert_eq!(q.quality, Quality::DVD);
    }

    #[test]
    fn test_dvd9() {
        let q = parse_quality("Movie.2020.DVD9.x264-GROUP");
        assert_eq!(q.quality, Quality::DVD);
    }

    #[test]
    fn test_dvdr() {
        let q = parse_quality("Movie.2020.DVDR.x264-GROUP");
        assert_eq!(q.quality, Quality::DVD);
    }

    #[test]
    fn test_webdl_underscore() {
        let q = parse_quality("Show.S01E01.1080p.WEB_DL.x264-GROUP");
        assert_eq!(q.quality, Quality::WEBDL1080p);
    }

    #[test]
    fn test_webdl_no_separator() {
        let q = parse_quality("Show.S01E01.1080p.WEBDL.x264-GROUP");
        assert_eq!(q.quality, Quality::WEBDL1080p);
    }

    #[test]
    fn test_pdtv() {
        let q = parse_quality("Show.Name.S01E01.PDTV.x264-GROUP");
        assert_eq!(q.quality, Quality::SDTV);
    }

    #[test]
    fn test_dsr() {
        let q = parse_quality("Show.Name.S01E01.DSR.x264-GROUP");
        assert_eq!(q.quality, Quality::SDTV);
    }

    #[test]
    fn test_bluray_720p_bdrip() {
        let q = parse_quality("Movie.2020.720p.BDRip.x264-GROUP");
        assert_eq!(q.quality, Quality::Bluray720p);
    }

    #[test]
    fn test_hdtv_2160p() {
        let q = parse_quality("Show.S01E01.2160p.HDTV.H265-GROUP");
        assert_eq!(q.quality, Quality::HDTV2160p);
    }

    #[test]
    fn test_webrip_2160p() {
        let q = parse_quality("Movie.2024.2160p.WEBRip.DDP5.1.x265-GROUP");
        assert_eq!(q.quality, Quality::WEBRip2160p);
    }

    #[test]
    fn test_bare_web_2160p_is_webdl() {
        // Bare "WEB" (no -DL or Rip suffix) should be treated as WEBDL
        let q = parse_quality("Daredevil.Born.Again.S02E04.DV.2160p.WEB.h265-ETHEL");
        assert_eq!(q.quality, Quality::WEBDL2160p);
    }

    #[test]
    fn test_bare_web_1080p_is_webdl() {
        let q = parse_quality("Show.S01E01.1080p.WEB.h264-GROUP");
        assert_eq!(q.quality, Quality::WEBDL1080p);
    }

    #[test]
    fn test_webdl_720p() {
        let q = parse_quality("Show.S01E01.720p.WEB-DL.DD5.1-GROUP");
        assert_eq!(q.quality, Quality::WEBDL720p);
    }

    #[test]
    fn test_webrip_720p() {
        let q = parse_quality("Show.S01E01.720p.WEBRip.x264-GROUP");
        assert_eq!(q.quality, Quality::WEBRip720p);
    }

    #[test]
    fn test_bluray_2160p_no_remux() {
        let q = parse_quality("Movie.2024.2160p.BluRay.HEVC.DTS-HD-GROUP");
        assert_eq!(q.quality, Quality::Bluray2160p);
    }

    #[test]
    fn test_remux_defaults_to_2160p_without_resolution() {
        // When no resolution is specified, Remux defaults to 2160p
        let q = parse_quality("Movie.2024.Remux.AVC.DTS-HD-GROUP");
        assert_eq!(q.quality, Quality::Remux2160p);
    }

    #[test]
    fn test_resolution_only_2160p() {
        let q = parse_quality("Show.S01E01.2160p.x265-GROUP");
        assert_eq!(q.quality, Quality::HDTV2160p);
    }

    #[test]
    fn test_resolution_only_480p() {
        let q = parse_quality("Show.S01E01.480p.x264-GROUP");
        assert_eq!(q.quality, Quality::SDTV);
    }

    #[test]
    fn test_proper_and_repack() {
        let q = parse_quality("Show.S01E01.720p.HDTV.PROPER.REPACK.x264-GROUP");
        assert_eq!(q.quality, Quality::HDTV720p);
        assert_eq!(q.revision.version, 2);
    }

    #[test]
    fn test_real_without_proper() {
        let q = parse_quality("Show.S01E01.720p.HDTV.REAL.x264-GROUP");
        assert_eq!(q.revision.real, 1);
        assert_eq!(q.revision.version, 1);
    }

    #[test]
    fn test_bluray_no_resolution_defaults_1080p() {
        let q = parse_quality("Movie.2020.BluRay.x264-GROUP");
        assert_eq!(q.quality, Quality::Bluray1080p);
    }

    #[test]
    fn test_webdl_no_resolution_defaults_720p() {
        let q = parse_quality("Show.S01E01.WEB-DL.DD5.1-GROUP");
        assert_eq!(q.quality, Quality::WEBDL720p);
    }

    #[test]
    fn test_webrip_no_resolution_defaults_720p() {
        let q = parse_quality("Show.S01E01.WEBRip.x264-GROUP");
        assert_eq!(q.quality, Quality::WEBRip720p);
    }
}
