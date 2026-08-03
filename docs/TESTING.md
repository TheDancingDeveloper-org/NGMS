# Testing StackArr

Tests are evidence against an identified contract. New behavior starts with a
failing test, and contract fixtures are reviewed artifacts rather than snapshots
updated automatically after a failure.

## Test tiers

### Unit tests

Unit tests live beside Rust modules and cover domain logic without network or
database dependencies.

```bash
cargo test --workspace --lib
```

### Property tests

Release-name parsing and custom-format matching accept hostile, combinatorial
input. Add `proptest` cases beside their unit suites and ensure failures persist
the minimal reproducer. Property tests run as part of the workspace test gate.

### Database and integration tests

Database tests use MariaDB 11.4 LTS and an isolated schema. Tests must create
their own state and may not depend on ordering. NNTP integration tests use the
published mock NNTP server rather than a live provider. The exact container
command will be exposed by `just test-integration` during P1.

### Compatibility and conformance tests

P2 introduces `contracts/arr-v3/` with one directory per façade and client.
Each fixture contains a sanitized request, the expected status and headers, and
the expected JSON body. Dynamic IDs, ports, and timestamps are represented by
explicit normalizers in fixture metadata; the structural diff is otherwise
strict.

To add a golden file after the P2 harness lands:

1. Capture traffic from the pinned reference application through the recording
   proxy.
2. Remove credentials, tokens, personal paths, and user data.
3. Add only the narrowest required normalizers.
4. Replay against the reference application and confirm a zero diff.
5. Replay against StackArr and keep the generated test red.
6. Implement the endpoint, make the test green, and include the fixture diff in
   review.

`just test-compat` runs generated OpenAPI contract tests. `just conformance`
replays golden traffic and reports path coverage once the harness exists.

### End-to-end tests

The React UI uses Playwright in `ui/e2e`. Golden paths and key failure paths are
required for GUI changes.

```bash
npm --prefix ui ci --no-audit --no-fund
npm --prefix ui run test:e2e
```

Container and live-client E2E tests must use disposable data and document every
external prerequisite. They must never contain real credentials.

## Mandatory Rust gate

```bash
cargo build --workspace
cargo fmt --all -- --check
cargo clippy --workspace --all-features -- -D warnings
cargo test --workspace --all-features
```

Coverage is a per-crate ratchet: a change may improve coverage but may not lower
the checked-in baseline without an explicit, reviewed justification.

## Coverage ratchet

`coverage-baseline.json` records line coverage for every workspace crate. The
CI `coverage` job regenerates the numbers with `cargo llvm-cov` and fails when
any crate falls below its recorded percentage, so a change that adds untested
code has to add tests with it.

```bash
just coverage
```

The recipe needs `cargo-llvm-cov` (`cargo install cargo-llvm-cov`). Integration
tests, benches, and examples are excluded from the measurement; they exercise
library code rather than being library code. The baseline allows a 0.1 point
tolerance so instrumentation noise does not fail a build.

When a change legitimately moves the numbers — new tests, or a deletion that
removes well-covered code — re-record the baseline and include the diff in
review:

```bash
just coverage-update
```

A crate that is missing from the baseline fails the job rather than passing
silently, so a new workspace member cannot land unrecorded.
