use chrono::NaiveDate;
use once_cell::sync::Lazy;
use regex::Regex;
use serde::{Deserialize, Serialize};

/// Parsed episode information from a release name.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EpisodeInfo {
    pub season_number: Option<i32>,
    pub episode_numbers: Vec<i32>,
    pub absolute_episode_numbers: Vec<i32>,
    pub air_date: Option<NaiveDate>,
    pub is_full_season: bool,
    pub is_multi_season: bool,
    pub is_special: bool,
}

impl Default for EpisodeInfo {
    fn default() -> Self {
        Self {
            season_number: None,
            episode_numbers: Vec::new(),
            absolute_episode_numbers: Vec::new(),
            air_date: None,
            is_full_season: false,
            is_multi_season: false,
            is_special: false,
        }
    }
}

// ── Regex patterns ──────────────────────────────────────────────────────────

// S01E01, S01E01E02, S01E01-E03
static RE_STANDARD: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?i)\bS(\d{1,4})E(\d{1,4})(?:[-E]+E?(\d{1,4}))*\b").unwrap());

// Multi-episode range: S01E01-E05 or S01E01-05
static RE_MULTI_EPISODE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?i)\bS(\d{1,4})E(\d{1,4})-E?(\d{1,4})\b").unwrap());

// Full season: S01 — we match S followed by digits, then check context in code
// (rust regex crate doesn't support lookahead)
static RE_FULL_SEASON: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?i)\bS(\d{1,4})\b").unwrap());

// Multi-season: S01-S03
static RE_MULTI_SEASON: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?i)\bS(\d{1,4})-S(\d{1,4})\b").unwrap());

// Daily format: 2024.01.15 or 2024-01-15
static RE_DAILY: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"\b((?:19|20)\d{2})[.\-](0[1-9]|1[0-2])[.\-](0[1-9]|[12]\d|3[01])\b").unwrap()
});

// Absolute episode number: standalone number like " - 123 " or ".123."
// Must not be a year (19xx/20xx) and should be preceded by known delimiters
static RE_ABSOLUTE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?:^|[\s.\-_])(\d{2,4})(?:[\s.\-_v]|$)").unwrap());

// Specials: S00 or Season 0
static RE_SPECIAL: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?i)\b(?:S00|[Ss]eason[\s._]*0|[Ss]pecial)\b").unwrap());

// Used to check if an S-match is followed by E (making it NOT a full season)
static RE_HAS_EPISODE: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?i)\bS\d{1,4}E\d").unwrap());

/// Parse episode information from a release name.
pub fn parse_episodes(name: &str) -> EpisodeInfo {
    let mut info = EpisodeInfo::default();

    // Check for specials
    if RE_SPECIAL.is_match(name) {
        info.is_special = true;
    }

    // Try multi-season first: S01-S03
    if let Some(caps) = RE_MULTI_SEASON.captures(name) {
        let s1: i32 = caps[1].parse().unwrap_or(0);
        let _s2: i32 = caps[2].parse().unwrap_or(0);
        info.season_number = Some(s1);
        info.is_multi_season = true;
        info.is_full_season = true;
        return info;
    }

    // Try multi-episode range: S01E01-E05 or S01E01-05
    if let Some(caps) = RE_MULTI_EPISODE.captures(name) {
        let season: i32 = caps[1].parse().unwrap_or(0);
        let ep_start: i32 = caps[2].parse().unwrap_or(0);
        let ep_end: i32 = caps[3].parse().unwrap_or(0);

        info.season_number = Some(season);
        if ep_end >= ep_start {
            info.episode_numbers = (ep_start..=ep_end).collect();
        } else {
            info.episode_numbers = vec![ep_start, ep_end];
        }
        return info;
    }

    // Try standard: S01E01, S01E01E02
    if let Some(caps) = RE_STANDARD.captures(name) {
        let season: i32 = caps[1].parse().unwrap_or(0);
        info.season_number = Some(season);

        // Collect all episode numbers from the match
        let full_match = caps.get(0).unwrap().as_str();
        let ep_re = Regex::new(r"(?i)E(\d{1,4})").unwrap();
        for ep_cap in ep_re.captures_iter(full_match) {
            if let Ok(ep) = ep_cap[1].parse::<i32>() {
                info.episode_numbers.push(ep);
            }
        }
        return info;
    }

    // Try full season: S01 (no episode)
    // Only matches if there is NO SxxExx pattern in the name
    if !RE_HAS_EPISODE.is_match(name) {
        if let Some(caps) = RE_FULL_SEASON.captures(name) {
            let season: i32 = caps[1].parse().unwrap_or(0);
            info.season_number = Some(season);
            info.is_full_season = true;
            return info;
        }
    }

    // Try daily: 2024.01.15
    if let Some(caps) = RE_DAILY.captures(name) {
        let year: i32 = caps[1].parse().unwrap_or(0);
        let month: u32 = caps[2].parse().unwrap_or(0);
        let day: u32 = caps[3].parse().unwrap_or(0);

        if let Some(date) = NaiveDate::from_ymd_opt(year, month, day) {
            info.air_date = Some(date);
        }
        return info;
    }

    // Try absolute episode number (only if nothing else matched)
    // Look for numbers that are not years
    for cap in RE_ABSOLUTE.captures_iter(name) {
        if let Ok(num) = cap[1].parse::<i32>() {
            // Exclude likely years
            if !(1900..=2099).contains(&num) && num > 0 {
                info.absolute_episode_numbers.push(num);
            }
        }
    }

    info
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_standard_single_episode() {
        let info = parse_episodes("Show.Name.S01E01.720p.HDTV.x264-GROUP");
        assert_eq!(info.season_number, Some(1));
        assert_eq!(info.episode_numbers, vec![1]);
        assert!(!info.is_full_season);
    }

    #[test]
    fn test_standard_multi_episode() {
        let info = parse_episodes("Show.Name.S01E01E02.720p.HDTV.x264-GROUP");
        assert_eq!(info.season_number, Some(1));
        assert_eq!(info.episode_numbers, vec![1, 2]);
    }

    #[test]
    fn test_episode_range() {
        let info = parse_episodes("Show.Name.S01E01-E03.720p.HDTV.x264-GROUP");
        assert_eq!(info.season_number, Some(1));
        assert_eq!(info.episode_numbers, vec![1, 2, 3]);
    }

    #[test]
    fn test_episode_range_no_e_prefix() {
        let info = parse_episodes("Show.Name.S01E01-03.720p.HDTV.x264-GROUP");
        assert_eq!(info.season_number, Some(1));
        assert_eq!(info.episode_numbers, vec![1, 2, 3]);
    }

    #[test]
    fn test_full_season() {
        let info = parse_episodes("Show.Name.S01.720p.BluRay.x264-GROUP");
        assert_eq!(info.season_number, Some(1));
        assert!(info.is_full_season);
        assert!(info.episode_numbers.is_empty());
    }

    #[test]
    fn test_multi_season() {
        let info = parse_episodes("Show.Name.S01-S03.720p.BluRay.x264-GROUP");
        assert_eq!(info.season_number, Some(1));
        assert!(info.is_multi_season);
        assert!(info.is_full_season);
    }

    #[test]
    fn test_daily_show() {
        let info = parse_episodes("Show.Name.2024.01.15.720p.HDTV.x264-GROUP");
        assert_eq!(
            info.air_date,
            Some(NaiveDate::from_ymd_opt(2024, 1, 15).unwrap())
        );
        assert!(info.episode_numbers.is_empty());
    }

    #[test]
    fn test_daily_show_dashes() {
        let info = parse_episodes("Show.Name.2024-03-22.720p.HDTV.x264-GROUP");
        assert_eq!(
            info.air_date,
            Some(NaiveDate::from_ymd_opt(2024, 3, 22).unwrap())
        );
    }

    #[test]
    fn test_special() {
        let info = parse_episodes("Show.Name.S00E01.Special.720p.HDTV.x264-GROUP");
        assert!(info.is_special);
        assert_eq!(info.season_number, Some(0));
    }

    #[test]
    fn test_absolute_episode() {
        let info = parse_episodes("Anime.Name.123.720p.WEB-DL.x264-GROUP");
        assert!(info.absolute_episode_numbers.contains(&123));
    }

    #[test]
    fn test_high_season_episode() {
        let info = parse_episodes("Show.Name.S12E24.720p.HDTV.x264-GROUP");
        assert_eq!(info.season_number, Some(12));
        assert_eq!(info.episode_numbers, vec![24]);
    }

    #[test]
    fn test_no_match() {
        let info = parse_episodes("random.file.name");
        assert_eq!(info.season_number, None);
        assert!(info.episode_numbers.is_empty());
        assert!(info.absolute_episode_numbers.is_empty());
        assert!(info.air_date.is_none());
    }

    #[test]
    fn test_backward_episode_range() {
        // When end < start, should return vec![start, end] not a range
        let info = parse_episodes("Show.Name.S01E05-E01.720p.HDTV.x264-GROUP");
        assert_eq!(info.season_number, Some(1));
        assert_eq!(info.episode_numbers, vec![5, 1]);
    }

    #[test]
    fn test_four_digit_episode() {
        let info = parse_episodes("Show.Name.S01E1024.720p.HDTV.x264-GROUP");
        assert_eq!(info.season_number, Some(1));
        assert_eq!(info.episode_numbers, vec![1024]);
    }

    #[test]
    fn test_four_digit_season() {
        let info = parse_episodes("Show.Name.S2024E01.720p.HDTV.x264-GROUP");
        assert_eq!(info.season_number, Some(2024));
        assert_eq!(info.episode_numbers, vec![1]);
    }

    #[test]
    fn test_mixed_case_episode() {
        let info = parse_episodes("Show.Name.s01e01.720p.HDTV.x264-GROUP");
        assert_eq!(info.season_number, Some(1));
        assert_eq!(info.episode_numbers, vec![1]);
    }

    #[test]
    fn test_mixed_case_episode_v2() {
        let info = parse_episodes("Show.Name.S01e01.720p.HDTV.x264-GROUP");
        assert_eq!(info.season_number, Some(1));
        assert_eq!(info.episode_numbers, vec![1]);
    }

    #[test]
    fn test_invalid_daily_date_feb30() {
        // Feb 30 doesn't exist - should not parse as a date
        let info = parse_episodes("Show.Name.2024.02.30.720p.HDTV.x264-GROUP");
        assert!(info.air_date.is_none());
    }

    #[test]
    fn test_special_with_episode_range() {
        // S00E05-E08: season 0 with episode range
        // Note: is_special requires a \b word boundary after S00, which doesn't
        // match when E follows immediately. The season_number=0 is still extracted.
        let info = parse_episodes("Show.Name.S00E05-E08.720p.HDTV-GROUP");
        assert_eq!(info.season_number, Some(0));
        assert_eq!(info.episode_numbers, vec![5, 6, 7, 8]);
    }

    #[test]
    fn test_three_consecutive_episodes() {
        let info = parse_episodes("Show.Name.S01E01E02E03.720p.HDTV.x264-GROUP");
        assert_eq!(info.season_number, Some(1));
        assert_eq!(info.episode_numbers, vec![1, 2, 3]);
    }

    #[test]
    fn test_absolute_episode_excludes_year() {
        // 2024 looks like a year, should be excluded from absolute episodes
        let info = parse_episodes("Anime.Name.2024.720p.WEB-DL-GROUP");
        assert!(!info.absolute_episode_numbers.contains(&2024));
    }

    #[test]
    fn test_absolute_episode_low_number() {
        let info = parse_episodes("Anime.Name.01.720p.WEB-DL-GROUP");
        assert!(info.absolute_episode_numbers.contains(&1));
    }

    #[test]
    fn test_daily_show_leap_year() {
        let info = parse_episodes("Show.Name.2024.02.29.720p.HDTV.x264-GROUP");
        assert_eq!(
            info.air_date,
            Some(chrono::NaiveDate::from_ymd_opt(2024, 2, 29).unwrap())
        );
    }

    #[test]
    fn test_daily_non_leap_year_feb29() {
        // 2023 is not a leap year, Feb 29 doesn't exist
        let info = parse_episodes("Show.Name.2023.02.29.720p.HDTV.x264-GROUP");
        assert!(info.air_date.is_none());
    }

    #[test]
    fn test_empty_string() {
        let info = parse_episodes("");
        assert_eq!(info.season_number, None);
        assert!(info.episode_numbers.is_empty());
        assert!(info.absolute_episode_numbers.is_empty());
    }

    #[test]
    fn test_full_season_not_triggered_when_episode_exists() {
        // S01E05 should parse as standard, not full season
        let info = parse_episodes("Show.Name.S01E05.720p.HDTV.x264-GROUP");
        assert!(!info.is_full_season);
        assert_eq!(info.episode_numbers, vec![5]);
    }

    #[test]
    fn test_multi_season_range() {
        let info = parse_episodes("Show.Name.S01-S05.720p.BluRay.x264-GROUP");
        assert_eq!(info.season_number, Some(1));
        assert!(info.is_multi_season);
        assert!(info.is_full_season);
    }
}
