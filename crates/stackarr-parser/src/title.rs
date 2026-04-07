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
}
