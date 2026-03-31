# Phase 1: User System + Web Login

## Goal

Add server-local user accounts with invite-only registration, session-based auth, and a login/register flow in the client web app. Migrate existing `remote_clients` to the new `user_devices` model. Mount the client app at `/app`.

---

## 1. Database Migration (`migrations/006_users.sql`)

```sql
-- USERS
CREATE TABLE users (
    id BIGSERIAL PRIMARY KEY,
    username TEXT NOT NULL UNIQUE,
    display_name TEXT NOT NULL,
    password_hash TEXT NOT NULL,
    role TEXT NOT NULL DEFAULT 'user',  -- 'admin' | 'user'
    avatar_url TEXT,
    enabled BOOLEAN NOT NULL DEFAULT true,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- SESSIONS (web login sessions)
CREATE TABLE user_sessions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id BIGINT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    token_hash TEXT NOT NULL UNIQUE,
    user_agent TEXT,
    ip_address INET,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    expires_at TIMESTAMPTZ NOT NULL,
    last_active TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX idx_user_sessions_user ON user_sessions(user_id);
CREATE INDEX idx_user_sessions_expires ON user_sessions(expires_at);

-- USER DEVICES (replaces remote_clients)
CREATE TABLE user_devices (
    id SERIAL PRIMARY KEY,
    user_id BIGINT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    device_token UUID NOT NULL UNIQUE,
    device_name TEXT,
    device_type TEXT,  -- 'web', 'desktop', 'mobile', 'tv'
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    last_seen TIMESTAMPTZ,
    revoked BOOLEAN NOT NULL DEFAULT false
);
CREATE INDEX idx_user_devices_token ON user_devices(device_token);
CREATE INDEX idx_user_devices_user ON user_devices(user_id);

-- INVITES
CREATE TABLE invites (
    id SERIAL PRIMARY KEY,
    code TEXT NOT NULL UNIQUE,
    created_by BIGINT NOT NULL REFERENCES users(id),
    claimed_by BIGINT REFERENCES users(id),
    role TEXT NOT NULL DEFAULT 'user',
    expires_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX idx_invites_code ON invites(code);

-- Migrate remote_clients data (Rust migration code handles user linkage)
-- Keep remote_clients table for backwards compat during transition
```

---

## 2. Rust Models

### New file: `crates/stackarr-core/src/models/user.rs`

```rust
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "text", rename_all = "snake_case")]
#[serde(rename_all = "camelCase")]
pub enum UserRole {
    Admin,
    User,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct User {
    pub id: i64,
    pub username: String,
    pub display_name: String,
    #[serde(skip_serializing)]
    pub password_hash: String,
    pub role: String,  // cast to UserRole in application code
    pub avatar_url: Option<String>,
    pub enabled: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct UserSession {
    pub id: Uuid,
    pub user_id: i64,
    pub token_hash: String,
    pub user_agent: Option<String>,
    pub ip_address: Option<String>,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub last_active: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct UserDevice {
    pub id: i32,
    pub user_id: i64,
    pub device_token: Uuid,
    pub device_name: Option<String>,
    pub device_type: Option<String>,
    pub created_at: DateTime<Utc>,
    pub last_seen: Option<DateTime<Utc>>,
    pub revoked: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct Invite {
    pub id: i32,
    pub code: String,
    pub created_by: i64,
    pub claimed_by: Option<i64>,
    pub role: String,
    pub expires_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}
```

Register this module in `crates/stackarr-core/src/models/mod.rs`.

---

## 3. Database Methods (`crates/stackarr-core/src/db.rs`)

Add these method groups to the `Database` impl block. Follow existing patterns (sqlx `query_as`, `query`, `fetch_one/optional/all`, return `crate::Result<T>`).

### User CRUD
```rust
pub async fn create_user(&self, username: &str, display_name: &str, password_hash: &str, role: &str) -> crate::Result<User>
pub async fn get_user_by_id(&self, id: i64) -> crate::Result<Option<User>>
pub async fn get_user_by_username(&self, username: &str) -> crate::Result<Option<User>>
pub async fn list_users(&self) -> crate::Result<Vec<User>>
pub async fn update_user(&self, id: i64, display_name: Option<&str>, role: Option<&str>, enabled: Option<bool>) -> crate::Result<bool>
pub async fn update_user_password(&self, id: i64, password_hash: &str) -> crate::Result<bool>
pub async fn delete_user(&self, id: i64) -> crate::Result<bool>
pub async fn count_users(&self) -> crate::Result<i64>  // for first-boot detection
```

### Session management
```rust
pub async fn create_session(&self, user_id: i64, token_hash: &str, user_agent: Option<&str>, ip_address: Option<&str>, expires_at: DateTime<Utc>) -> crate::Result<Uuid>
pub async fn validate_session(&self, token_hash: &str) -> crate::Result<Option<User>>  // joins users, checks expires_at + enabled
pub async fn touch_session(&self, token_hash: &str) -> crate::Result<()>  // update last_active
pub async fn delete_session(&self, session_id: Uuid, user_id: i64) -> crate::Result<bool>
pub async fn delete_all_sessions(&self, user_id: i64) -> crate::Result<()>
pub async fn list_sessions(&self, user_id: i64) -> crate::Result<Vec<UserSession>>
pub async fn cleanup_expired_sessions(&self) -> crate::Result<u64>  // DELETE WHERE expires_at < NOW()
```

### User devices (replaces remote_client methods)
```rust
pub async fn create_user_device(&self, user_id: i64, device_token: Uuid, device_name: Option<&str>, device_type: Option<&str>) -> crate::Result<i32>
pub async fn validate_user_device(&self, device_token: Uuid) -> crate::Result<Option<User>>  // joins users, checks revoked + enabled
pub async fn touch_user_device(&self, device_token: Uuid) -> crate::Result<()>
pub async fn list_user_devices(&self, user_id: i64) -> crate::Result<Vec<UserDevice>>
pub async fn revoke_user_device(&self, device_id: i32, user_id: i64) -> crate::Result<bool>
pub async fn delete_user_device(&self, device_id: i32, user_id: i64) -> crate::Result<bool>
pub async fn link_device_to_user(&self, device_token: Uuid, user_id: i64) -> crate::Result<bool>  // for claim code -> login flow
```

### Invites
```rust
pub async fn create_invite(&self, code: &str, created_by: i64, role: &str, expires_at: Option<DateTime<Utc>>) -> crate::Result<Invite>
pub async fn validate_invite(&self, code: &str) -> crate::Result<Option<Invite>>  // unclaimed + not expired
pub async fn claim_invite(&self, code: &str, user_id: i64) -> crate::Result<bool>
pub async fn list_invites(&self, created_by: Option<i64>) -> crate::Result<Vec<Invite>>
pub async fn delete_invite(&self, id: i32) -> crate::Result<bool>
```

### Data migration helper
```rust
pub async fn migrate_remote_clients_to_user_devices(&self, admin_user_id: i64) -> crate::Result<u64>
// INSERT INTO user_devices (user_id, device_token, device_name, created_at, last_seen, revoked)
// SELECT $1, client_token, client_name, created_at, last_seen, revoked FROM remote_clients
// WHERE NOT EXISTS (SELECT 1 FROM user_devices WHERE device_token = remote_clients.client_token)
```

---

## 4. Password Hashing

### Add dependency to `crates/stackarr-core/Cargo.toml`
```toml
argon2 = "0.5"
```

### New file: `crates/stackarr-core/src/auth.rs`
```rust
use argon2::{Argon2, PasswordHash, PasswordHasher, PasswordVerifier};
use argon2::password_hash::rand_core::OsRng;
use argon2::password_hash::SaltString;
use sha2::{Sha256, Digest};
use rand::RngCore;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;

pub fn hash_password(password: &str) -> crate::Result<String> {
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();  // argon2id with OWASP defaults
    Ok(argon2.hash_password(password.as_bytes(), &salt)
        .map_err(|e| crate::Error::Other(anyhow::anyhow!("password hash failed: {e}")))?
        .to_string())
}

pub fn verify_password(password: &str, hash: &str) -> crate::Result<bool> {
    let parsed = PasswordHash::new(hash)
        .map_err(|e| crate::Error::Other(anyhow::anyhow!("invalid hash: {e}")))?;
    Ok(Argon2::default().verify_password(password.as_bytes(), &parsed).is_ok())
}

pub fn generate_session_token() -> String {
    let mut bytes = [0u8; 32];
    rand::rng().fill_bytes(&mut bytes);
    URL_SAFE_NO_PAD.encode(bytes)
}

pub fn hash_token(token: &str) -> String {
    let hash = Sha256::digest(token.as_bytes());
    hex::encode(hash)
}

pub fn generate_invite_code() -> String {
    // 8-char alphanumeric, no ambiguous chars
    use rand::Rng;
    const CHARSET: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZ23456789";
    let mut rng = rand::rng();
    (0..8).map(|_| CHARSET[rng.random_range(0..CHARSET.len())] as char).collect()
}
```

Also add `sha2`, `hex`, `rand`, `base64` to stackarr-core Cargo.toml if not already present.

---

## 5. Auth Middleware (`crates/stackarr-web/src/middleware.rs`)

### New types (add alongside existing extractors)

```rust
#[derive(Debug, Clone)]
pub struct AuthenticatedUser {
    pub user_id: i64,
    pub username: String,
    pub role: UserRole,
    pub auth_method: AuthMethod,
}

#[derive(Debug, Clone)]
pub enum AuthMethod {
    Session,
    DeviceToken,
    ApiKey,
}

pub struct RequireUser(pub AuthenticatedUser);
pub struct RequireAdmin(pub AuthenticatedUser);
```

### RequireUser extractor (implements FromRequestParts)

Resolution order:
1. **Cookie**: Read `stackarr_session` cookie → `hash_token(cookie_value)` → `db.validate_session(token_hash)` → if user returned and enabled, return `AuthenticatedUser { auth_method: Session }`
2. **Bearer token**: Parse `Authorization: Bearer <token>` → try parse as UUID → `db.validate_user_device(uuid)` → if user returned and enabled, return `AuthenticatedUser { auth_method: DeviceToken }`
3. **API key fallback**: Check `X-Api-Key` header / `?apikey=` query / `Authorization: Bearer <non-uuid>` → match against `app_config.api_key` → resolve admin user from DB → return `AuthenticatedUser { auth_method: ApiKey }`
4. **First-boot bypass**: If `db.count_users() == 0`, allow unauthenticated (setup mode)
5. **None**: Return 401

### RequireAdmin extractor
Wraps `RequireUser`, checks `role == UserRole::Admin`, returns 403 if not.

### Keep existing extractors
`RequireApiKey` and `RequireAuth` continue to work for backwards compatibility. Gradually migrate routes to use `RequireUser`/`RequireAdmin`.

---

## 6. API Routes

### New file: `crates/stackarr-web/src/routes/auth.rs`

```rust
pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/v1/auth/login", post(login))
        .route("/api/v1/auth/logout", post(logout))
        .route("/api/v1/auth/register", post(register))
        .route("/api/v1/auth/me", get(me))
}
```

**POST /api/v1/auth/login** (no auth required, rate limited)
- Body: `{ username, password, deviceToken?: UUID, deviceName?: String }`
- Verify password with argon2
- If `deviceToken` provided and exists as unlinked device, link to user
- Create session, set cookie, return `{ user: { id, username, displayName, role }, token }`
- Token in both cookie AND response body (Tauri needs the body)

**POST /api/v1/auth/logout** (RequireUser)
- Delete current session from DB
- Clear cookie

**POST /api/v1/auth/register** (no auth required, rate limited)
- Body: `{ inviteCode, username, password, displayName }`
- Validate invite code (unclaimed, not expired)
- Create user with invite's role
- Claim invite
- Create session, set cookie, return user + token

**GET /api/v1/auth/me** (RequireUser)
- Return current user from `AuthenticatedUser`

### New file: `crates/stackarr-web/src/routes/admin.rs`

```rust
pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/v1/admin/users", get(list_users).post(create_user))
        .route("/api/v1/admin/users/{id}", get(get_user).put(update_user).delete(delete_user))
        .route("/api/v1/admin/invites", get(list_invites).post(create_invite))
        .route("/api/v1/admin/invites/{id}", delete(delete_invite))
}
```

All admin routes use `RequireAdmin` extractor.

**POST /api/v1/admin/users** - Admin creates user directly (generates temp password)
**POST /api/v1/admin/invites** - Returns `{ code, expiresAt }`

### New file: `crates/stackarr-web/src/routes/user.rs`

```rust
pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/v1/user/profile", put(update_profile))
        .route("/api/v1/user/devices", get(list_devices))
        .route("/api/v1/user/devices/{id}", delete(revoke_device))
        .route("/api/v1/user/sessions", get(list_sessions))
        .route("/api/v1/user/sessions/{id}", delete(revoke_session))
}
```

All user routes use `RequireUser` extractor and scope to `authenticated_user.user_id`.

### Register routes in `routes/mod.rs`
```rust
pub mod auth;
pub mod admin;
pub mod user;
```

### Mount in `lib.rs`
Auth routes are PUBLIC (no middleware wrapping):
```rust
.merge(routes::auth::router())  // alongside health and system::public_router
```

Admin and user routes are PROTECTED (but use their own extractors, not RequireApiKey wrapper):
```rust
.merge(routes::admin::router())
.merge(routes::user::router())
```

---

## 7. First-Boot / Migration Logic

In the server startup (`src/main.rs` or wherever DB init happens):

```rust
// After running SQL migrations:
let user_count = db.count_users().await?;
if user_count == 0 {
    // Check if there's a legacy API key
    if let Some(api_key) = db.get_config_value("api_key").await? {
        // Create admin user with username "admin" and a generated password
        let temp_password = stackarr_core::auth::generate_session_token();
        let hash = stackarr_core::auth::hash_password(&temp_password)?;
        let admin = db.create_user("admin", "Admin", &hash, "admin").await?;
        tracing::info!("Created admin user. Temporary password: {temp_password}");
        tracing::info!("Please change this password after first login.");

        // Migrate remote_clients
        let migrated = db.migrate_remote_clients_to_user_devices(admin.id).await?;
        tracing::info!("Migrated {migrated} remote clients to user devices");
    }
    // If no API key either, system is in first-boot mode (handled by middleware)
}
```

---

## 8. Client App Changes

### Update `client/vite.config.ts`
```typescript
export default defineConfig({
  base: '/app/',  // ADD THIS for web builds
  plugins: [react()],
  server: {
    port: 3001,
    proxy: {
      '/api': { target: 'http://192.168.0.30:9111', changeOrigin: true }
    }
  }
})
```

### Add dependencies to `client/package.json`
```
npm install @tanstack/react-query
```

### New file: `client/src/context/AuthContext.tsx`
```typescript
interface AuthUser {
  id: number
  username: string
  displayName: string
  role: 'admin' | 'user'
}

interface AuthState {
  user: AuthUser | null
  loading: boolean
  login(username: string, password: string, deviceToken?: string): Promise<void>
  logout(): Promise<void>
  register(inviteCode: string, username: string, password: string, displayName: string): Promise<void>
}
```

On mount, call `GET /api/v1/auth/me`. If 401, show login. For Tauri, also send stored token as Bearer header.

### New pages
- `client/src/pages/LoginPage.tsx` - username/password form, link to register
- `client/src/pages/RegisterPage.tsx` - invite code + username + password + display name

### Update `client/src/App.tsx`
- Wrap in `AuthProvider` and `QueryClientProvider`
- If not authenticated → show LoginPage (or RegisterPage if URL has invite code)
- If authenticated → show existing Browse/Series/Movie/Player routes
- Keep ServerConnect for Tauri builds (runs before auth, finds the server)

### Update `client/src/api.ts`
- Add session cookie support (automatic with `credentials: 'include'`)
- Add fallback Bearer token from localStorage (Tauri)
- Add auth API methods: `login()`, `logout()`, `register()`, `getMe()`

### Mount client in Axum (`crates/stackarr-web/src/lib.rs`)
```rust
// In build_router():
let client_ui_dir = std::env::var("STACKARR_CLIENT_DIR").unwrap_or_else(|_| "/client".to_string());
let client_index = PathBuf::from(&client_ui_dir).join("index.html");
if client_index.exists() {
    let client_service = ServeDir::new(&client_ui_dir)
        .fallback(ServeFile::new(&client_index));
    router = router.nest_service("/app", client_service);
}
```

---

## 9. Admin UI Changes (`ui/`)

Add a basic User Management page to the existing admin UI:

### New page: `ui/src/pages/Users.tsx`
- List all users (GET /api/v1/admin/users)
- Create user button → modal with username, display name, temp password, role
- Create invite button → modal, shows generated code
- Edit user (enable/disable, change role)
- Delete user (with confirmation)

### Update `ui/src/App.tsx` or sidebar
- Add "Users" nav item (admin only, always visible since admin UI is admin-only)

---

## 10. Cookie Handling in Axum

Add cookie setting to auth routes. Use `axum_extra::extract::cookie::CookieJar` or set `Set-Cookie` header manually:

```rust
use axum::http::header::{SET_COOKIE, HeaderValue};

fn session_cookie(token: &str, max_age_days: u32) -> String {
    format!(
        "stackarr_session={token}; HttpOnly; SameSite=Lax; Path=/; Max-Age={}",
        max_age_days * 86400
    )
}

fn clear_session_cookie() -> String {
    "stackarr_session=; HttpOnly; SameSite=Lax; Path=/; Max-Age=0".to_string()
}
```

Add `Secure` flag when not in dev mode (check config or env).

---

## Files to Create
- `migrations/006_users.sql`
- `crates/stackarr-core/src/models/user.rs`
- `crates/stackarr-core/src/auth.rs`
- `crates/stackarr-web/src/routes/auth.rs`
- `crates/stackarr-web/src/routes/admin.rs`
- `crates/stackarr-web/src/routes/user.rs`
- `client/src/context/AuthContext.tsx`
- `client/src/pages/LoginPage.tsx`
- `client/src/pages/RegisterPage.tsx`
- `ui/src/pages/Users.tsx`

## Files to Modify
- `crates/stackarr-core/Cargo.toml` (add argon2, sha2, hex)
- `crates/stackarr-core/src/models/mod.rs` (add user module)
- `crates/stackarr-core/src/lib.rs` (pub mod auth)
- `crates/stackarr-core/src/db.rs` (add all user/session/device/invite methods)
- `crates/stackarr-web/src/middleware.rs` (add AuthenticatedUser, RequireUser, RequireAdmin)
- `crates/stackarr-web/src/routes/mod.rs` (add auth, admin, user modules)
- `crates/stackarr-web/src/lib.rs` (mount /app, register new routes)
- `client/src/App.tsx` (auth-aware routing)
- `client/src/api.ts` (auth methods, cookie support)
- `client/vite.config.ts` (add base: '/app/')
- `client/package.json` (add @tanstack/react-query)
- `src/main.rs` (first-boot user creation, remote_clients migration)

## Verification
1. `cargo test --workspace --lib` passes
2. Start server → first boot creates admin user → log shows temp password
3. `POST /api/v1/auth/login` with admin creds → returns session + cookie
4. `GET /api/v1/auth/me` with cookie → returns admin user
5. `POST /api/v1/admin/invites` → returns invite code
6. `POST /api/v1/auth/register` with invite code → creates user, returns session
7. Client app at `/app` → shows login → login → shows library
8. Existing API key auth still works for all admin routes
9. Existing remote client tokens (migrated) still work for streaming
