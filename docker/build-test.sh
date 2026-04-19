#!/usr/bin/env bash
# build-test.sh — Build the three test Docker images from Dockerfile.test
#
# Usage:
#   ./docker/build-test.sh              — Build all three images
#   ./docker/build-test.sh app          — Build only the app image
#   ./docker/build-test.sh runner       — Build only the test runner image
#   ./docker/build-test.sh playwright   — Build only the Playwright image
#
# Images produced:
#   stackarr-test-app:latest        — StackArr runtime (for E2E tests)
#   stackarr-test-runner:latest     — Rust test runner (unit + integration tests)
#   stackarr-test-playwright:latest — Playwright E2E test runner
#
# In CI, call this script before docker-compose.test.yml.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
DOCKERFILE="$SCRIPT_DIR/Dockerfile.test"
# ── Colors ──────────────────────────────────────────────────
RED='\033[0;31m'
GREEN='\033[0;32m'
CYAN='\033[0;36m'
BOLD='\033[1m'
NC='\033[0m'

log()  { echo -e "${CYAN}[$(date +%H:%M:%S)]${NC} $*"; }
ok()   { echo -e "${GREEN}  ✓ $*${NC}"; }
fail() { echo -e "${RED}  ✗ $*${NC}"; }

build_target() {
  local target="$1"
  local tag="$2"
  log "Building ${BOLD}${tag}${NC} (target: ${target})..."
  docker build \
    -f "$DOCKERFILE" \
    --target "$target" \
    -t "$tag" \
    "$REPO_ROOT"
  ok "$tag built"
}

# ── Main ────────────────────────────────────────────────────
case "${1:-all}" in
  app)
    build_target app stackarr-test-app:latest
    ;;
  runner)
    build_target builder stackarr-test-runner:latest
    ;;
  playwright)
    build_target playwright stackarr-test-playwright:latest
    ;;
  all)
    log "Building all test images..."
    echo ""
    # Build app + runner in sequence (they share most layers)
    build_target app stackarr-test-app:latest
    echo ""
    build_target builder stackarr-test-runner:latest
    echo ""
    build_target playwright stackarr-test-playwright:latest
    echo ""
    log "All images built:"
    docker images --filter "reference=stackarr-test-*" --format "  {{.Repository}}:{{.Tag}}\t{{.Size}}"
    ;;
  *)
    echo "Usage: $0 {all|app|runner|playwright}"
    exit 1
    ;;
esac
