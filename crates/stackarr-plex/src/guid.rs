// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 The StackArr Authors

use regex::Regex;
use std::sync::LazyLock;

use crate::types::{ExtractedIds, PlexGuid};

// Compiled regex patterns for extracting external IDs from Plex GUID strings.
static RE_TMDB: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"tmdb://(\d+)").unwrap());
static RE_IMDB: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"imdb://(tt\d+)").unwrap());
static RE_TVDB: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"tvdb://(\d+)").unwrap());
static RE_HAMA_TVDB: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"hama://tvdb\d?-(\d+)").unwrap());
static RE_HAMA_ANIDB: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"hama://anidb\d?-(\d+)").unwrap());

/// Extract TMDB, IMDB, and TVDB IDs from Plex GUID entries.
///
/// Plex stores external IDs in the `Guid` array on metadata items, e.g.:
/// - `tmdb://12345`
/// - `imdb://tt1234567`
/// - `tvdb://67890`
/// - `hama://tvdb3-12345` (anime agents)
pub fn extract_ids(guids: &[PlexGuid]) -> ExtractedIds {
    let mut ids = ExtractedIds::default();

    for guid in guids {
        let s = &guid.id;

        if ids.tmdb_id.is_none()
            && let Some(cap) = RE_TMDB.captures(s)
        {
            ids.tmdb_id = cap[1].parse().ok();
        }
        if ids.imdb_id.is_none()
            && let Some(cap) = RE_IMDB.captures(s)
        {
            ids.imdb_id = Some(cap[1].to_string());
        }
        if ids.tvdb_id.is_none() {
            if let Some(cap) = RE_TVDB.captures(s) {
                ids.tvdb_id = cap[1].parse().ok();
            }
            // Fallback: HAMA agent for anime
            if ids.tvdb_id.is_none()
                && let Some(cap) = RE_HAMA_TVDB.captures(s)
            {
                ids.tvdb_id = cap[1].parse().ok();
            }
        }
    }

    ids
}

/// Extract IDs from the legacy top-level `guid` field (older Plex agents).
/// Format: `com.plexapp.agents.themoviedb://12345?lang=en`
pub fn extract_ids_from_legacy_guid(guid: &str) -> ExtractedIds {
    let mut ids = ExtractedIds::default();

    if guid.contains("themoviedb://") || guid.contains("tmdb://") {
        if let Some(cap) = RE_TMDB
            .captures(guid)
            .or_else(|| Regex::new(r"themoviedb://(\d+)").unwrap().captures(guid))
        {
            ids.tmdb_id = cap[1].parse().ok();
        }
    } else if guid.contains("imdb://") || guid.contains("thetvdb://") {
        if let Some(cap) = RE_IMDB.captures(guid) {
            ids.imdb_id = Some(cap[1].to_string());
        }
        if let Some(cap) = Regex::new(r"thetvdb://(\d+)")
            .unwrap()
            .captures(guid)
            .or_else(|| RE_TVDB.captures(guid))
        {
            ids.tvdb_id = cap[1].parse().ok();
        }
    } else if guid.contains("hama://") {
        if let Some(cap) = RE_HAMA_TVDB.captures(guid) {
            ids.tvdb_id = cap[1].parse().ok();
        }
        if let Some(cap) = RE_HAMA_ANIDB.captures(guid) {
            // Store AniDB in tvdb_id field for now — downstream can map it
            ids.tvdb_id = cap[1].parse().ok();
        }
    }

    ids
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_tmdb_imdb_tvdb() {
        let guids = vec![
            PlexGuid {
                id: "tmdb://550".to_string(),
            },
            PlexGuid {
                id: "imdb://tt0137523".to_string(),
            },
            PlexGuid {
                id: "tvdb://81189".to_string(),
            },
        ];
        let ids = extract_ids(&guids);
        assert_eq!(ids.tmdb_id, Some(550));
        assert_eq!(ids.imdb_id, Some("tt0137523".to_string()));
        assert_eq!(ids.tvdb_id, Some(81189));
    }

    #[test]
    fn test_extract_hama() {
        let guids = vec![PlexGuid {
            id: "hama://tvdb3-12345".to_string(),
        }];
        let ids = extract_ids(&guids);
        assert_eq!(ids.tvdb_id, Some(12345));
    }

    #[test]
    fn test_legacy_guid() {
        let ids = extract_ids_from_legacy_guid("com.plexapp.agents.themoviedb://550?lang=en");
        assert_eq!(ids.tmdb_id, Some(550));
    }

    // ── extract_ids edge cases ─────────────────────────────────────────

    #[test]
    fn test_extract_empty_guids() {
        let ids = extract_ids(&[]);
        assert!(ids.tmdb_id.is_none());
        assert!(ids.imdb_id.is_none());
        assert!(ids.tvdb_id.is_none());
    }

    #[test]
    fn test_extract_tmdb_only() {
        let guids = vec![PlexGuid {
            id: "tmdb://12345".to_string(),
        }];
        let ids = extract_ids(&guids);
        assert_eq!(ids.tmdb_id, Some(12345));
        assert!(ids.imdb_id.is_none());
        assert!(ids.tvdb_id.is_none());
    }

    #[test]
    fn test_extract_imdb_only() {
        let guids = vec![PlexGuid {
            id: "imdb://tt9876543".to_string(),
        }];
        let ids = extract_ids(&guids);
        assert!(ids.tmdb_id.is_none());
        assert_eq!(ids.imdb_id, Some("tt9876543".to_string()));
    }

    #[test]
    fn test_extract_first_value_wins() {
        // When multiple tmdb GUIDs exist, first one wins
        let guids = vec![
            PlexGuid {
                id: "tmdb://111".to_string(),
            },
            PlexGuid {
                id: "tmdb://222".to_string(),
            },
        ];
        let ids = extract_ids(&guids);
        assert_eq!(ids.tmdb_id, Some(111));
    }

    #[test]
    fn test_extract_hama_anidb() {
        let guids = vec![PlexGuid {
            id: "hama://anidb2-9876".to_string(),
        }];
        let ids = extract_ids(&guids);
        // anidb stored in tvdb_id field for now
        assert_eq!(ids.tvdb_id, None); // hama anidb only works in legacy
    }

    #[test]
    fn test_extract_hama_tvdb_variant() {
        let guids = vec![PlexGuid {
            id: "hama://tvdb-54321".to_string(),
        }];
        let ids = extract_ids(&guids);
        assert_eq!(ids.tvdb_id, Some(54321));
    }

    #[test]
    fn test_extract_non_matching_guid() {
        let guids = vec![PlexGuid {
            id: "local://12345".to_string(),
        }];
        let ids = extract_ids(&guids);
        assert!(ids.tmdb_id.is_none());
        assert!(ids.imdb_id.is_none());
        assert!(ids.tvdb_id.is_none());
    }

    // ── legacy guid formats ────────────────────────────────────────────

    #[test]
    fn test_legacy_guid_imdb() {
        let ids = extract_ids_from_legacy_guid("com.plexapp.agents.imdb://tt1234567?lang=en");
        assert_eq!(ids.imdb_id, Some("tt1234567".to_string()));
    }

    #[test]
    fn test_legacy_guid_thetvdb() {
        let ids = extract_ids_from_legacy_guid("com.plexapp.agents.thetvdb://81189?lang=en");
        assert_eq!(ids.tvdb_id, Some(81189));
    }

    #[test]
    fn test_legacy_guid_hama_tvdb() {
        let ids = extract_ids_from_legacy_guid("hama://tvdb3-12345");
        assert_eq!(ids.tvdb_id, Some(12345));
    }

    #[test]
    fn test_legacy_guid_hama_anidb() {
        let ids = extract_ids_from_legacy_guid("hama://anidb-9999");
        assert_eq!(ids.tvdb_id, Some(9999)); // stored in tvdb_id
    }

    #[test]
    fn test_legacy_guid_unknown_agent() {
        let ids = extract_ids_from_legacy_guid("com.plexapp.agents.xbmc://12345");
        assert!(ids.tmdb_id.is_none());
        assert!(ids.imdb_id.is_none());
        assert!(ids.tvdb_id.is_none());
    }
}
