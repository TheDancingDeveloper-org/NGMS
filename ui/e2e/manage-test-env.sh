#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-only
# Copyright (C) 2026 The StackArr Authors

# manage-test-env.sh — Manage the NGMS GUI test environment on Node B
#
# Usage:
#   ./e2e/manage-test-env.sh up       — Start fresh test environment (nuke + recreate)
#   ./e2e/manage-test-env.sh down     — Tear down (stop containers, keep data)
#   ./e2e/manage-test-env.sh nuke     — Full teardown (stop + wipe all data)
#   ./e2e/manage-test-env.sh status   — Check container status
#   ./e2e/manage-test-env.sh logs     — Show recent logs
#   ./e2e/manage-test-env.sh reset    — Nuke + start fresh (full cycle)
#   ./e2e/manage-test-env.sh test     — Reset env, then run Playwright live tests
set -euo pipefail

# ── Config ──────────────────────────────────────────────────
NODE_B="${NODE_B_SSH:-node-b}"
REMOTE_DIR="/mnt/2tnvme/docker/volumes/ngms_test"
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
# Repo root: ui/e2e/../../ = repo root
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
LOCAL_COMPOSE="$REPO_ROOT/tests/e2e/docker-compose.ngms-test.yml"
LOCAL_CONFIG="$REPO_ROOT/tests/e2e/config-ngms-test.toml"
STACKARR_URL="${PLAYWRIGHT_BASE_URL:-http://node-b:9311}"

SSH_OPTS="-o ConnectTimeout=10 -o BatchMode=yes"

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
NC='\033[0m'

log()  { echo -e "${CYAN}[$(date +%H:%M:%S)]${NC} $*"; }
ok()   { echo -e "${GREEN}  ✓ $*${NC}"; }
fail() { echo -e "${RED}  ✗ $*${NC}"; }
warn() { echo -e "${YELLOW}  ⚠ $*${NC}"; }

remote() { ssh $SSH_OPTS "$NODE_B" "$@"; }

# ── Commands ────────────────────────────────────────────────

cmd_sync() {
  log "Syncing compose and config to Node B..."
  scp $SSH_OPTS "$LOCAL_COMPOSE" "$NODE_B:$REMOTE_DIR/docker-compose.yml"
  scp $SSH_OPTS "$LOCAL_CONFIG" "$NODE_B:$REMOTE_DIR/config/stackarr.toml"
  ok "Files synced"
}

cmd_nuke() {
  log "Nuking test environment (all data will be lost)..."
  remote "cd $REMOTE_DIR && docker compose down --remove-orphans 2>/dev/null || true"
  # Wipe data dirs via docker (avoids permission issues)
  remote "docker run --rm -v $REMOTE_DIR:/target alpine sh -c '
    rm -rf /target/pgdata/* /target/indexarr-data/* /target/config/db* /target/config/*.db
    rm -rf /target/downloads/torrent/incomplete/* /target/downloads/torrent/complete/*
    rm -rf /target/downloads/usenet/incomplete/* /target/downloads/usenet/complete/*
  '" 2>/dev/null || true
  ok "Environment nuked"
}

cmd_up() {
  cmd_sync
  log "Starting test environment..."
  remote "cd $REMOTE_DIR && docker compose pull --quiet 2>/dev/null && docker compose up -d 2>&1"
  log "Waiting for StackArr to be healthy..."
  local attempt=0
  while [[ $attempt -lt 60 ]]; do
    if curl -sf "$STACKARR_URL/health" >/dev/null 2>&1; then
      ok "StackArr healthy at $STACKARR_URL"
      return 0
    fi
    sleep 1
    attempt=$((attempt + 1))
  done
  fail "StackArr not healthy after 60s"
  cmd_logs
  return 1
}

cmd_down() {
  log "Stopping test environment..."
  remote "cd $REMOTE_DIR && docker compose down 2>&1"
  ok "Environment stopped"
}

cmd_status() {
  log "Container status:"
  remote "docker ps --filter 'name=ngms-test' --format 'table {{.Names}}\t{{.Status}}\t{{.Ports}}'" || true
  echo ""
  # Quick health check
  if curl -sf "$STACKARR_URL/health" >/dev/null 2>&1; then
    local status
    status=$(curl -sf "$STACKARR_URL/api/v1/system/status" 2>/dev/null)
    local first_boot
    first_boot=$(echo "$status" | python3 -c "import sys,json; print(json.load(sys.stdin).get('firstBoot','?'))" 2>/dev/null || echo "?")
    ok "StackArr reachable — firstBoot=$first_boot"
  else
    warn "StackArr not reachable at $STACKARR_URL"
  fi
}

cmd_logs() {
  log "Recent logs:"
  remote "cd $REMOTE_DIR && docker compose logs --tail=30 2>&1"
}

cmd_reset() {
  cmd_nuke
  cmd_up
}

cmd_test() {
  log "Full test cycle: reset environment + run Playwright live tests"
  cmd_reset
  echo ""
  log "Running Playwright live tests..."
  cd "$SCRIPT_DIR/.."
  PLAYWRIGHT_LIVE=1 npx playwright test live.spec.ts
}

# ── Main ────────────────────────────────────────────────────

case "${1:-help}" in
  up)     cmd_up ;;
  down)   cmd_down ;;
  nuke)   cmd_nuke ;;
  status) cmd_status ;;
  logs)   cmd_logs ;;
  reset)  cmd_reset ;;
  sync)   cmd_sync ;;
  test)   cmd_test ;;
  *)
    echo "Usage: $0 {up|down|nuke|status|logs|reset|sync|test}"
    echo ""
    echo "Commands:"
    echo "  up      — Sync config + start containers + wait for health"
    echo "  down    — Stop containers (keep data)"
    echo "  nuke    — Stop containers + wipe all data"
    echo "  status  — Show container status + health check"
    echo "  logs    — Show recent container logs"
    echo "  reset   — Nuke + start fresh"
    echo "  sync    — Copy compose/config to Node B (no restart)"
    echo "  test    — Reset env + run Playwright live tests"
    ;;
esac
