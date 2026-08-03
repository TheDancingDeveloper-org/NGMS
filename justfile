set shell := ["bash", "-euo", "pipefail", "-c"]

# Build the two web bundles required by Rust's embed-ui feature.
web-assets:
    npm --prefix ui ci --no-audit --no-fund
    npm --prefix ui run build
    npm --prefix client ci --no-audit --no-fund
    npm --prefix client run build

# Run every mandatory local Rust gate.
check: web-assets
    python3 scripts/check_dependency_sources.py
    python3 scripts/check_workspace_docs.py
    cargo fmt --all -- --check
    cargo build --workspace --all-features --locked
    cargo clippy --workspace --all-features --locked -- -D warnings
    cargo test --workspace --all-features --locked

test: web-assets
    cargo test --workspace --all-features --locked

# Record per-crate coverage and enforce the ratchet against coverage-baseline.json.
coverage: web-assets
    cargo llvm-cov --workspace --all-features --locked --ignore-filename-regex '(^|/)(tests|benches|examples)/' --lcov --output-path target/lcov.info
    python3 scripts/coverage_ratchet.py --lcov target/lcov.info

# Re-record the baseline after a change that legitimately moves coverage.
coverage-update: web-assets
    cargo llvm-cov --workspace --all-features --locked --ignore-filename-regex '(^|/)(tests|benches|examples)/' --lcov --output-path target/lcov.info
    python3 scripts/coverage_ratchet.py --lcov target/lcov.info --update

# P2 replaces this smoke assertion with generated façade contract tests.
test-compat:
    test -f docs/API-COMPATIBILITY.md

test-e2e:
    npm --prefix ui ci --no-audit --no-fund
    npm --prefix ui exec playwright install --with-deps chromium
    npm --prefix ui run test:e2e

# P2 replaces this smoke assertion with capture/replay and coverage reporting.
conformance:
    test -f docs/UNIFIED-ARR-PLAN.md
    test -f docs/API-COMPATIBILITY.md

smoke:
    cargo build --workspace --locked
    python3 scripts/check_dependency_sources.py
    python3 scripts/check_workspace_docs.py
