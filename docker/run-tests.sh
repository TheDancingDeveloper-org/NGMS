#!/usr/bin/env bash
# run-tests.sh — Orchestrate the StackArr Docker test suite
#
# Expects pre-built images. Run build-test.sh first, or pass --build.
#
# Usage:
#   ./docker/run-tests.sh              — Run all test suites (expects images exist)
#   ./docker/run-tests.sh --build      — Build images then run all tests
#   ./docker/run-tests.sh unit         — Unit tests only
#   ./docker/run-tests.sh integration  — Integration tests only
#   ./docker/run-tests.sh e2e          — All E2E tests (mocked + live)
#   ./docker/run-tests.sh e2e-mocked   — Mocked E2E tests only
#   ./docker/run-tests.sh e2e-live     — Live E2E tests only
#   ./docker/run-tests.sh build        — Build images only (alias for build-test.sh)
#   ./docker/run-tests.sh down         — Tear down everything
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
COMPOSE_FILE="$SCRIPT_DIR/docker-compose.test.yml"
COMPOSE="docker compose -f $COMPOSE_FILE -p stackarr-test"

# ── Colors ──────────────────────────────────────────────────
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
BOLD='\033[1m'
NC='\033[0m'

log()     { echo -e "${CYAN}[$(date +%H:%M:%S)]${NC} $*"; }
ok()      { echo -e "${GREEN}  ✓ $*${NC}"; }
fail()    { echo -e "${RED}  ✗ $*${NC}"; }
warn()    { echo -e "${YELLOW}  ⚠ $*${NC}"; }
header()  { echo -e "\n${BOLD}════════════════════════════════════════${NC}"; echo -e "${BOLD}  $*${NC}"; echo -e "${BOLD}════════════════════════════════════════${NC}\n"; }

FAILURES=0
DO_BUILD=0

# Check for --build flag
for arg in "$@"; do
  if [ "$arg" = "--build" ]; then
    DO_BUILD=1
  fi
done

# Strip --build from args
ARGS=()
for arg in "$@"; do
  [ "$arg" != "--build" ] && ARGS+=("$arg")
done
set -- "${ARGS[@]+"${ARGS[@]}"}"

run_suite() {
  local name="$1"
  shift
  log "Running: $name"
  if "$@"; then
    ok "$name passed"
  else
    fail "$name FAILED"
    FAILURES=$((FAILURES + 1))
  fi
}

check_images() {
  local missing=0
  for img in stackarr-test-app:latest stackarr-test-runner:latest stackarr-test-playwright:latest; do
    if ! docker image inspect "$img" >/dev/null 2>&1; then
      warn "Image $img not found"
      missing=1
    fi
  done
  if [ $missing -eq 1 ]; then
    if [ $DO_BUILD -eq 1 ]; then
      cmd_build
    else
      fail "Required images not found. Run './docker/build-test.sh' first, or pass --build."
      exit 1
    fi
  fi
}

# ── Commands ────────────────────────────────────────────────

cmd_build() {
  header "Building test images"
  "$SCRIPT_DIR/build-test.sh" all
}

cmd_unit() {
  header "Unit Tests"
  run_suite "Unit tests" $COMPOSE run --rm test-unit
}

cmd_integration() {
  header "Integration Tests"
  log "Starting Postgres..."
  $COMPOSE up -d postgres
  log "Waiting for Postgres health..."
  local attempt=0
  while [ $attempt -lt 30 ]; do
    if $COMPOSE exec postgres pg_isready -U stackarr >/dev/null 2>&1; then
      ok "Postgres ready"
      break
    fi
    sleep 1
    attempt=$((attempt + 1))
  done
  if [ $attempt -ge 30 ]; then
    fail "Postgres not ready after 30s"
    return 1
  fi

  run_suite "Integration tests" $COMPOSE run --rm test-integration
}

cmd_e2e_mocked() {
  header "E2E Tests (Mocked)"
  run_suite "Mocked E2E" $COMPOSE run --rm test-e2e-mocked
}

cmd_e2e_live() {
  header "E2E Tests (Live)"

  log "Starting full stack (Postgres + StackArr + Indexarr)..."
  $COMPOSE up -d postgres indexarr stackarr

  log "Waiting for StackArr to be healthy..."
  local attempt=0
  while [ $attempt -lt 90 ]; do
    if $COMPOSE exec stackarr curl -sf http://localhost:9111/health >/dev/null 2>&1; then
      ok "StackArr healthy"
      break
    fi
    sleep 2
    attempt=$((attempt + 1))
  done

  if [ $attempt -ge 90 ]; then
    fail "StackArr not healthy after 180s"
    log "Recent logs:"
    $COMPOSE logs --tail=30 stackarr
    return 1
  fi

  # Show system status
  local status
  status=$($COMPOSE exec stackarr curl -sf http://localhost:9111/api/v1/system/status 2>/dev/null || echo "{}")
  log "System status: $status"

  run_suite "Live E2E" $COMPOSE run --rm test-e2e-live
}

cmd_e2e() {
  cmd_e2e_mocked
  cmd_e2e_live
}

cmd_down() {
  header "Tearing down"
  $COMPOSE down -v --remove-orphans 2>/dev/null || true
  ok "All containers and volumes removed"
}

cmd_all() {
  check_images

  # Unit tests (no services needed)
  cmd_unit

  # Integration tests (needs Postgres)
  cmd_integration

  # E2E tests (needs full stack)
  cmd_e2e_live

  # Teardown
  cmd_down

  # Summary
  header "Test Summary"
  if [ $FAILURES -eq 0 ]; then
    ok "All test suites passed!"
  else
    fail "$FAILURES test suite(s) failed"
    exit 1
  fi
}

# ── Cleanup trap ────────────────────────────────────────────
cleanup() {
  if [ "${NO_TEARDOWN:-}" != "1" ]; then
    log "Cleaning up..."
    $COMPOSE down -v --remove-orphans 2>/dev/null || true
  fi
}

# Only trap on full run
case "${1:-all}" in
  all) trap cleanup EXIT ;;
esac

# ── Main ────────────────────────────────────────────────────
case "${1:-all}" in
  build)       cmd_build ;;
  unit)        check_images; cmd_unit ;;
  integration) check_images; cmd_integration ;;
  e2e)         check_images; cmd_e2e ;;
  e2e-mocked)  check_images; cmd_e2e_mocked ;;
  e2e-live)    check_images; cmd_e2e_live ;;
  down)        cmd_down ;;
  all)         cmd_all ;;
  *)
    echo "Usage: $0 [--build] {all|build|unit|integration|e2e|e2e-mocked|e2e-live|down}"
    echo ""
    echo "Commands:"
    echo "  all          — Run all tests + teardown (default)"
    echo "  build        — Build Docker images only"
    echo "  unit         — Run unit tests"
    echo "  integration  — Run integration tests (with Postgres)"
    echo "  e2e          — Run all E2E tests (mocked + live)"
    echo "  e2e-mocked   — Run mocked E2E tests only"
    echo "  e2e-live     — Run live E2E tests only"
    echo "  down         — Tear down all containers and volumes"
    echo ""
    echo "Flags:"
    echo "  --build      — Build images before running tests"
    echo ""
    echo "Environment:"
    echo "  NO_TEARDOWN=1    — Skip cleanup on exit (for debugging)"
    echo "  NZB_LIBS_PATH=.. — Override path to nzb-* libs (default: /home/sprooty/Working/libs)"
    exit 1
    ;;
esac
