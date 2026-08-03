#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-only
# Copyright (C) 2026 The StackArr Authors

# setup-existing.sh — One-time initialisation for Stack 2 (Existing/Upgrade)
#
# Run this ONCE to bootstrap the "existing" stack with data.
# Subsequent test runs use test-existing.sh (volumes persist).
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
source "$SCRIPT_DIR/common.sh"

COMPOSE_FILE="$SCRIPT_DIR/docker-compose.existing.yml"
PROJECT="stackarr-existing"
BASE_URL="http://localhost:9212"
MEDIA_BASE="/mnt/4tnvme/docker/volumes/stackrr_test_media/existing"

# ── Start ────────────────────────────────────────────────

section "Stack 2 — Setup Existing"

mkdir -p "$MEDIA_BASE/TV" "$MEDIA_BASE/Movies"

compose_up "$COMPOSE_FILE" "$PROJECT"
wait_for_health 90

# ── Check if already initialised ─────────────────────────

api GET /api/v1/system/status
if [[ "$(echo "$API_BODY" | jq -r '.firstBoot')" == "false" ]]; then
    log "Stack already initialised — skipping first-boot setup"
    log "Run test-existing.sh to validate."
    exit 0
fi

# ── First-Boot Setup ─────────────────────────────────────

section "First-Boot Setup"

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
  "instanceName": "StackArr-Test-Existing"
}
JSON
)

api POST /api/v1/setup/init "$SETUP_PAYLOAD"
assert_status 201 "POST /setup/init"
assert_json ".success" "true" "setup success"

# Capture API key
API_KEY=$(echo "$API_BODY" | jq -r '.apiKey // empty')
if [[ -n "$API_KEY" ]]; then
    ok "API key received: ${API_KEY:0:8}..."
    # Save for test-existing.sh
    echo "$API_KEY" > "$SCRIPT_DIR/.api-key-existing"
else
    warn "No API key returned from setup"
fi

# ── Add Indexers ──────────────────────────────────────────

section "Add Indexers"

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
assert_status 201 "POST /indexer — 1337x"

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
assert_status 201 "POST /indexer — Indexarr NZB"

# ── Add Series — Scrubs ──────────────────────────────────

section "Add Series — Scrubs"

api GET /api/v1/qualityprofile
DEFAULT_PROFILE_ID=$(echo "$API_BODY" | jq '.[0].id')

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

# ── Add a Tag ─────────────────────────────────────────────

api POST /api/v1/tag '{"label": "existing-test"}'
assert_status 201 "POST /tag — create"

# ── Done ──────────────────────────────────────────────────

section "Setup Complete"

api GET /api/v1/system/status
assert_json ".firstBoot" "false" "firstBoot is false"

api GET /api/v1/series
SERIES_COUNT=$(echo "$API_BODY" | jq 'length')
log "Series in library: $SERIES_COUNT"

api GET /api/v1/indexer
INDEXER_COUNT=$(echo "$API_BODY" | jq 'length')
log "Indexers configured: $INDEXER_COUNT"

ok "Stack 2 is initialised. Run test-existing.sh to validate."
echo ""

summary
