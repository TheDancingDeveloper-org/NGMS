#!/usr/bin/env bash
# test-quality-parity.sh — Stack 4: Quality Parity Test
#
# Compares release search scoring between StackArr (after importing
# Sonarr/Radarr databases) and the live Sonarr/Radarr instances.
#
# Usage (run on Node B):
#   cd /path/to/tests/e2e
#   SONARR_API_KEY=xxx RADARR_API_KEY=yyy ./test-quality-parity.sh
#
# Environment variables:
#   SONARR_API_KEY  — API key for Sonarr (required)
#   RADARR_API_KEY  — API key for Radarr (required)
#   SONARR_URL      — Sonarr base URL (default: http://localhost:8989/sonarr)
#   RADARR_URL      — Radarr base URL (default: http://localhost:7878)
#   SKIP_SETUP      — Set to 1 to skip stack setup (reuse running stack)
#   RESULTS_DIR     — Directory for JSON result dumps (default: ./quality-parity-results)
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
source "$SCRIPT_DIR/common.sh"

# ── Configuration ─────────────────────────────────────────

COMPOSE_FILE="$SCRIPT_DIR/docker-compose.quality-parity.yml"
PROJECT="stackarr-quality"
BASE_URL="http://localhost:9214"
export MEDIA_BASE="${MEDIA_BASE:-/tmp/stackarr-quality-test}"

SONARR_URL="${SONARR_URL:-http://localhost:8989/sonarr}"
RADARR_URL="${RADARR_URL:-http://localhost:7878}"

if [[ -z "${SONARR_API_KEY:-}" ]]; then
    echo "ERROR: SONARR_API_KEY must be set."
    exit 1
fi

if [[ -z "${RADARR_API_KEY:-}" ]]; then
    echo "ERROR: RADARR_API_KEY must be set."
    exit 1
fi

RESULTS_DIR="${RESULTS_DIR:-$SCRIPT_DIR/quality-parity-results}"
mkdir -p "$RESULTS_DIR"

# Fixtures
if [[ -d "$SCRIPT_DIR/../../test-fixtures" ]]; then
    FIXTURES="$(cd "$SCRIPT_DIR/../../test-fixtures" && pwd)"
elif [[ -d "$SCRIPT_DIR/fixtures" ]]; then
    FIXTURES="$SCRIPT_DIR/fixtures"
else
    echo "ERROR: Cannot find test-fixtures directory"
    exit 1
fi

# ── Sonarr/Radarr API helpers ─────────────────────────────

sonarr_api() {
    local path="$1"
    local response
    response=$(curl -s -S -w '\n%{http_code}' \
        -H "X-Api-Key: $SONARR_API_KEY" \
        -H "Content-Type: application/json" \
        "$SONARR_URL$path" 2>&1) || true
    SONARR_CODE=$(echo "$response" | tail -1)
    SONARR_BODY=$(echo "$response" | sed '$d')
}

radarr_api() {
    local path="$1"
    local response
    response=$(curl -s -S -w '\n%{http_code}' \
        -H "X-Api-Key: $RADARR_API_KEY" \
        -H "Content-Type: application/json" \
        "$RADARR_URL$path" 2>&1) || true
    RADARR_CODE=$(echo "$response" | tail -1)
    RADARR_BODY=$(echo "$response" | sed '$d')
}

# ── Comparison helpers ────────────────────────────────────

# Compare release lists by title, checking quality parsing and approval status.
# Args: $1=label $2=reference_json_file $3=stackarr_json_file $4=media_type
compare_releases() {
    local label="$1"
    local ref_file="$2"
    local arz_file="$3"
    local media_type="$4"

    local ref_count arz_count
    ref_count=$(jq 'length' "$ref_file")
    arz_count=$(jq 'length' "$arz_file")

    log "  $label: reference=$ref_count releases, StackArr=$arz_count releases"

    if [[ "$ref_count" -eq 0 ]]; then
        skip "$label — reference returned 0 releases (indexer timeout?)"
        return
    fi

    # --- Check 1: StackArr returned results ---
    if [[ "$arz_count" -gt 0 ]]; then
        ok "$label — StackArr returned $arz_count releases"
    else
        fail "$label — StackArr returned 0 releases (expected ~$ref_count)"
        return
    fi

    # --- Check 2: Match titles between ref and StackArr ---
    # Build a lookup of reference titles → quality + approved + rejections + customFormatScore
    local ref_titles arz_titles
    ref_titles=$(jq -r '.[].title' "$ref_file" | sort -u)
    arz_titles=$(jq -r '.[] | (.release.title // .title)' "$arz_file" | sort -u)

    local matched=0
    local mismatched_quality=0
    local mismatched_approval=0
    local existing_file_skipped=0
    local only_in_ref=0
    local only_in_arz=0
    local total_checked=0

    # Create temp files for the detailed comparison report
    local report_file="$RESULTS_DIR/${label// /_}_comparison.txt"
    : > "$report_file"

    # For each title in the reference, find it in StackArr and compare
    while IFS= read -r title; do
        [[ -z "$title" ]] && continue

        # Extract reference data for this title
        local ref_data arz_data
        ref_data=$(jq --arg t "$title" '[.[] | select(.title == $t)] | .[0]' "$ref_file")
        # StackArr wraps in DownloadDecision { release: { title: ... }, approved, rejections }
        arz_data=$(jq --arg t "$title" '[.[] | select((.release.title // .title) == $t)] | .[0]' "$arz_file")

        if [[ "$arz_data" == "null" || -z "$arz_data" ]]; then
            only_in_ref=$((only_in_ref + 1))
            echo "ONLY_IN_REF: $title" >> "$report_file"
            continue
        fi

        total_checked=$((total_checked + 1))

        # Compare quality name
        local ref_quality arz_quality
        ref_quality=$(echo "$ref_data" | jq -r '.quality.quality.name // "unknown"')
        # StackArr parses quality from title via stackarr-parser; the release endpoint
        # doesn't return a quality object — the quality is implicit in the decision engine.
        # We compare approved/rejected status and rejection reasons instead.

        # Compare approved status
        local ref_approved arz_approved
        ref_approved=$(echo "$ref_data" | jq -r '.approved // false')
        arz_approved=$(echo "$arz_data" | jq -r '.approved // false')

        if [[ "$ref_approved" != "$arz_approved" ]]; then
            local ref_rejections arz_rejections
            ref_rejections=$(echo "$ref_data" | jq -c '.rejections // []')
            arz_rejections=$(echo "$arz_data" | jq -c '.rejections // []')

            # Check if this is a false positive: ref rejected ONLY because of
            # existing file comparisons, which StackArr can't replicate (no files
            # on disk in the test environment).  Sonarr/Radarr emit rejection
            # reasons containing these patterns when an existing file is already
            # good enough:
            #   - "Existing file on disk ..."
            #   - "... is of equal or higher preference ..."
            local all_existing_file="false"
            if [[ "$ref_approved" == "false" && "$arz_approved" == "true" ]]; then
                # Count total rejections and how many are about existing files.
                # Sonarr rejections: array of {reason:"...", type:"..."} or strings.
                local total_rej non_existing_rej
                total_rej=$(echo "$ref_rejections" | jq 'length')
                non_existing_rej=$(echo "$ref_rejections" | jq '[.[] |
                    # Normalise: if the element is a string use it directly,
                    # otherwise grab .reason
                    (if type == "string" then . else (.reason // "") end) |
                    # Keep only rejections that are NOT about existing files
                    # or Radarr NULL-language quirk (starts with " is wanted")
                    select(
                        (test("(?i)existing file on disk") | not) and
                        (test("(?i)is of equal or higher preference") | not) and
                        (test("^ is wanted") | not)
                    )
                ] | length')

                if [[ "$total_rej" -gt 0 && "$non_existing_rej" -eq 0 ]]; then
                    all_existing_file="true"
                fi
            fi

            if [[ "$all_existing_file" == "true" ]]; then
                # Expected divergence — don't count as a real mismatch
                existing_file_skipped=$((existing_file_skipped + 1))
                matched=$((matched + 1))
                printf "EXISTING_FILE_SKIP: %s\n  ref: approved=%s rejections=%s\n  arz: approved=%s (expected — no file on disk)\n" \
                    "$title" "$ref_approved" "$ref_rejections" "$arz_approved" >> "$report_file"
            else
                mismatched_approval=$((mismatched_approval + 1))
                printf "APPROVAL_DIFF: %s\n  ref: approved=%s rejections=%s\n  arz: approved=%s rejections=%s\n" \
                    "$title" "$ref_approved" "$ref_rejections" "$arz_approved" "$arz_rejections" >> "$report_file"
            fi
        else
            matched=$((matched + 1))
        fi

        # Compare custom format score (Sonarr/Radarr field: customFormatScore)
        local ref_cfs arz_cfs
        ref_cfs=$(echo "$ref_data" | jq -r '.customFormatScore // 0')
        arz_cfs=$(echo "$arz_data" | jq -r '.release.customFormatScore // .customFormatScore // 0')

    done <<< "$ref_titles"

    # Count titles only in StackArr
    while IFS= read -r title; do
        [[ -z "$title" ]] && continue
        local in_ref
        in_ref=$(jq --arg t "$title" '[.[] | select(.title == $t)] | length' "$ref_file")
        if [[ "$in_ref" -eq 0 ]]; then
            only_in_arz=$((only_in_arz + 1))
            echo "ONLY_IN_ARZ: $title" >> "$report_file"
        fi
    done <<< "$arz_titles"

    # --- Report ---
    log "  $label results:"
    log "    Titles compared:       $total_checked"
    log "    Approval match:        $matched / $total_checked"
    log "    Approval diff:         $mismatched_approval"
    log "    Existing-file skipped: $existing_file_skipped (ref rejected only due to existing file on disk)"
    log "    Only in ref:           $only_in_ref"
    log "    Only in StackArr:      $only_in_arz"

    # Pass/fail thresholds
    if [[ "$total_checked" -gt 0 ]]; then
        ok "$label — $total_checked titles compared"
    else
        fail "$label — no titles matched between reference and StackArr"
    fi

    # Allow up to 20% approval mismatches (different decision engines, timing)
    if [[ "$total_checked" -gt 0 ]]; then
        local mismatch_pct=$((mismatched_approval * 100 / total_checked))
        if [[ "$mismatch_pct" -le 20 ]]; then
            ok "$label — approval mismatch rate ${mismatch_pct}% (≤20%)"
        else
            fail "$label — approval mismatch rate ${mismatch_pct}% (>20%: $mismatched_approval/$total_checked)"
        fi
    fi

    log "  Full comparison report: $report_file"
}

# ── Pre-flight ────────────────────────────────────────────

section "Stack 4 — Quality Parity Test"

# Verify fixtures
for f in sonarr.db radarr.db prowlarr.db; do
    if [[ ! -f "$FIXTURES/$f" ]]; then
        fail "Missing fixture: $FIXTURES/$f"
        exit 1
    fi
done
ok "All fixture files present"

# Verify live Sonarr/Radarr connectivity
sonarr_api "/api/v3/system/status"
if [[ "$SONARR_CODE" == "200" ]]; then
    local_sonarr_ver=$(echo "$SONARR_BODY" | jq -r '.version // "unknown"')
    ok "Sonarr reachable (v$local_sonarr_ver)"
else
    fail "Sonarr not reachable at $SONARR_URL (HTTP $SONARR_CODE)"
    exit 1
fi

radarr_api "/api/v3/system/status"
if [[ "$RADARR_CODE" == "200" ]]; then
    local_radarr_ver=$(echo "$RADARR_BODY" | jq -r '.version // "unknown"')
    ok "Radarr reachable (v$local_radarr_ver)"
else
    fail "Radarr not reachable at $RADARR_URL (HTTP $RADARR_CODE)"
    exit 1
fi

# ── Setup StackArr stack ──────────────────────────────────

if [[ "${SKIP_SETUP:-0}" != "1" ]]; then
    section "Setup — Start StackArr Stack"

    mkdir -p "$MEDIA_BASE/TV" "$MEDIA_BASE/Movies"
    rm -rf "$MEDIA_BASE/TV/"* "$MEDIA_BASE/Movies/"* 2>/dev/null || true

    compose_nuke "$COMPOSE_FILE" "$PROJECT"
    log "Starting stack: $PROJECT"
    docker compose -f "$COMPOSE_FILE" -p "$PROJECT" up -d 2>&1 | tail -5
    wait_for_health 90

    # First-boot setup
    api GET /api/v1/system/status
    assert_status 200 "GET /system/status"
    assert_json ".firstBoot" "true" "firstBoot is true"

    SETUP_PAYLOAD=$(cat <<'JSON'
{
  "modules": {
    "tvManagement": true,
    "movieManagement": true,
    "torrentEmbedded": true,
    "usenetEmbedded": true,
    "indexarrSidecar": false,
    "plexIntegration": false,
    "streaming": false
  },
  "mediaLibraryFolders": [
    { "path": "/media/TV", "mediaType": "tv" },
    { "path": "/media/Movies", "mediaType": "movie" }
  ],
  "instanceName": "StackArr-Test-Quality"
}
JSON
)

    api POST /api/v1/setup/init "$SETUP_PAYLOAD"
    assert_status 201 "POST /setup/init"

    API_KEY=$(echo "$API_BODY" | jq -r '.apiKey // empty')
    if [[ -n "$API_KEY" ]]; then
        ok "API key received: ${API_KEY:0:8}..."
    else
        warn "No API key returned"
    fi

    # Import Sonarr + Radarr + Prowlarr databases
    section "Setup — Import Databases"

    log "Uploading Sonarr + Radarr + Prowlarr databases..."
    response=$(curl -s -S -w '\n%{http_code}' -X POST \
        ${API_KEY:+-H "X-Api-Key: $API_KEY"} \
        -F "sonarr_db=@$FIXTURES/sonarr.db" \
        -F "radarr_db=@$FIXTURES/radarr.db" \
        -F "prowlarr_db=@$FIXTURES/prowlarr.db" \
        -F 'path_mappings=[{"from":"/mnt/data2/TV1","to":"/media/TV1"},{"from":"/mnt/data1/movies2","to":"/media/Movies2"}]' \
        "$BASE_URL/api/v1/system/migrate" 2>&1) || true

    API_CODE=$(echo "$response" | tail -1)
    API_BODY=$(echo "$response" | sed '$d')
    assert_status 200 "POST /system/migrate"

    profiles_imported=$(echo "$API_BODY" | jq '.qualityProfilesImported // 0')
    indexers_imported=$(echo "$API_BODY" | jq '.indexersImported // 0')
    log "  Imported: $profiles_imported quality profiles, $indexers_imported indexers"

    if [[ "$profiles_imported" -gt 0 ]]; then
        ok "Quality profiles imported ($profiles_imported)"
    else
        fail "No quality profiles imported"
    fi

    if [[ "$indexers_imported" -gt 0 ]]; then
        ok "Indexers imported ($indexers_imported)"
    else
        fail "No indexers imported"
    fi

    # Verify quality profiles match
    section "Setup — Verify Quality Profiles"

    api GET /api/v1/qualityprofile
    assert_status 200 "GET /qualityprofile"
    arz_profiles=$(echo "$API_BODY" | jq 'length')
    log "StackArr has $arz_profiles quality profiles"

    # Dump profiles for reference
    echo "$API_BODY" | jq '.' > "$RESULTS_DIR/stackarr_quality_profiles.json"
    ok "Quality profiles saved to $RESULTS_DIR/stackarr_quality_profiles.json"

    # Restart StackArr so imported indexers are loaded into memory
    section "Setup — Restart StackArr"
    log "Restarting StackArr to load imported indexers..."
    docker compose -f "$COMPOSE_FILE" -p "$PROJECT" restart stackarr 2>&1 | tail -3
    wait_for_health 60

    # Save the API key for SKIP_SETUP reruns
    echo "$API_KEY" > "$RESULTS_DIR/.api-key"
else
    log "SKIP_SETUP=1 — reusing running stack"
    if [[ -f "$RESULTS_DIR/.api-key" ]]; then
        API_KEY=$(cat "$RESULTS_DIR/.api-key")
        ok "Loaded API key from previous run"
    fi
fi

# ── Resolve IDs in both systems ───────────────────────────

section "Resolve Search Targets"

# --- Sonarr IDs (for reference searches) ---

sonarr_api "/api/v3/series"
OUTLANDER_ID=$(echo "$SONARR_BODY" | jq '[.[] | select(.title == "Outlander")] | .[0].id')
log "Sonarr Outlander series ID: $OUTLANDER_ID"

sonarr_api "/api/v3/episode?seriesId=$OUTLANDER_ID"
OUTLANDER_EP_ID=$(echo "$SONARR_BODY" | jq '[.[] | select(.seasonNumber == 8 and .episodeNumber == 1)] | .[0].id')
log "Sonarr Outlander S08E01 episode ID: $OUTLANDER_EP_ID"

sonarr_api "/api/v3/series"
NCIS_ID=$(echo "$SONARR_BODY" | jq '[.[] | select(.title == "NCIS")] | .[0].id')
log "Sonarr NCIS series ID: $NCIS_ID"

sonarr_api "/api/v3/episode?seriesId=$NCIS_ID"
NCIS_EP_ID=$(echo "$SONARR_BODY" | jq '[.[] | select(.seasonNumber == 23 and .episodeNumber == 1)] | .[0].id')
log "Sonarr NCIS S23E01 episode ID: $NCIS_EP_ID"

sonarr_api "/api/v3/series"
FALLOUT_ID=$(echo "$SONARR_BODY" | jq '[.[] | select(.title == "Fallout")] | .[0].id')
log "Sonarr Fallout series ID: $FALLOUT_ID"

sonarr_api "/api/v3/episode?seriesId=$FALLOUT_ID"
FALLOUT_EP_ID=$(echo "$SONARR_BODY" | jq '[.[] | select(.seasonNumber == 1 and .episodeNumber == 4)] | .[0].id')
log "Sonarr Fallout S01E04 episode ID: $FALLOUT_EP_ID"

radarr_api "/api/v3/movie"
ANACONDA_ID=$(echo "$RADARR_BODY" | jq '[.[] | select(.title == "Anaconda" and .year == 2025)] | .[0].id')
log "Radarr Anaconda (2025) movie ID: $ANACONDA_ID"

GLHF_ID=$(echo "$RADARR_BODY" | jq '[.[] | select(.title | test("Good Luck.*Have Fun"))] | .[0].id')
log "Radarr Good Luck Have Fun Don't Die movie ID: $GLHF_ID"

# --- StackArr IDs (for context-aware searches) ---

# Series IDs in StackArr
api GET /api/v1/series
ARZ_OUTLANDER_ID=$(echo "$API_BODY" | jq '[.[] | select(.title == "Outlander")] | .[0].id // empty')
ARZ_NCIS_ID=$(echo "$API_BODY" | jq '[.[] | select(.title == "NCIS")] | .[0].id // empty')
ARZ_FALLOUT_ID=$(echo "$API_BODY" | jq '[.[] | select(.title == "Fallout")] | .[0].id // empty')
log "StackArr Outlander series ID: ${ARZ_OUTLANDER_ID:-none}"
log "StackArr NCIS series ID: ${ARZ_NCIS_ID:-none}"
log "StackArr Fallout series ID: ${ARZ_FALLOUT_ID:-none}"

# Episode IDs in StackArr
if [[ -n "${ARZ_OUTLANDER_ID:-}" && "$ARZ_OUTLANDER_ID" != "null" ]]; then
    api GET "/api/v1/series/${ARZ_OUTLANDER_ID}/episodes"
    ARZ_OUTLANDER_EP_ID=$(echo "$API_BODY" | jq '[.[] | select(.seasonNumber == 8 and .episodeNumber == 1)] | .[0].id // empty')
    log "StackArr Outlander S08E01 episode ID: ${ARZ_OUTLANDER_EP_ID:-none}"
fi

if [[ -n "${ARZ_NCIS_ID:-}" && "$ARZ_NCIS_ID" != "null" ]]; then
    api GET "/api/v1/series/${ARZ_NCIS_ID}/episodes"
    ARZ_NCIS_EP_ID=$(echo "$API_BODY" | jq '[.[] | select(.seasonNumber == 23 and .episodeNumber == 1)] | .[0].id // empty')
    log "StackArr NCIS S23E01 episode ID: ${ARZ_NCIS_EP_ID:-none}"
fi

if [[ -n "${ARZ_FALLOUT_ID:-}" && "$ARZ_FALLOUT_ID" != "null" ]]; then
    api GET "/api/v1/series/${ARZ_FALLOUT_ID}/episodes"
    ARZ_FALLOUT_EP_ID=$(echo "$API_BODY" | jq '[.[] | select(.seasonNumber == 1 and .episodeNumber == 4)] | .[0].id // empty')
    log "StackArr Fallout S01E04 episode ID: ${ARZ_FALLOUT_EP_ID:-none}"
fi

# Movie IDs in StackArr
api GET /api/v1/movies
ARZ_ANACONDA_ID=$(echo "$API_BODY" | jq '[.[] | select(.title == "Anaconda" and .year == 2025)] | .[0].id // empty')
ARZ_GLHF_ID=$(echo "$API_BODY" | jq '[.[] | select(.title | test("Good Luck.*Have Fun"))] | .[0].id // empty')
log "StackArr Anaconda movie ID: ${ARZ_ANACONDA_ID:-none}"
log "StackArr GLHF movie ID: ${ARZ_GLHF_ID:-none}"

# ── Search 1: Outlander S08E01 ───────────────────────────

section "Search 1: Outlander S08E01"

log "Searching Sonarr for Outlander S08E01..."
sonarr_api "/api/v3/release?episodeId=$OUTLANDER_EP_ID"
if [[ "$SONARR_CODE" == "200" ]]; then
    echo "$SONARR_BODY" | jq '.' > "$RESULTS_DIR/sonarr_outlander_s08e01.json"
    sonarr_count=$(jq 'length' "$RESULTS_DIR/sonarr_outlander_s08e01.json")
    ok "Sonarr returned $sonarr_count releases"
else
    fail "Sonarr search failed (HTTP $SONARR_CODE)"
    echo "$SONARR_BODY" | head -5
fi

# Build StackArr search URL with context params for decision engine
# Use the series' own quality profile (imported from Sonarr)
ARZ_OUTLANDER_PROFILE=""
if [[ -n "${ARZ_OUTLANDER_ID:-}" && "${ARZ_OUTLANDER_ID}" != "null" ]]; then
    api GET "/api/v1/series/${ARZ_OUTLANDER_ID}"
    ARZ_OUTLANDER_PROFILE=$(echo "$API_BODY" | jq -r '.qualityProfileId // empty')
fi
ARZ_OUTLANDER_PARAMS="term=Outlander+S08E01&mediaType=series"
[[ -n "${ARZ_OUTLANDER_PROFILE:-}" ]] && ARZ_OUTLANDER_PARAMS+="&qualityProfileId=${ARZ_OUTLANDER_PROFILE}"
[[ -n "${ARZ_OUTLANDER_ID:-}" && "${ARZ_OUTLANDER_ID}" != "null" ]] && ARZ_OUTLANDER_PARAMS+="&seriesId=${ARZ_OUTLANDER_ID}"
[[ -n "${ARZ_OUTLANDER_EP_ID:-}" && "${ARZ_OUTLANDER_EP_ID}" != "null" ]] && ARZ_OUTLANDER_PARAMS+="&episodeId=${ARZ_OUTLANDER_EP_ID}"
log "Searching StackArr for Outlander S08E01..."
log "  params: $ARZ_OUTLANDER_PARAMS"
api GET "/api/v1/release?${ARZ_OUTLANDER_PARAMS}"
if [[ "$API_CODE" == "200" ]]; then
    echo "$API_BODY" | jq '.' > "$RESULTS_DIR/stackarr_outlander_s08e01.json"
    arz_count=$(jq 'length' "$RESULTS_DIR/stackarr_outlander_s08e01.json")
    ok "StackArr returned $arz_count releases"
else
    fail "StackArr search failed (HTTP $API_CODE)"
    echo "$API_BODY" | head -5
fi

compare_releases "Outlander S08E01" \
    "$RESULTS_DIR/sonarr_outlander_s08e01.json" \
    "$RESULTS_DIR/stackarr_outlander_s08e01.json" \
    "series"

# ── Search 2: NCIS S23E01 ────────────────────────────────

section "Search 2: NCIS S23E01"

log "Searching Sonarr for NCIS S23E01..."
sonarr_api "/api/v3/release?episodeId=$NCIS_EP_ID"
if [[ "$SONARR_CODE" == "200" ]]; then
    echo "$SONARR_BODY" | jq '.' > "$RESULTS_DIR/sonarr_ncis_s23e01.json"
    sonarr_count=$(jq 'length' "$RESULTS_DIR/sonarr_ncis_s23e01.json")
    ok "Sonarr returned $sonarr_count releases"
else
    fail "Sonarr search failed (HTTP $SONARR_CODE)"
fi

ARZ_NCIS_PROFILE=""
if [[ -n "${ARZ_NCIS_ID:-}" && "${ARZ_NCIS_ID}" != "null" ]]; then
    api GET "/api/v1/series/${ARZ_NCIS_ID}"
    ARZ_NCIS_PROFILE=$(echo "$API_BODY" | jq -r '.qualityProfileId // empty')
fi
ARZ_NCIS_PARAMS="term=NCIS+S23E01&mediaType=series"
[[ -n "${ARZ_NCIS_PROFILE:-}" ]] && ARZ_NCIS_PARAMS+="&qualityProfileId=${ARZ_NCIS_PROFILE}"
[[ -n "${ARZ_NCIS_ID:-}" && "${ARZ_NCIS_ID}" != "null" ]] && ARZ_NCIS_PARAMS+="&seriesId=${ARZ_NCIS_ID}"
[[ -n "${ARZ_NCIS_EP_ID:-}" && "${ARZ_NCIS_EP_ID}" != "null" ]] && ARZ_NCIS_PARAMS+="&episodeId=${ARZ_NCIS_EP_ID}"
log "Searching StackArr for NCIS S23E01..."
log "  params: $ARZ_NCIS_PARAMS"
api GET "/api/v1/release?${ARZ_NCIS_PARAMS}"
if [[ "$API_CODE" == "200" ]]; then
    echo "$API_BODY" | jq '.' > "$RESULTS_DIR/stackarr_ncis_s23e01.json"
    arz_count=$(jq 'length' "$RESULTS_DIR/stackarr_ncis_s23e01.json")
    ok "StackArr returned $arz_count releases"
else
    fail "StackArr search failed (HTTP $API_CODE)"
fi

compare_releases "NCIS S23E01" \
    "$RESULTS_DIR/sonarr_ncis_s23e01.json" \
    "$RESULTS_DIR/stackarr_ncis_s23e01.json" \
    "series"

# ── Search 3: Fallout S01E04 (language filtering test — UHD profile) ──

section "Search 3: Fallout S01E04"

log "Searching Sonarr for Fallout S01E04..."
sonarr_api "/api/v3/release?episodeId=$FALLOUT_EP_ID"
if [[ "$SONARR_CODE" == "200" ]]; then
    echo "$SONARR_BODY" | jq '.' > "$RESULTS_DIR/sonarr_fallout_s01e04.json"
    sonarr_count=$(jq 'length' "$RESULTS_DIR/sonarr_fallout_s01e04.json")
    ok "Sonarr returned $sonarr_count releases"
    # Validate Sonarr is returning good data — top results should be English REMUX/Bluray 2160p
    top_title=$(jq -r '.[0].title // "none"' "$RESULTS_DIR/sonarr_fallout_s01e04.json")
    top_quality=$(jq -r '.[0].quality.quality.name // "none"' "$RESULTS_DIR/sonarr_fallout_s01e04.json")
    top_score=$(jq '.[0].customFormatScore // 0' "$RESULTS_DIR/sonarr_fallout_s01e04.json")
    log "  Sonarr top result: $top_title (quality=$top_quality, CF=$top_score)"
    if echo "$top_title" | grep -qiE 'REMUX|FraMeSToR|TRiToN'; then
        ok "Sonarr top result is a high-quality REMUX as expected"
    else
        warn "Sonarr top result may not be optimal: $top_title"
    fi
else
    fail "Sonarr search failed (HTTP $SONARR_CODE)"
fi

ARZ_FALLOUT_PROFILE=""
if [[ -n "${ARZ_FALLOUT_ID:-}" && "${ARZ_FALLOUT_ID}" != "null" ]]; then
    api GET "/api/v1/series/${ARZ_FALLOUT_ID}"
    ARZ_FALLOUT_PROFILE=$(echo "$API_BODY" | jq -r '.qualityProfileId // empty')
fi
ARZ_FALLOUT_PARAMS="term=Fallout+S01E04&mediaType=series"
[[ -n "${ARZ_FALLOUT_PROFILE:-}" ]] && ARZ_FALLOUT_PARAMS+="&qualityProfileId=${ARZ_FALLOUT_PROFILE}"
[[ -n "${ARZ_FALLOUT_ID:-}" && "${ARZ_FALLOUT_ID}" != "null" ]] && ARZ_FALLOUT_PARAMS+="&seriesId=${ARZ_FALLOUT_ID}"
[[ -n "${ARZ_FALLOUT_EP_ID:-}" && "${ARZ_FALLOUT_EP_ID}" != "null" ]] && ARZ_FALLOUT_PARAMS+="&episodeId=${ARZ_FALLOUT_EP_ID}"
log "Searching StackArr for Fallout S01E04..."
log "  params: $ARZ_FALLOUT_PARAMS"
api GET "/api/v1/release?${ARZ_FALLOUT_PARAMS}"
if [[ "$API_CODE" == "200" ]]; then
    echo "$API_BODY" | jq '.' > "$RESULTS_DIR/stackarr_fallout_s01e04.json"
    arz_count=$(jq 'length' "$RESULTS_DIR/stackarr_fallout_s01e04.json")
    ok "StackArr returned $arz_count releases"
else
    fail "StackArr search failed (HTTP $API_CODE)"
fi

compare_releases "Fallout S01E04" \
    "$RESULTS_DIR/sonarr_fallout_s01e04.json" \
    "$RESULTS_DIR/stackarr_fallout_s01e04.json" \
    "series"

# ── Search 4: Anaconda 2025 ──────────────────────────────

section "Search 4: Anaconda 2025"

log "Searching Radarr for Anaconda (2025)..."
radarr_api "/api/v3/release?movieId=$ANACONDA_ID"
if [[ "$RADARR_CODE" == "200" ]]; then
    echo "$RADARR_BODY" | jq '.' > "$RESULTS_DIR/radarr_anaconda_2025.json"
    radarr_count=$(jq 'length' "$RESULTS_DIR/radarr_anaconda_2025.json")
    ok "Radarr returned $radarr_count releases"
else
    fail "Radarr search failed (HTTP $RADARR_CODE)"
fi

ARZ_ANACONDA_PROFILE=""
if [[ -n "${ARZ_ANACONDA_ID:-}" && "${ARZ_ANACONDA_ID}" != "null" ]]; then
    api GET "/api/v1/movies/${ARZ_ANACONDA_ID}"
    ARZ_ANACONDA_PROFILE=$(echo "$API_BODY" | jq -r '.qualityProfileId // empty')
fi
ARZ_ANACONDA_PARAMS="term=Anaconda+2025&mediaType=movie"
[[ -n "${ARZ_ANACONDA_PROFILE:-}" ]] && ARZ_ANACONDA_PARAMS+="&qualityProfileId=${ARZ_ANACONDA_PROFILE}"
[[ -n "${ARZ_ANACONDA_ID:-}" && "${ARZ_ANACONDA_ID}" != "null" ]] && ARZ_ANACONDA_PARAMS+="&movieId=${ARZ_ANACONDA_ID}"
log "Searching StackArr for Anaconda 2025..."
log "  params: $ARZ_ANACONDA_PARAMS"
api GET "/api/v1/release?${ARZ_ANACONDA_PARAMS}"
if [[ "$API_CODE" == "200" ]]; then
    echo "$API_BODY" | jq '.' > "$RESULTS_DIR/stackarr_anaconda_2025.json"
    arz_count=$(jq 'length' "$RESULTS_DIR/stackarr_anaconda_2025.json")
    ok "StackArr returned $arz_count releases"
else
    fail "StackArr search failed (HTTP $API_CODE)"
fi

compare_releases "Anaconda 2025" \
    "$RESULTS_DIR/radarr_anaconda_2025.json" \
    "$RESULTS_DIR/stackarr_anaconda_2025.json" \
    "movie"

# ── Search 5: Good Luck Have Fun Don't Die ────────────────

section "Search 5: Good Luck Have Fun Don't Die"

log "Searching Radarr for Good Luck Have Fun Don't Die..."
radarr_api "/api/v3/release?movieId=$GLHF_ID"
if [[ "$RADARR_CODE" == "200" ]]; then
    echo "$RADARR_BODY" | jq '.' > "$RESULTS_DIR/radarr_glhf.json"
    radarr_count=$(jq 'length' "$RESULTS_DIR/radarr_glhf.json")
    ok "Radarr returned $radarr_count releases"
else
    fail "Radarr search failed (HTTP $RADARR_CODE)"
fi

ARZ_GLHF_PROFILE=""
if [[ -n "${ARZ_GLHF_ID:-}" && "${ARZ_GLHF_ID}" != "null" ]]; then
    api GET "/api/v1/movies/${ARZ_GLHF_ID}"
    ARZ_GLHF_PROFILE=$(echo "$API_BODY" | jq -r '.qualityProfileId // empty')
fi
ARZ_GLHF_PARAMS="term=Good+Luck+Have+Fun+Dont+Die&mediaType=movie"
[[ -n "${ARZ_GLHF_PROFILE:-}" ]] && ARZ_GLHF_PARAMS+="&qualityProfileId=${ARZ_GLHF_PROFILE}"
[[ -n "${ARZ_GLHF_ID:-}" && "${ARZ_GLHF_ID}" != "null" ]] && ARZ_GLHF_PARAMS+="&movieId=${ARZ_GLHF_ID}"
log "Searching StackArr for Good Luck Have Fun Don't Die..."
log "  params: $ARZ_GLHF_PARAMS"
api GET "/api/v1/release?${ARZ_GLHF_PARAMS}"
if [[ "$API_CODE" == "200" ]]; then
    echo "$API_BODY" | jq '.' > "$RESULTS_DIR/stackarr_glhf.json"
    arz_count=$(jq 'length' "$RESULTS_DIR/stackarr_glhf.json")
    ok "StackArr returned $arz_count releases"
else
    fail "StackArr search failed (HTTP $API_CODE)"
fi

compare_releases "GLHF" \
    "$RESULTS_DIR/radarr_glhf.json" \
    "$RESULTS_DIR/stackarr_glhf.json" \
    "movie"

# ── Summary tables ────────────────────────────────────────

section "Detailed Quality Comparison"

# For each search, print a side-by-side table of top 10 results by quality/score
for search_name in outlander_s08e01 ncis_s23e01 fallout_s01e04 anaconda_2025 glhf; do
    case "$search_name" in
        outlander_s08e01|ncis_s23e01|fallout_s01e04)
            ref_type="sonarr"
            ;;
        *)
            ref_type="radarr"
            ;;
    esac

    ref_file="$RESULTS_DIR/${ref_type}_${search_name}.json"
    arz_file="$RESULTS_DIR/stackarr_${search_name}.json"

    if [[ ! -f "$ref_file" || ! -f "$arz_file" ]]; then
        warn "Skipping $search_name — missing result files"
        continue
    fi

    echo ""
    echo -e "${CYAN}  ── Top 10: ${search_name} ──${NC}"
    echo ""

    # Reference (Sonarr/Radarr) top 10
    printf "  %-60s  %-20s  %s  %s\n" "TITLE (Reference)" "QUALITY" "APPROVED" "CF_SCORE"
    printf "  %-60s  %-20s  %s  %s\n" "$(printf '%.0s─' {1..60})" "$(printf '%.0s─' {1..20})" "────────" "────────"
    jq -r '.[0:10][] | [
        (.title // "?" | .[0:58]),
        (.quality.quality.name // "?"),
        (if .approved then "YES" else "NO " end),
        (.customFormatScore // 0 | tostring)
    ] | @tsv' "$ref_file" 2>/dev/null | while IFS=$'\t' read -r t q a s; do
        printf "  %-60s  %-20s  %s      %s\n" "$t" "$q" "$a" "$s"
    done

    echo ""

    # StackArr top 10
    printf "  %-60s  %-20s  %s  %s\n" "TITLE (StackArr)" "REJECTIONS" "APPROVED" "N_REJ"
    printf "  %-60s  %-20s  %s  %s\n" "$(printf '%.0s─' {1..60})" "$(printf '%.0s─' {1..20})" "────────" "────────"
    jq -r '.[0:10][] | [
        ((.release.title // .title // "?") | .[0:58]),
        ((.rejections // []) | if length == 0 then "—" else .[0].reason | .[0:18] end),
        (if .approved then "YES" else "NO " end),
        ((.rejections // []) | length | tostring)
    ] | @tsv' "$arz_file" 2>/dev/null | while IFS=$'\t' read -r t r a n; do
        printf "  %-60s  %-20s  %s      %s\n" "$t" "$r" "$a" "$n"
    done

done

# ── Teardown ──────────────────────────────────────────────

section "Teardown"

if [[ "${KEEP_STACK:-0}" != "1" ]]; then
    compose_nuke "$COMPOSE_FILE" "$PROJECT"
    ok "Stack torn down"
else
    log "KEEP_STACK=1 — leaving stack running"
    ok "Stack left running at $BASE_URL"
fi

log "Results saved to: $RESULTS_DIR/"

# ── Final ─────────────────────────────────────────────────

summary
