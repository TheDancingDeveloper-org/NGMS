# Authentication and Authorization

StackArr uses a multi-method authentication system that supports web sessions, API keys, device tokens, HTTP Basic Auth, and a first-boot bypass. Authentication is enforced at the Axum middleware layer using extractors.

## Source Files

| File | Purpose |
|------|---------|
| `crates/stackarr-web/src/middleware.rs` | Auth extractors, auth middleware, rate limiting |
| `crates/stackarr-core/src/auth.rs` | Password hashing (Argon2id), token generation, invite codes |
| `crates/stackarr-web/src/routes/auth.rs` | Login, logout, register, setup, /me endpoints |
| `crates/stackarr-web/src/routes/admin.rs` | User CRUD, invite management (admin-only) |
| `crates/stackarr-core/src/models/user.rs` | User, UserSession, UserDevice, Invite models |
| `migrations/006_users.sql` | Users, sessions, devices, invites schema |

## Authentication Methods

### 1. Session-Based (Forms)

The default method (`auth.method = "forms"`). Users log in via `POST /api/v1/auth/login` with username and password. On success, the server:

1. Verifies the password against the stored Argon2id hash (via `spawn_blocking`).
2. Generates a 32-byte random session token (base64url-encoded, 43 characters).
3. Stores the SHA-256 hash of the token in the `user_sessions` table with a 30-day expiry.
4. Returns the raw token in the JSON body and as an `HttpOnly; SameSite=Lax` cookie named `stackarr_session`.

Subsequent requests are authenticated by the `stackarr_session` cookie. The extractor hashes the cookie value with SHA-256 and looks up the hash in `user_sessions`. Sessions are touched (last_active updated) in a background task on each validated request.

### 2. API Key

A single system-wide API key stored in `app_config` (key = `api_key`) and cached in `AppState.cached_api_key` via `ArcSwap`. Compatible with Sonarr/Radarr API conventions.

The key can be provided via (checked in this order):
1. `X-Api-Key` header
2. `Authorization: Bearer <key>` header
3. `?apikey=<key>` query parameter

API key authentication always resolves to a synthetic admin user (`user_id: 0, username: "admin", role: "admin"`).

### 3. Device Tokens

Long-lived tokens for mobile and desktop (Tauri) clients. A device token is a UUID v4 stored in the `user_devices` table, linked to a specific user.

Device tokens are created during login when the `deviceName` field is provided in the login request. The token UUID is returned in the `deviceToken` response field.

Clients present device tokens via `Authorization: Bearer <uuid>` or `X-Api-Key: <uuid>`. The extractor detects UUID format and validates against `user_devices`. The `last_seen` timestamp is updated in a background task.

For backward compatibility, UUID tokens are also checked against the legacy `remote_clients` table (from the bootstrap/remote access system). Legacy client tokens resolve to a synthetic user (`user_id: 0, username: "client", role: "user"`).

### 4. HTTP Basic Auth

When `auth.method = "basic"`, the server accepts `Authorization: Basic <base64(username:password)>` headers. The middleware decodes the credentials, looks up the user by username, and verifies the password with Argon2id.

On failure in basic mode, the server returns `401` with a `WWW-Authenticate: Basic realm="StackArr"` header, which triggers the browser's native auth dialog.

### 5. First-Boot Bypass

When no users exist in the database AND no API key is cached, all requests are allowed through as an admin user. This permits initial setup via the UI or API without credentials.

Once the first user is created via `POST /api/v1/auth/setup`, the bypass is permanently disabled.

## Auth Extractors

Four Axum `FromRequestParts` extractors enforce authentication at the route level.

### `RequireApiKey`

Validates only the system API key. Does not resolve to a user.

- Checks: `X-Api-Key` header, `Authorization: Bearer`, `?apikey=` query param.
- First-boot bypass: if no API key is stored (empty cache), all requests pass.
- Used on: legacy/external API routes that need Sonarr/Radarr compatibility.

### `RequireAuth`

Accepts the admin API key OR a valid remote client token. Returns an `AuthType` enum (`ApiKey` or `ClientToken`).

- First tries the API key match.
- Then tries parsing the key as a UUID and validating against `remote_clients`.
- Used on: routes shared between admin and remote streaming clients.

### `RequireUser`

The primary user-aware extractor. Returns an `AuthenticatedUser` with `user_id`, `username`, `role`, and `auth_method`.

Resolution order:
1. `stackarr_session` cookie -- hash with SHA-256, validate against `user_sessions`.
2. Bearer/API key as UUID -- validate against `user_devices`, then fall back to `remote_clients`.
3. Bearer/API key as string -- match against the cached system API key.
4. `Authorization: Basic` header -- decode and verify credentials.
5. First-boot bypass -- if no users exist and no API key is cached, grant admin access.
6. Return 401.

### `RequireAdmin`

Wraps `RequireUser`. After resolving the user, checks that `role == "admin"`. Returns 403 Forbidden if the user is not an admin.

## Auth Middleware Layer

The `require_auth_middleware` function runs as an Axum middleware layer on protected routes. Its behavior depends on the configured `auth_method`:

| `auth_method` | Behavior |
|---------------|----------|
| `"none"` | All requests pass through without authentication. |
| `"forms"` | Runs `RequireUser` extraction. Returns plain 401 on failure (frontend redirects to login page). |
| `"basic"` | Runs `RequireUser` extraction, then falls back to HTTP Basic Auth. Returns 401 with `WWW-Authenticate` header on failure. |

The auth method is read from `AppState.cached_auth_method` (an `ArcSwap<String>`), which is loaded from `app_config` at startup and updated when the general settings are saved.

## Roles and Permissions (RBAC)

Two roles exist: `admin` and `user`.

### Admin

- Full access to all routes.
- User management: create, update, disable, delete users.
- Invite management: create, list, delete invite codes.
- System configuration: change auth method, API key, server settings.
- Cannot delete their own account (self-deletion guard).

### User

- Access to media browsing, streaming, search, queue viewing.
- Watch progress tracking, watchlist, ratings.
- Media requests (submit requests for new content).
- Cannot access `/api/v1/admin/*` routes.
- Cannot modify system configuration or manage other users.

Roles are stored as a `TEXT` column on the `users` table. The `RequireAdmin` extractor enforces admin-only access.

## Session Lifecycle

### Creation

Sessions are created on login (`/api/v1/auth/login`), registration (`/api/v1/auth/register`), and first-boot setup (`/api/v1/auth/setup`).

Each session records:
- `token_hash`: SHA-256 of the raw session token (hex-encoded, 64 chars).
- `user_agent`: from the request `User-Agent` header.
- `ip_address`: from `X-Forwarded-For`, `X-Real-IP`, or connection info.
- `expires_at`: 30 days from creation.
- `last_active`: updated on each validated request (background task).

### Validation

The `RequireUser` extractor calls `db.validate_session(&token_hash)`, which checks the hash exists and `expires_at > NOW()`. On success, `db.touch_session(&hash)` updates `last_active` in a spawned background task.

### Expiration

Sessions expire after 30 days (`Max-Age=2592000` on the cookie). Expired sessions remain in the database until cleaned up.

### Logout

`POST /api/v1/auth/logout` deletes the session row from `user_sessions` and clears the cookie by setting `Max-Age=0`.

## Device Tokens

### Creation

Created during login when the `deviceName` field is present in the request body. The server generates a UUID v4 and inserts it into `user_devices` linked to the authenticated user.

### Schema

```sql
CREATE TABLE user_devices (
    id SERIAL PRIMARY KEY,
    user_id BIGINT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    device_token UUID NOT NULL UNIQUE,
    device_name TEXT,
    device_type TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    last_seen TIMESTAMPTZ,
    revoked BOOLEAN NOT NULL DEFAULT false
);
```

### Validation

The `RequireUser` extractor calls `db.validate_user_device(token_uuid)` which checks the UUID exists and `revoked = false`. On success, `last_seen` is updated in a background task.

### Revocation

Device tokens can be revoked by setting `revoked = true`. Revoked tokens are rejected during validation.

### Legacy Remote Clients

The `remote_clients` table (from `004_remote_access.sql`) is the predecessor to `user_devices`. UUID tokens that fail `user_devices` validation are also checked against `remote_clients` for backward compatibility. Legacy tokens resolve to a generic user role.

## Invite System

Invites allow admins to grant registration access to new users.

### Code Generation

`generate_invite_code()` produces an 8-character alphanumeric code using a reduced charset: `ABCDEFGHJKLMNPQRSTUVWXYZ23456789`. The characters `0`, `1`, `I`, and `O` are excluded to avoid visual ambiguity.

### Creation (Admin)

`POST /api/v1/admin/invites` creates an invite with:
- `code`: the generated 8-char code.
- `created_by`: the admin's user ID.
- `role`: the role the new user will receive (`"admin"` or `"user"`, defaults to `"user"`).
- `expires_at`: optional, computed from `expiresInHours`. If omitted, the invite does not expire.

If bootstrap integration is enabled, the invite is also registered with the bootstrap discovery node for remote claiming.

### Claiming (Registration)

`POST /api/v1/auth/register` requires a valid `inviteCode`. The server calls `db.validate_invite(&code)` which checks the code exists, is not already claimed (`claimed_by IS NULL`), and is not expired. On successful registration, `db.claim_invite(&code, user_id)` sets `claimed_by`.

### Schema

```sql
CREATE TABLE invites (
    id SERIAL PRIMARY KEY,
    code TEXT NOT NULL UNIQUE,
    created_by BIGINT NOT NULL REFERENCES users(id),
    claimed_by BIGINT REFERENCES users(id),
    role TEXT NOT NULL DEFAULT 'user',
    expires_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
```

## Password Handling

### Hashing

Passwords are hashed with **Argon2id** using the `argon2` crate with default parameters. A random salt is generated per hash using `OsRng`. Hashing is performed inside `tokio::task::spawn_blocking` to avoid blocking the async runtime.

The stored format is the standard PHC string: `$argon2id$v=19$m=...,t=...,p=...$<salt>$<hash>`.

### Verification

`verify_password(password, hash)` parses the stored PHC string and verifies the candidate password. Also run via `spawn_blocking`.

### Requirements

Passwords must be at least 6 characters. This is enforced at the route level (login, register, setup, admin create/update user).

## API Endpoints

### Public (no auth required)

| Method | Path | Description |
|--------|------|-------------|
| `GET` | `/api/v1/auth/status` | Returns `setupRequired` (bool) and `registrationEnabled` (bool). |
| `POST` | `/api/v1/auth/setup` | First-boot: create admin user, generate API key, create session. Only works when no users exist. |
| `POST` | `/api/v1/auth/login` | Authenticate with username/password. Optionally create a device token via `deviceName`. |
| `POST` | `/api/v1/auth/register` | Register with invite code. Creates user and session. |

### Authenticated (RequireUser)

| Method | Path | Description |
|--------|------|-------------|
| `GET` | `/api/v1/auth/me` | Returns the current user's profile and auth method. |
| `POST` | `/api/v1/auth/logout` | Deletes the current session and clears the cookie. |

### Admin-Only (RequireAdmin)

| Method | Path | Description |
|--------|------|-------------|
| `GET` | `/api/v1/admin/users` | List all users. |
| `POST` | `/api/v1/admin/users` | Create a new user (username, password, displayName, role). |
| `PUT` | `/api/v1/admin/users/{id}` | Update user (displayName, role, enabled, avatarUrl, password). |
| `DELETE` | `/api/v1/admin/users/{id}` | Delete a user. Cannot delete self. |
| `GET` | `/api/v1/admin/invites` | List all invites. |
| `POST` | `/api/v1/admin/invites` | Create an invite (role, expiresInHours). |
| `DELETE` | `/api/v1/admin/invites/{id}` | Delete an invite. |

## Configuration

### TOML Config

```toml
[auth]
method = "forms"           # "forms", "basic", or "none"
# api_key = "your-key"    # Optional static API key
```

The default is `"forms"`.

### Runtime Config (app_config table)

The `auth_method` and `api_key` values are stored in the `app_config` table and cached in `AppState` via `ArcSwap`. They can be changed at runtime through the general settings API without a restart.

| Key | Type | Description |
|-----|------|-------------|
| `api_key` | string | System-wide API key for external integrations. |
| `auth_method` | string | Active auth mode: `"none"`, `"basic"`, or `"forms"`. |

## Token and Key Formats

| Token Type | Format | Length | Storage |
|------------|--------|--------|---------|
| Session token | base64url (no padding), 32 random bytes | 43 chars | SHA-256 hash stored in `user_sessions.token_hash` (64 hex chars) |
| API key | base64url (no padding), 32 random bytes | 43 chars | Plaintext in `app_config.value`, cached in memory |
| Device token | UUID v4 | 36 chars (hyphenated) | Plaintext in `user_devices.device_token` |
| Invite code | Uppercase alphanumeric (reduced charset) | 8 chars | Plaintext in `invites.code` |

## Rate Limiting

Auth-sensitive endpoints (login, register) use a per-IP rate limiter built on the `governor` crate. The `RateLimit` extractor checks a keyed rate limiter (`DashMapStateStore<IpAddr>`) and returns 429 Too Many Requests if the limit is exceeded.

Client IP is resolved from `X-Forwarded-For`, then `X-Real-IP`, falling back to loopback.

## Sensitive Field Redaction

The `redact_sensitive_fields` function recursively walks JSON values and masks fields whose names contain `api_key`, `apikey`, `password`, `secret`, `token`, or `auth_token`. Masked values show the first 4 and last 4 characters with an ellipsis (e.g., `abcd...mnop`). Values 8 characters or shorter are fully replaced with asterisks.
