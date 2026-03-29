#!/usr/bin/env bash
# test-existing.sh — Stack 2: Validate existing data survives restart/upgrade
#
# Prerequisites: run setup-existing.sh at least once.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
source "$SCRIPT_DIR/common.sh"

COMPOSE_FILE="$SCRIPT_DIR/docker-compose.existing.yml"
PROJECT="ngms-existing"
BASE_URL="http://localhost:9212"

# Load API key from setup phase
if [[ -f "$SCRIPT_DIR/.api-key-existing" ]]; then
    API_KEY=$(cat "$SCRIPT_DIR/.api-key-existing")
    log "Loaded API key: ${API_KEY:0:8}..."
fi

# ── Restart Stack (simulate upgrade) ─────────────────────

section "Stack 2 — Existing/Upgrade Validation"

log "Restarting stack to simulate upgrade..."
compose_down "$COMPOSE_FILE" "$PROJECT"
sleep 2
compose_up "$COMPOSE_FILE" "$PROJECT"
wait_for_health 90

# ── 1. System Status ─────────────────────────────────────

section "1. System Status (post-restart)"

api GET /api/v1/system/status
assert_status 200 "GET /system/status"
assert_json ".firstBoot" "false" "firstBoot still false after restart"
assert_json_exists ".modules" "modules present"

api GET /health
assert_status 200 "GET /health"

# ── 2. Data Persistence — Series ──────────────────────────

section "2. Data Persistence — Series"

api GET /api/v1/series
assert_status 200 "GET /series"
assert_json_min_length "." 1 "series survived restart"

SCRUBS=$(echo "$API_BODY" | jq '[.[] | select(.title | test("scrubs"; "i"))][0] // empty')
if [[ -n "$SCRUBS" && "$SCRUBS" != "null" ]]; then
    SCRUBS_ID=$(echo "$SCRUBS" | jq '.id')
    ok "Scrubs found (ID: $SCRUBS_ID)"

    api GET "/api/v1/series/${SCRUBS_ID}/episodes"
    assert_status 200 "GET episodes for Scrubs"
else
    fail "Scrubs not found after restart"
    SCRUBS_ID=""
fi

# ── 3. Data Persistence — Indexers ────────────────────────

section "3. Data Persistence — Indexers"

api GET /api/v1/indexer
assert_status 200 "GET /indexer"
assert_json_min_length "." 2 "indexers survived restart"

TORRENT_INDEXER=$(echo "$API_BODY" | jq '[.[] | select(.protocol == "torrent")][0] // empty')
USENET_INDEXER=$(echo "$API_BODY" | jq '[.[] | select(.protocol == "usenet")][0] // empty')

if [[ -n "$TORRENT_INDEXER" && "$TORRENT_INDEXER" != "null" ]]; then
    ok "Torrent indexer present: $(echo "$TORRENT_INDEXER" | jq -r '.name')"
else
    fail "No torrent indexer found after restart"
fi

if [[ -n "$USENET_INDEXER" && "$USENET_INDEXER" != "null" ]]; then
    ok "Usenet indexer present: $(echo "$USENET_INDEXER" | jq -r '.name')"
else
    fail "No usenet indexer found after restart"
fi

# ── 4. Data Persistence — Quality Profiles ────────────────

section "4. Data Persistence — Quality Profiles"

api GET /api/v1/qualityprofile
assert_status 200 "GET /qualityprofile"
assert_json_min_length "." 1 "quality profiles survived restart"

# ── 5. Data Persistence — Media Library Folders ───────────

section "5. Data Persistence — Media Library Folders"

api GET /api/v1/medialibraryfolder
assert_status 200 "GET /medialibraryfolder"
assert_json_min_length "." 2 "media library folders survived restart"

# ── 6. Data Persistence — Tags ────────────────────────────

section "6. Data Persistence — Tags"

api GET /api/v1/tag
assert_status 200 "GET /tag"
assert_json_min_length "." 1 "tags survived restart"

TAG=$(echo "$API_BODY" | jq '[.[] | select(.label == "existing-test")][0] // empty')
if [[ -n "$TAG" && "$TAG" != "null" ]]; then
    ok "Tag 'existing-test' persisted"
else
    fail "Tag 'existing-test' not found after restart"
fi

# ── 7. Data Persistence — Naming Config ───────────────────

section "7. Data Persistence — Naming Config"

api GET /api/v1/config/naming
assert_status 200 "GET /config/naming"

# ── 8. Engine Status ──────────────────────────────────────

section "8. Engine Status"

api GET /api/v1/torrent/status
assert_status 200 "GET /torrent/status — engine alive after restart"

api GET /api/v1/torrent/list
assert_status 200 "GET /torrent/list"

api GET /api/v1/usenet/status
assert_status 200 "GET /usenet/status — engine alive after restart"

# ── 9. Functional Tests ──────────────────────────────────

section "9. Functional Tests (post-restart)"

# Search releases
api GET "/api/v1/release?term=Scrubs+S01E02&mediaType=series&qualityProfileId=1"
if [[ "$API_CODE" == "200" ]]; then
    RELEASE_COUNT=$(echo "$API_BODY" | jq 'if type == "array" then length else 0 end')
    ok "Release search works after restart ($RELEASE_COUNT results)"
else
    warn "Release search returned HTTP $API_CODE (indexers may need warm-up)"
fi

# Queue
api GET /api/v1/queue
assert_status 200 "GET /queue"

# History
api GET "/api/v1/history?page=1&pageSize=10"
assert_status 200 "GET /history"

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

# System health
api GET /api/v1/system/health
assert_status 200 "GET /system/health"

# Metrics
api GET /metrics
assert_status 200 "GET /metrics"

# Logs
api GET /api/v1/log
assert_status 200 "GET /log"

# ── 10. Write Operations (non-destructive) ────────────────

section "10. Write Operations (non-destructive)"

# Create + delete tag
api POST /api/v1/tag '{"label": "upgrade-test-run"}'
assert_status 201 "POST /tag — create new tag"
NEW_TAG_ID=$(echo "$API_BODY" | jq '.id')

api DELETE "/api/v1/tag/${NEW_TAG_ID}"
if [[ "$API_CODE" =~ ^2 ]]; then
    ok "DELETE /tag/$NEW_TAG_ID — cleanup"
else
    warn "DELETE /tag/$NEW_TAG_ID — HTTP $API_CODE"
fi

# Update series monitored status (toggle and restore)
if [[ -n "${SCRUBS_ID:-}" ]]; then
    api PUT "/api/v1/series/${SCRUBS_ID}" '{"monitored": false}'
    if [[ "$API_CODE" =~ ^2 ]]; then
        ok "PUT /series — set unmonitored"
    else
        fail "PUT /series — HTTP $API_CODE"
    fi

    api PUT "/api/v1/series/${SCRUBS_ID}" '{"monitored": true}'
    if [[ "$API_CODE" =~ ^2 ]]; then
        ok "PUT /series — restore monitored"
    else
        fail "PUT /series — HTTP $API_CODE"
    fi
fi

# ── Done (leave stack running) ────────────────────────────

section "Stack Left Running"
log "Volumes are preserved. To stop: docker compose -f docker-compose.existing.yml -p ngms-existing down"

summary
