use std::collections::HashMap;
use std::sync::Mutex;

use regex::Regex;
use serde::{Deserialize, Serialize};
use stackarr_parser::quality::parse_quality;

// ── Types ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CustomFormatDef {
    pub id: i64,
    pub name: String,
    pub specifications: Vec<FormatSpec>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FormatSpec {
    pub field: FormatField,
    pub pattern: String,
    pub negate: bool,
    pub required: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum FormatField {
    ReleaseName,
    Quality,
    Language,
    ReleaseGroup,
    IndexerFlag,
    Size,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CustomFormatResult {
    pub total_score: i32,
    pub matched_formats: Vec<MatchedFormat>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MatchedFormat {
    pub format_id: i64,
    pub format_name: String,
    pub score: i32,
}

/// Optional extra context for scoring a release beyond just the title.
#[derive(Debug, Clone, Default)]
pub struct ReleaseContext<'a> {
    /// Indexer flags associated with the release (e.g. "freeleech", "internal").
    pub indexer_flags: &'a [String],
    /// File size in bytes, if known.
    pub size_bytes: Option<u64>,
}

// ── Regex cache ────────────────────────────────────────────────────────────

/// Compiled regex — either a fast `regex::Regex` or a `fancy_regex::Regex`
/// (the latter supports lookahead/lookbehind assertions).
#[derive(Clone)]
enum CompiledRegex {
    Standard(Regex),
    Fancy(fancy_regex::Regex),
}

impl CompiledRegex {
    fn is_match(&self, text: &str) -> bool {
        match self {
            Self::Standard(re) => re.is_match(text),
            Self::Fancy(re) => re.is_match(text).unwrap_or(false),
        }
    }
}

/// Thread-safe cache for compiled regexes keyed by pattern string.
struct RegexCache {
    inner: Mutex<HashMap<String, CompiledRegex>>,
}

impl RegexCache {
    fn new() -> Self {
        Self {
            inner: Mutex::new(HashMap::new()),
        }
    }

    fn get_or_compile(&self, pattern: &str) -> Result<CompiledRegex, String> {
        let mut map = self.inner.lock().unwrap();
        if let Some(re) = map.get(pattern) {
            return Ok(re.clone());
        }

        // Normalize the pattern: strip JS-style /pattern/flags delimiters
        // and convert flags to inline syntax.
        let normalized = normalize_regex_pattern(pattern);

        // Try the fast regex crate first (no backtracking, but no lookaheads)
        let compiled = match Regex::new(&normalized) {
            Ok(re) => CompiledRegex::Standard(re),
            Err(_) => {
                // Fall back to fancy-regex which supports lookaheads/lookbehinds
                match fancy_regex::Regex::new(&normalized) {
                    Ok(re) => CompiledRegex::Fancy(re),
                    Err(e) => return Err(format!("{e}")),
                }
            }
        };

        map.insert(pattern.to_string(), compiled.clone());
        Ok(compiled)
    }
}

/// Normalize regex patterns from Sonarr/Radarr format to Rust regex syntax.
///
/// Handles:
/// - JS-style `/pattern/flags` → strip delimiters, convert flags to `(?flags)`
/// - Case-insensitive flag `i` → `(?i)` prefix
fn normalize_regex_pattern(pattern: &str) -> String {
    let trimmed = pattern.trim();

    // Check for JS/C#-style regex delimiters: /pattern/flags
    if let Some(after_slash) = trimmed.strip_prefix('/')
        && let Some(last_slash) = after_slash.rfind('/')
    {
        let inner = &after_slash[..last_slash];
        let flags = &after_slash[last_slash + 1..];

        let mut prefix = String::new();
        if flags.contains('i') {
            prefix.push_str("(?i)");
        }

        return format!("{prefix}{inner}");
    }

    trimmed.to_string()
}

// ── Engine ─────────────────────────────────────────────────────────────────

pub struct CustomFormatEngine {
    cache: RegexCache,
}

impl Default for CustomFormatEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl CustomFormatEngine {
    pub fn new() -> Self {
        Self {
            cache: RegexCache::new(),
        }
    }

    /// Score a release against all custom formats.
    /// Returns list of matched format IDs with their names.
    ///
    /// This is a convenience wrapper around [`score_release_with_context`] that
    /// uses default (empty) context. Prefer the `_with_context` variant when
    /// indexer flags or file size are available.
    pub fn score_release(
        &self,
        release_title: &str,
        formats: &[CustomFormatDef],
        profile_scores: &[(i64, i32)], // (format_id, score)
    ) -> CustomFormatResult {
        self.score_release_with_context(
            release_title,
            formats,
            profile_scores,
            &ReleaseContext::default(),
        )
    }

    /// Score a release against all custom formats with additional context.
    pub fn score_release_with_context(
        &self,
        release_title: &str,
        formats: &[CustomFormatDef],
        profile_scores: &[(i64, i32)],
        context: &ReleaseContext<'_>,
    ) -> CustomFormatResult {
        let score_map: HashMap<i64, i32> = profile_scores.iter().copied().collect();

        let mut matched_formats = Vec::new();
        let mut total_score: i32 = 0;

        for format in formats {
            if self.matches_format(release_title, format, context) {
                let score = score_map.get(&format.id).copied().unwrap_or(0);
                total_score = total_score.saturating_add(score);
                matched_formats.push(MatchedFormat {
                    format_id: format.id,
                    format_name: format.name.clone(),
                    score,
                });
            }
        }

        CustomFormatResult {
            total_score,
            matched_formats,
        }
    }

    /// Check if a release matches a single custom format definition.
    ///
    /// Rules:
    /// - ALL required specs must match
    /// - If there are any non-required specs, at least one must match
    fn matches_format(
        &self,
        release_title: &str,
        format: &CustomFormatDef,
        context: &ReleaseContext<'_>,
    ) -> bool {
        if format.specifications.is_empty() {
            return false;
        }

        let (required, optional): (Vec<&FormatSpec>, Vec<&FormatSpec>) =
            format.specifications.iter().partition(|s| s.required);

        // All required specs must match
        for spec in &required {
            if !self.spec_matches(release_title, spec, context) {
                return false;
            }
        }

        // If there are optional specs, at least one must match
        if !optional.is_empty() {
            let any_optional = optional
                .iter()
                .any(|s| self.spec_matches(release_title, s, context));
            if !any_optional {
                return false;
            }
        }

        true
    }

    /// Test whether a single spec matches the release title.
    fn spec_matches(
        &self,
        release_title: &str,
        spec: &FormatSpec,
        context: &ReleaseContext<'_>,
    ) -> bool {
        let raw_match = match spec.field {
            FormatField::ReleaseName => self.regex_matches(release_title, &spec.pattern),
            FormatField::Quality => {
                // Parse the quality from the release title and match the spec
                // pattern (as a regex) against the quality variant name.
                let quality_model = parse_quality(release_title);
                let quality_name = format!("{:?}", quality_model.quality);
                self.regex_matches(&quality_name, &spec.pattern)
            }
            FormatField::ReleaseGroup => {
                // Extract release group (last segment after the last dash) and match
                let group = extract_release_group(release_title);
                self.regex_matches(&group, &spec.pattern)
            }
            FormatField::Language => {
                // Match language tokens in the release title
                self.regex_matches(release_title, &spec.pattern)
            }
            FormatField::IndexerFlag => {
                // Match the spec pattern against each indexer flag from context.
                if context.indexer_flags.is_empty() {
                    false
                } else {
                    context
                        .indexer_flags
                        .iter()
                        .any(|flag| self.regex_matches(flag, &spec.pattern))
                }
            }
            FormatField::Size => {
                // Pattern format: "min-max" in bytes (e.g. "0-5368709120" for 0-5GB).
                // Matches when size_bytes falls within the range [min, max].
                match context.size_bytes {
                    None => false,
                    Some(size) => parse_size_range(&spec.pattern)
                        .map(|(min, max)| size >= min && size <= max)
                        .unwrap_or(false),
                }
            }
        };

        if spec.negate { !raw_match } else { raw_match }
    }

    fn regex_matches(&self, text: &str, pattern: &str) -> bool {
        match self.cache.get_or_compile(pattern) {
            Ok(compiled) => compiled.is_match(text),
            Err(e) => {
                tracing::warn!(pattern, error = %e, "Invalid regex in custom format spec");
                false
            }
        }
    }
}

/// Parse a size range pattern in the format "min-max" (both in bytes).
/// Returns `Some((min, max))` on success, `None` on parse failure.
fn parse_size_range(pattern: &str) -> Option<(u64, u64)> {
    let (min_s, max_s) = pattern.split_once('-')?;
    let min: u64 = min_s.trim().parse().ok()?;
    let max: u64 = max_s.trim().parse().ok()?;
    Some((min, max))
}

/// Extract the release group from a release title.
/// Typically the last segment after a dash, e.g. "Title.S01E01.720p-GROUP" -> "GROUP"
fn extract_release_group(title: &str) -> String {
    // Remove file extension if present (extensions are <= 4 chars)
    let base = title
        .rsplit_once('.')
        .filter(|(_, ext)| ext.len() <= 4)
        .map(|(base, _)| base)
        .unwrap_or(title);

    // Extract the release group (last segment after a dash)
    let group = base.rsplit_once('-').map(|(_, group)| group).unwrap_or("");

    // If the group still contains a dot with a long suffix (> 4 chars),
    // strip that suffix too (e.g. "GROUP.torrent" -> "GROUP")
    group
        .rsplit_once('.')
        .filter(|(_, ext)| ext.len() > 4)
        .map(|(name, _)| name.to_string())
        .unwrap_or_else(|| group.to_string())
}

// ── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn engine() -> CustomFormatEngine {
        CustomFormatEngine::new()
    }

    #[test]
    fn test_single_required_spec_matches() {
        let e = engine();
        let format = CustomFormatDef {
            id: 1,
            name: "Remux".to_string(),
            specifications: vec![FormatSpec {
                field: FormatField::ReleaseName,
                pattern: r"\b(REMUX)\b".to_string(),
                negate: false,
                required: true,
            }],
        };

        let result = e.score_release(
            "Movie.2024.1080p.REMUX.AVC.DTS-HD-GROUP",
            &[format.clone()],
            &[(1, 100)],
        );
        assert_eq!(result.total_score, 100);
        assert_eq!(result.matched_formats.len(), 1);
        assert_eq!(result.matched_formats[0].format_name, "Remux");
    }

    #[test]
    fn test_single_required_spec_no_match() {
        let e = engine();
        let format = CustomFormatDef {
            id: 1,
            name: "Remux".to_string(),
            specifications: vec![FormatSpec {
                field: FormatField::ReleaseName,
                pattern: r"\b(REMUX)\b".to_string(),
                negate: false,
                required: true,
            }],
        };

        let result = e.score_release("Movie.2024.1080p.BluRay.x264-GROUP", &[format], &[(1, 100)]);
        assert_eq!(result.total_score, 0);
        assert!(result.matched_formats.is_empty());
    }

    #[test]
    fn test_multiple_required_specs_all_must_match() {
        let e = engine();
        let format = CustomFormatDef {
            id: 2,
            name: "4K Remux".to_string(),
            specifications: vec![
                FormatSpec {
                    field: FormatField::ReleaseName,
                    pattern: r"\b(REMUX)\b".to_string(),
                    negate: false,
                    required: true,
                },
                FormatSpec {
                    field: FormatField::ReleaseName,
                    pattern: r"2160p".to_string(),
                    negate: false,
                    required: true,
                },
            ],
        };

        // Both match
        let result = e.score_release(
            "Movie.2024.2160p.REMUX.HEVC-GROUP",
            &[format.clone()],
            &[(2, 200)],
        );
        assert_eq!(result.total_score, 200);
        assert_eq!(result.matched_formats.len(), 1);

        // Only one matches
        let result = e.score_release("Movie.2024.1080p.REMUX.AVC-GROUP", &[format], &[(2, 200)]);
        assert_eq!(result.total_score, 0);
        assert!(result.matched_formats.is_empty());
    }

    #[test]
    fn test_negate_spec() {
        let e = engine();
        let format = CustomFormatDef {
            id: 3,
            name: "Not x265".to_string(),
            specifications: vec![FormatSpec {
                field: FormatField::ReleaseName,
                pattern: r"(?i)\bx265\b".to_string(),
                negate: true,
                required: true,
            }],
        };

        // x265 present -> negated -> no match
        let result = e.score_release(
            "Movie.2024.1080p.BluRay.x265-GROUP",
            &[format.clone()],
            &[(3, 50)],
        );
        assert_eq!(result.total_score, 0);

        // x265 absent -> negated -> match
        let result = e.score_release("Movie.2024.1080p.BluRay.x264-GROUP", &[format], &[(3, 50)]);
        assert_eq!(result.total_score, 50);
    }

    #[test]
    fn test_score_with_multiple_matching_formats() {
        let e = engine();
        let formats = vec![
            CustomFormatDef {
                id: 1,
                name: "Remux".to_string(),
                specifications: vec![FormatSpec {
                    field: FormatField::ReleaseName,
                    pattern: r"\bREMUX\b".to_string(),
                    negate: false,
                    required: true,
                }],
            },
            CustomFormatDef {
                id: 2,
                name: "4K".to_string(),
                specifications: vec![FormatSpec {
                    field: FormatField::ReleaseName,
                    pattern: r"2160p".to_string(),
                    negate: false,
                    required: true,
                }],
            },
        ];

        let result = e.score_release(
            "Movie.2024.2160p.REMUX.HEVC-GROUP",
            &formats,
            &[(1, 100), (2, 50)],
        );
        assert_eq!(result.total_score, 150);
        assert_eq!(result.matched_formats.len(), 2);
    }

    #[test]
    fn test_empty_format_list_returns_zero() {
        let e = engine();
        let result = e.score_release("Some.Release.Title", &[], &[]);
        assert_eq!(result.total_score, 0);
        assert!(result.matched_formats.is_empty());
    }

    #[test]
    fn test_required_and_optional_specs() {
        let e = engine();
        // Quality patterns match against the parsed Quality variant name
        // (e.g. "Bluray1080p", "Bluray2160p", "Bluray720p").
        let format = CustomFormatDef {
            id: 4,
            name: "BluRay+Quality".to_string(),
            specifications: vec![
                FormatSpec {
                    field: FormatField::ReleaseName,
                    pattern: r"(?i)\bBluRay\b".to_string(),
                    negate: false,
                    required: true,
                },
                FormatSpec {
                    field: FormatField::Quality,
                    pattern: r"1080p".to_string(),
                    negate: false,
                    required: false,
                },
                FormatSpec {
                    field: FormatField::Quality,
                    pattern: r"2160p".to_string(),
                    negate: false,
                    required: false,
                },
            ],
        };

        // Required matches, one optional matches (Bluray1080p contains "1080p")
        let result = e.score_release(
            "Movie.2024.1080p.BluRay.x264-GROUP",
            &[format.clone()],
            &[(4, 75)],
        );
        assert_eq!(result.total_score, 75);

        // Required matches, no optional matches (Bluray720p doesn't contain "1080p" or "2160p")
        let result = e.score_release(
            "Movie.2024.720p.BluRay.x264-GROUP",
            &[format.clone()],
            &[(4, 75)],
        );
        assert_eq!(result.total_score, 0);

        // Required doesn't match
        let result = e.score_release("Movie.2024.1080p.WEB-DL-GROUP", &[format], &[(4, 75)]);
        assert_eq!(result.total_score, 0);
    }

    #[test]
    fn test_release_group_extraction() {
        assert_eq!(extract_release_group("Movie.2024.1080p-GROUP"), "GROUP");
        assert_eq!(extract_release_group("Movie.2024.1080p-GROUP.mkv"), "GROUP");
        assert_eq!(extract_release_group("NoGroup"), "");
    }

    #[test]
    fn test_release_group_spec() {
        let e = engine();
        let format = CustomFormatDef {
            id: 5,
            name: "Preferred Group".to_string(),
            specifications: vec![FormatSpec {
                field: FormatField::ReleaseGroup,
                pattern: r"^(FraMeSToR|EPSiLON)$".to_string(),
                negate: false,
                required: true,
            }],
        };

        let result = e.score_release(
            "Movie.2024.2160p.REMUX-FraMeSToR",
            &[format.clone()],
            &[(5, 150)],
        );
        assert_eq!(result.total_score, 150);

        let result = e.score_release("Movie.2024.2160p.REMUX-NOGROUP", &[format], &[(5, 150)]);
        assert_eq!(result.total_score, 0);
    }

    #[test]
    fn test_format_no_score_in_profile() {
        let e = engine();
        let format = CustomFormatDef {
            id: 99,
            name: "Unscored".to_string(),
            specifications: vec![FormatSpec {
                field: FormatField::ReleaseName,
                pattern: r".*".to_string(),
                negate: false,
                required: true,
            }],
        };

        // Format matches but has no score entry -> defaults to 0
        let result = e.score_release("anything", &[format], &[]);
        assert_eq!(result.total_score, 0);
        assert_eq!(result.matched_formats.len(), 1);
        assert_eq!(result.matched_formats[0].score, 0);
    }

    #[test]
    fn test_empty_specifications_never_match() {
        let e = engine();
        let format = CustomFormatDef {
            id: 1,
            name: "Empty".to_string(),
            specifications: vec![],
        };
        let result = e.score_release("anything", &[format], &[(1, 100)]);
        assert_eq!(result.total_score, 0);
        assert!(result.matched_formats.is_empty());
    }

    #[test]
    fn test_invalid_regex_no_panic() {
        let e = engine();
        let format = CustomFormatDef {
            id: 1,
            name: "Bad Regex".to_string(),
            specifications: vec![FormatSpec {
                field: FormatField::ReleaseName,
                pattern: r"[invalid".to_string(),
                negate: false,
                required: true,
            }],
        };
        let result = e.score_release("test", &[format], &[(1, 50)]);
        // Invalid regex should not match, not panic
        assert_eq!(result.total_score, 0);
    }

    #[test]
    fn test_negated_optional_spec() {
        let e = engine();
        let format = CustomFormatDef {
            id: 1,
            name: "Not HDR".to_string(),
            specifications: vec![
                FormatSpec {
                    field: FormatField::ReleaseName,
                    pattern: r"1080p".to_string(),
                    negate: false,
                    required: true,
                },
                FormatSpec {
                    field: FormatField::ReleaseName,
                    pattern: r"(?i)\bHDR\b".to_string(),
                    negate: true,
                    required: false,
                },
            ],
        };
        // Has 1080p, doesn't have HDR → negated optional matches
        let result = e.score_release(
            "Movie.2024.1080p.BluRay-GROUP",
            &[format.clone()],
            &[(1, 50)],
        );
        assert_eq!(result.total_score, 50);

        // Has 1080p, has HDR → negated optional doesn't match
        let result = e.score_release("Movie.2024.1080p.HDR.BluRay-GROUP", &[format], &[(1, 50)]);
        assert_eq!(result.total_score, 0);
    }

    #[test]
    fn test_negative_scores() {
        let e = engine();
        let format = CustomFormatDef {
            id: 1,
            name: "Low Quality".to_string(),
            specifications: vec![FormatSpec {
                field: FormatField::ReleaseName,
                pattern: r"(?i)\bCAM\b".to_string(),
                negate: false,
                required: true,
            }],
        };
        let result = e.score_release("Movie.2024.CAM.x264-GROUP", &[format], &[(1, -10000)]);
        assert_eq!(result.total_score, -10000);
    }

    #[test]
    fn test_saturating_score_addition() {
        let e = engine();
        let formats = vec![
            CustomFormatDef {
                id: 1,
                name: "F1".to_string(),
                specifications: vec![FormatSpec {
                    field: FormatField::ReleaseName,
                    pattern: r".*".to_string(),
                    negate: false,
                    required: true,
                }],
            },
            CustomFormatDef {
                id: 2,
                name: "F2".to_string(),
                specifications: vec![FormatSpec {
                    field: FormatField::ReleaseName,
                    pattern: r".*".to_string(),
                    negate: false,
                    required: true,
                }],
            },
        ];
        let result = e.score_release("anything", &formats, &[(1, i32::MAX), (2, 1)]);
        // Should saturate, not overflow
        assert_eq!(result.total_score, i32::MAX);
    }

    #[test]
    fn test_release_group_extraction_no_extension() {
        assert_eq!(extract_release_group("Movie.2024.1080p-GROUP"), "GROUP");
    }

    #[test]
    fn test_release_group_extraction_long_ext_not_stripped() {
        // Extension > 4 chars shouldn't be stripped
        assert_eq!(
            extract_release_group("Movie.2024.1080p-GROUP.torrent"),
            "GROUP"
        );
    }

    #[test]
    fn test_release_group_extraction_no_dash() {
        assert_eq!(extract_release_group("NoDashHere"), "");
    }

    #[test]
    fn test_release_group_spec_matching() {
        let e = engine();
        let format = CustomFormatDef {
            id: 1,
            name: "Scene".to_string(),
            specifications: vec![FormatSpec {
                field: FormatField::ReleaseGroup,
                pattern: r"^(LOL|KILLERS|DEMAND)$".to_string(),
                negate: false,
                required: true,
            }],
        };
        let result = e.score_release("Show.S01E01.720p.HDTV-LOL", &[format.clone()], &[(1, 25)]);
        assert_eq!(result.total_score, 25);

        let result = e.score_release("Show.S01E01.720p.HDTV-UNKNOWN", &[format], &[(1, 25)]);
        assert_eq!(result.total_score, 0);
    }

    #[test]
    fn test_indexer_flag_matches_with_context() {
        let e = engine();
        let format = CustomFormatDef {
            id: 1,
            name: "Freeleech".to_string(),
            specifications: vec![FormatSpec {
                field: FormatField::IndexerFlag,
                pattern: r"(?i)freeleech".to_string(),
                negate: false,
                required: true,
            }],
        };

        // No flags -> no match
        let result = e.score_release("any.release", &[format.clone()], &[(1, 50)]);
        assert_eq!(result.total_score, 0);

        // With matching flag -> match
        let flags = vec!["freeleech".to_string()];
        let ctx = ReleaseContext {
            indexer_flags: &flags,
            size_bytes: None,
        };
        let result =
            e.score_release_with_context("any.release", &[format.clone()], &[(1, 50)], &ctx);
        assert_eq!(result.total_score, 50);

        // With non-matching flag -> no match
        let flags = vec!["internal".to_string()];
        let ctx = ReleaseContext {
            indexer_flags: &flags,
            size_bytes: None,
        };
        let result = e.score_release_with_context("any.release", &[format], &[(1, 50)], &ctx);
        assert_eq!(result.total_score, 0);
    }

    #[test]
    fn test_size_field_matches_within_range() {
        let e = engine();
        // Pattern: 0 to 5 GB (5368709120 bytes)
        let format = CustomFormatDef {
            id: 1,
            name: "Small".to_string(),
            specifications: vec![FormatSpec {
                field: FormatField::Size,
                pattern: "0-5368709120".to_string(),
                negate: false,
                required: true,
            }],
        };

        // No size context -> no match
        let result = e.score_release("any.release", &[format.clone()], &[(1, 50)]);
        assert_eq!(result.total_score, 0);

        // Size within range -> match
        let ctx = ReleaseContext {
            indexer_flags: &[],
            size_bytes: Some(1_000_000_000),
        };
        let result =
            e.score_release_with_context("any.release", &[format.clone()], &[(1, 50)], &ctx);
        assert_eq!(result.total_score, 50);

        // Size exactly at max -> match
        let ctx = ReleaseContext {
            indexer_flags: &[],
            size_bytes: Some(5_368_709_120),
        };
        let result =
            e.score_release_with_context("any.release", &[format.clone()], &[(1, 50)], &ctx);
        assert_eq!(result.total_score, 50);

        // Size over max -> no match
        let ctx = ReleaseContext {
            indexer_flags: &[],
            size_bytes: Some(6_000_000_000),
        };
        let result = e.score_release_with_context("any.release", &[format], &[(1, 50)], &ctx);
        assert_eq!(result.total_score, 0);
    }

    #[test]
    fn test_language_field_matching() {
        let e = engine();
        let format = CustomFormatDef {
            id: 1,
            name: "English Only".to_string(),
            specifications: vec![FormatSpec {
                field: FormatField::Language,
                pattern: r"(?i)\bENGLISH\b".to_string(),
                negate: false,
                required: true,
            }],
        };
        let result = e.score_release(
            "Movie.2024.ENGLISH.1080p.BluRay-GROUP",
            &[format],
            &[(1, 30)],
        );
        assert_eq!(result.total_score, 30);
    }

    #[test]
    fn test_regex_cache_reuse() {
        let e = engine();
        let format = CustomFormatDef {
            id: 1,
            name: "Test".to_string(),
            specifications: vec![FormatSpec {
                field: FormatField::ReleaseName,
                pattern: r"\btest\b".to_string(),
                negate: false,
                required: true,
            }],
        };
        // Call twice - second call should use cached regex
        let r1 = e.score_release("test release", &[format.clone()], &[(1, 10)]);
        let r2 = e.score_release("test again", &[format], &[(1, 10)]);
        assert_eq!(r1.total_score, 10);
        assert_eq!(r2.total_score, 10);
    }

    // ── Fancy regex (lookahead) tests ─────────────────────────────────

    #[test]
    fn test_negative_lookahead_dv_without_hdr() {
        // Sonarr/Radarr "DV without HDR fallback" custom format pattern
        let e = engine();
        let format = CustomFormatDef {
            id: 1,
            name: "DV without HDR".to_string(),
            specifications: vec![FormatSpec {
                field: FormatField::ReleaseName,
                pattern: r"(?i)^(?!.*(HDR|HULU|REMUX))(?=.*\b(DV|Dovi|Dolby[- .]?Vision)\b).*"
                    .to_string(),
                negate: false,
                required: true,
            }],
        };

        // DV without HDR → should match
        let result = e.score_release(
            "Movie.2024.2160p.WEB-DL.DV.H265-GROUP",
            &[format.clone()],
            &[(1, -10000)],
        );
        assert_eq!(result.total_score, -10000);

        // DV with HDR → negative lookahead excludes → no match
        let result = e.score_release(
            "Movie.2024.2160p.WEB-DL.DV.HDR.H265-GROUP",
            &[format.clone()],
            &[(1, -10000)],
        );
        assert_eq!(result.total_score, 0);

        // DV with REMUX → negative lookahead excludes → no match
        let result = e.score_release(
            "Movie.2024.2160p.REMUX.DV.HEVC-GROUP",
            &[format.clone()],
            &[(1, -10000)],
        );
        assert_eq!(result.total_score, 0);

        // No DV → positive lookahead fails → no match
        let result = e.score_release(
            "Movie.2024.2160p.WEB-DL.HDR.H265-GROUP",
            &[format],
            &[(1, -10000)],
        );
        assert_eq!(result.total_score, 0);
    }

    #[test]
    fn test_js_style_regex_delimiters() {
        // Sonarr stores patterns with JS-style /pattern/flags syntax
        let e = engine();
        let format = CustomFormatDef {
            id: 1,
            name: "Case Insensitive".to_string(),
            specifications: vec![FormatSpec {
                field: FormatField::ReleaseName,
                pattern: r"/\b(remux)\b/i".to_string(),
                negate: false,
                required: true,
            }],
        };

        // Uppercase REMUX → should match due to /i flag
        let result = e.score_release(
            "Movie.2024.1080p.REMUX.AVC-GROUP",
            &[format.clone()],
            &[(1, 100)],
        );
        assert_eq!(result.total_score, 100);

        // Lowercase remux → should also match
        let result = e.score_release("Movie.2024.1080p.remux.AVC-GROUP", &[format], &[(1, 100)]);
        assert_eq!(result.total_score, 100);
    }

    #[test]
    fn test_js_regex_with_lookahead_and_case_insensitive() {
        // Full Sonarr-style pattern with JS delimiters and lookahead
        let e = engine();
        let format = CustomFormatDef {
            id: 1,
            name: "DV no fallback".to_string(),
            specifications: vec![FormatSpec {
                field: FormatField::ReleaseName,
                pattern: r"/^(?!.*(HDR|HULU|REMUX))(?=.*\b(DV|Dovi|Dolby[- .]?Vision)\b).*/i"
                    .to_string(),
                negate: false,
                required: true,
            }],
        };

        // dv (lowercase) without hdr → should match (case insensitive)
        let result = e.score_release(
            "Movie.2024.2160p.WEB-DL.dv.H265-GROUP",
            &[format.clone()],
            &[(1, -10000)],
        );
        assert_eq!(result.total_score, -10000);

        // DV with Hdr → should NOT match
        let result = e.score_release(
            "Movie.2024.2160p.WEB-DL.DV.Hdr.H265-GROUP",
            &[format],
            &[(1, -10000)],
        );
        assert_eq!(result.total_score, 0);
    }

    #[test]
    fn test_normalize_regex_pattern() {
        assert_eq!(normalize_regex_pattern(r"/test/i"), "(?i)test");
        assert_eq!(normalize_regex_pattern(r"/foo\/bar/"), "foo\\/bar");
        assert_eq!(normalize_regex_pattern(r"plain pattern"), "plain pattern");
        assert_eq!(
            normalize_regex_pattern(r"/^(?!.*HDR).*DV.*/i"),
            "(?i)^(?!.*HDR).*DV.*"
        );
    }
}
