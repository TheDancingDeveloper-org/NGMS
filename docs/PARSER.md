# Parser — Release Name Engine

`ngms-parser` is a zero-dependency (no DB, no async) crate that extracts structured metadata from media release names like `"Show.Name.S01E02.720p.HDTV.x264-GROUP"`.

## Entry Point

```rust
use ngms_parser::{parse_release, ParsedRelease, clean_title};

let result = parse_release("Breaking.Bad.S01E02.720p.BluRay.x264-DEMAND");
// result.title = "Breaking Bad"
// result.quality.quality = Quality::Bluray720p
// result.episode_info.season_number = Some(1)
// result.episode_info.episode_numbers = [2]
// result.release_group = Some("DEMAND")

let clean = clean_title("Breaking Bad");
// clean = "breaking bad"
```

**Note:** `EpisodeInfo` is not re-exported from the crate root. It is accessible as a field on `ParsedRelease.episode_info` (returned by `parse_release()`). If you need the type directly, import it as `ngms_parser::episode::EpisodeInfo`.

## Modules

### title.rs — Title Extraction

`parse_title(name: &str) -> String`

Extracts the title by:
1. Finding the first episode/quality/year marker
2. Taking everything before it
3. Replacing dots, underscores, and hyphens with spaces
4. Trimming whitespace

`clean_title(s: &str) -> String`

Normalizes for database matching:
- Lowercase
- Strip non-alphanumeric characters (except spaces)
- Collapse multiple spaces to single space
- Trim

### episode.rs — Episode Information

`parse_episodes(name: &str) -> EpisodeInfo`

Detected patterns (case-insensitive):

| Pattern | Example | Result |
|---------|---------|--------|
| Standard | `S01E02` | season=1, episodes=[2] |
| Multi-episode | `S01E01E02` | season=1, episodes=[1,2] |
| Episode range | `S01E01-E05` or `S01E01-05` | season=1, episodes=[1,2,3,4,5] |
| Full season | `S01` (no episode) | season=1, is_full_season=true |
| Multi-season | `S01-S03` | season=1, is_multi_season=true |
| Daily | `2024.01.15` or `2024-01-15` | air_date=2024-01-15 |
| Absolute | `042` (standalone number) | absolute_episode_numbers=[42] |
| Special | `S00E01` | season=0, is_special=true |

### quality.rs — Quality Detection

`parse_quality(name: &str) -> QualityModel`

Detection priority (first match wins):

| Keyword | Quality |
|---------|---------|
| `Remux` + `2160p` | Remux2160p |
| `Remux` + `1080p` | Remux1080p |
| `BluRay`/`Blu-Ray` + resolution | Bluray{resolution} |
| `WEB-DL`/`WEBDL` + resolution | WEBDL{resolution} |
| `WEBRip`/`WEB-Rip` + resolution | WEBRip{resolution} |
| `HDTV` + resolution | HDTV{resolution} |
| `DVDRip` | DVD |
| `Raw-HD` | Raw |

Resolution detection: `2160p`/`4K`, `1080p`, `720p`, `480p`. Default resolution assumed from source if not specified.

**Revision detection**:
```rust
pub struct Revision {
    pub version: i32,    // 1=original, 2=PROPER or REPACK
    pub real: i32,       // 1 if REAL tag present
    pub is_repack: bool, // true if REPACK tag
}
```

| Tag | Effect |
|-----|--------|
| `PROPER` | version=2 |
| `REPACK` | version=2, is_repack=true |
| `REAL` | real=1 |

### language.rs — Language Detection

`parse_languages(name: &str) -> Vec<Language>`

Returns `vec![Language::Unknown]` if no language tag found. Supports aliases:

| Tags (case-insensitive) | Language |
|--------------------------|----------|
| `TRUEFRENCH`, `FRENCH`, `VFF` | French |
| `LATINO`, `SPANISH`, `CASTELLANO` | Spanish |
| `GERMAN`, `DEUTSCH` | German |
| `MULTI` | Multi |
| `ENGLISH`, `ENG` | English |
| (27 languages total) | ... |

### release.rs — Main Parser

`parse_release(name: &str) -> ParsedRelease`

Orchestrates all sub-parsers and also extracts:

| Field | Pattern | Example |
|-------|---------|---------|
| `release_group` | Last segment after `-` | `x264-GROUP` → `"GROUP"` |
| `release_hash` | 8-char hex in `[...]` | `[ABCD1234]` → `"ABCD1234"` |
| `year` | 4-digit 1900-2099 | `(2024)` → `2024` |
| `edition` | Edition tags | `Directors.Cut` → `"Director's Cut"` |
| `imdb_id` | `tt` + 7-8 digits | `tt0903747` |

## Usage in Other Crates

| Crate | Uses |
|-------|------|
| `ngms-media` | `clean_title()` on create/update for matching |
| `ngms-import` | `parse_release()` on imported files for quality/language |
| `ngms-indexer` | `parse_release()` on search results for quality info |
| `ngms-quality` | `Quality` enum for profile matching |
| `ngms-migrate` | `clean_title()` for *arr data normalization |

## Testing

Comprehensive inline tests in each module:

```bash
cargo test -p ngms-parser
```

Test categories:
- Standard TV releases (S##E## variations)
- Movie releases with years
- 4K/UHD releases
- Multi-episode files
- Daily show releases
- Anime absolute numbering
- Full season packs
- Quality detection for all source types
- Language tag detection
- Release group extraction
- Edge cases (no quality, no episode info, etc.)
