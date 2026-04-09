use once_cell::sync::Lazy;
use regex::Regex;
use stackarr_parser::Quality;
use stackarr_stream::types::MediaInfo;

// Matches `{token}` or `{token:padding}` patterns in naming format strings.
static RE_TOKEN: Lazy<Regex> = Lazy::new(|| Regex::new(r"\{([^}]+)\}").unwrap());

// ── MediaInfo for naming ────────────────────────────────────────────────────

/// Display-ready media information extracted from ffprobe, used by naming tokens.
#[derive(Debug, Clone, Default)]
pub struct NamingMediaInfo {
    pub video_codec: String,
    pub audio_codec: String,
    pub audio_channels: String,
    pub dynamic_range: String,
    pub video_bit_depth: String,
}

impl NamingMediaInfo {
    /// Build display-ready naming info from ffprobe `MediaInfo`.
    pub fn from_media_info(info: &MediaInfo) -> Self {
        let video = info.video_streams.first();
        let audio = info
            .audio_streams
            .iter()
            .find(|a| a.is_default)
            .or(info.audio_streams.first());

        let video_codec = video
            .map(|v| map_video_codec(&v.codec))
            .unwrap_or_default()
            .to_string();

        let audio_codec = audio
            .map(|a| map_audio_codec(&a.codec, a.channels))
            .unwrap_or_default()
            .to_string();

        let audio_channels = audio
            .map(|a| map_audio_channels(a.channels))
            .unwrap_or_default()
            .to_string();

        let dynamic_range = video
            .map(|v| {
                if v.is_dolby_vision && v.is_hdr {
                    "DV HDR".to_string()
                } else if v.is_dolby_vision {
                    "DV".to_string()
                } else if v.is_hdr {
                    match v.color_transfer.as_str() {
                        "arib-std-b67" => "HLG".to_string(),
                        _ => "HDR".to_string(),
                    }
                } else {
                    String::new()
                }
            })
            .unwrap_or_default();

        let video_bit_depth = video
            .map(|v| {
                // Infer bit depth from profile name (Main 10 = 10-bit, etc.)
                let profile = v.profile.to_lowercase();
                if profile.contains("10") || profile.contains("hi10") {
                    "10".to_string()
                } else if profile.contains("12") {
                    "12".to_string()
                } else if v.is_hdr || v.is_dolby_vision {
                    // HDR/DV content is virtually always 10-bit
                    "10".to_string()
                } else if !profile.is_empty() {
                    "8".to_string()
                } else {
                    String::new()
                }
            })
            .unwrap_or_default();

        Self {
            video_codec,
            audio_codec,
            audio_channels,
            dynamic_range,
            video_bit_depth,
        }
    }
}

/// Map ffprobe video codec name to Sonarr-style display name.
fn map_video_codec(codec: &str) -> &str {
    match codec {
        "hevc" | "h265" => "x265",
        "h264" => "x264",
        "av1" => "AV1",
        "vp9" => "VP9",
        "mpeg2video" => "MPEG2",
        "vc1" => "VC1",
        c if c.starts_with("dvh") || c.starts_with("dva") => "x265",
        _ => codec,
    }
}

/// Map ffprobe audio codec name to Sonarr-style display name.
fn map_audio_codec(codec: &str, channels: u32) -> &str {
    match codec {
        "truehd" => {
            if channels >= 8 {
                "TrueHD Atmos"
            } else {
                "TrueHD"
            }
        }
        "eac3" | "eac3_eae" => "DDP",
        "ac3" => "DD",
        "aac" => "AAC",
        "dts" => "DTS",
        "flac" => "FLAC",
        "opus" => "Opus",
        "vorbis" => "Vorbis",
        "pcm_s16le" | "pcm_s24le" | "pcm_s32le" => "PCM",
        "mp3" | "mp2" => "MP3",
        _ => codec,
    }
}

/// Map channel count to display string.
fn map_audio_channels(channels: u32) -> &'static str {
    match channels {
        1 => "1.0",
        2 => "2.0",
        3 => "2.1",
        6 => "5.1",
        8 => "7.1",
        _ => "",
    }
}

// ── Token pre-processing (Sonarr syntax) ────────────────────────────────────

/// Result of pre-processing a raw token from the naming format.
struct TokenParts<'a> {
    /// The cleaned token name for matching.
    name: &'a str,
    /// Leading conditional prefix character (e.g. `-`, ` `, `.`).
    /// If present, only emitted when the resolved value is non-empty.
    prefix: Option<char>,
    /// Whether to prepend `[` before the value (if non-empty).
    /// Sonarr splits brackets across tokens: `{[Mediainfo AudioCodec}` has only `[`.
    open_bracket: bool,
    /// Whether to append `]` after the value (if non-empty).
    close_bracket: bool,
}

/// Pre-process a raw token to extract Sonarr syntax: conditional prefixes and
/// bracket wrapping. E.g. `"-Release Group"` → prefix=`-`, name=`"Release Group"`.
/// `"[MediaInfo VideoCodec]"` → open_bracket + close_bracket, name=`"MediaInfo VideoCodec"`.
/// `"[Mediainfo AudioCodec"` → open_bracket only (Sonarr split-bracket format).
fn preprocess_token(raw: &str) -> TokenParts<'_> {
    let mut name = raw;
    let mut prefix = None;
    let mut open_bracket = false;
    let mut close_bracket = false;

    // Check for conditional prefix (Sonarr: {-Release Group}, { Release Group}, {.Release Group})
    if let Some(rest) = name.strip_prefix('-').or_else(|| name.strip_prefix('.')) {
        prefix = Some(name.as_bytes()[0] as char);
        name = rest.trim_start();
    } else if name.starts_with(' ') {
        prefix = Some(' ');
        name = name.trim_start();
    }

    // Check for bracket wrapping — handle both full `[token]` and split `[token` / `token]`
    if let Some(stripped) = name.strip_prefix('[') {
        open_bracket = true;
        name = stripped;
    }
    if let Some(stripped) = name.strip_suffix(']') {
        close_bracket = true;
        name = stripped;
    }
    // Trim any whitespace left after bracket stripping
    name = name.trim();

    TokenParts {
        name,
        prefix,
        open_bracket,
        close_bracket,
    }
}

/// Apply Sonarr formatting: conditional prefix and bracket wrapping.
fn format_token_value(value: String, parts: &TokenParts<'_>) -> String {
    if value.is_empty() {
        return value;
    }
    let mut result = String::with_capacity(value.len() + 3);
    if let Some(pfx) = parts.prefix {
        result.push(pfx);
    }
    if parts.open_bracket {
        result.push('[');
    }
    result.push_str(&value);
    if parts.close_bracket {
        result.push(']');
    }
    result
}

/// Resolve a MediaInfo token name to its value.
fn resolve_media_info_token(name: &str, info: Option<&NamingMediaInfo>) -> Option<String> {
    // Normalize casing: both "MediaInfo" and "Mediainfo" should work
    let normalized = if name.starts_with("Mediainfo ") {
        name.replacen("Mediainfo ", "MediaInfo ", 1)
    } else {
        name.to_string()
    };

    match normalized.as_str() {
        "MediaInfo VideoCodec" => Some(info.map(|i| i.video_codec.clone()).unwrap_or_default()),
        "MediaInfo VideoDynamicRangeType" | "MediaInfo VideoDynamicRange" => {
            Some(info.map(|i| i.dynamic_range.clone()).unwrap_or_default())
        }
        "MediaInfo AudioCodec" => Some(info.map(|i| i.audio_codec.clone()).unwrap_or_default()),
        "MediaInfo AudioChannels" => {
            Some(info.map(|i| i.audio_channels.clone()).unwrap_or_default())
        }
        "MediaInfo VideoBitDepth" => {
            Some(info.map(|i| i.video_bit_depth.clone()).unwrap_or_default())
        }
        _ if normalized.starts_with("MediaInfo") => Some(String::new()),
        _ => None,
    }
}

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
/// Supports all Sonarr-compatible tokens including MediaInfo tokens (when
/// `media_info` is provided), conditional prefix tokens (`{-Release Group}`),
/// and bracket-wrapped tokens (`{[MediaInfo VideoCodec]}`).
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
    media_info: Option<&NamingMediaInfo>,
) -> String {
    RE_TOKEN
        .replace_all(format, |caps: &regex::Captures| {
            let raw_token = &caps[1];
            let parts = preprocess_token(raw_token);
            let (name, padding) = parse_token_padding(parts.name);

            let value = match name {
                "Series Title" | "Series TitleYear" | "Series CleanTitle" => {
                    series_title.to_string()
                }
                "season" => pad_number(season, padding),
                "episode" => pad_number(episode, padding),
                "Episode Title" | "Episode CleanTitle" => episode_title.unwrap_or("").to_string(),
                "Quality Title" | "Quality Full" => quality_title(quality).to_string(),
                "Release Year" => String::new(),
                "Release Group" => release_group.unwrap_or("").to_string(),
                "Absolute Episode" => absolute_episode
                    .map(|n| pad_number(n, padding))
                    .unwrap_or_default(),
                "Custom Formats" | "Preferred Words" | "Original Title" => String::new(),
                _ => {
                    if let Some(val) = resolve_media_info_token(name, media_info) {
                        val
                    } else {
                        tracing::warn!(token = name, "unknown naming token in episode format");
                        String::new()
                    }
                }
            };

            format_token_value(value, &parts)
        })
        .to_string()
}

// ── Movie filename builder ──────────────────────────────────────────────────

/// Build a target filename from a naming format string and movie metadata.
///
/// Supports all Radarr-compatible tokens including MediaInfo tokens (when
/// `media_info` is provided), conditional prefix tokens, and bracket-wrapped tokens.
#[allow(clippy::too_many_arguments)]
pub fn build_movie_filename(
    format: &str,
    movie_title: &str,
    year: Option<i32>,
    quality: &Quality,
    edition: Option<&str>,
    release_group: Option<&str>,
    media_info: Option<&NamingMediaInfo>,
) -> String {
    RE_TOKEN
        .replace_all(format, |caps: &regex::Captures| {
            let raw_token = &caps[1];
            let parts = preprocess_token(raw_token);
            let (name, _padding) = parse_token_padding(parts.name);

            let value = match name {
                "Movie Title" | "Movie TitleYear" | "Movie CleanTitle" => movie_title.to_string(),
                "Release Year" => year.map(|y| y.to_string()).unwrap_or_default(),
                "Quality Title" | "Quality Full" => quality_title(quality).to_string(),
                "Edition Tags" | "Edition" => edition.unwrap_or("").to_string(),
                "Release Group" => release_group.unwrap_or("").to_string(),
                "Custom Formats" | "Original Title" | "IMDB Id" | "TMDB Id" => String::new(),
                _ => {
                    if let Some(val) = resolve_media_info_token(name, media_info) {
                        val
                    } else {
                        tracing::warn!(token = name, "unknown naming token in movie format");
                        String::new()
                    }
                }
            };

            format_token_value(value, &parts)
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
            None,
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
            None,
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
            None,
        );
        assert_eq!(result, "Test ");
    }

    #[test]
    fn test_sonarr_compatible_episode_tokens() {
        // Sonarr default format uses {Series TitleYear}, {Episode CleanTitle}, {Quality Full}
        let result = build_episode_filename(
            "{Series TitleYear} - S{season:00}E{episode:00} - {Episode CleanTitle} [{Quality Full}]",
            "Veronika",
            3,
            5,
            Some("Episode Five"),
            &Quality::WEBDL1080p,
            None,
            None,
            None,
        );
        assert_eq!(result, "Veronika - S03E05 - Episode Five [WEBDL-1080p]");
    }

    #[test]
    fn test_sonarr_series_clean_title_alias() {
        let result = build_episode_filename(
            "{Series CleanTitle} - S{season:00}E{episode:00}",
            "The Office",
            1,
            1,
            None,
            &Quality::Unknown,
            None,
            None,
            None,
        );
        assert_eq!(result, "The Office - S01E01");
    }

    #[test]
    fn test_radarr_compatible_movie_tokens() {
        let result = build_movie_filename(
            "{Movie CleanTitle} ({Release Year}) [{Quality Full}]",
            "Inception",
            Some(2010),
            &Quality::Bluray1080p,
            None,
            None,
            None,
        );
        assert_eq!(result, "Inception (2010) [Bluray-1080p]");
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
            let result = build_episode_filename(
                "[{Quality Title}]",
                "X",
                1,
                1,
                None,
                &quality,
                None,
                None,
                None,
            );
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
            None,
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
            None,
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
            None,
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
            None,
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
            None,
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

    // ── MediaInfo token tests ────────────────────────────────────────

    fn test_media_info() -> NamingMediaInfo {
        NamingMediaInfo {
            video_codec: "x265".to_string(),
            audio_codec: "DDP".to_string(),
            audio_channels: "5.1".to_string(),
            dynamic_range: "DV HDR".to_string(),
            video_bit_depth: "10".to_string(),
        }
    }

    #[test]
    fn test_episode_with_media_info_tokens() {
        let mi = test_media_info();
        let result = build_episode_filename(
            "{Series Title} - S{season:00}E{episode:00} - {Episode Title} [{Quality Title}] {MediaInfo VideoDynamicRangeType} {MediaInfo VideoCodec}",
            "Shrinking",
            3,
            11,
            Some("And That's Our Time"),
            &Quality::WEBDL2160p,
            Some("NTb"),
            None,
            Some(&mi),
        );
        assert_eq!(
            result,
            "Shrinking - S03E11 - And That's Our Time [WEBDL-2160p] DV HDR x265"
        );
    }

    #[test]
    fn test_episode_media_info_audio_tokens() {
        let mi = test_media_info();
        let result = build_episode_filename(
            "{Series Title} [{Mediainfo AudioCodec} {Mediainfo AudioChannels}]",
            "Show",
            1,
            1,
            None,
            &Quality::Unknown,
            None,
            None,
            Some(&mi),
        );
        assert_eq!(result, "Show [DDP 5.1]");
    }

    #[test]
    fn test_episode_media_info_none_produces_empty() {
        let result = build_episode_filename(
            "{Series Title} {MediaInfo VideoCodec}",
            "Show",
            1,
            1,
            None,
            &Quality::Unknown,
            None,
            None,
            None,
        );
        assert_eq!(result, "Show ");
    }

    #[test]
    fn test_episode_video_bit_depth() {
        let mi = test_media_info();
        let result = build_episode_filename(
            "{Series Title} {MediaInfo VideoBitDepth}bit",
            "Show",
            1,
            1,
            None,
            &Quality::Unknown,
            None,
            None,
            Some(&mi),
        );
        assert_eq!(result, "Show 10bit");
    }

    #[test]
    fn test_movie_with_media_info_tokens() {
        let mi = test_media_info();
        let result = build_movie_filename(
            "{Movie Title} ({Release Year}) [{Quality Title}] {MediaInfo VideoDynamicRangeType} {MediaInfo VideoCodec}",
            "Dune",
            Some(2024),
            &Quality::WEBDL2160p,
            None,
            Some("NTb"),
            Some(&mi),
        );
        assert_eq!(result, "Dune (2024) [WEBDL-2160p] DV HDR x265");
    }

    // ── Conditional prefix tokens ────────────────────────────────────

    #[test]
    fn test_conditional_prefix_dash_with_value() {
        let result = build_episode_filename(
            "{Series Title} - S{season:00}E{episode:00}{-Release Group}",
            "Show",
            1,
            1,
            None,
            &Quality::Unknown,
            Some("NTb"),
            None,
            None,
        );
        assert_eq!(result, "Show - S01E01-NTb");
    }

    #[test]
    fn test_conditional_prefix_dash_empty_value() {
        let result = build_episode_filename(
            "{Series Title} - S{season:00}E{episode:00}{-Release Group}",
            "Show",
            1,
            1,
            None,
            &Quality::Unknown,
            None,
            None,
            None,
        );
        assert_eq!(result, "Show - S01E01");
    }

    #[test]
    fn test_conditional_prefix_dot() {
        let result = build_episode_filename(
            "{Series Title}{.Release Group}",
            "Show",
            1,
            1,
            None,
            &Quality::Unknown,
            Some("NTb"),
            None,
            None,
        );
        assert_eq!(result, "Show.NTb");
    }

    // ── Bracket-wrapped tokens ───────────────────────────────────────

    #[test]
    fn test_bracket_wrapped_media_info_with_value() {
        let mi = test_media_info();
        let result = build_episode_filename(
            "{Series Title} {[MediaInfo VideoCodec]}",
            "Show",
            1,
            1,
            None,
            &Quality::Unknown,
            None,
            None,
            Some(&mi),
        );
        assert_eq!(result, "Show [x265]");
    }

    #[test]
    fn test_bracket_wrapped_media_info_empty_value() {
        let result = build_episode_filename(
            "{Series Title} {[MediaInfo VideoCodec]}",
            "Show",
            1,
            1,
            None,
            &Quality::Unknown,
            None,
            None,
            None,
        );
        assert_eq!(result, "Show ");
    }

    #[test]
    fn test_bracket_wrapped_dynamic_range() {
        let mi = test_media_info();
        let result = build_episode_filename(
            "{Series Title} {[MediaInfo VideoDynamicRangeType]}",
            "Show",
            1,
            1,
            None,
            &Quality::Unknown,
            None,
            None,
            Some(&mi),
        );
        assert_eq!(result, "Show [DV HDR]");
    }

    // ── Full Sonarr format (the exact user scenario) ─────────────────

    #[test]
    fn test_full_sonarr_format_with_media_info() {
        let mi = test_media_info();
        let result = build_episode_filename(
            "{Series TitleYear} - S{season:00}E{episode:00} - {Episode Title} {Custom Formats} {[MediaInfo VideoDynamicRangeType]}{[Mediainfo AudioCodec} {Mediainfo AudioChannels]}{[MediaInfo VideoCodec]}{-Release Group}",
            "Shrinking",
            3,
            11,
            Some("And That's Our Time"),
            &Quality::WEBDL2160p,
            Some("NTb"),
            None,
            Some(&mi),
        );
        // Custom Formats drops, MediaInfo tokens populate, conditional prefix applies
        assert_eq!(
            result,
            "Shrinking - S03E11 - And That's Our Time  [DV HDR][DDP 5.1][x265]-NTb"
        );
    }

    // ── NamingMediaInfo::from_media_info tests ───────────────────────

    #[test]
    fn test_naming_media_info_hevc_hdr10() {
        let info = MediaInfo {
            container: "matroska".to_string(),
            duration_secs: 3600.0,
            bitrate: 20_000_000,
            video_streams: vec![stackarr_stream::types::VideoStream {
                index: 0,
                codec: "hevc".to_string(),
                width: 3840,
                height: 2160,
                bitrate: 18_000_000,
                profile: "Main 10".to_string(),
                level: 51,
                is_hdr: true,
                is_dolby_vision: false,
                color_transfer: "smpte2084".to_string(),
                frame_rate: 23.976,
            }],
            audio_streams: vec![stackarr_stream::types::AudioStream {
                index: 0,
                codec: "truehd".to_string(),
                channels: 8,
                language: "eng".to_string(),
                title: String::new(),
                bitrate: 4_000_000,
                is_default: true,
            }],
            subtitle_streams: vec![],
        };
        let nmi = NamingMediaInfo::from_media_info(&info);
        assert_eq!(nmi.video_codec, "x265");
        assert_eq!(nmi.audio_codec, "TrueHD Atmos");
        assert_eq!(nmi.audio_channels, "7.1");
        assert_eq!(nmi.dynamic_range, "HDR");
        assert_eq!(nmi.video_bit_depth, "10");
    }

    #[test]
    fn test_naming_media_info_dv_hdr() {
        let info = MediaInfo {
            container: "matroska".to_string(),
            duration_secs: 3600.0,
            bitrate: 20_000_000,
            video_streams: vec![stackarr_stream::types::VideoStream {
                index: 0,
                codec: "hevc".to_string(),
                width: 3840,
                height: 2160,
                bitrate: 18_000_000,
                profile: "Main 10".to_string(),
                level: 51,
                is_hdr: true,
                is_dolby_vision: true,
                color_transfer: "smpte2084".to_string(),
                frame_rate: 23.976,
            }],
            audio_streams: vec![stackarr_stream::types::AudioStream {
                index: 0,
                codec: "eac3".to_string(),
                channels: 6,
                language: "eng".to_string(),
                title: String::new(),
                bitrate: 640_000,
                is_default: true,
            }],
            subtitle_streams: vec![],
        };
        let nmi = NamingMediaInfo::from_media_info(&info);
        assert_eq!(nmi.video_codec, "x265");
        assert_eq!(nmi.audio_codec, "DDP");
        assert_eq!(nmi.audio_channels, "5.1");
        assert_eq!(nmi.dynamic_range, "DV HDR");
        assert_eq!(nmi.video_bit_depth, "10");
    }

    #[test]
    fn test_naming_media_info_sdr_h264() {
        let info = MediaInfo {
            container: "mp4".to_string(),
            duration_secs: 1800.0,
            bitrate: 5_000_000,
            video_streams: vec![stackarr_stream::types::VideoStream {
                index: 0,
                codec: "h264".to_string(),
                width: 1920,
                height: 1080,
                bitrate: 4_500_000,
                profile: "High".to_string(),
                level: 41,
                is_hdr: false,
                is_dolby_vision: false,
                color_transfer: "bt709".to_string(),
                frame_rate: 24.0,
            }],
            audio_streams: vec![stackarr_stream::types::AudioStream {
                index: 0,
                codec: "aac".to_string(),
                channels: 2,
                language: "eng".to_string(),
                title: String::new(),
                bitrate: 192_000,
                is_default: true,
            }],
            subtitle_streams: vec![],
        };
        let nmi = NamingMediaInfo::from_media_info(&info);
        assert_eq!(nmi.video_codec, "x264");
        assert_eq!(nmi.audio_codec, "AAC");
        assert_eq!(nmi.audio_channels, "2.0");
        assert_eq!(nmi.dynamic_range, "");
        assert_eq!(nmi.video_bit_depth, "8");
    }

    #[test]
    fn test_naming_media_info_hlg() {
        let info = MediaInfo {
            container: "mkv".to_string(),
            duration_secs: 1800.0,
            bitrate: 10_000_000,
            video_streams: vec![stackarr_stream::types::VideoStream {
                index: 0,
                codec: "hevc".to_string(),
                width: 3840,
                height: 2160,
                bitrate: 9_000_000,
                profile: "Main 10".to_string(),
                level: 51,
                is_hdr: true,
                is_dolby_vision: false,
                color_transfer: "arib-std-b67".to_string(),
                frame_rate: 50.0,
            }],
            audio_streams: vec![],
            subtitle_streams: vec![],
        };
        let nmi = NamingMediaInfo::from_media_info(&info);
        assert_eq!(nmi.dynamic_range, "HLG");
    }

    #[test]
    fn test_naming_media_info_empty() {
        let info = MediaInfo {
            container: String::new(),
            duration_secs: 0.0,
            bitrate: 0,
            video_streams: vec![],
            audio_streams: vec![],
            subtitle_streams: vec![],
        };
        let nmi = NamingMediaInfo::from_media_info(&info);
        assert_eq!(nmi.video_codec, "");
        assert_eq!(nmi.audio_codec, "");
        assert_eq!(nmi.audio_channels, "");
        assert_eq!(nmi.dynamic_range, "");
        assert_eq!(nmi.video_bit_depth, "");
    }

    // ── Codec mapping tests ──────────────────────────────────────────

    #[test]
    fn test_map_video_codec() {
        assert_eq!(map_video_codec("hevc"), "x265");
        assert_eq!(map_video_codec("h265"), "x265");
        assert_eq!(map_video_codec("h264"), "x264");
        assert_eq!(map_video_codec("av1"), "AV1");
        assert_eq!(map_video_codec("vp9"), "VP9");
        assert_eq!(map_video_codec("mpeg2video"), "MPEG2");
        assert_eq!(map_video_codec("dvhe"), "x265"); // DV codec name
    }

    #[test]
    fn test_map_audio_codec() {
        assert_eq!(map_audio_codec("truehd", 8), "TrueHD Atmos");
        assert_eq!(map_audio_codec("truehd", 6), "TrueHD");
        assert_eq!(map_audio_codec("eac3", 6), "DDP");
        assert_eq!(map_audio_codec("ac3", 6), "DD");
        assert_eq!(map_audio_codec("aac", 2), "AAC");
        assert_eq!(map_audio_codec("dts", 6), "DTS");
        assert_eq!(map_audio_codec("flac", 2), "FLAC");
        assert_eq!(map_audio_codec("opus", 2), "Opus");
    }

    #[test]
    fn test_map_audio_channels() {
        assert_eq!(map_audio_channels(1), "1.0");
        assert_eq!(map_audio_channels(2), "2.0");
        assert_eq!(map_audio_channels(6), "5.1");
        assert_eq!(map_audio_channels(8), "7.1");
        assert_eq!(map_audio_channels(4), "");
    }

    // ── Preprocess token tests ───────────────────────────────────────

    #[test]
    fn test_preprocess_plain_token() {
        let p = preprocess_token("Release Group");
        assert_eq!(p.name, "Release Group");
        assert_eq!(p.prefix, None);
        assert!(!p.open_bracket);
        assert!(!p.close_bracket);
    }

    #[test]
    fn test_preprocess_dash_prefix() {
        let p = preprocess_token("-Release Group");
        assert_eq!(p.name, "Release Group");
        assert_eq!(p.prefix, Some('-'));
        assert!(!p.open_bracket);
    }

    #[test]
    fn test_preprocess_dot_prefix() {
        let p = preprocess_token(".Release Group");
        assert_eq!(p.name, "Release Group");
        assert_eq!(p.prefix, Some('.'));
    }

    #[test]
    fn test_preprocess_space_prefix() {
        let p = preprocess_token(" Release Group");
        assert_eq!(p.name, "Release Group");
        assert_eq!(p.prefix, Some(' '));
    }

    #[test]
    fn test_preprocess_bracket_wrap_full() {
        let p = preprocess_token("[MediaInfo VideoCodec]");
        assert_eq!(p.name, "MediaInfo VideoCodec");
        assert_eq!(p.prefix, None);
        assert!(p.open_bracket);
        assert!(p.close_bracket);
    }

    #[test]
    fn test_preprocess_bracket_open_only() {
        // Sonarr split-bracket: {[Mediainfo AudioCodec} — only opening bracket
        let p = preprocess_token("[Mediainfo AudioCodec");
        assert_eq!(p.name, "Mediainfo AudioCodec");
        assert!(p.open_bracket);
        assert!(!p.close_bracket);
    }

    #[test]
    fn test_preprocess_bracket_close_only() {
        // Sonarr split-bracket: { Mediainfo AudioChannels]} — trailing bracket with space prefix
        let p = preprocess_token(" Mediainfo AudioChannels]");
        assert_eq!(p.name, "Mediainfo AudioChannels");
        assert_eq!(p.prefix, Some(' '));
        assert!(!p.open_bracket);
        assert!(p.close_bracket);
    }
}
