# Plan: User Accounts + Client Web App

## Context

StackArr currently has no user system — just a single admin API key and anonymous device tokens via claim codes. The user wants to:
1. Add server-local user accounts (invite-only, admin + user roles)
2. Let users access from multiple devices (web login + device claim codes, same identity)
3. Build a proper client web app with streaming, requests, watch progress, watchlist, ratings, and notifications

The existing `/client` Tauri+React app has basic browsing/streaming and server discovery. We evolve it rather than starting fresh.

---

## Account Model

**Two-tier: Admin + User.** Admin has full management access. Users can browse, stream, request media, and manage their own profile/devices.

**Identity flow:**
- Admin creates invite code → shares with user
- User opens web app → enters invite code → creates account (username + password)
- User logs in on web → session cookie
- User can also link additional devices: user generates a device-link code from their profile, enters on TV/phone → device gets a long-lived token tied to their account
- Claim codes (bootstrap) still handle NAT traversal for finding the server; device auth requires login after connecting

**Migration from current system:**
- First boot with no users → setup wizard creates admin account
- Existing `remote_clients` → migrated to `user_devices` owned by auto-created admin
- Existing API key → continues working, resolves to admin user (backwards compat for external tools)

---

## Database Schema (migration `006_users.sql`)

### Core tables

```sql
users (id, username, display_name, password_hash, role, avatar_url, enabled, created_at, updated_at)
user_sessions (id UUID, user_id, token_hash, user_agent, ip_address, created_at, expires_at, last_active)
user_devices (id, user_id, device_token UUID, device_name, device_type, created_at, last_seen, revoked)
invites (id, code, created_by, claimed_by, role, expires_at, created_at)
```

### Feature tables

```sql
watch_progress (id, user_id, media_file_id, media_type, media_id, episode_id, position_secs, duration_secs, completed, updated_at)
  UNIQUE(user_id, media_file_id)

media_requests (id, user_id, media_type, tmdb_id, title, year, status, approved_by, note, created_at, updated_at)
  UNIQUE(tmdb_id, media_type)

user_watchlist (id, user_id, tmdb_id, media_type, added_at)
  UNIQUE(user_id, tmdb_id, media_type)

user_ratings (id, user_id, media_type, media_id, rating 1-10, created_at)
  UNIQUE(user_id, media_type, media_id)

user_notifications (id, user_id, notification_type, title, body, data JSONB, read, created_at)
```

Also: `ALTER TABLE streaming_sessions ADD COLUMN user_id`

---

## Auth Architecture

### Middleware changes (`middleware.rs`)

```rust
struct AuthenticatedUser { user_id, username, role, auth_method }

// Resolution order:
// 1. stackarr_session cookie → sha256(cookie) lookup in user_sessions → load user
// 2. Authorization: Bearer <token> → lookup in user_devices → load linked user
// 3. X-Api-Key / ?apikey= → match app_config.api_key → resolve to admin user
// 4. None → 401

struct RequireUser(AuthenticatedUser);  // any authenticated user
struct RequireAdmin(AuthenticatedUser); // admin only
```

### Security
- Passwords: argon2id (OWASP params)
- Session tokens: 32-byte random, base64url, stored as sha256 in DB
- Cookie: `HttpOnly; SameSite=Lax; Secure; Path=/; Max-Age=30d`
- Login response also returns token in body (for Tauri clients that can't use cookies)
- Rate limit auth endpoints: 5 req/min per IP

---

## API Endpoints

### Auth (`routes/auth.rs`)
```
POST /api/v1/auth/login      { username, password, deviceToken? }  → session + user
POST /api/v1/auth/logout      → invalidate session
POST /api/v1/auth/register    { inviteCode, username, password, displayName }  → session + user
GET  /api/v1/auth/me          → current user
```

### User self-service (`routes/user.rs`)
```
PUT    /api/v1/user/profile
GET    /api/v1/user/devices
DELETE /api/v1/user/devices/:id
GET    /api/v1/user/sessions
DELETE /api/v1/user/sessions/:id
```

### Watch progress (`routes/progress.rs`)
```
PUT    /api/v1/user/progress/:mediaFileId   { positionSecs, durationSecs, completed }
GET    /api/v1/user/progress/continue       → Continue Watching list
GET    /api/v1/user/progress/:mediaFileId
DELETE /api/v1/user/progress/:mediaFileId
```

### Media requests (`routes/requests.rs`)
```
POST   /api/v1/requests                    { mediaType, tmdbId, title, year }
GET    /api/v1/requests                    (users: own, admins: all)
PUT    /api/v1/requests/:id/approve        (admin) → triggers library add
PUT    /api/v1/requests/:id/decline        (admin)
DELETE /api/v1/requests/:id                (admin)
```

### Watchlist + Ratings
```
GET/POST/DELETE  /api/v1/user/watchlist[/:tmdbId/:mediaType]
PUT/GET/DELETE   /api/v1/user/ratings[/:mediaType/:mediaId]
```

### Notifications
```
GET  /api/v1/user/notifications            (paginated)
PUT  /api/v1/user/notifications/:id/read
PUT  /api/v1/user/notifications/read-all
GET  /api/v1/user/notifications/unread-count
```

### Admin user management (`routes/admin.rs`)
```
GET/POST       /api/v1/admin/users[/:id]
PUT/DELETE     /api/v1/admin/users/:id
POST/GET       /api/v1/admin/invites[/:id]
DELETE         /api/v1/admin/invites/:id
```

---

## Client Web App

### Serving strategy
```
/        → admin UI (existing, requires admin)
/app     → client web app (requires any user)
/api/v1  → API endpoints
```

Client Vite config gets `base: '/app/'`. Axum mounts client build at `/app` via `nest_service`.

### Routes
```
/app/login              → username/password
/app/register/:code     → invite code pre-filled
/app/home               → Continue Watching, Recently Added, Recommended
/app/browse             → series/movies grid with search
/app/series/:id         → series detail + seasons/episodes
/app/movie/:id          → movie detail
/app/play/:fileId       → video player with progress reporting
/app/discover           → TMDB trending, request new media
/app/requests           → user's request list + status
/app/watchlist          → personal watchlist
/app/settings           → profile, devices, sessions
/app/connect            → server discovery (Tauri only)
```

### Key components
- `AuthProvider` — React context, checks `/auth/me` on mount
- `ProgressReporter` — debounced position updates every 10s + on pause/close
- `ContinueWatchingRow` — horizontal scroll of in-progress media
- `RequestButton` — on discover/detail pages
- `NotificationBell` — unread count badge, dropdown
- Platform detection: `'__TAURI_INTERNALS__' in window` for Tauri-specific features

### PWA
- `manifest.json` for installability
- Service worker for offline shell + push notifications (Phase 5)

---

## Implementation Phases

### Phase 1: User system + web login
- Migration 006 (users, sessions, devices, invites)
- Auth routes, middleware extractors, password hashing (argon2)
- Admin user management routes + UI page
- Client app: auth context, login, register pages
- Mount client at `/app`
- Migrate remote_clients → user_devices

### Phase 2: Watch progress + Continue Watching
- Progress routes
- ProgressReporter in player
- HomePage with ContinueWatchingRow

### Phase 3: Media requests
- Request routes
- Integration with stackarr-media for auto-add on approve
- Client: RequestButton, RequestsPage
- Admin: request management UI

### Phase 4: Watchlist + Ratings
- Watchlist/rating routes
- Toggle buttons on media cards
- WatchlistPage, rating component

### Phase 5: Notifications + PWA
- Notification creation hooks in import/request pipelines
- Notification routes + bell UI
- PWA manifest + service worker

---

## Key files to modify

| Area | Files |
|------|-------|
| Migration | `migrations/006_users.sql` |
| DB methods | `crates/stackarr-core/src/db.rs` |
| Auth middleware | `crates/stackarr-web/src/middleware.rs` |
| New routes | `crates/stackarr-web/src/routes/{auth,admin,user,progress,requests}.rs` |
| Router | `crates/stackarr-web/src/lib.rs` (mount `/app`, register routes) |
| Client app | `client/src/{App,api}.tsx`, `client/src/pages/*.tsx` |
| Client config | `client/vite.config.ts` (base: '/app/') |
| Deps | `crates/stackarr-core/Cargo.toml` (add argon2) |

## Verification
- `cargo test --workspace --lib` after each phase
- Manual: create admin → create invite → register user → login → browse → play → check progress
- Test device linking: claim code → login on device → verify device appears in user profile
- Test backwards compat: existing API key still works for admin operations
