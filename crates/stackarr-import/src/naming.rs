use once_cell::sync::Lazy;
use regex::Regex;
use stackarr_parser::Quality;

// Matches `{token}` or `{token:padding}` patterns in naming format strings.
static RE_TOKEN: Lazy<Regex> = Lazy::new(|| Regex::new(r"\{([^}]+)\}").unwrap());

// ── Quality display ─────────────────────────────────────────────────────────

/// Human-readable quality name (e.g. "Bluray-1080p", "WEBDL-2160p").
fn quality_title(q: &Quality) -> &'static str {
    match q {
        Quality::Unknown => "Unknown",
        Quality::SDTV => "SDTV",
        Quality::DVD => "DVD",
        Quality::WEBDL480p => "WEBDL-480p",
        Quality::HDTV720p => "HDTV-720p",
        Quality::HDTV1080p => "HDTV-1080p",
        Quality::Raw => "Raw-HD",
        Quality::WEBDL720p => "WEBDL-720p",
        Quality::Bluray720p => "Bluray-720p",
        Quality::WEBDL1080p => "WEBDL-1080p",
        Quality::Bluray1080p => "Bluray-1080p",
        Quality::HDTV2160p => "HDTV-2160p",
        Quality::WEBDL2160p => "WEBDL-2160p",
        Quality::Bluray2160p => "Bluray-2160p",
        Quality::DVDRip => "DVDRip",
        Quality::WEBRip480p => "WEBRip-480p",
        Quality::WEBRip720p => "WEBRip-720p",
        Quality::WEBRip1080p => "WEBRip-1080p",
        Quality::WEBRip2160p => "WEBRip-2160p",
        Quality::Remux1080p => "Remux-1080p",
        Quality::Remux2160p => "Remux-2160p",
    }
}

// ── Sanitization ────────────────────────────────────────────────────────────

/// Characters that are illegal in filenames on Windows/Linux/macOS.
const ILLEGAL_CHARS: &[char] = &['/', '\\', '*', '?', '"', '<', '>', '|'];

/// Sanitize a filename by replacing illegal characters, handling colons via the
/// configured replacement strategy, and trimming whitespace.
///
/// `colon_replacement` values:
///   - `"smart"` — replace `: ` with ` - `, standalone `:` with `-`
///   - `"dash"` — replace `:` with `-`
///   - `"space"` — replace `:` with ` `
///   - `"spacedash"` — replace `:` with ` -`
///   - anything else — just remove colons
pub fn sanitize_filename(name: &str, colon_replacement: &str) -> String {
    // Remove illegal characters first (except colons, handled below)
    let cleaned: String = name
        .chars()
        .filter(|c| !ILLEGAL_CHARS.contains(c))
        .collect();

    // Collapse multiple spaces in the original input before colon handling
    let mut collapsed = String::with_capacity(cleaned.len());
    let mut prev_space = false;
    for ch in cleaned.chars() {
        if ch == ' ' {
            if !prev_space {
                collapsed.push(' ');
            }
            prev_space = true;
        } else {
            prev_space = false;
            collapsed.push(ch);
        }
    }

    // Now handle colons according to the replacement strategy
    let result = match colon_replacement {
        "smart" => collapsed.replace(": ", " - ").replace(':', "-"),
        "dash" => collapsed.replace(':', "-"),
        "space" => collapsed.replace(':', " "),
        "spacedash" => collapsed.replace(':', " -"),
        _ => collapsed.replace(':', ""),
    };

    result.trim().to_string()
}

// ── Padding helper ──────────────────────────────────────────────────────────

/// Given a token like `season:00` or `episode:000`, extract the name portion
/// and the padding width. Returns `(name, padding_width)`.
fn parse_token_padding(token: &str) -> (&str, Option<usize>) {
    if let Some((name, pad)) = token.split_once(':') {
        // Padding is determined by the number of digits: "00" = 2, "000" = 3
        let width = pad.len();
        if width > 0 && pad.chars().all(|c| c == '0') {
            Some((name, Some(width)))
        } else {
            // Not a valid padding pattern, treat the whole thing as the name
            Some((token, None))
        }
    } else {
        Some((token, None))
    }
    .unwrap_or((token, None))
}

/// Format a number with the given zero-padding width.
fn pad_number(n: i32, width: Option<usize>) -> String {
    match width {
        Some(w) => format!("{n:0>w$}"),
        None => n.to_string(),
    }
}

// ── Episode filename builder ────────────────────────────────────────────────

/// Build a target filename from a naming format string and episode metadata.
///
/// Supported tokens: `{Series Title}`, `{season:00}`, `{episode:00}`,
/// `{Episode Title}`, `{Quality Title}`, `{Release Year}`, `{Release Group}`,
/// `{Absolute Episode}`.
#[allow(clippy::too_many_arguments)]
pub fn build_episode_filename(
    format: &str,
    series_title: &str,
    season: i32,
    episode: i32,
    episode_title: Option<&str>,
    quality: &Quality,
    release_group: Option<&str>,
    absolute_episode: Option<i32>,
) -> String {
    RE_TOKEN
        .replace_all(format, |caps: &regex::Captures| {
            let raw_token = &caps[1];
            let (name, padding) = parse_token_padding(raw_token);

            match name {
                "Series Title" => series_title.to_string(),
                "season" => pad_number(season, padding),
                "episode" => pad_number(episode, padding),
                "Episode Title" => episode_title.unwrap_or("").to_string(),
                "Quality Title" => quality_title(quality).to_string(),
                "Release Year" => "".to_string(), // not applicable for episodes in most cases
                "Release Group" => release_group.unwrap_or("").to_string(),
                "Absolute Episode" => absolute_episode
                    .map(|n| pad_number(n, padding))
                    .unwrap_or_default(),
                _ => String::new(),
            }
        })
        .to_string()
}

// ── Movie filename builder ──────────────────────────────────────────────────

/// Build a target filename from a naming format string and movie metadata.
///
/// Supported tokens: `{Movie Title}`, `{Release Year}`, `{Quality Title}`,
/// `{Edition Tags}`, `{Release Group}`.
pub fn build_movie_filename(
    format: &str,
    movie_title: &str,
    year: Option<i32>,
    quality: &Quality,
    edition: Option<&str>,
    release_group: Option<&str>,
) -> String {
    RE_TOKEN
        .replace_all(format, |caps: &regex::Captures| {
            let raw_token = &caps[1];
            let (name, _padding) = parse_token_padding(raw_token);

            match name {
                "Movie Title" => movie_title.to_string(),
                "Release Year" => year.map(|y| y.to_string()).unwrap_or_default(),
                "Quality Title" => quality_title(quality).to_string(),
                "Edition Tags" => edition.unwrap_or("").to_string(),
                "Release Group" => release_group.unwrap_or("").to_string(),
                _ => String::new(),
            }
        })
        .to_string()
}

// ── Season folder builder ───────────────────────────────────────────────────

/// Build a season folder name from a format string and season number.
///
/// Supported tokens: `{season:00}`.
pub fn build_season_folder(format: &str, season: i32) -> String {
    RE_TOKEN
        .replace_all(format, |caps: &regex::Captures| {
            let raw_token = &caps[1];
            let (name, padding) = parse_token_padding(raw_token);

            match name {
                "season" => pad_number(season, padding),
                _ => String::new(),
            }
        })
        .to_string()
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_episode_standard() {
        let result = build_episode_filename(
            "{Series Title} - S{season:00}E{episode:00} - {Episode Title} [{Quality Title}]",
            "The Office",
            1,
            1,
            Some("Pilot"),
            &Quality::HDTV720p,
            None,
            None,
        );
        assert_eq!(result, "The Office - S01E01 - Pilot [HDTV-720p]");
    }

    #[test]
    fn test_build_episode_with_group() {
        let result = build_episode_filename(
            "{Series Title} - S{season:00}E{episode:00} - {Episode Title} [{Quality Title}]-{Release Group}",
            "Breaking Bad",
            5,
            16,
            Some("Felina"),
            &Quality::Bluray1080p,
            Some("GROUP"),
            None,
        );
        assert_eq!(
            result,
            "Breaking Bad - S05E16 - Felina [Bluray-1080p]-GROUP"
        );
    }

    #[test]
    fn test_build_episode_anime_absolute() {
        let result = build_episode_filename(
            "{Series Title} - S{season:00}E{episode:00} - {Absolute Episode} - {Episode Title} [{Quality Title}]",
            "Naruto",
            1,
            1,
            Some("Enter Naruto"),
            &Quality::WEBDL1080p,
            None,
            Some(1),
        );
        assert_eq!(result, "Naruto - S01E01 - 1 - Enter Naruto [WEBDL-1080p]");
    }

    #[test]
    fn test_build_episode_three_digit_padding() {
        let result = build_episode_filename(
            "{Series Title} - {Absolute Episode:000}",
            "One Piece",
            0,
            0,
            None,
            &Quality::Unknown,
            None,
            Some(42),
        );
        assert_eq!(result, "One Piece - 042");
    }

    #[test]
    fn test_build_movie_standard() {
        let result = build_movie_filename(
            "{Movie Title} ({Release Year}) [{Quality Title}]",
            "Inception",
            Some(2010),
            &Quality::Bluray1080p,
            None,
            None,
        );
        assert_eq!(result, "Inception (2010) [Bluray-1080p]");
    }

    #[test]
    fn test_build_movie_with_edition() {
        let result = build_movie_filename(
            "{Movie Title} ({Release Year}) {Edition Tags} [{Quality Title}]",
            "Blade Runner",
            Some(1982),
            &Quality::Remux2160p,
            Some("Directors Cut"),
            None,
        );
        assert_eq!(result, "Blade Runner (1982) Directors Cut [Remux-2160p]");
    }

    #[test]
    fn test_build_movie_no_year() {
        let result = build_movie_filename(
            "{Movie Title} ({Release Year}) [{Quality Title}]",
            "Test Movie",
            None,
            &Quality::WEBDL1080p,
            None,
            None,
        );
        assert_eq!(result, "Test Movie () [WEBDL-1080p]");
    }

    #[test]
    fn test_build_season_folder() {
        assert_eq!(build_season_folder("Season {season:00}", 3), "Season 03");
        assert_eq!(build_season_folder("Season {season:00}", 12), "Season 12");
        assert_eq!(build_season_folder("S{season:000}", 5), "S005");
    }

    #[test]
    fn test_sanitize_smart_colon() {
        // ": " is replaced with " - "
        assert_eq!(
            sanitize_filename("Show: The Beginning", "smart"),
            "Show - The Beginning"
        );
        assert_eq!(
            sanitize_filename("Doctor Who: Series 14", "smart"),
            "Doctor Who - Series 14"
        );
        // Standalone ":" (no trailing space) is replaced with "-"
        assert_eq!(
            sanitize_filename("Title:Subtitle", "smart"),
            "Title-Subtitle"
        );
    }

    #[test]
    fn test_sanitize_dash_colon() {
        assert_eq!(
            sanitize_filename("Star Trek: Picard", "dash"),
            "Star Trek- Picard"
        );
    }

    #[test]
    fn test_sanitize_space_colon() {
        // "space" mode replaces ":" with " ", resulting in double space (not collapsed)
        assert_eq!(
            sanitize_filename("Star Trek: Picard", "space"),
            "Star Trek  Picard"
        );
    }

    #[test]
    fn test_sanitize_illegal_chars() {
        assert_eq!(
            sanitize_filename("file/name*with?bad<chars>", "smart"),
            "filenamewithbadchars"
        );
    }

    #[test]
    fn test_sanitize_collapses_spaces() {
        assert_eq!(
            sanitize_filename("too   many   spaces", "smart"),
            "too many spaces"
        );
    }

    #[test]
    fn test_unknown_token_replaced_with_empty() {
        let result = build_episode_filename(
            "{Series Title} {Unknown Token}",
            "Test",
            1,
            1,
            None,
            &Quality::Unknown,
            None,
            None,
        );
        assert_eq!(result, "Test ");
    }

    // ── Episode filename edge cases ───────────────────────────────────

    #[test]
    fn test_build_episode_high_season_numbers() {
        let result = build_episode_filename(
            "{Series Title} - S{season:00}E{episode:00}",
            "Grey's Anatomy",
            20,
            15,
            None,
            &Quality::WEBDL1080p,
            None,
            None,
        );
        assert_eq!(result, "Grey's Anatomy - S20E15");
    }

    #[test]
    fn test_build_episode_specials_season_zero() {
        let result = build_episode_filename(
            "{Series Title} - S{season:00}E{episode:00} - {Episode Title}",
            "Doctor Who",
            0,
            5,
            Some("Special Episode"),
            &Quality::HDTV1080p,
            None,
            None,
        );
        assert_eq!(result, "Doctor Who - S00E05 - Special Episode");
    }

    #[test]
    fn test_build_episode_no_episode_title() {
        let result = build_episode_filename(
            "{Series Title} - S{season:00}E{episode:00} - {Episode Title}",
            "Unnamed Show",
            1,
            1,
            None,
            &Quality::HDTV720p,
            None,
            None,
        );
        assert_eq!(result, "Unnamed Show - S01E01 - ");
    }

    #[test]
    fn test_build_episode_single_digit_no_padding() {
        let result = build_episode_filename(
            "{Series Title} S{season}E{episode}",
            "Test",
            1,
            1,
            None,
            &Quality::Unknown,
            None,
            None,
        );
        assert_eq!(result, "Test S1E1");
    }

    #[test]
    fn test_build_episode_all_quality_types() {
        let qualities = vec![
            (Quality::SDTV, "SDTV"),
            (Quality::DVD, "DVD"),
            (Quality::WEBDL480p, "WEBDL-480p"),
            (Quality::WEBRip480p, "WEBRip-480p"),
            (Quality::HDTV720p, "HDTV-720p"),
            (Quality::WEBDL720p, "WEBDL-720p"),
            (Quality::WEBRip720p, "WEBRip-720p"),
            (Quality::Bluray720p, "Bluray-720p"),
            (Quality::HDTV1080p, "HDTV-1080p"),
            (Quality::WEBDL1080p, "WEBDL-1080p"),
            (Quality::WEBRip1080p, "WEBRip-1080p"),
            (Quality::Bluray1080p, "Bluray-1080p"),
            (Quality::Remux1080p, "Remux-1080p"),
            (Quality::HDTV2160p, "HDTV-2160p"),
            (Quality::WEBDL2160p, "WEBDL-2160p"),
            (Quality::WEBRip2160p, "WEBRip-2160p"),
            (Quality::Bluray2160p, "Bluray-2160p"),
            (Quality::Remux2160p, "Remux-2160p"),
            (Quality::Raw, "Raw-HD"),
        ];

        for (quality, expected_name) in qualities {
            let result =
                build_episode_filename("[{Quality Title}]", "X", 1, 1, None, &quality, None, None);
            assert_eq!(
                result,
                format!("[{expected_name}]"),
                "quality {quality:?} mismatch"
            );
        }
    }

    #[test]
    fn test_build_episode_absolute_with_padding() {
        let result = build_episode_filename(
            "{Series Title} - {Absolute Episode:000}",
            "Bleach",
            1,
            1,
            None,
            &Quality::Unknown,
            None,
            Some(366),
        );
        assert_eq!(result, "Bleach - 366");
    }

    #[test]
    fn test_build_episode_absolute_no_value() {
        let result = build_episode_filename(
            "{Series Title} - {Absolute Episode:000}",
            "Show",
            1,
            1,
            None,
            &Quality::Unknown,
            None,
            None,
        );
        assert_eq!(result, "Show - ");
    }

    // ── Movie filename edge cases ─────────────────────────────────────

    #[test]
    fn test_build_movie_with_release_group() {
        let result = build_movie_filename(
            "{Movie Title} ({Release Year}) [{Quality Title}]-{Release Group}",
            "Dune",
            Some(2021),
            &Quality::Remux2160p,
            None,
            Some("FraMeSToR"),
        );
        assert_eq!(result, "Dune (2021) [Remux-2160p]-FraMeSToR");
    }

    #[test]
    fn test_build_movie_no_optional_tokens() {
        let result = build_movie_filename(
            "{Movie Title}",
            "Simple Movie",
            None,
            &Quality::Unknown,
            None,
            None,
        );
        assert_eq!(result, "Simple Movie");
    }

    #[test]
    fn test_build_movie_all_tokens() {
        let result = build_movie_filename(
            "{Movie Title} ({Release Year}) {Edition Tags} [{Quality Title}]-{Release Group}",
            "Gladiator",
            Some(2000),
            &Quality::Bluray2160p,
            Some("Extended Edition"),
            Some("EPSiLON"),
        );
        assert_eq!(
            result,
            "Gladiator (2000) Extended Edition [Bluray-2160p]-EPSiLON"
        );
    }

    // ── Season folder edge cases ──────────────────────────────────────

    #[test]
    fn test_build_season_folder_season_zero() {
        assert_eq!(build_season_folder("Season {season:00}", 0), "Season 00");
    }

    #[test]
    fn test_build_season_folder_high_season() {
        assert_eq!(build_season_folder("Season {season:00}", 99), "Season 99");
    }

    #[test]
    fn test_build_season_folder_no_padding() {
        assert_eq!(build_season_folder("Season {season}", 3), "Season 3");
    }

    #[test]
    fn test_build_season_folder_specials() {
        assert_eq!(build_season_folder("Specials", 0), "Specials");
    }

    // ── Sanitization edge cases ───────────────────────────────────────

    #[test]
    fn test_sanitize_spacedash_colon() {
        assert_eq!(
            sanitize_filename("Star Trek: Picard", "spacedash"),
            "Star Trek - Picard"
        );
    }

    #[test]
    fn test_sanitize_remove_colon() {
        assert_eq!(
            sanitize_filename("Title: Subtitle", "delete"),
            "Title Subtitle"
        );
    }

    #[test]
    fn test_sanitize_empty_string() {
        assert_eq!(sanitize_filename("", "smart"), "");
    }

    #[test]
    fn test_sanitize_only_illegal_chars() {
        assert_eq!(sanitize_filename("/*?\"<>|", "smart"), "");
    }

    #[test]
    fn test_sanitize_preserves_normal_chars() {
        assert_eq!(
            sanitize_filename("Normal Title 2024", "smart"),
            "Normal Title 2024"
        );
    }

    #[test]
    fn test_sanitize_trims_whitespace() {
        assert_eq!(sanitize_filename("  Title  ", "smart"), "Title");
    }

    #[test]
    fn test_sanitize_backslash_removed() {
        assert_eq!(sanitize_filename("path\\to\\file", "smart"), "pathtofile");
    }

    #[test]
    fn test_sanitize_multiple_colons_smart() {
        assert_eq!(
            sanitize_filename("Title: Part One: Remastered", "smart"),
            "Title - Part One - Remastered"
        );
    }

    // ── Token padding helper ──────────────────────────────────────────

    #[test]
    fn test_parse_token_padding_with_two_digit() {
        let (name, padding) = parse_token_padding("season:00");
        assert_eq!(name, "season");
        assert_eq!(padding, Some(2));
    }

    #[test]
    fn test_parse_token_padding_with_three_digit() {
        let (name, padding) = parse_token_padding("episode:000");
        assert_eq!(name, "episode");
        assert_eq!(padding, Some(3));
    }

    #[test]
    fn test_parse_token_padding_no_padding() {
        let (name, padding) = parse_token_padding("Series Title");
        assert_eq!(name, "Series Title");
        assert_eq!(padding, None);
    }

    #[test]
    fn test_parse_token_padding_non_zero_padding() {
        // "12" is not a valid zero-padding pattern
        let (name, padding) = parse_token_padding("season:12");
        assert_eq!(name, "season:12");
        assert_eq!(padding, None);
    }

    #[test]
    fn test_pad_number_no_padding() {
        assert_eq!(pad_number(5, None), "5");
        assert_eq!(pad_number(42, None), "42");
    }

    #[test]
    fn test_pad_number_with_padding() {
        assert_eq!(pad_number(1, Some(2)), "01");
        assert_eq!(pad_number(10, Some(2)), "10");
        assert_eq!(pad_number(1, Some(3)), "001");
        assert_eq!(pad_number(100, Some(3)), "100");
        assert_eq!(pad_number(1000, Some(3)), "1000"); // exceeds padding, still works
    }

    #[test]
    fn test_sanitize_colon_smart() {
        assert_eq!(sanitize_filename("Show: Name", "smart"), "Show - Name");
    }

    #[test]
    fn test_sanitize_colon_dash() {
        assert_eq!(sanitize_filename("Show: Name", "dash"), "Show- Name");
    }

    #[test]
    fn test_sanitize_colon_space() {
        assert_eq!(sanitize_filename("Show: Name", "space"), "Show  Name");
    }

    #[test]
    fn test_sanitize_colon_spacedash() {
        assert_eq!(sanitize_filename("Show: Name", "spacedash"), "Show - Name");
    }

    #[test]
    fn test_sanitize_removes_illegal_chars() {
        assert_eq!(
            sanitize_filename("Show/Name*With?Bad\"Chars", "smart"),
            "ShowNameWithBadChars"
        );
    }

    #[test]
    fn test_sanitize_removes_angle_brackets() {
        assert_eq!(sanitize_filename("Show<Name>Here", "smart"), "ShowNameHere");
    }

    #[test]
    fn test_sanitize_removes_pipe() {
        assert_eq!(sanitize_filename("Show|Name", "smart"), "ShowName");
    }

    #[test]
    fn test_sanitize_collapses_spaces_new() {
        assert_eq!(sanitize_filename("Show   Name", "smart"), "Show Name");
    }

    #[test]
    fn test_sanitize_empty_string_new() {
        assert_eq!(sanitize_filename("", "smart"), "");
    }

    #[test]
    fn test_build_episode_zero_padding_3_digits() {
        let result = build_episode_filename(
            "{Series Title} - S{season:00}E{episode:000} - {Episode Title}",
            "Show",
            1,
            5,
            Some("Pilot"),
            &stackarr_parser::Quality::HDTV720p,
            None,
            None,
        );
        assert_eq!(result, "Show - S01E005 - Pilot");
    }

    #[test]
    fn test_build_episode_no_padding() {
        let result = build_episode_filename(
            "S{season}E{episode}",
            "Show",
            1,
            5,
            Some("Pilot"),
            &stackarr_parser::Quality::HDTV720p,
            None,
            None,
        );
        assert_eq!(result, "S1E5");
    }

    #[test]
    fn test_build_episode_with_release_group() {
        let result = build_episode_filename(
            "{Series Title} - S{season:00}E{episode:00} [{Release Group}]",
            "Show",
            1,
            5,
            Some("Pilot"),
            &stackarr_parser::Quality::HDTV720p,
            Some("LOL"),
            None,
        );
        assert_eq!(result, "Show - S01E05 [LOL]");
    }

    #[test]
    fn test_build_episode_with_absolute() {
        let result = build_episode_filename(
            "{Series Title} - {Absolute Episode} - {Episode Title}",
            "Anime",
            1,
            5,
            Some("Fight"),
            &stackarr_parser::Quality::WEBDL1080p,
            None,
            Some(42),
        );
        assert_eq!(result, "Anime - 42 - Fight");
    }

    #[test]
    fn test_build_episode_with_quality() {
        let result = build_episode_filename(
            "{Series Title} - S{season:00}E{episode:00} [{Quality Title}]",
            "Show",
            1,
            5,
            Some("Pilot"),
            &stackarr_parser::Quality::Bluray1080p,
            None,
            None,
        );
        assert_eq!(result, "Show - S01E05 [Bluray-1080p]");
    }

    #[test]
    fn test_build_movie_basic() {
        let result = build_movie_filename(
            "{Movie Title} ({Release Year}) [{Quality Title}]",
            "Inception",
            Some(2010),
            &stackarr_parser::Quality::Bluray1080p,
            None,
            None,
        );
        assert_eq!(result, "Inception (2010) [Bluray-1080p]");
    }

    #[test]
    fn test_build_movie_with_edition_new() {
        let result = build_movie_filename(
            "{Movie Title} ({Release Year}) {Edition Tags} [{Quality Title}]",
            "Movie",
            Some(2024),
            &stackarr_parser::Quality::Remux2160p,
            Some("Directors Cut"),
            None,
        );
        assert_eq!(result, "Movie (2024) Directors Cut [Remux-2160p]");
    }

    #[test]
    fn test_build_movie_with_release_group_new() {
        let result = build_movie_filename(
            "{Movie Title} ({Release Year})-{Release Group}",
            "Movie",
            Some(2024),
            &stackarr_parser::Quality::WEBDL1080p,
            None,
            Some("FraMeSToR"),
        );
        assert_eq!(result, "Movie (2024)-FraMeSToR");
    }

    #[test]
    fn test_build_movie_no_year_new() {
        let result = build_movie_filename(
            "{Movie Title} ({Release Year})",
            "Movie",
            None,
            &stackarr_parser::Quality::WEBDL1080p,
            None,
            None,
        );
        assert_eq!(result, "Movie ()");
    }

    #[test]
    fn test_build_season_folder_padding() {
        assert_eq!(build_season_folder("Season {season:00}", 1), "Season 01");
        assert_eq!(build_season_folder("Season {season:00}", 12), "Season 12");
        assert_eq!(build_season_folder("Season {season}", 5), "Season 5");
    }

    #[test]
    fn test_build_season_folder_specials_new() {
        assert_eq!(build_season_folder("Season {season:00}", 0), "Season 00");
    }

    #[test]
    fn test_quality_title_all_variants() {
        use stackarr_parser::Quality::*;
        assert_eq!(quality_title(&Unknown), "Unknown");
        assert_eq!(quality_title(&SDTV), "SDTV");
        assert_eq!(quality_title(&DVD), "DVD");
        assert_eq!(quality_title(&HDTV720p), "HDTV-720p");
        assert_eq!(quality_title(&WEBDL1080p), "WEBDL-1080p");
        assert_eq!(quality_title(&WEBRip1080p), "WEBRip-1080p");
        assert_eq!(quality_title(&Bluray1080p), "Bluray-1080p");
        assert_eq!(quality_title(&Remux1080p), "Remux-1080p");
        assert_eq!(quality_title(&HDTV2160p), "HDTV-2160p");
        assert_eq!(quality_title(&WEBDL2160p), "WEBDL-2160p");
        assert_eq!(quality_title(&Bluray2160p), "Bluray-2160p");
        assert_eq!(quality_title(&Remux2160p), "Remux-2160p");
        assert_eq!(quality_title(&Raw), "Raw-HD");
    }
}
