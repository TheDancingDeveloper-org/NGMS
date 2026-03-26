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
    // First handle colons according to the replacement strategy
    let without_colons = match colon_replacement {
        "smart" => name
            .replace(": ", " - ")
            .replace(':', "-"),
        "dash" => name.replace(':', "-"),
        "space" => name.replace(':', " "),
        "spacedash" => name.replace(':', " -"),
        _ => name.replace(':', ""),
    };

    // Remove remaining illegal characters
    let cleaned: String = without_colons
        .chars()
        .filter(|c| !ILLEGAL_CHARS.contains(c))
        .collect();

    // Collapse multiple spaces, trim
    let mut result = String::with_capacity(cleaned.len());
    let mut prev_space = false;
    for ch in cleaned.chars() {
        if ch == ' ' {
            if !prev_space {
                result.push(' ');
            }
            prev_space = true;
        } else {
            prev_space = false;
            result.push(ch);
        }
    }

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
        Some(w) => format!("{:0>width$}", n, width = w),
        None => n.to_string(),
    }
}

// ── Episode filename builder ────────────────────────────────────────────────

/// Build a target filename from a naming format string and episode metadata.
///
/// Supported tokens: `{Series Title}`, `{season:00}`, `{episode:00}`,
/// `{Episode Title}`, `{Quality Title}`, `{Release Year}`, `{Release Group}`,
/// `{Absolute Episode}`.
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
                "Absolute Episode" => {
                    absolute_episode
                        .map(|n| pad_number(n, padding))
                        .unwrap_or_default()
                }
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
        assert_eq!(
            result,
            "The Office - S01E01 - Pilot [HDTV-720p]"
        );
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
        assert_eq!(
            result,
            "Naruto - S01E01 - 1 - Enter Naruto [WEBDL-1080p]"
        );
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
        assert_eq!(
            result,
            "Blade Runner (1982) Directors Cut [Remux-2160p]"
        );
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
        assert_eq!(
            build_season_folder("S{season:000}", 5),
            "S005"
        );
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
        assert_eq!(
            sanitize_filename("Star Trek: Picard", "space"),
            "Star Trek Picard"
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
}
