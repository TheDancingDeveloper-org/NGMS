use std::collections::HashMap;
use std::sync::Mutex;

use regex::Regex;
use serde::{Deserialize, Serialize};

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

// ── Regex cache ────────────────────────────────────────────────────────────

/// Thread-safe cache for compiled regexes keyed by pattern string.
struct RegexCache {
    inner: Mutex<HashMap<String, Regex>>,
}

impl RegexCache {
    fn new() -> Self {
        Self {
            inner: Mutex::new(HashMap::new()),
        }
    }

    fn get_or_compile(&self, pattern: &str) -> Result<Regex, regex::Error> {
        let mut map = self.inner.lock().unwrap();
        if let Some(re) = map.get(pattern) {
            return Ok(re.clone());
        }
        let re = Regex::new(pattern)?;
        map.insert(pattern.to_string(), re.clone());
        Ok(re)
    }
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
    pub fn score_release(
        &self,
        release_title: &str,
        formats: &[CustomFormatDef],
        profile_scores: &[(i64, i32)], // (format_id, score)
    ) -> CustomFormatResult {
        let score_map: HashMap<i64, i32> =
            profile_scores.iter().copied().collect();

        let mut matched_formats = Vec::new();
        let mut total_score: i32 = 0;

        for format in formats {
            if self.matches_format(release_title, format) {
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
    fn matches_format(&self, release_title: &str, format: &CustomFormatDef) -> bool {
        if format.specifications.is_empty() {
            return false;
        }

        let (required, optional): (Vec<&FormatSpec>, Vec<&FormatSpec>) = format
            .specifications
            .iter()
            .partition(|s| s.required);

        // All required specs must match
        for spec in &required {
            if !self.spec_matches(release_title, spec) {
                return false;
            }
        }

        // If there are optional specs, at least one must match
        if !optional.is_empty() {
            let any_optional = optional.iter().any(|s| self.spec_matches(release_title, s));
            if !any_optional {
                return false;
            }
        }

        true
    }

    /// Test whether a single spec matches the release title.
    fn spec_matches(&self, release_title: &str, spec: &FormatSpec) -> bool {
        let raw_match = match spec.field {
            FormatField::ReleaseName => self.regex_matches(release_title, &spec.pattern),
            FormatField::Quality => {
                // Compare the pattern against the release title as a quality string.
                // In a full implementation this would parse the quality from the title
                // and compare, but for now we treat it as a regex against the title.
                self.regex_matches(release_title, &spec.pattern)
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
                // Indexer flags are not in the release title; this would need
                // additional context. For now, treat as not matching.
                false
            }
            FormatField::Size => {
                // Size comparison would need the file size as context.
                // For now, treat as not matching.
                false
            }
        };

        if spec.negate {
            !raw_match
        } else {
            raw_match
        }
    }

    fn regex_matches(&self, text: &str, pattern: &str) -> bool {
        match self.cache.get_or_compile(pattern) {
            Ok(re) => re.is_match(text),
            Err(e) => {
                tracing::warn!(pattern, error = %e, "Invalid regex in custom format spec");
                false
            }
        }
    }
}

/// Extract the release group from a release title.
/// Typically the last segment after a dash, e.g. "Title.S01E01.720p-GROUP" -> "GROUP"
fn extract_release_group(title: &str) -> String {
    // Remove file extension if present
    let base = title
        .rsplit_once('.')
        .filter(|(_, ext)| ext.len() <= 4)
        .map(|(base, _)| base)
        .unwrap_or(title);

    base.rsplit_once('-')
        .map(|(_, group)| group.to_string())
        .unwrap_or_default()
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

        let result = e.score_release(
            "Movie.2024.1080p.BluRay.x264-GROUP",
            &[format],
            &[(1, 100)],
        );
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
        let result = e.score_release(
            "Movie.2024.1080p.REMUX.AVC-GROUP",
            &[format],
            &[(2, 200)],
        );
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
        let result = e.score_release(
            "Movie.2024.1080p.BluRay.x264-GROUP",
            &[format],
            &[(3, 50)],
        );
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

        // Required matches, one optional matches
        let result = e.score_release(
            "Movie.2024.1080p.BluRay.x264-GROUP",
            &[format.clone()],
            &[(4, 75)],
        );
        assert_eq!(result.total_score, 75);

        // Required matches, no optional matches
        let result = e.score_release(
            "Movie.2024.720p.BluRay.x264-GROUP",
            &[format.clone()],
            &[(4, 75)],
        );
        assert_eq!(result.total_score, 0);

        // Required doesn't match
        let result = e.score_release(
            "Movie.2024.1080p.WEB-DL-GROUP",
            &[format],
            &[(4, 75)],
        );
        assert_eq!(result.total_score, 0);
    }

    #[test]
    fn test_release_group_extraction() {
        assert_eq!(extract_release_group("Movie.2024.1080p-GROUP"), "GROUP");
        assert_eq!(
            extract_release_group("Movie.2024.1080p-GROUP.mkv"),
            "GROUP"
        );
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

        let result = e.score_release(
            "Movie.2024.2160p.REMUX-NOGROUP",
            &[format],
            &[(5, 150)],
        );
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
}
