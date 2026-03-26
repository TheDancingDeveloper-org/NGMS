#!/usr/bin/env bash
# common.sh — Shared helpers for StackArr e2e test harness
set -euo pipefail

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
NC='\033[0m'

# Counters
PASS_COUNT=0
FAIL_COUNT=0
SKIP_COUNT=0

# API key (set after setup/init returns one)
API_KEY="${API_KEY:-}"

log()  { echo -e "${CYAN}[$(date +%H:%M:%S)]${NC} $*"; }
ok()   { echo -e "${GREEN}  ✓ $*${NC}"; PASS_COUNT=$((PASS_COUNT + 1)); }
fail() { echo -e "${RED}  ✗ $*${NC}"; FAIL_COUNT=$((FAIL_COUNT + 1)); }
warn() { echo -e "${YELLOW}  ⚠ $*${NC}"; }
skip() { echo -e "${YELLOW}  ⊘ $*${NC}"; SKIP_COUNT=$((SKIP_COUNT + 1)); }

section() {
    echo ""
    echo -e "${CYAN}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
    echo -e "${CYAN}  $*${NC}"
    echo -e "${CYAN}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
}

summary() {
    echo ""
    echo -e "${CYAN}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
    echo -e "${CYAN}  TEST SUMMARY${NC}"
    echo -e "${CYAN}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
    echo -e "  ${GREEN}Passed:  ${PASS_COUNT}${NC}"
    echo -e "  ${RED}Failed:  ${FAIL_COUNT}${NC}"
    echo -e "  ${YELLOW}Skipped: ${SKIP_COUNT}${NC}"
    echo ""
    if [[ $FAIL_COUNT -gt 0 ]]; then
        echo -e "  ${RED}RESULT: FAIL${NC}"
        return 1
    else
        echo -e "  ${GREEN}RESULT: PASS${NC}"
        return 0
    fi
}

# ── API helpers ──────────────────────────────────────────

# Usage: api GET /api/v1/series
# Usage: api POST /api/v1/series '{"name":"foo"}'
api() {
    local method="$1"
    local path="$2"
    local data="${3:-}"
    local url="${BASE_URL}${path}"

    local args=(-s -S -w '\n%{http_code}' -X "$method" -H 'Content-Type: application/json')
    if [[ -n "$API_KEY" ]]; then
        args+=(-H "X-Api-Key: $API_KEY")
    fi
    if [[ -n "$data" ]]; then
        args+=(-d "$data")
    fi

    local response
    response=$(curl "${args[@]}" "$url" 2>&1) || true

    local http_code
    http_code=$(echo "$response" | tail -1)
    local body
    body=$(echo "$response" | sed '$d')

    # Export for callers
    API_BODY="$body"
    API_CODE="$http_code"
}

# Upload multipart file
api_upload() {
    local path="$1"
    local field="$2"
    local file="$3"
    local url="${BASE_URL}${path}"

    local response
    response=$(curl -s -S -w '\n%{http_code}' -X POST -F "${field}=@${file}" "$url" 2>&1) || true

    API_CODE=$(echo "$response" | tail -1)
    API_BODY=$(echo "$response" | sed '$d')
}

# Assert HTTP status code
assert_status() {
    local expected="$1"
    local label="$2"
    if [[ "$API_CODE" == "$expected" ]]; then
        ok "$label (HTTP $API_CODE)"
    else
        fail "$label — expected HTTP $expected, got $API_CODE"
        echo "    Response: ${API_BODY:0:200}"
    fi
}

# Assert JSON field equals value (uses jq)
assert_json() {
    local jq_expr="$1"
    local expected="$2"
    local label="$3"
    local actual
    actual=$(echo "$API_BODY" | jq -r "$jq_expr" 2>/dev/null) || actual="<jq error>"
    if [[ "$actual" == "$expected" ]]; then
        ok "$label (=$expected)"
    else
        fail "$label — expected '$expected', got '$actual'"
    fi
}

# Assert JSON field is not null/empty
assert_json_exists() {
    local jq_expr="$1"
    local label="$2"
    local actual
    actual=$(echo "$API_BODY" | jq -r "$jq_expr" 2>/dev/null) || actual=""
    if [[ -n "$actual" && "$actual" != "null" ]]; then
        ok "$label (=$actual)"
    else
        fail "$label — value is null or empty"
    fi
}

# Assert JSON array has at least N items
assert_json_min_length() {
    local jq_expr="$1"
    local min="$2"
    local label="$3"
    local count
    count=$(echo "$API_BODY" | jq "$jq_expr | length" 2>/dev/null) || count=0
    if [[ "$count" -ge "$min" ]]; then
        ok "$label (count=$count >= $min)"
    else
        fail "$label — expected >= $min items, got $count"
    fi
}

# ── Wait helpers ─────────────────────────────────────────

wait_for_health() {
    local url="${BASE_URL}/health"
    local max_attempts="${1:-60}"
    local attempt=0
    log "Waiting for $url ..."
    while [[ $attempt -lt $max_attempts ]]; do
        if curl -sf "$url" >/dev/null 2>&1; then
            ok "Service healthy after ${attempt}s"
            return 0
        fi
        sleep 1
        ((attempt++))
    done
    fail "Service not healthy after ${max_attempts}s"
    return 1
}

# Wait for a queue item to reach a target status
# Usage: wait_for_queue_status <download_id> <target_status> <timeout_secs>
wait_for_queue_status() {
    local download_id="$1"
    local target="$2"
    local timeout="${3:-300}"
    local elapsed=0

    log "Waiting for download $download_id to reach status '$target' (timeout ${timeout}s)..."
    while [[ $elapsed -lt $timeout ]]; do
        api GET "/api/v1/queue"
        local status
        status=$(echo "$API_BODY" | jq -r ".[] | select(.downloadId == \"$download_id\") | .status" 2>/dev/null) || status=""

        if [[ "$status" == "$target" ]]; then
            ok "Download $download_id reached status '$target' in ${elapsed}s"
            return 0
        fi

        # Check if it disappeared from queue (completed + imported)
        if [[ -z "$status" && "$target" == "completed" ]]; then
            # Check history for import success
            api GET "/api/v1/history?page=1&pageSize=50"
            local imported
            imported=$(echo "$API_BODY" | jq -r ".records[]? | select(.downloadId == \"$download_id\" and .eventType == \"import_success\") | .id" 2>/dev/null) || imported=""
            if [[ -n "$imported" ]]; then
                ok "Download $download_id imported successfully in ${elapsed}s"
                return 0
            fi
        fi

        if [[ -n "$status" ]]; then
            log "  ... status=$status (${elapsed}s)"
        fi
        sleep 5
        ((elapsed += 5))
    done
    fail "Download $download_id did not reach '$target' within ${timeout}s (last status: ${status:-unknown})"
    return 1
}

# ── Docker helpers ───────────────────────────────────────

compose_up() {
    local compose_file="$1"
    local project="$2"
    log "Starting stack: $project"
    docker compose -f "$compose_file" -p "$project" up -d --wait 2>&1 | tail -5
}

compose_down() {
    local compose_file="$1"
    local project="$2"
    log "Stopping stack: $project"
    docker compose -f "$compose_file" -p "$project" down 2>&1 | tail -3
}

compose_nuke() {
    local compose_file="$1"
    local project="$2"
    log "Nuking stack: $project (including volumes)"
    docker compose -f "$compose_file" -p "$project" down -v 2>&1 | tail -3
}

compose_logs() {
    local compose_file="$1"
    local project="$2"
    local service="${3:-}"
    if [[ -n "$service" ]]; then
        docker compose -f "$compose_file" -p "$project" logs --tail=50 "$service"
    else
        docker compose -f "$compose_file" -p "$project" logs --tail=50
    fi
}
