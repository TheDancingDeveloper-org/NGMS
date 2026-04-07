use once_cell::sync::Lazy;
use regex::Regex;
use serde::{Deserialize, Serialize};

/// Languages that can be detected in release names.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Language {
    Unknown,
    English,
    French,
    Spanish,
    German,
    Italian,
    Portuguese,
    Japanese,
    Chinese,
    Korean,
    Russian,
    Polish,
    Dutch,
    Swedish,
    Norwegian,
    Danish,
    Finnish,
    Turkish,
    Arabic,
    Hindi,
    Czech,
    Hungarian,
    Romanian,
    Greek,
    Hebrew,
    Thai,
    Vietnamese,
    Indonesian,
    Multi,
}

struct CompiledLanguagePattern {
    regex: Regex,
    language: Language,
}

static LANGUAGE_PATTERNS: Lazy<Vec<CompiledLanguagePattern>> = Lazy::new(|| {
    let patterns: &[(&str, Language)] = &[
        (r"(?i)\b(?:MULTi)\b", Language::Multi),
        (r"(?i)\b(?:ENGLISH|ENG)\b", Language::English),
        (
            r"(?i)\b(?:FRENCH|VOSTFR|VFF|VFQ|TRUEFRENCH|FRE)\b",
            Language::French,
        ),
        (
            r"(?i)\b(?:SPANISH|ESPANOL|SPA|CASTELLANO|LATINO)\b",
            Language::Spanish,
        ),
        (
            r"(?i)\b(?:GERMAN|GER|DEUTSCH|DTS\-?GER)\b",
            Language::German,
        ),
        (r"(?i)\b(?:ITALIAN|ITA)\b", Language::Italian),
        (r"(?i)\b(?:PORTUGUESE|POR|PTBR)\b", Language::Portuguese),
        (r"(?i)\b(?:JAPANESE|JPN|JAP)\b", Language::Japanese),
        (
            r"(?i)\b(?:CHINESE|CHI|CHS|CHT|MANDARIN|CANTONESE)\b",
            Language::Chinese,
        ),
        (r"(?i)\b(?:KOREAN|KOR)\b", Language::Korean),
        (r"(?i)\b(?:RUSSIAN|RUS)\b", Language::Russian),
        (r"(?i)\b(?:POLISH|POL|PL)\b", Language::Polish),
        (r"(?i)\b(?:DUTCH|NLD|DUT|FLEMISH)\b", Language::Dutch),
        (r"(?i)\b(?:SWEDISH|SWE)\b", Language::Swedish),
        (r"(?i)\b(?:NORWEGIAN|NOR)\b", Language::Norwegian),
        (r"(?i)\b(?:DANISH|DAN)\b", Language::Danish),
        (r"(?i)\b(?:FINNISH|FIN)\b", Language::Finnish),
        (r"(?i)\b(?:TURKISH|TUR)\b", Language::Turkish),
        (r"(?i)\b(?:ARABIC|ARA)\b", Language::Arabic),
        (r"(?i)\b(?:HINDI|HIN)\b", Language::Hindi),
        (r"(?i)\b(?:CZECH|CZE|CES)\b", Language::Czech),
        (r"(?i)\b(?:HUNGARIAN|HUN)\b", Language::Hungarian),
        (r"(?i)\b(?:ROMANIAN|ROM|RON)\b", Language::Romanian),
        (r"(?i)\b(?:GREEK|GRE|ELL)\b", Language::Greek),
        (r"(?i)\b(?:HEBREW|HEB)\b", Language::Hebrew),
        (r"(?i)\b(?:THAI|THA)\b", Language::Thai),
        (r"(?i)\b(?:VIETNAMESE|VIE)\b", Language::Vietnamese),
        (r"(?i)\b(?:INDONESIAN|IND)\b", Language::Indonesian),
    ];

    patterns
        .iter()
        .map(|(pat, lang)| CompiledLanguagePattern {
            regex: Regex::new(pat).unwrap(),
            language: *lang,
        })
        .collect()
});

/// Detect languages mentioned in a release name.
///
/// Returns a vector of detected [`Language`] values. Returns
/// `vec![Language::Unknown]` if no language tag is found.
pub fn parse_languages(name: &str) -> Vec<Language> {
    let mut languages = Vec::new();

    for pattern in LANGUAGE_PATTERNS.iter() {
        if pattern.regex.is_match(name) {
            languages.push(pattern.language);
        }
    }

    if languages.is_empty() {
        languages.push(Language::Unknown);
    }

    languages
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_no_language() {
        let langs = parse_languages("Show.Name.S01E01.720p.HDTV.x264-GROUP");
        assert_eq!(langs, vec![Language::Unknown]);
    }

    #[test]
    fn test_english() {
        let langs = parse_languages("Show.Name.S01E01.ENGLISH.720p.HDTV.x264-GROUP");
        assert!(langs.contains(&Language::English));
    }

    #[test]
    fn test_french() {
        let langs = parse_languages("Show.Name.S01E01.FRENCH.720p.HDTV.x264-GROUP");
        assert!(langs.contains(&Language::French));
    }

    #[test]
    fn test_truefrench() {
        let langs = parse_languages("Show.Name.S01E01.TRUEFRENCH.1080p.BluRay-GROUP");
        assert!(langs.contains(&Language::French));
    }

    #[test]
    fn test_multi() {
        let langs = parse_languages("Show.Name.S01E01.MULTi.1080p.BluRay.x264-GROUP");
        assert!(langs.contains(&Language::Multi));
    }

    #[test]
    fn test_german() {
        let langs = parse_languages("Show.Name.S01E01.GERMAN.720p.HDTV.x264-GROUP");
        assert!(langs.contains(&Language::German));
    }

    #[test]
    fn test_japanese() {
        let langs = parse_languages("Anime.Name.S01E01.JPN.1080p.WEB-DL-GROUP");
        assert!(langs.contains(&Language::Japanese));
    }

    #[test]
    fn test_multiple_languages() {
        let langs = parse_languages("Show.Name.S01E01.MULTi.FRENCH.ENGLISH.1080p.BluRay-GROUP");
        assert!(langs.contains(&Language::Multi));
        assert!(langs.contains(&Language::French));
        assert!(langs.contains(&Language::English));
    }

    #[test]
    fn test_spanish_latino() {
        let langs = parse_languages("Movie.Name.2024.LATINO.1080p.WEB-DL-GROUP");
        assert!(langs.contains(&Language::Spanish));
    }

    #[test]
    fn test_korean() {
        let langs = parse_languages("KDrama.Name.S01E01.KOR.1080p.WEB-DL-GROUP");
        assert!(langs.contains(&Language::Korean));
    }

    #[test]
    fn test_eng_abbreviation() {
        let langs = parse_languages("Show.Name.S01E01.ENG.720p.HDTV.x264-GROUP");
        assert!(langs.contains(&Language::English));
    }

    #[test]
    fn test_fre_abbreviation() {
        let langs = parse_languages("Show.Name.S01E01.FRE.720p.HDTV.x264-GROUP");
        assert!(langs.contains(&Language::French));
    }

    #[test]
    fn test_spa_abbreviation() {
        let langs = parse_languages("Movie.Name.2024.SPA.1080p.BluRay-GROUP");
        assert!(langs.contains(&Language::Spanish));
    }

    #[test]
    fn test_ita_abbreviation() {
        let langs = parse_languages("Movie.Name.2024.ITA.1080p.BluRay-GROUP");
        assert!(langs.contains(&Language::Italian));
    }

    #[test]
    fn test_por_abbreviation() {
        let langs = parse_languages("Movie.Name.2024.POR.1080p.BluRay-GROUP");
        assert!(langs.contains(&Language::Portuguese));
    }

    #[test]
    fn test_ptbr_abbreviation() {
        let langs = parse_languages("Movie.Name.2024.PTBR.1080p.BluRay-GROUP");
        assert!(langs.contains(&Language::Portuguese));
    }

    #[test]
    fn test_chinese_variants() {
        assert!(parse_languages("Show.CHI.720p").contains(&Language::Chinese));
        assert!(parse_languages("Show.CHS.720p").contains(&Language::Chinese));
        assert!(parse_languages("Show.CHT.720p").contains(&Language::Chinese));
        assert!(parse_languages("Show.MANDARIN.720p").contains(&Language::Chinese));
        assert!(parse_languages("Show.CANTONESE.720p").contains(&Language::Chinese));
    }

    #[test]
    fn test_russian_abbreviation() {
        let langs = parse_languages("Movie.Name.2024.RUS.1080p.BluRay-GROUP");
        assert!(langs.contains(&Language::Russian));
    }

    #[test]
    fn test_polish_pl_abbreviation() {
        let langs = parse_languages("Movie.Name.2024.PL.1080p.BluRay-GROUP");
        assert!(langs.contains(&Language::Polish));
    }

    #[test]
    fn test_dutch_variants() {
        assert!(parse_languages("Show.NLD.720p").contains(&Language::Dutch));
        assert!(parse_languages("Show.DUT.720p").contains(&Language::Dutch));
        assert!(parse_languages("Show.FLEMISH.720p").contains(&Language::Dutch));
    }

    #[test]
    fn test_vostfr_is_french() {
        let langs = parse_languages("Movie.Name.2024.VOSTFR.1080p-GROUP");
        assert!(langs.contains(&Language::French));
    }

    #[test]
    fn test_vff_is_french() {
        let langs = parse_languages("Movie.Name.2024.VFF.1080p-GROUP");
        assert!(langs.contains(&Language::French));
    }

    #[test]
    fn test_castellano_is_spanish() {
        let langs = parse_languages("Movie.Name.2024.CASTELLANO.1080p-GROUP");
        assert!(langs.contains(&Language::Spanish));
    }

    #[test]
    fn test_dts_ger_is_german() {
        let langs = parse_languages("Movie.Name.2024.DTS-GER.1080p-GROUP");
        assert!(langs.contains(&Language::German));
    }

    #[test]
    fn test_deutsch_is_german() {
        let langs = parse_languages("Movie.Name.2024.DEUTSCH.1080p-GROUP");
        assert!(langs.contains(&Language::German));
    }

    #[test]
    fn test_lowercase_language() {
        let langs = parse_languages("Show.Name.S01E01.english.720p.HDTV.x264-GROUP");
        assert!(langs.contains(&Language::English));
    }

    #[test]
    fn test_czech_abbreviations() {
        assert!(parse_languages("Show.CZE.720p").contains(&Language::Czech));
        assert!(parse_languages("Show.CES.720p").contains(&Language::Czech));
    }

    #[test]
    fn test_turkish_abbreviation() {
        let langs = parse_languages("Movie.Name.2024.TUR.1080p-GROUP");
        assert!(langs.contains(&Language::Turkish));
    }

    #[test]
    fn test_arabic_abbreviation() {
        let langs = parse_languages("Movie.Name.2024.ARA.1080p-GROUP");
        assert!(langs.contains(&Language::Arabic));
    }

    #[test]
    fn test_hindi_abbreviation() {
        let langs = parse_languages("Movie.Name.2024.HIN.1080p-GROUP");
        assert!(langs.contains(&Language::Hindi));
    }

    #[test]
    fn test_empty_string_returns_unknown() {
        let langs = parse_languages("");
        assert_eq!(langs, vec![Language::Unknown]);
    }
}
