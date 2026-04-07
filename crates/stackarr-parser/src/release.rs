use once_cell::sync::Lazy;
use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::episode::{EpisodeInfo, parse_episodes};
use crate::language::{Language, parse_languages};
use crate::quality::{QualityModel, parse_quality};
use crate::title::parse_title;

/// A fully parsed release name with all extracted metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsedRelease {
    pub title: String,
    pub quality: QualityModel,
    pub episode_info: EpisodeInfo,
    pub languages: Vec<Language>,
    pub release_group: Option<String>,
    pub release_hash: Option<String>,
    pub year: Option<i32>,
    pub edition: Option<String>,
    pub imdb_id: Option<String>,
}

// ── Regex patterns ──────────────────────────────────────────────────────────

// Release group: last hyphen-separated segment (excluding file extension)
static RE_RELEASE_GROUP: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"-([A-Za-z0-9]+)(?:\.[a-zA-Z]{2,4})?$").unwrap());

// Release hash: 8-char hex in square brackets
static RE_RELEASE_HASH: Lazy<Regex> = Lazy::new(|| Regex::new(r"\[([0-9a-fA-F]{8})\]").unwrap());

// Year: 4-digit year (1900-2099)
static RE_YEAR: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?:^|[\s.\-_\(])((?:19|20)\d{2})(?:[\s.\-_\)]|$)").unwrap());

// Edition: Director's Cut, Extended, Unrated, etc.
static RE_EDITION: Lazy<Regex> = Lazy::new(|| {
    Regex::new(
        r"(?i)\b(Director'?s?.?Cut|Extended.?(?:Edition|Cut)?|Unrated|Theatrical|Ultimate.?(?:Edition|Cut)|Criterion|IMAX|Remastered|Anniversary.?Edition|Collector'?s?.?Edition|Special.?Edition|Deluxe.?Edition)\b",
    )
    .unwrap()
});

// IMDB ID: tt followed by 7-8 digits
static RE_IMDB: Lazy<Regex> = Lazy::new(|| Regex::new(r"\b(tt\d{7,8})\b").unwrap());

/// Parse a release name into a fully structured [`ParsedRelease`].
pub fn parse_release(name: &str) -> ParsedRelease {
    let title = parse_title(name);
    let quality = parse_quality(name);
    let episode_info = parse_episodes(name);
    let languages = parse_languages(name);

    let release_group = RE_RELEASE_GROUP.captures(name).map(|c| c[1].to_string());

    let release_hash = RE_RELEASE_HASH.captures(name).map(|c| c[1].to_string());

    let year = RE_YEAR
        .captures(name)
        .and_then(|c| c[1].parse::<i32>().ok());

    let edition = RE_EDITION.captures(name).map(|c| c[1].to_string());

    let imdb_id = RE_IMDB.captures(name).map(|c| c[1].to_string());

    ParsedRelease {
        title,
        quality,
        episode_info,
        languages,
        release_group,
        release_hash,
        year,
        edition,
        imdb_id,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::quality::Quality;

    #[test]
    fn test_standard_tv_release() {
        let r = parse_release("The.Office.S01E01.720p.HDTV.x264-LOL");
        assert_eq!(r.title, "The Office");
        assert_eq!(r.quality.quality, Quality::HDTV720p);
        assert_eq!(r.episode_info.season_number, Some(1));
        assert_eq!(r.episode_info.episode_numbers, vec![1]);
        assert_eq!(r.release_group.as_deref(), Some("LOL"));
    }

    #[test]
    fn test_movie_with_year() {
        let r = parse_release("Inception.2010.1080p.BluRay.x264-GROUP");
        assert_eq!(r.title, "Inception");
        assert_eq!(r.year, Some(2010));
        assert_eq!(r.quality.quality, Quality::Bluray1080p);
        assert_eq!(r.release_group.as_deref(), Some("GROUP"));
    }

    #[test]
    fn test_4k_webrip() {
        let r = parse_release("Movie.Name.2024.2160p.WEBRip.DDP5.1.x265-GROUP");
        assert_eq!(r.title, "Movie Name");
        assert_eq!(r.quality.quality, Quality::WEBRip2160p);
        assert_eq!(r.year, Some(2024));
    }

    #[test]
    fn test_multi_episode() {
        let r = parse_release("Show.S02E01E02.1080p.WEB-DL.DD5.1-GROUP");
        assert_eq!(r.episode_info.season_number, Some(2));
        assert_eq!(r.episode_info.episode_numbers, vec![1, 2]);
        assert_eq!(r.quality.quality, Quality::WEBDL1080p);
    }

    #[test]
    fn test_daily_show() {
        let r = parse_release("Late.Night.Show.2024.01.15.720p.HDTV.x264-GROUP");
        assert!(r.episode_info.air_date.is_some());
        let date = r.episode_info.air_date.unwrap();
        assert_eq!(date.to_string(), "2024-01-15");
    }

    #[test]
    fn test_release_hash() {
        let r = parse_release("[SubGroup] Anime Name - 01 [1080p] [ABCD1234].mkv");
        assert_eq!(r.release_hash.as_deref(), Some("ABCD1234"));
    }

    #[test]
    fn test_edition_directors_cut() {
        let r = parse_release("Movie.Name.2024.Directors.Cut.1080p.BluRay.x264-GROUP");
        assert!(r.edition.is_some());
        assert!(r.edition.as_ref().unwrap().contains("Directors"));
    }

    #[test]
    fn test_edition_extended() {
        let r = parse_release("Movie.Name.2024.Extended.Edition.1080p.BluRay.x264-GROUP");
        assert!(r.edition.is_some());
    }

    #[test]
    fn test_edition_unrated() {
        let r = parse_release("Movie.Name.2024.Unrated.1080p.BluRay.x264-GROUP");
        assert_eq!(r.edition.as_deref(), Some("Unrated"));
    }

    #[test]
    fn test_edition_remastered() {
        let r = parse_release("Movie.Name.1994.Remastered.1080p.BluRay.x264-GROUP");
        assert_eq!(r.edition.as_deref(), Some("Remastered"));
    }

    #[test]
    fn test_imdb_id() {
        let r = parse_release("Movie.Name.2024.1080p.BluRay.x264-GROUP.tt1234567");
        assert_eq!(r.imdb_id.as_deref(), Some("tt1234567"));
    }

    #[test]
    fn test_imdb_id_8_digits() {
        let r = parse_release("Movie.Name.2024.1080p.BluRay.x264-GROUP.tt12345678");
        assert_eq!(r.imdb_id.as_deref(), Some("tt12345678"));
    }

    #[test]
    fn test_proper_release() {
        let r = parse_release("Show.S01E01.720p.HDTV.PROPER.x264-GROUP");
        assert_eq!(r.quality.revision.version, 2);
    }

    #[test]
    fn test_repack_release() {
        let r = parse_release("Show.S01E01.720p.HDTV.REPACK.x264-GROUP");
        assert_eq!(r.quality.revision.version, 2);
    }

    #[test]
    fn test_full_season() {
        let r = parse_release("Show.Name.S03.1080p.BluRay.x264-GROUP");
        assert!(r.episode_info.is_full_season);
        assert_eq!(r.episode_info.season_number, Some(3));
    }

    #[test]
    fn test_language_detection() {
        let r = parse_release("Movie.Name.2024.FRENCH.1080p.BluRay.x264-GROUP");
        assert!(r.languages.contains(&Language::French));
    }

    #[test]
    fn test_multi_language() {
        let r = parse_release("Movie.Name.2024.MULTi.1080p.BluRay.x264-GROUP");
        assert!(r.languages.contains(&Language::Multi));
    }

    #[test]
    fn test_complex_release() {
        let r = parse_release(
            "The.Lord.of.the.Rings.The.Return.of.the.King.2003.Extended.Edition.2160p.BluRay.REMUX.HEVC.DTS-HD.MA.6.1-GROUP",
        );
        assert_eq!(r.title, "The Lord of the Rings The Return of the King");
        assert_eq!(r.year, Some(2003));
        assert!(r.edition.is_some());
        assert_eq!(r.quality.quality, Quality::Remux2160p);
        assert_eq!(r.release_group.as_deref(), Some("GROUP"));
    }

    #[test]
    fn test_webdl_with_dots() {
        let r = parse_release("Show.Name.S01E01.1080p.WEB.DL.DDP5.1.H.265-GROUP");
        assert_eq!(r.quality.quality, Quality::WEBDL1080p);
    }

    #[test]
    fn test_no_group() {
        let r = parse_release("Show.Name.S01E01.720p.HDTV");
        // No hyphen means no group extracted
        assert!(r.release_group.is_none());
    }

    #[test]
    fn test_special_episode() {
        let r = parse_release("Show.Name.S00E01.Christmas.Special.720p.HDTV-GROUP");
        assert!(r.episode_info.is_special);
    }

    #[test]
    fn test_anime_absolute() {
        let r = parse_release("Anime.Name.142.720p.WEB-DL.x264-SubGroup");
        assert!(r.episode_info.absolute_episode_numbers.contains(&142));
    }

    #[test]
    fn test_dvdrip_movie() {
        let r = parse_release("Movie.Name.2005.DVDRip.x264-GROUP");
        assert_eq!(r.quality.quality, Quality::DVDRip);
        assert_eq!(r.year, Some(2005));
    }

    #[test]
    fn test_real_proper() {
        let r = parse_release("Show.S01E01.720p.HDTV.REAL.PROPER.x264-GROUP");
        assert_eq!(r.quality.revision.version, 2);
        assert_eq!(r.quality.revision.real, 1);
    }

    #[test]
    fn test_year_in_parentheses() {
        let r = parse_release("Movie.Name.(2024).1080p.BluRay.x264-GROUP");
        assert_eq!(r.year, Some(2024));
    }

    #[test]
    fn test_edition_imax() {
        let r = parse_release("Movie.Name.2024.IMAX.1080p.BluRay.x264-GROUP");
        assert_eq!(r.edition.as_deref(), Some("IMAX"));
    }

    #[test]
    fn test_edition_theatrical() {
        let r = parse_release("Movie.Name.2024.Theatrical.1080p.BluRay.x264-GROUP");
        assert_eq!(r.edition.as_deref(), Some("Theatrical"));
    }

    #[test]
    fn test_edition_criterion() {
        let r = parse_release("Movie.Name.2024.Criterion.1080p.BluRay.x264-GROUP");
        assert_eq!(r.edition.as_deref(), Some("Criterion"));
    }

    #[test]
    fn test_edition_anniversary() {
        let r = parse_release("Movie.Name.1994.Anniversary.Edition.1080p.BluRay.x264-GROUP");
        assert!(r.edition.is_some());
        assert!(r.edition.as_ref().unwrap().contains("Anniversary"));
    }

    #[test]
    fn test_release_hash_lowercase() {
        let r = parse_release("[SubGroup] Anime Name - 01 [1080p] [abcd1234].mkv");
        assert_eq!(r.release_hash.as_deref(), Some("abcd1234"));
    }

    #[test]
    fn test_no_release_hash_short() {
        // Only 4 hex chars — should NOT match 8-char requirement
        let r = parse_release("[SubGroup] Anime Name - 01 [AB12].mkv");
        assert!(r.release_hash.is_none());
    }

    #[test]
    fn test_release_group_with_numbers() {
        let r = parse_release("Show.S01E01.720p.HDTV.x264-GRP123");
        assert_eq!(r.release_group.as_deref(), Some("GRP123"));
    }

    #[test]
    fn test_release_group_from_file_ext() {
        let r = parse_release("Show.S01E01.720p.HDTV.x264-GRP.mkv");
        assert_eq!(r.release_group.as_deref(), Some("GRP"));
    }

    #[test]
    fn test_imdb_id_6_digits_not_matched() {
        let r = parse_release("Movie.Name.2024.1080p.BluRay.x264-GROUP.tt123456");
        assert!(r.imdb_id.is_none());
    }

    #[test]
    fn test_no_year_extracted() {
        let r = parse_release("Show.Name.S01E01.720p.HDTV.x264-GROUP");
        assert!(r.year.is_none());
    }

    #[test]
    fn test_anime_no_standard_episode() {
        let r = parse_release("[SubGroup] Anime Name - 42 [720p].mkv");
        assert!(r.episode_info.absolute_episode_numbers.contains(&42));
        assert!(r.episode_info.episode_numbers.is_empty());
    }

    #[test]
    fn test_old_movie_year() {
        let r = parse_release("Classic.Movie.1954.720p.BluRay.x264-GROUP");
        assert_eq!(r.year, Some(1954));
        assert_eq!(r.title, "Classic Movie");
    }

    #[test]
    fn test_special_edition() {
        let r = parse_release("Movie.Name.2024.Special.Edition.1080p.BluRay-GROUP");
        assert!(r.edition.is_some());
        assert!(r.edition.as_ref().unwrap().contains("Special"));
    }

    #[test]
    fn test_deluxe_edition() {
        let r = parse_release("Movie.Name.2024.Deluxe.Edition.1080p.BluRay-GROUP");
        assert!(r.edition.is_some());
    }
}
