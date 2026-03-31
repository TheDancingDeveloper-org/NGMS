# Quality Parity Test Results

**Date:** 2026-03-27
**NGMS Image:** ghcr.io/ausagentsmith-org/ngms:latest (v0.1.0)
**Sonarr:** v4.0.17.2953
**Radarr:** v6.1.2.10359
**Quality Profile (both):** ProfSync UHD (cutoff=19/Remux-2160p, upgradeAllowed=true, cutoffFormatScore=1499)

## Summary

| Search | Reference | NGMS | Titles Matched | Approval Match | Mismatch Rate |
|--------|-----------|----------|----------------|----------------|---------------|
| Outlander S08E01 (TV) | 53 | 48 | 36 | 3/36 (8%) | **91%** |
| NCIS S23E01 (TV) | 96 | 91 | 63 | 3/63 (5%) | **95%** |
| Anaconda 2025 (Movie) | 404 | 297 | 235 | 35/235 (15%) | **85%** |
| Good Luck Have Fun Don't Die (Movie) | 79 | 64 | 39 | 0/39 (0%) | **100%** |

**Result: FAIL** (21 passed, 4 failed)

All 4 searches fail the 20% approval mismatch threshold. NGMS approves nearly everything; Sonarr/Radarr reject most releases.

## Root Causes

NGMS's decision engine is missing 3 key inputs during release search that Sonarr/Radarr provide:

### 1. No existing file context (217 occurrences)

The release search endpoint (`/api/v1/release`) passes `existing_quality: None` and `existing_custom_format_score: None` to the `DecisionContext`. This means:
- `QualityCutoffSpec` never fires ("Existing file on disk is of equal or higher preference")
- Custom format cutoff checks never fire ("Existing file on disk has equal or higher Custom Format score")

In Sonarr/Radarr, when a series/movie already has a file on disk, releases are rejected if the existing file is already at or above the quality cutoff. This is the single largest source of mismatches.

**Top Sonarr/Radarr rejections NGMS misses:**
```
217x  "Existing file on disk is of equal or higher preference: WEBDL-2160p v1"
 34x  "Existing file on disk has a equal or higher Custom Format score: 400"
 16x  "Existing file on disk has a equal or higher Custom Format score: 2450"
 12x  "Existing file on disk has a equal or higher Custom Format score: 1900"
```

### 2. No custom format scoring (all releases score 0)

The release search endpoint hardcodes `release_custom_format_score: 0` in the `DecisionContext`. Custom formats imported from Sonarr/Radarr (DD+, NF, ProfSync tiers, WEB tiers, SDR, etc.) are not evaluated against release titles.

This means:
- `CustomFormatScoreSpec` ("Custom Formats X have score below minimum") never fires for negative scores
- Releases with LQ custom formats (score -10000) are approved instead of rejected
- Ranking by custom format score is meaningless (all tied at 0)

### 3. No "quality not wanted in profile" checks (232 occurrences)

NGMS's `QualityAllowedSpec` is not rejecting qualities that are disabled in the profile. The ProfSync UHD profile only allows 2160p qualities, but NGMS approves 1080p, 720p, 480p, and SDTV releases.

```
 79x  "WEBDL-1080p is not wanted in profile"
 38x  "Bluray-1080p is not wanted in profile"
 32x  "WEBDL-720p is not wanted in profile"
 19x  "Remux-2160p is not wanted in profile"
 17x  "WEBRip-1080p is not wanted in profile"
 11x  "HDTV-1080p is not wanted in profile"
  8x  "Bluray-720p is not wanted in profile"
  7x  "HDTV-720p is not wanted in profile"
  6x  "WEBRip-480p is not wanted in profile"
  6x  "SDTV is not wanted in profile"
```

This suggests `QualityAllowedSpec` is not parsing the release title to extract quality, or the profile `items` JSON is not being checked correctly.

### 4. No language profile checks (156 occurrences)

Radarr rejects releases where the detected language doesn't match the profile. NGMS has no language filtering.

```
156x  " is wanted, but found English"
 34x  " is wanted, but found German"
 34x  " is wanted, but found French"
```

Note: The " is wanted" with blank before it suggests the Radarr profile has no language set, but it still rejects English — this may be a Radarr quirk with the profile configuration.

### 5. No queue/blocklist awareness (30 occurrences)

```
 30x  "Release in queue is of equal or higher preference: WEBDL-2160p v1"
```

NGMS checks queue by download_id (guid) but Sonarr checks by media item — if any release for the same episode is already in queue at a higher quality, it rejects new ones.

## Release Count Differences

| Search | Only in Reference | Only in NGMS |
|--------|-------------------|------------------|
| Outlander S08E01 | 7 | 1 |
| NCIS S23E01 | 3 | 0 |
| Anaconda 2025 | 71 | 8 |
| GLHF | 16 | 2 |

"Only in Reference" releases come from torrent indexers that NGMS doesn't search (Prowlarr proxied torrent indexers have no direct API in NGMS). "Only in NGMS" are releases found by NGMS's indexers that Sonarr/Radarr didn't return (timing differences).

## Detailed Comparison Reports

Per-release comparison files with APPROVAL_DIFF and ONLY_IN_REF/ONLY_IN_ARZ entries:

- `quality-parity-results/Outlander_S08E01_comparison.txt` (107 lines)
- `quality-parity-results/NCIS_S23E01_comparison.txt` (183 lines)
- `quality-parity-results/Anaconda_2025_comparison.txt` (679 lines)
- `quality-parity-results/GLHF_comparison.txt` (135 lines)

Raw JSON results from both systems saved in `quality-parity-results/`.

## Recommended Fix Priority

1. **QualityAllowedSpec** — Parse quality from release title and check against profile items. This is the most fundamental check and should be straightforward since `ngms-parser` already extracts quality.
2. **Existing file context** — When searching for a specific series/movie, look up existing episode/movie files and pass their quality + CF score to the decision engine.
3. **Custom format scoring** — Evaluate imported custom format rules against release titles to compute `release_custom_format_score`.
4. **Language filtering** — Extract language from release title and compare against profile language preferences.
5. **Queue awareness by media** — Check if the same episode/movie (not just same guid) already has a queued download at equal/higher quality.

## How to Re-run

```bash
cd tests/e2e
SONARR_API_KEY=<key> RADARR_API_KEY=<key> ./test-quality-parity.sh

# Fast re-run (reuse stack):
SONARR_API_KEY=<key> RADARR_API_KEY=<key> SKIP_SETUP=1 KEEP_STACK=1 ./test-quality-parity.sh
```
