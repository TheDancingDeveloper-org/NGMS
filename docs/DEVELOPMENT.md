# Development

## Prerequisites

- **Rust** 1.88+ nightly (edition 2024 requires nightly features)
- **Node.js** 22+
- **PostgreSQL** 17 (local or Docker)
- **System libs**: `build-essential pkg-config libssl-dev cmake gcc-12 g++-12` (Linux)
- **FFmpeg** (optional) — required for streaming transcoding: `ffmpeg`, `ffprobe`
- **Tauri prerequisites** (optional) — required for the client app: see [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/)

## Quick Start

```bash
# 1. Start PostgreSQL
docker compose -f docker/docker-compose.dev.yml up -d
# Postgres available at localhost:5433

# 2. Run backend
cargo run -- --config stackarr.toml --database-url postgresql://stackarr:stackarr@localhost:5433/stackarr

# 3. Run frontend (separate terminal)
cd ui && npm ci && npm run dev
# UI at http://localhost:3000 (proxies API to :8989)

# 4. Run client app (optional, separate terminal)
cd client && npm ci && npm run dev
# Standalone Tauri + React app for remote library browsing and video playback
```

## Build Commands

```bash
# Backend
cargo build                        # Debug build
cargo build --release              # Release build
cargo check --workspace            # Fast compile check (no codegen)
cargo clippy --workspace           # Lint

# Frontend (admin UI)
cd ui
npm ci                             # Install deps
npm run build                      # Production build → dist/
npm run dev                        # Dev server with HMR
npm run lint                       # ESLint
npx tsc --noEmit -p tsconfig.app.json  # Type check only

# Client app (Tauri + React — remote player)
cd client
npm ci                             # Install deps
npm run dev                        # Tauri dev mode with HMR
npm run build                      # Production build → dist/
```

## Testing

```bash
# Unit tests (all crates)
cargo test --workspace --lib

# Run a specific crate's tests
cargo test -p stackarr-parser
cargo test -p stackarr-import
cargo test -p stackarr-notify

# Integration tests (requires Postgres on :5433)
cargo test -p stackarr-migrate -- --ignored

# Single test
cargo test -p stackarr-parser test_parse_standard_release
```

### Test Patterns

**Unit tests** — inline `#[cfg(test)]` modules in each file:
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_quality() {
        let result = parse_release("Show.S01E01.720p.HDTV.x264-GROUP");
        assert_eq!(result.quality.quality, Quality::HDTV720p);
    }

    #[tokio::test]
    async fn test_service_list() {
        // async tests for service layer
    }
}
```

**Integration tests** — in `crates/*/tests/`:
```rust
#[tokio::test]
#[ignore] // Requires running Postgres
async fn test_full_migration() {
    let db = TestDb::new("postgresql://stackarr:stackarr@localhost:5433/stackarr").await;
    // ...
}
```

**Mock-based tests** — wiremock for HTTP clients:
```rust
#[tokio::test]
async fn test_tmdb_search() {
    let mock_server = MockServer::start().await;
    Mock::given(method("GET")).and(path("/3/search/tv"))
        .respond_with(ResponseTemplate::new(200).set_body_json(mock_response))
        .mount(&mock_server).await;

    let client = TmdbClient::with_base_url("key".into(), mock_server.uri());
    let result = client.search_series("test", None).await.unwrap();
    assert_eq!(result.results.len(), 1);
}
```

## Project Structure Conventions

### Adding a New API Endpoint

1. **Route file**: Add handler in `crates/stackarr-web/src/routes/your_route.rs`
2. **Router function**: Export `pub fn router() -> Router<Arc<AppState>>`
3. **Register**: Add `.merge(routes::your_route::router())` in `crates/stackarr-web/src/lib.rs`
4. **Service** (if needed): Add to appropriate crate (e.g., `stackarr-media`)
5. **Models** (if new): Add to `stackarr-core/src/models.rs`

### Adding a New Crate

1. Create `crates/stackarr-yourname/Cargo.toml` and `src/lib.rs`
2. Add to workspace members in root `Cargo.toml`
3. Add workspace dependency: `stackarr-yourname = { path = "crates/stackarr-yourname" }`
4. Import in consuming crates

### Adding a Database Migration

1. Create `migrations/NNN_description.sql`
2. Add model structs in `stackarr-core/src/models.rs` with `FromRow`, `Serialize`, `Deserialize`
3. Run to verify: `cargo run` (migrations auto-run on startup)

### Adding a Frontend Page

1. Create `ui/src/pages/YourPage.tsx`
2. Add route in `ui/src/App.tsx`
3. Add API types in `ui/src/api/types.ts`
4. Add hooks in `ui/src/hooks/useApi.ts`
5. Add nav link in `ui/src/components/Sidebar.tsx`

## Code Style

### Rust
- Workspace-level clippy lints: `clippy::all = warn`
- Workspace-level: `unused = "warn"`
- Error handling: `thiserror` for typed errors, `anyhow` for context
- No `unwrap()` in production code
- Prefer `?` propagation over match-and-return
- Structured logging with `tracing`

### TypeScript
- Strict mode, no unused locals/parameters
- Functional components only
- TanStack Query for all server state — no local state for API data
- Tailwind utility classes, no inline styles

## Debugging

### Backend
```bash
# Verbose logging
STACKARR_LOG_LEVEL=debug cargo run -- --config stackarr.toml

# Trace-level (very verbose)
STACKARR_LOG_LEVEL=trace cargo run -- --config stackarr.toml

# Single crate trace
RUST_LOG=stackarr_parser=trace cargo run -- --config stackarr.toml
```

### Frontend
- React Query Devtools available in dev mode
- Browser DevTools → Network tab for API calls
- Vite HMR for instant feedback

### Database
```bash
# Connect to dev Postgres
psql postgresql://stackarr:stackarr@localhost:5433/stackarr

# Common queries
SELECT * FROM series;
SELECT * FROM enabled_modules;
SELECT * FROM queue ORDER BY added_at DESC;
SELECT * FROM history ORDER BY occurred_at DESC LIMIT 20;
```

## Performance Notes

- The torrent engine (librtbit) and usenet engine (nzb-web) add significant compile time. If not working on download features, they still compile as workspace members.
- `cargo check` is much faster than `cargo build` for iteration.
- Rust cache (`Swatinem/rust-cache`) is used in CI to speed up builds.
- Frontend builds are fast — Vite + Tailwind v4 with no heavy dependencies.
