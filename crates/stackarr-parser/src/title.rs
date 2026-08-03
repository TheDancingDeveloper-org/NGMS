// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 The StackArr Authors

use once_cell::sync::Lazy;
use regex::Regex;

// Pattern that marks the end of a title — episode info, quality, year, etc.
static RE_TITLE_STOP: Lazy<Regex> = Lazy::new(|| {
    Regex::new(
        r"(?i)(?:[\.\s_\-](?:S\d{1,4}(?:E\d{1,4}|[\-]S\d)|(?:19|20)\d{2}[\.\s_\-](?:0[1-9]|1[0-2])[\.\s_\-](?:0[1-9]|[12]\d|3[01])|(?:19|20)\d{2}|480[pi]|720[pi]|1080[pi]|2160[pi]|HDTV|WEB[\.\-_ ]?DL|WEBRip|BluRay|BDRip|BRRip|DVDRip|DVD|PDTV|Remux|PROPER|REPACK|REAL|x264|x265|h\.?264|h\.?265|HEVC|XviD|AAC|DTS|DD5|DDP|AC3|FLAC|MP3|TrueHD|Atmos))"
    ).unwrap()
});

static RE_DOTS_UNDERSCORES: Lazy<Regex> = Lazy::new(|| Regex::new(r"[._]").unwrap());

static RE_NON_ALNUM: Lazy<Regex> = Lazy::new(|| Regex::new(r"[^a-z0-9\s]").unwrap());

static RE_MULTI_SPACE: Lazy<Regex> = Lazy::new(|| Regex::new(r"\s{2,}").unwrap());

// 4-digit year token, used to drop year markers from both query and release
// titles before token comparison.
static RE_YEAR_TOKEN: Lazy<Regex> = Lazy::new(|| Regex::new(r"\b(?:19|20)\d{2}\b").unwrap());

// Tokens dropped before comparison so that "Your Friends & Neighbors" matches
// "Your Friends and Neighbors" and "The Office" matches "Office".
const STOP_TOKENS: &[&str] = &["the", "a", "an", "and"];

/// Extract the series or movie title from a release name.
///
/// Returns everything before episode/quality/year markers, with dots and
/// underscores replaced by spaces and the result trimmed.
pub fn parse_title(name: &str) -> String {
    // Find where the title ends (first episode/quality marker)
    let title_part = if let Some(m) = RE_TITLE_STOP.find(name) {
        &name[..m.start()]
    } else {
        // No marker found — use everything
        name
    };

    // Replace dots and underscores with spaces
    let cleaned = RE_DOTS_UNDERSCORES.replace_all(title_part, " ");
    let trimmed = cleaned.trim();

    // Remove trailing hyphens/dashes left over
    trimmed
        .trim_end_matches(|c: char| c == '-' || c.is_whitespace())
        .to_string()
}

/// Clean a title for comparison: lowercase, strip non-alphanumeric except
/// spaces, and collapse multiple spaces.
pub fn clean_title(s: &str) -> String {
    let lower = s.to_lowercase();
    let stripped = RE_NON_ALNUM.replace_all(&lower, "");
    let collapsed = RE_MULTI_SPACE.replace_all(&stripped, " ");
    collapsed.trim().to_string()
}

/// Tokenize a title for fuzzy comparison.
///
/// Converts release-name separators (`.` `_` `-`) to spaces, replaces `&` with
/// `and`, strips year markers, then lowercases and drops stopwords.
fn tokenize_for_match(s: &str) -> Vec<String> {
    let with_and = s.replace('&', " and ");
    let separated: String = with_and
        .chars()
        .map(|c| {
            if c == '.' || c == '_' || c == '-' {
                ' '
            } else {
                c
            }
        })
        .collect();
    let no_year = RE_YEAR_TOKEN.replace_all(&separated, " ");
    let cleaned = clean_title(&no_year);
    cleaned
        .split_whitespace()
        .filter(|t| !STOP_TOKENS.contains(t))
        .map(|t| t.to_string())
        .collect()
}

/// Returns true if `release_title` plausibly matches the search query.
///
/// Used by auto-search to drop indexer results that match only on
/// season/episode numbers and not on the show name. Tolerant of `&` vs `and`,
/// year inclusion, leading articles, and release-name punctuation.
///
/// Match rule: every non-stopword query token must appear in the release
/// title's token set. Tokenizes the *full* release name rather than relying
/// on `parse_title`, which is too aggressive (e.g. matches `REAL` in
/// "Real Housewives" against the REPACK/REAL stop keyword).
pub fn title_matches(query: &str, release_title: &str) -> bool {
    let query_tokens = tokenize_for_match(query);
    if query_tokens.is_empty() {
        return true;
    }
    let release_tokens: std::collections::HashSet<String> =
        tokenize_for_match(release_title).into_iter().collect();
    query_tokens.iter().all(|t| release_tokens.contains(t))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_title_standard() {
        assert_eq!(
            parse_title("Show.Name.S01E01.720p.HDTV.x264-GROUP"),
            "Show Name"
        );
    }

    #[test]
    fn test_parse_title_with_year() {
        assert_eq!(
            parse_title("Movie.Name.2024.1080p.BluRay.x264-GROUP"),
            "Movie Name"
        );
    }

    #[test]
    fn test_parse_title_underscores() {
        assert_eq!(
            parse_title("Show_Name_S01E01_720p_HDTV_x264-GROUP"),
            "Show Name"
        );
    }

    #[test]
    fn test_parse_title_spaces() {
        assert_eq!(
            parse_title("Show Name S01E01 720p HDTV x264-GROUP"),
            "Show Name"
        );
    }

    #[test]
    fn test_parse_title_daily() {
        assert_eq!(
            parse_title("Talk.Show.2024.01.15.720p.HDTV.x264-GROUP"),
            "Talk Show"
        );
    }

    #[test]
    fn test_parse_title_multi_word() {
        assert_eq!(
            parse_title("The.Great.British.Bake.Off.S01E01.720p.HDTV.x264-GROUP"),
            "The Great British Bake Off"
        );
    }

    #[test]
    fn test_clean_title_basic() {
        assert_eq!(clean_title("Show Name"), "show name");
    }

    #[test]
    fn test_clean_title_special_chars() {
        assert_eq!(clean_title("Mr. Robot"), "mr robot");
    }

    #[test]
    fn test_clean_title_extra_spaces() {
        assert_eq!(clean_title("  Show   Name  "), "show name");
    }

    #[test]
    fn test_clean_title_mixed() {
        assert_eq!(
            clean_title("Marvel's Agents of S.H.I.E.L.D."),
            "marvels agents of shield"
        );
    }

    // ── Tests documenting clean_title behavior on folder names ────────────
    // Disk folder names typically include year and imdb tags.
    // These tests show that clean_title alone cannot match Sonarr/Radarr-
    // imported clean_titles, which is why disk_scan uses path-based fallback.

    #[test]
    fn test_clean_title_folder_with_year() {
        // Folder: "Killers of the Flower Moon (2023)"
        // Radarr DB clean_title: "killersflowermoon" (strips articles + no year)
        // Our clean_title keeps articles and includes year digits
        assert_eq!(
            clean_title("Killers of the Flower Moon (2023)"),
            "killers of the flower moon 2023"
        );
    }

    #[test]
    fn test_clean_title_folder_with_year_and_imdb() {
        // Sonarr folder: "The Orville (2017) [imdb-tt5691552]"
        // Sonarr DB clean_title: "theorville"
        // Our clean_title retains all alphanumeric content
        assert_eq!(
            clean_title("The Orville (2017) [imdb-tt5691552]"),
            "the orville 2017 imdbtt5691552"
        );
    }

    #[test]
    fn test_clean_title_folder_colon_in_title() {
        // Folder: "13 Hours The Secret Soldiers of Benghazi (2016)"
        // Radarr DB clean_title: "13hourssecretsoldiersbenghazi"
        // Our clean_title preserves articles
        assert_eq!(
            clean_title("13 Hours The Secret Soldiers of Benghazi (2016)"),
            "13 hours the secret soldiers of benghazi 2016"
        );
    }

    #[test]
    fn test_clean_title_simple_movie_name() {
        assert_eq!(clean_title("The Creator"), "the creator");
    }

    #[test]
    fn test_clean_title_numeric_title() {
        assert_eq!(
            clean_title("1883 (2021) [imdb-tt13991232]"),
            "1883 2021 imdbtt13991232"
        );
    }

    #[test]
    fn test_parse_title_with_number_in_title() {
        assert_eq!(parse_title("24.S01E01.720p.HDTV.x264-GROUP"), "24");
    }

    #[test]
    fn test_parse_title_movie_with_number() {
        assert_eq!(
            parse_title("Ocean's.Eleven.2001.1080p.BluRay.x264-GROUP"),
            "Ocean's Eleven"
        );
    }

    #[test]
    fn test_parse_title_trailing_hyphen() {
        assert_eq!(
            parse_title("Some-Show-.S01E01.720p.HDTV-GROUP"),
            "Some-Show"
        );
    }

    #[test]
    fn test_parse_title_no_markers() {
        assert_eq!(parse_title("just.a.file.name"), "just a file name");
    }

    #[test]
    fn test_parse_title_empty() {
        assert_eq!(parse_title(""), "");
    }

    #[test]
    fn test_parse_title_consecutive_dots() {
        assert_eq!(
            parse_title("Show...Name.S01E01.720p.HDTV-GROUP"),
            "Show   Name"
        );
    }

    #[test]
    fn test_parse_title_codec_stop() {
        assert_eq!(parse_title("Show.Name.x264.720p-GROUP"), "Show Name");
    }

    #[test]
    fn test_parse_title_audio_stop() {
        assert_eq!(parse_title("Show.Name.DTS.720p-GROUP"), "Show Name");
    }

    #[test]
    fn test_clean_title_apostrophe() {
        assert_eq!(clean_title("It's Always Sunny"), "its always sunny");
    }

    #[test]
    fn test_clean_title_empty() {
        assert_eq!(clean_title(""), "");
    }

    #[test]
    fn test_clean_title_pure_numbers() {
        assert_eq!(clean_title("1883"), "1883");
    }

    #[test]
    fn test_clean_title_hyphens_removed() {
        assert_eq!(clean_title("Spider-Man"), "spiderman");
    }

    // ── title_matches: regression tests for auto-search filter ─────────────

    #[test]
    fn test_title_matches_ampersand_vs_and() {
        // Series in DB: "Your Friends & Neighbors"
        // Release uses spelled-out "and"
        assert!(title_matches(
            "Your Friends & Neighbors",
            "Your.Friends.and.Neighbors.S02E02.Lady.Bits.2160p.ATVP.WEB-DL.DD+5.1.Atmos.DoVi.HDR.H.265-playWEB"
        ));
    }

    #[test]
    fn test_title_matches_year_in_query_not_release() {
        // Series in DB: "Matlock (2024)" — query carries the year, but
        // parse_title strips year from the release.
        assert!(title_matches(
            "Matlock (2024)",
            "Matlock.2024.S02E13.2160p.WEB.h265-ETHEL"
        ));
    }

    #[test]
    fn test_title_matches_year_in_release_not_query() {
        // Series in DB: just "Matlock"
        assert!(title_matches(
            "Matlock",
            "Matlock.2024.S02E13.2160p.WEB.h265-ETHEL"
        ));
    }

    #[test]
    fn test_title_matches_real_housewives() {
        assert!(title_matches(
            "The Real Housewives of Beverly Hills",
            "The.Real.Housewives.of.Beverly.Hills.S15E17.Drama.on.the.Dance.Floor.1080p.AMZN.WEB-DL.DDP2.0.H.264-NTb"
        ));
    }

    #[test]
    fn test_title_matches_case_insensitive() {
        assert!(title_matches(
            "The Real Housewives of Beverly Hills",
            "The.Real.Housewives.Of.Beverly.Hills.S15E17.1080p.AMZN.WEB-DL.DDP2.0.H.264-Kitsune"
        ));
    }

    #[test]
    fn test_title_matches_apostrophe() {
        assert!(title_matches(
            "It's Always Sunny in Philadelphia",
            "Its.Always.Sunny.in.Philadelphia.S15E01.720p.WEB.h264-GROUP"
        ));
    }

    #[test]
    fn test_title_matches_rejects_unrelated() {
        // Indexer returned a different show that happens to match the
        // episode number — must be rejected.
        assert!(!title_matches(
            "Matlock",
            "Better.Call.Saul.S02E13.1080p.WEB-DL-GROUP"
        ));
    }

    #[test]
    fn test_title_matches_rejects_partial_word() {
        // "Friends" must not match "Your Friends and Neighbors" reversed
        assert!(!title_matches(
            "Your Friends and Neighbors",
            "Friends.S01E01.720p.HDTV-GROUP"
        ));
    }

    #[test]
    fn test_title_matches_strips_articles() {
        // Query has "The", release omits it
        assert!(title_matches("The Office", "Office.S01E01.720p.HDTV-GROUP"));
    }
}
