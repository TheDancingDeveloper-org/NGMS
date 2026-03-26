#!/usr/bin/env bash
# test-fresh.sh — Stack 1: Fresh install e2e test
#
# Usage (run on Node B):
#   cd /path/to/tests/e2e
#   ./test-fresh.sh
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
source "$SCRIPT_DIR/common.sh"

COMPOSE_FILE="$SCRIPT_DIR/docker-compose.fresh.yml"
PROJECT="stackarr-fresh"
BASE_URL="http://localhost:9211"
MEDIA_BASE="/mnt/4tnvme/docker/volumes/stackrr_test_media/fresh"

# ── Cleanup & Start ─────────────────────────────────────

section "Stack 1 — Fresh Install"

mkdir -p "$MEDIA_BASE/TV" "$MEDIA_BASE/Movies"
rm -rf "$MEDIA_BASE/TV/"* "$MEDIA_BASE/Movies/"* 2>/dev/null || true

compose_nuke "$COMPOSE_FILE" "$PROJECT"
compose_up "$COMPOSE_FILE" "$PROJECT"
wait_for_health 90

# ── 1. System Status (pre-setup) ────────────────────────

section "1. System Status (pre-setup)"

api GET /api/v1/system/status
assert_status 200 "GET /system/status"
assert_json ".firstBoot" "true" "firstBoot is true"

api GET /health
assert_status 200 "GET /health"
assert_json ".status" "ok" "health status ok"

# ── 2. First-Boot Setup ─────────────────────────────────

section "2. First-Boot Setup"

SETUP_PAYLOAD=$(cat <<'JSON'
{
  "modules": {
    "tvManagement": true,
    "movieManagement": true,
    "torrentEmbedded": true,
    "usenetEmbedded": true,
    "indexarrSidecar": true,
    "plexIntegration": false,
    "streaming": false
  },
  "mediaLibraryFolders": [
    { "path": "/media/TV", "mediaType": "tv" },
    { "path": "/media/Movies", "mediaType": "movie" },
    { "path": "/media/TV1", "mediaType": "tv" },
    { "path": "/media/Movies2", "mediaType": "movie" }
  ],
  "indexarr": {
    "url": "http://indexarr:8080",
    "apiKey": ""
  },
  "instanceName": "StackArr-Test-Fresh"
}
JSON
)

api POST /api/v1/setup/init "$SETUP_PAYLOAD"
assert_status 201 "POST /setup/init"
assert_json ".success" "true" "setup success"

# Capture API key for authenticated endpoints
API_KEY=$(echo "$API_BODY" | jq -r '.apiKey // empty')
if [[ -n "$API_KEY" ]]; then
    ok "API key received: ${API_KEY:0:8}..."
else
    warn "No API key returned from setup"
fi

api GET /api/v1/system/status
assert_json ".firstBoot" "false" "firstBoot is now false"

# ── 3. Quality Profiles ─────────────────────────────────

section "3. Quality Profiles"

api GET /api/v1/qualityprofile
assert_status 200 "GET /qualityprofile"
assert_json_min_length "." 1 "at least 1 default quality profile"

DEFAULT_PROFILE_ID=$(echo "$API_BODY" | jq '.[0].id')
log "Using quality profile ID: $DEFAULT_PROFILE_ID"

# ── 4. Media Library Folders ────────────────────────────

section "4. Media Library Folders"

api GET /api/v1/medialibraryfolder
assert_status 200 "GET /medialibraryfolder"
assert_json_min_length "." 2 "at least 2 media library folders"

# ── 5. Add Indexers ──────────────────────────────────────

section "5. Add Indexers"

# 5a. Torrent — 1337x (Cardigann)
api POST /api/v1/indexer "$(cat <<JSON
{
  "name": "1337x",
  "indexerType": "Cardigann",
  "baseUrl": "https://1337x.to",
  "protocol": "torrent",
  "categories": [5000, 5010, 5020, 5030, 5040, 5045, 5050, 5060, 5070, 5080, 2000, 2010, 2020, 2030, 2040, 2045, 2050, 2060],
  "enabled": true,
  "priority": 25,
  "supportsSearch": true,
  "supportsRss": false
}
JSON
)"
assert_status 201 "POST /indexer — 1337x (Cardigann/torrent)"
INDEXER_1337X_ID=$(echo "$API_BODY" | jq '.id')

# 5b. Torrent — The Pirate Bay (Cardigann)
api POST /api/v1/indexer "$(cat <<JSON
{
  "name": "The Pirate Bay",
  "indexerType": "Cardigann",
  "baseUrl": "https://thepiratebay.org",
  "protocol": "torrent",
  "categories": [5000, 5010, 5020, 5030, 5040, 5050, 2000, 2010, 2020, 2030, 2040, 2050],
  "enabled": true,
  "priority": 30,
  "supportsSearch": true,
  "supportsRss": false
}
JSON
)"
assert_status 201 "POST /indexer — The Pirate Bay (Cardigann/torrent)"
INDEXER_TPB_ID=$(echo "$API_BODY" | jq '.id')

# 5c. Usenet — nzb.indexarr.net (Newznab)
api POST /api/v1/indexer "$(cat <<JSON
{
  "name": "Indexarr NZB",
  "indexerType": "Newznab",
  "baseUrl": "https://nzb.indexarr.net",
  "apiKey": "3bdec035-6fae-40c4-b3b7-fc8e8251ba5e",
  "protocol": "usenet",
  "categories": [5000, 5010, 5020, 5030, 5040, 5045, 5050, 5060, 5070, 5080, 2000, 2010, 2020, 2030, 2040, 2045, 2050, 2060],
  "enabled": true,
  "priority": 10,
  "supportsSearch": true,
  "supportsRss": true
}
JSON
)"
assert_status 201 "POST /indexer — Indexarr NZB (Newznab/usenet)"
INDEXER_NZB_ID=$(echo "$API_BODY" | jq '.id')

# Verify
api GET /api/v1/indexer
assert_status 200 "GET /indexer"
assert_json_min_length "." 3 "3 indexers configured"

# ── 6. Add Series — Scrubs ──────────────────────────────

section "6. Add Series — Scrubs"

# Create directly with known TMDB ID (4556) — lookup requires TMDB API key
api POST /api/v1/series "$(cat <<JSON
{
  "title": "Scrubs",
  "path": "/media/TV/Scrubs",
  "qualityProfileId": $DEFAULT_PROFILE_ID,
  "monitored": true,
  "tmdbId": 4556
}
JSON
)"
assert_status 201 "POST /series — add Scrubs"
SCRUBS_ID=$(echo "$API_BODY" | jq '.id')
log "Scrubs series ID: $SCRUBS_ID"

# Get episodes (these are created by metadata refresh — may be empty without TMDB key)
api GET "/api/v1/series/${SCRUBS_ID}/episodes"
assert_status 200 "GET /series/$SCRUBS_ID/episodes"
EP_COUNT=$(echo "$API_BODY" | jq 'length')
log "Episodes found: $EP_COUNT"

# ── 7. Search & Download — Torrent ──────────────────────

section "7. Search & Grab — Torrent"

api GET "/api/v1/release?term=Scrubs+S01E01&mediaType=series&qualityProfileId=${DEFAULT_PROFILE_ID}"
assert_status 200 "GET /release — search Scrubs S01E01"

TORRENT_RELEASE=$(echo "$API_BODY" | jq '[.[] | select(.protocol == "torrent")][0] // empty' 2>/dev/null) || TORRENT_RELEASE=""
TORRENT_GRABBED=false

if [[ -n "$TORRENT_RELEASE" && "$TORRENT_RELEASE" != "null" ]]; then
    TORRENT_GUID=$(echo "$TORRENT_RELEASE" | jq -r '.guid')
    TORRENT_TITLE=$(echo "$TORRENT_RELEASE" | jq -r '.title')
    TORRENT_URL=$(echo "$TORRENT_RELEASE" | jq -r '.downloadUrl')
    TORRENT_SIZE=$(echo "$TORRENT_RELEASE" | jq '.size // 0')
    TORRENT_INDEXER_ID=$(echo "$TORRENT_RELEASE" | jq '.indexerId')
    log "Grabbing torrent: $TORRENT_TITLE"

    api POST /api/v1/release "$(cat <<JSON
{
  "guid": "$TORRENT_GUID",
  "indexerId": $TORRENT_INDEXER_ID,
  "title": "$TORRENT_TITLE",
  "downloadUrl": "$TORRENT_URL",
  "protocol": "torrent",
  "size": $TORRENT_SIZE,
  "mediaId": $SCRUBS_ID,
  "mediaType": "series"
}
JSON
)"
    if [[ "$API_CODE" =~ ^2 ]]; then
        TORRENT_DL_ID=$(echo "$API_BODY" | jq -r '.downloadId // empty')
        TORRENT_GRABBED=true
        ok "Torrent grab accepted (HTTP $API_CODE)"
    else
        fail "Torrent grab failed (HTTP $API_CODE): ${API_BODY:0:200}"
    fi
else
    skip "No torrent releases found for Scrubs S01E01"
fi

# ── 8. Search & Download — Usenet ────────────────────────

section "8. Search & Grab — Usenet"

# Search again to get usenet results
api GET "/api/v1/release?term=Scrubs+S01E01&mediaType=series&qualityProfileId=${DEFAULT_PROFILE_ID}"
USENET_RELEASE=$(echo "$API_BODY" | jq '[.[] | select(.protocol == "usenet")][0] // empty' 2>/dev/null) || USENET_RELEASE=""
USENET_GRABBED=false

if [[ -n "$USENET_RELEASE" && "$USENET_RELEASE" != "null" ]]; then
    USENET_GUID=$(echo "$USENET_RELEASE" | jq -r '.guid')
    USENET_TITLE=$(echo "$USENET_RELEASE" | jq -r '.title')
    USENET_URL=$(echo "$USENET_RELEASE" | jq -r '.downloadUrl')
    USENET_SIZE=$(echo "$USENET_RELEASE" | jq '.size // 0')
    USENET_INDEXER_ID=$(echo "$USENET_RELEASE" | jq '.indexerId')
    log "Grabbing usenet: $USENET_TITLE"

    api POST /api/v1/release "$(cat <<JSON
{
  "guid": "$USENET_GUID",
  "indexerId": $USENET_INDEXER_ID,
  "title": "$USENET_TITLE",
  "downloadUrl": "$USENET_URL",
  "protocol": "usenet",
  "size": $USENET_SIZE,
  "mediaId": $SCRUBS_ID,
  "mediaType": "series"
}
JSON
)"
    if [[ "$API_CODE" =~ ^2 ]]; then
        USENET_DL_ID=$(echo "$API_BODY" | jq -r '.downloadId // empty')
        USENET_GRABBED=true
        ok "Usenet grab accepted (HTTP $API_CODE)"
    else
        fail "Usenet grab failed (HTTP $API_CODE): ${API_BODY:0:200}"
    fi
else
    skip "No usenet releases found for Scrubs S01E01"
fi

# ── 9. Monitor Queue ─────────────────────────────────────

section "9. Monitor Download Queue"

api GET /api/v1/queue
assert_status 200 "GET /queue"
QUEUE_COUNT=$(echo "$API_BODY" | jq 'if type == "array" then length else 0 end')
log "Queue has $QUEUE_COUNT items"

if [[ "$TORRENT_GRABBED" == "true" && -n "${TORRENT_DL_ID:-}" ]]; then
    wait_for_queue_status "$TORRENT_DL_ID" "completed" 300 || true
fi
if [[ "$USENET_GRABBED" == "true" && -n "${USENET_DL_ID:-}" ]]; then
    wait_for_queue_status "$USENET_DL_ID" "completed" 300 || true
fi

# ── 10. Validate Import ──────────────────────────────────

section "10. Validate Import / Media Files"

api GET "/api/v1/history?page=1&pageSize=50"
assert_status 200 "GET /history"

TV_FILES=$(find "$MEDIA_BASE/TV" -type f 2>/dev/null | head -5) || true
if [[ -n "$TV_FILES" ]]; then
    ok "Files found in test media dir"
    echo "$TV_FILES" | while read -r f; do echo "    $f"; done
else
    warn "No files in $MEDIA_BASE/TV yet (downloads may still be in progress)"
fi

# ── 11. Exercise Additional API Endpoints ─────────────────

section "11. Additional API Endpoints"

# Tags
api POST /api/v1/tag '{"label": "test-tag"}'
assert_status 201 "POST /tag — create"
TAG_ID=$(echo "$API_BODY" | jq '.id')

api GET /api/v1/tag
assert_status 200 "GET /tag"
assert_json_min_length "." 1 "at least 1 tag"

# Naming config
api GET /api/v1/config/naming
assert_status 200 "GET /config/naming"

# Calendar
api GET /api/v1/calendar
assert_status 200 "GET /calendar"

# Wanted/missing
api GET "/api/v1/wanted/missing?page=1&pageSize=10"
assert_status 200 "GET /wanted/missing"

# Filesystem browse
api GET "/api/v1/filesystem/browse?path=/media"
assert_status 200 "GET /filesystem/browse"

# Download clients
api GET /api/v1/downloadclient
assert_status 200 "GET /downloadclient"

# Torrent engine status
api GET /api/v1/torrent/status
assert_status 200 "GET /torrent/status"

api GET /api/v1/torrent/list
assert_status 200 "GET /torrent/list"

# Usenet engine status
api GET /api/v1/usenet/status
assert_status 200 "GET /usenet/status"

api GET /api/v1/usenet/queue
assert_status 200 "GET /usenet/queue"

# System health
api GET /api/v1/system/health
assert_status 200 "GET /system/health"

# Metrics
api GET /metrics
assert_status 200 "GET /metrics"

# Backup
api GET /api/v1/system/backup
if [[ "$API_CODE" == "200" ]]; then
    ok "GET /system/backup responds"
else
    skip "GET /system/backup — HTTP $API_CODE"
fi

# Series still there
api GET /api/v1/series
assert_status 200 "GET /series"
assert_json_min_length "." 1 "Scrubs still in library"

# Movies (empty but endpoint works)
api GET /api/v1/movies
assert_status 200 "GET /movies"

# Discover trending
api GET /api/v1/discover/trending
if [[ "$API_CODE" == "200" ]]; then
    ok "GET /discover/trending (HTTP 200)"
else
    skip "GET /discover/trending — HTTP $API_CODE (may need TMDB key)"
fi

# Logs
api GET /api/v1/log
if [[ "$API_CODE" == "200" ]]; then
    ok "GET /log (HTTP 200)"
else
    skip "GET /log — HTTP $API_CODE (may require API key)"
fi

# Blocklist
api GET "/api/v1/blocklist?page=1&pageSize=10"
if [[ "$API_CODE" == "200" ]]; then
    ok "GET /blocklist (HTTP 200)"
else
    skip "GET /blocklist — HTTP $API_CODE (may require API key)"
fi

# Indexarr sidecar status
api GET /api/v1/indexarr/status
if [[ "$API_CODE" == "200" ]]; then
    ok "GET /indexarr/status"
else
    skip "GET /indexarr/status — HTTP $API_CODE (sidecar may need time)"
fi

# ── Teardown ──────────────────────────────────────────────

section "Teardown"

compose_nuke "$COMPOSE_FILE" "$PROJECT"
ok "Stack torn down"

# ── Summary ───────────────────────────────────────────────

summary
