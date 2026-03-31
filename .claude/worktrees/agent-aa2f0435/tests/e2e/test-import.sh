#!/usr/bin/env bash
# test-import.sh — Stack 3: Import test (Sonarr/Radarr/Prowlarr + SABnzbd)
#
# Usage (run on Node B):
#   cd /path/to/tests/e2e
#   ./test-import.sh
#
# This script:
#   1. Nukes any previous import stack
#   2. Starts a clean stack
#   3. Completes first-boot (minimal — modules only)
#   4. Imports Sonarr + Radarr + Prowlarr databases
#   5. Validates imported series, movies, indexers, quality profiles, etc.
#   6. Imports SABnzbd config (preview + apply)
#   7. Validates usenet servers created
#   8. Tears down
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
source "$SCRIPT_DIR/common.sh"

COMPOSE_FILE="$SCRIPT_DIR/docker-compose.import.yml"
PROJECT="stackarr-import"
BASE_URL="http://localhost:9213"
MEDIA_BASE="/mnt/4tnvme/docker/volumes/stackrr_test_media/import"
# Fixtures: use repo test-fixtures if running from repo, else local fixtures/ dir
if [[ -d "$SCRIPT_DIR/../../test-fixtures" ]]; then
    FIXTURES="$(cd "$SCRIPT_DIR/../../test-fixtures" && pwd)"
elif [[ -d "$SCRIPT_DIR/fixtures" ]]; then
    FIXTURES="$SCRIPT_DIR/fixtures"
else
    echo "ERROR: Cannot find test-fixtures directory"
    exit 1
fi

# ── Pre-flight: check fixtures exist ─────────────────────

section "Stack 3 — Import Test"

for f in sonarr.db radarr.db prowlarr.db sabnzbd.ini; do
    if [[ ! -f "$FIXTURES/$f" ]]; then
        fail "Missing fixture: $FIXTURES/$f"
        echo "    Copy test-fixtures/*.db and sabnzbd.ini into tests/e2e/fixtures/"
        exit 1
    fi
done
ok "All fixture files present"

# ── Cleanup & Start ─────────────────────────────────────

mkdir -p "$MEDIA_BASE/TV" "$MEDIA_BASE/Movies"
rm -rf "$MEDIA_BASE/TV/"* "$MEDIA_BASE/Movies/"* 2>/dev/null || true

compose_nuke "$COMPOSE_FILE" "$PROJECT"
compose_up "$COMPOSE_FILE" "$PROJECT"
wait_for_health 90

# ── 1. First-Boot Setup (minimal) ────────────────────────

section "1. First-Boot Setup"

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
  "instanceName": "StackArr-Test-Import"
}
JSON
)

api POST /api/v1/setup/init "$SETUP_PAYLOAD"
assert_status 201 "POST /setup/init"
assert_json ".success" "true" "setup success"

API_KEY=$(echo "$API_BODY" | jq -r '.apiKey // empty')
if [[ -n "$API_KEY" ]]; then
    ok "API key received: ${API_KEY:0:8}..."
else
    warn "No API key returned"
fi

api GET /api/v1/system/status
assert_json ".firstBoot" "false" "firstBoot is now false"

# ── 2. Baseline counts (before import) ───────────────────

section "2. Baseline (pre-import)"

api GET /api/v1/series
SERIES_BEFORE=$(echo "$API_BODY" | jq 'length')
log "Series before import: $SERIES_BEFORE"

api GET /api/v1/movies
MOVIES_BEFORE=$(echo "$API_BODY" | jq 'length')
log "Movies before import: $MOVIES_BEFORE"

api GET /api/v1/indexer
INDEXERS_BEFORE=$(echo "$API_BODY" | jq 'length')
log "Indexers before import: $INDEXERS_BEFORE"

api GET /api/v1/qualityprofile
PROFILES_BEFORE=$(echo "$API_BODY" | jq 'length')
log "Quality profiles before import: $PROFILES_BEFORE"

api GET /api/v1/downloadclient
CLIENTS_BEFORE=$(echo "$API_BODY" | jq 'length')
log "Download clients before import: $CLIENTS_BEFORE"

# ── 3. Import ALL three databases ─────────────────────────

section "3. Import Sonarr + Radarr + Prowlarr (all at once)"

RESPONSE=$(curl -s -S -w '\n%{http_code}' -X POST \
    ${API_KEY:+-H "X-Api-Key: $API_KEY"} \
    -F "sonarr_db=@$FIXTURES/sonarr.db" \
    -F "radarr_db=@$FIXTURES/radarr.db" \
    -F "prowlarr_db=@$FIXTURES/prowlarr.db" \
    "${BASE_URL}/api/v1/system/migrate" 2>&1) || true

API_CODE=$(echo "$RESPONSE" | tail -1)
API_BODY=$(echo "$RESPONSE" | sed '$d')

assert_status 200 "POST /system/migrate — all three DBs"

# Parse migration report
SERIES_IMPORTED=$(echo "$API_BODY" | jq '.seriesImported // 0')
MOVIES_IMPORTED=$(echo "$API_BODY" | jq '.moviesImported // 0')
EPISODES_IMPORTED=$(echo "$API_BODY" | jq '.episodesImported // 0')
MEDIA_FILES_IMPORTED=$(echo "$API_BODY" | jq '.mediaFilesImported // 0')
PROFILES_IMPORTED=$(echo "$API_BODY" | jq '.qualityProfilesImported // 0')
INDEXERS_IMPORTED=$(echo "$API_BODY" | jq '.indexersImported // 0')
CLIENTS_IMPORTED=$(echo "$API_BODY" | jq '.downloadClientsImported // 0')
HISTORY_IMPORTED=$(echo "$API_BODY" | jq '.historyEventsImported // 0')
BLOCKLIST_IMPORTED=$(echo "$API_BODY" | jq '.blocklistEntriesImported // 0')
WARNINGS=$(echo "$API_BODY" | jq '.warnings | length')
DRY_RUN=$(echo "$API_BODY" | jq '.dryRun')

log "Migration report:"
log "  Series:           $SERIES_IMPORTED"
log "  Movies:           $MOVIES_IMPORTED"
log "  Episodes:         $EPISODES_IMPORTED"
log "  Media files:      $MEDIA_FILES_IMPORTED"
log "  Quality profiles: $PROFILES_IMPORTED"
log "  Indexers:         $INDEXERS_IMPORTED"
log "  Download clients: $CLIENTS_IMPORTED"
log "  History events:   $HISTORY_IMPORTED"
log "  Blocklist:        $BLOCKLIST_IMPORTED"
log "  Warnings:         $WARNINGS"
log "  Dry run:          $DRY_RUN"

assert_json ".dryRun" "false" "not a dry run"

if [[ "$SERIES_IMPORTED" -gt 0 ]]; then
    ok "Series imported: $SERIES_IMPORTED"
else
    fail "No series imported from Sonarr DB"
fi

if [[ "$MOVIES_IMPORTED" -gt 0 ]]; then
    ok "Movies imported: $MOVIES_IMPORTED"
else
    fail "No movies imported from Radarr DB"
fi

if [[ "$INDEXERS_IMPORTED" -gt 0 ]]; then
    ok "Indexers imported: $INDEXERS_IMPORTED"
else
    fail "No indexers imported from Prowlarr DB"
fi

if [[ "$EPISODES_IMPORTED" -gt 0 ]]; then
    ok "Episodes imported: $EPISODES_IMPORTED"
else
    warn "No episodes imported (may be expected depending on Sonarr DB)"
fi

if [[ "$PROFILES_IMPORTED" -gt 0 ]]; then
    ok "Quality profiles imported: $PROFILES_IMPORTED"
else
    warn "No quality profiles imported"
fi

if [[ "$CLIENTS_IMPORTED" -gt 0 ]]; then
    ok "Download clients imported: $CLIENTS_IMPORTED"
else
    warn "No download clients imported"
fi

# ── 4. Validate imported data via API ─────────────────────

section "4. Validate Imported Data"

# Series
api GET /api/v1/series
assert_status 200 "GET /series"
SERIES_TOTAL=$(echo "$API_BODY" | jq 'length')
if [[ "$SERIES_TOTAL" -gt "$SERIES_BEFORE" ]]; then
    ok "Series count increased: $SERIES_BEFORE -> $SERIES_TOTAL"
else
    fail "Series count did not increase after import ($SERIES_TOTAL)"
fi

# Log a few imported series titles
echo "$API_BODY" | jq -r '.[0:5][] | "    \(.title) (TMDB: \(.tmdbId // "n/a"))"' 2>/dev/null || true

# Movies
api GET /api/v1/movies
assert_status 200 "GET /movies"
MOVIES_TOTAL=$(echo "$API_BODY" | jq 'length')
if [[ "$MOVIES_TOTAL" -gt "$MOVIES_BEFORE" ]]; then
    ok "Movies count increased: $MOVIES_BEFORE -> $MOVIES_TOTAL"
else
    fail "Movies count did not increase after import ($MOVIES_TOTAL)"
fi

echo "$API_BODY" | jq -r '.[0:5][] | "    \(.title) (\(.year // "n/a"))"' 2>/dev/null || true

# Indexers
api GET /api/v1/indexer
assert_status 200 "GET /indexer"
INDEXERS_TOTAL=$(echo "$API_BODY" | jq 'length')
if [[ "$INDEXERS_TOTAL" -gt "$INDEXERS_BEFORE" ]]; then
    ok "Indexers count increased: $INDEXERS_BEFORE -> $INDEXERS_TOTAL"
else
    fail "Indexers count did not increase after import ($INDEXERS_TOTAL)"
fi

# Check for both torrent and usenet indexers
TORRENT_COUNT=$(echo "$API_BODY" | jq '[.[] | select(.protocol == "torrent")] | length')
USENET_COUNT=$(echo "$API_BODY" | jq '[.[] | select(.protocol == "usenet")] | length')
log "  Torrent indexers: $TORRENT_COUNT"
log "  Usenet indexers:  $USENET_COUNT"

# Quality profiles
api GET /api/v1/qualityprofile
assert_status 200 "GET /qualityprofile"
PROFILES_TOTAL=$(echo "$API_BODY" | jq 'length')
if [[ "$PROFILES_TOTAL" -gt "$PROFILES_BEFORE" ]]; then
    ok "Quality profiles count increased: $PROFILES_BEFORE -> $PROFILES_TOTAL"
else
    warn "Quality profiles unchanged ($PROFILES_TOTAL) — may have been merged with defaults"
fi

echo "$API_BODY" | jq -r '.[] | "    [\(.id)] \(.name)"' 2>/dev/null || true

# Download clients
api GET /api/v1/downloadclient
assert_status 200 "GET /downloadclient"
CLIENTS_TOTAL=$(echo "$API_BODY" | jq 'length')
log "Download clients after import: $CLIENTS_TOTAL"

# Media library folders (root folders imported from *arr)
api GET /api/v1/medialibraryfolder
assert_status 200 "GET /medialibraryfolder"
FOLDERS_TOTAL=$(echo "$API_BODY" | jq 'length')
log "Media library folders: $FOLDERS_TOTAL"
if [[ "$FOLDERS_TOTAL" -ge 2 ]]; then
    ok "Media library folders present ($FOLDERS_TOTAL)"
else
    warn "Only $FOLDERS_TOTAL media library folders"
fi

# Tags
api GET /api/v1/tag
assert_status 200 "GET /tag"
TAGS_TOTAL=$(echo "$API_BODY" | jq 'length')
log "Tags imported: $TAGS_TOTAL"

# History
api GET "/api/v1/history?page=1&pageSize=10"
assert_status 200 "GET /history"

# Blocklist
api GET "/api/v1/blocklist?page=1&pageSize=10"
assert_status 200 "GET /blocklist"

# Naming config (should reflect imported naming from *arr)
api GET /api/v1/config/naming
assert_status 200 "GET /config/naming"

# ── 5. Import Sonarr-only (idempotent / additive test) ───

section "5. Import Sonarr-Only"

RESPONSE=$(curl -s -S -w '\n%{http_code}' -X POST \
    ${API_KEY:+-H "X-Api-Key: $API_KEY"} \
    -F "sonarr_db=@$FIXTURES/sonarr.db" \
    "${BASE_URL}/api/v1/system/migrate" 2>&1) || true

API_CODE=$(echo "$RESPONSE" | tail -1)
API_BODY=$(echo "$RESPONSE" | sed '$d')

assert_status 200 "POST /system/migrate — Sonarr only"
SONARR_ONLY_SERIES=$(echo "$API_BODY" | jq '.seriesImported // 0')
log "Sonarr-only import: $SONARR_ONLY_SERIES series"
ok "Sonarr-only import completed"

# ── 6. Import Radarr-only ─────────────────────────────────

section "6. Import Radarr-Only"

RESPONSE=$(curl -s -S -w '\n%{http_code}' -X POST \
    ${API_KEY:+-H "X-Api-Key: $API_KEY"} \
    -F "radarr_db=@$FIXTURES/radarr.db" \
    "${BASE_URL}/api/v1/system/migrate" 2>&1) || true

API_CODE=$(echo "$RESPONSE" | tail -1)
API_BODY=$(echo "$RESPONSE" | sed '$d')

assert_status 200 "POST /system/migrate — Radarr only"
RADARR_ONLY_MOVIES=$(echo "$API_BODY" | jq '.moviesImported // 0')
log "Radarr-only import: $RADARR_ONLY_MOVIES movies"
ok "Radarr-only import completed"

# ── 7. Import with no files (expect 400) ──────────────────

section "7. Error Cases"

RESPONSE=$(curl -s -S -w '\n%{http_code}' -X POST \
    ${API_KEY:+-H "X-Api-Key: $API_KEY"} \
    -H "Content-Type: multipart/form-data" \
    "${BASE_URL}/api/v1/system/migrate" 2>&1) || true

API_CODE=$(echo "$RESPONSE" | tail -1)
API_BODY=$(echo "$RESPONSE" | sed '$d')

if [[ "$API_CODE" == "400" ]]; then
    ok "Empty migrate rejected with HTTP 400"
else
    fail "Empty migrate returned HTTP $API_CODE (expected 400)"
fi

# ── 8. SABnzbd Import — Preview ──────────────────────────

section "8. SABnzbd Import — Preview"

RESPONSE=$(curl -s -S -w '\n%{http_code}' -X POST \
    ${API_KEY:+-H "X-Api-Key: $API_KEY"} \
    -F "file=@$FIXTURES/sabnzbd.ini" \
    "${BASE_URL}/api/v1/usenet/import-sabnzbd" 2>&1) || true

API_CODE=$(echo "$RESPONSE" | tail -1)
API_BODY=$(echo "$RESPONSE" | sed '$d')

assert_status 200 "POST /usenet/import-sabnzbd — preview"

# Parse preview
SERVER_COUNT=$(echo "$API_BODY" | jq '.servers | length')
CATEGORY_COUNT=$(echo "$API_BODY" | jq '.categories | length')
RSS_COUNT=$(echo "$API_BODY" | jq '.rssFeeds | length')
PREVIEW_WARNINGS=$(echo "$API_BODY" | jq '.warnings | length')
SKIPPED_FIELDS=$(echo "$API_BODY" | jq '.skippedFields | length')

log "SABnzbd preview:"
log "  Servers:        $SERVER_COUNT"
log "  Categories:     $CATEGORY_COUNT"
log "  RSS feeds:      $RSS_COUNT"
log "  Warnings:       $PREVIEW_WARNINGS"
log "  Skipped fields: $SKIPPED_FIELDS"

if [[ "$SERVER_COUNT" -gt 0 ]]; then
    ok "SABnzbd servers found: $SERVER_COUNT"
    # Show server names
    echo "$API_BODY" | jq -r '.servers[] | "    \(.name) — \(.host):\(.port) (ssl=\(.ssl), conns=\(.connections))"' 2>/dev/null || true
else
    fail "No servers in SABnzbd preview"
fi

# Check for masked passwords
MASKED_COUNT=$(echo "$API_BODY" | jq '[.servers[] | select(.passwordMasked == true)] | length')
log "Servers with masked passwords: $MASKED_COUNT"

# Save the preview for apply step
SABNZBD_PREVIEW="$API_BODY"

# ── 9. SABnzbd Import — Apply ────────────────────────────

section "9. SABnzbd Import — Apply"

if [[ "$MASKED_COUNT" -gt 0 ]]; then
    # Try applying with masked passwords — should fail
    api POST /api/v1/usenet/import-sabnzbd/apply "$SABNZBD_PREVIEW"
    if [[ "$API_CODE" == "400" ]]; then
        ok "Apply with masked passwords correctly rejected (HTTP 400)"
    else
        warn "Apply with masked passwords returned HTTP $API_CODE (expected 400)"
    fi

    # Unmask passwords by replacing *** with the known password
    SABNZBD_UNMASKED=$(echo "$SABNZBD_PREVIEW" | jq '
        .servers |= [.[] | .password = "podoxydyg5r" | .passwordMasked = false]
    ')
    api POST /api/v1/usenet/import-sabnzbd/apply "$SABNZBD_UNMASKED"
else
    api POST /api/v1/usenet/import-sabnzbd/apply "$SABNZBD_PREVIEW"
fi

if [[ "$API_CODE" =~ ^2 ]]; then
    ok "SABnzbd import applied (HTTP $API_CODE)"
    SERVERS_ADDED=$(echo "$API_BODY" | jq '.serversAdded // 0')
    CATEGORIES_ADDED=$(echo "$API_BODY" | jq '.categoriesAdded // 0')
    log "  Servers added:    $SERVERS_ADDED"
    log "  Categories added: $CATEGORIES_ADDED"

    if [[ "$SERVERS_ADDED" -gt 0 ]]; then
        ok "Usenet servers added to download clients: $SERVERS_ADDED"
    else
        warn "No usenet servers added"
    fi
else
    fail "SABnzbd import apply failed (HTTP $API_CODE): ${API_BODY:0:300}"
fi

# ── 10. Validate SABnzbd import in download clients ──────

section "10. Validate SABnzbd Import"

api GET /api/v1/downloadclient
assert_status 200 "GET /downloadclient"
EMBEDDED_USENET=$(echo "$API_BODY" | jq '[.[] | select(.clientType == "embedded_usenet")] | length')
log "Embedded usenet download clients: $EMBEDDED_USENET"

if [[ "$EMBEDDED_USENET" -gt 0 ]]; then
    ok "Embedded usenet clients created from SABnzbd import"
    echo "$API_BODY" | jq -r '.[] | select(.clientType == "embedded_usenet") | "    [\(.id)] \(.name) (enabled=\(.enabled))"' 2>/dev/null || true
else
    warn "No embedded_usenet clients found — checking client_type field name..."
    # Might be a different field name
    echo "$API_BODY" | jq '.' 2>/dev/null | head -20
fi

# Check usenet servers endpoint (reads from TOML)
api GET /api/v1/usenet/servers
if [[ "$API_CODE" == "200" ]]; then
    TOML_SERVERS=$(echo "$API_BODY" | jq 'if type == "array" then length else 0 end')
    ok "Usenet servers from TOML: $TOML_SERVERS"
else
    skip "GET /usenet/servers — HTTP $API_CODE"
fi

# ── 11. Verify data is queryable post-import ──────────────

section "11. Post-Import Functional Tests"

# Series detail — pick the first imported series
api GET /api/v1/series
FIRST_SERIES_ID=$(echo "$API_BODY" | jq '.[0].id // empty')
if [[ -n "$FIRST_SERIES_ID" ]]; then
    api GET "/api/v1/series/$FIRST_SERIES_ID"
    assert_status 200 "GET /series/$FIRST_SERIES_ID — detail view"
    SERIES_TITLE=$(echo "$API_BODY" | jq -r '.title')
    ok "Series detail: $SERIES_TITLE"

    # Episodes for this series
    api GET "/api/v1/series/$FIRST_SERIES_ID/episodes"
    assert_status 200 "GET episodes for $SERIES_TITLE"
    EP_COUNT=$(echo "$API_BODY" | jq 'length')
    log "  Episodes: $EP_COUNT"
fi

# Movie detail — pick the first imported movie
api GET /api/v1/movies
FIRST_MOVIE_ID=$(echo "$API_BODY" | jq '.[0].id // empty')
if [[ -n "$FIRST_MOVIE_ID" ]]; then
    api GET "/api/v1/movies/$FIRST_MOVIE_ID"
    assert_status 200 "GET /movies/$FIRST_MOVIE_ID — detail view"
    MOVIE_TITLE=$(echo "$API_BODY" | jq -r '.title')
    ok "Movie detail: $MOVIE_TITLE"
fi

# Queue, calendar, wanted still work
api GET /api/v1/queue
assert_status 200 "GET /queue"

api GET /api/v1/calendar
assert_status 200 "GET /calendar"

api GET "/api/v1/wanted/missing?page=1&pageSize=10"
assert_status 200 "GET /wanted/missing"

# System health
api GET /api/v1/system/health
assert_status 200 "GET /system/health"

# ── Teardown ──────────────────────────────────────────────

section "Teardown"

compose_nuke "$COMPOSE_FILE" "$PROJECT"
ok "Stack torn down"

# ── Summary ───────────────────────────────────────────────

summary
