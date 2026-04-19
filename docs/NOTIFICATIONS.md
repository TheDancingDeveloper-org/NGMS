# Notification System

StackArr has two independent notification subsystems:

1. **External providers** -- outbound notifications sent to third-party services (Discord, Slack, Telegram, webhooks, email) when system events occur (grabs, imports, failures, etc.).
2. **In-app user notifications** -- per-user notification records stored in PostgreSQL, with read/unread tracking, push subscription management, and automatic cleanup.

Both subsystems are independent. External providers are configured globally by admins; in-app notifications are scoped to individual user accounts.

---

## External Notification Providers

### Supported Provider Types

| Type | Config Keys | Transport |
|------|------------|-----------|
| `webhook` | `url` | POST JSON event payload to arbitrary URL |
| `discord` | `webhook_url` (or `url`) | POST `{"content": "<summary>"}` to Discord webhook |
| `telegram` | `bot_token`, `chat_id` | POST to `https://api.telegram.org/bot<token>/sendMessage` with HTML parse mode |
| `slack` | `webhook_url` (or `url`) | POST `{"text": "<summary>"}` to Slack incoming webhook |
| `email` | `smtp_url`, `from`, `to` | POST JSON to an SMTP relay endpoint |

### Config Format (JSONB)

Each provider stores its configuration as a JSONB `config` column. The required fields per type:

```jsonc
// webhook
{ "url": "https://example.com/hook" }

// discord
{ "webhook_url": "https://discord.com/api/webhooks/..." }

// telegram
{ "bot_token": "123456:ABC-DEF...", "chat_id": "-1001234567890" }

// slack
{ "webhook_url": "https://hooks.slack.com/services/T.../B.../xxx" }

// email
{ "smtp_url": "https://smtp-relay.example.com/send", "from": "stackarr@example.com", "to": "user@example.com" }
```

Discord and Slack accept either `webhook_url` or `url` as the config key (for convenience).

### Event Filtering

Each provider row has five boolean columns that control which events it receives:

| Column | Event |
|--------|-------|
| `on_grab` | A release was grabbed from an indexer |
| `on_import` | A download was imported to the library |
| `on_upgrade` | An existing file was replaced with a higher quality version |
| `on_health_issue` | A system health check detected a problem |
| `on_failure` | A download or import failed |

When creating a provider, all five default to `true` if omitted. Providers with `enabled = false` are skipped entirely.

---

## Event Types

The `NotificationEvent` enum (in `stackarr-notify`) defines five event variants. Each variant is serialized as tagged JSON with `"type"` as the discriminator.

### Grab

Fired when a release is sent to a download client.

```json
{
  "type": "Grab",
  "title": "Breaking Bad S01E01",
  "quality": "HDTV-720p",
  "indexer": "NZBGeek"
}
```

Summary: `Grabbed: Breaking Bad S01E01 [HDTV-720p]`

### Import

Fired when a completed download is imported into the media library.

```json
{
  "type": "Import",
  "title": "The Wire S03E05",
  "quality": "Bluray-1080p"
}
```

Summary: `Imported: The Wire S03E05 [Bluray-1080p]`

### Upgrade

Fired when a file is replaced by a higher quality version.

```json
{
  "type": "Upgrade",
  "title": "Movie Title",
  "oldQuality": "HDTV-720p",
  "newQuality": "Bluray-1080p"
}
```

Summary: `Upgraded: Movie Title [Bluray-1080p]`

### HealthIssue

Fired when an internal health check detects a problem.

```json
{
  "type": "HealthIssue",
  "source": "Indexer",
  "message": "NZBGeek unavailable"
}
```

Summary: `Health: Indexer - NZBGeek unavailable`

### DownloadFailure

Fired when a download or import operation fails.

```json
{
  "type": "DownloadFailure",
  "title": "Some.Release.720p",
  "message": "Import failed after 3 attempts: file not found"
}
```

Summary: `Failed: Some.Release.720p - Import failed after 3 attempts: file not found`

---

## Dispatch Flow

The primary entry point for sending external notifications is `stackarr_notify::dispatch_event(pool, event)`. This function:

1. **Loads all enabled providers** from the `notification_providers` table (`WHERE enabled = true`).
2. **Filters by event type** -- calls `wants_event()` on each row, checking the boolean column that corresponds to the event variant (e.g., `on_grab` for `Grab` events).
3. **Builds a concrete provider** from the row's `provider_type` and `config` JSONB via `build_provider()`. If config is malformed or the type is unknown, the row is skipped with a warning log.
4. **Sends the event** to each matching provider. Errors from individual providers are logged but never propagated -- a failing Discord webhook does not prevent Telegram from receiving the same event.

### Call Sites

`dispatch_event` is called from:

- **`stackarr-web/routes/releases.rs`** -- after a release is grabbed (Grab event).
- **`stackarr-scheduler/src/lib.rs`** -- after a completed download is imported (Import event), after import failure (DownloadFailure event), and after download failure/blocklisting (DownloadFailure event).

### Provider Construction

The `build_provider_from_config(provider_type, config)` public function constructs a provider instance from a type string and JSONB config without touching the database. This is used by the test endpoint to validate a configuration before saving.

---

## Provider CRUD API

All provider endpoints require admin authentication (`RequireAdmin` extractor). Responses use `camelCase` JSON keys. Sensitive fields in `config` (tokens, keys) are redacted in responses via `redact_sensitive_fields`.

### List Providers

```
GET /api/v1/notification/provider
```

Returns an array of all providers ordered by ID.

**Response** `200`:
```json
[
  {
    "id": 1,
    "name": "My Discord",
    "providerType": "discord",
    "config": { "webhookUrl": "**REDACTED**" },
    "onGrab": true,
    "onImport": true,
    "onUpgrade": true,
    "onHealthIssue": true,
    "onFailure": true,
    "enabled": true
  }
]
```

### Get Provider

```
GET /api/v1/notification/provider/{id}
```

**Response** `200`: Single provider object. `404` if not found.

### Create Provider

```
POST /api/v1/notification/provider
```

**Request body**:
```json
{
  "name": "My Discord",
  "providerType": "discord",
  "config": { "webhookUrl": "https://discord.com/api/webhooks/..." },
  "onGrab": true,
  "onImport": true,
  "onUpgrade": false,
  "onHealthIssue": true,
  "onFailure": true,
  "enabled": true
}
```

- `name` is required (non-empty).
- `providerType` must be one of: `webhook`, `discord`, `telegram`, `slack`, `email`.
- `config` is validated for required fields per provider type.
- Boolean fields (`onGrab`, `onImport`, `onUpgrade`, `onHealthIssue`, `onFailure`, `enabled`) default to `true` if omitted.

**Response** `201`: Created provider object. `400` on validation error.

### Update Provider

```
PUT /api/v1/notification/provider/{id}
```

**Request body**: Same shape as create, but all fields are optional. Only provided fields are updated (uses `COALESCE` in SQL).

**Response** `200`: Updated provider object. `404` if not found.

### Delete Provider

```
DELETE /api/v1/notification/provider/{id}
```

**Response** `204` on success. `404` if not found.

---

## Testing Providers

Two test endpoints allow sending a test notification without waiting for a real event.

### Test a Saved Provider

```
POST /api/v1/notification/provider/{id}/test
```

Loads the provider's type and config from the database, builds it, and sends a test `HealthIssue` event with source `"test"` and message `"This is a test notification from StackArr"`.

### Test an Unsaved Configuration

```
POST /api/v1/notification/provider/test
```

**Request body**:
```json
{
  "providerType": "telegram",
  "config": { "botToken": "123:ABC", "chatId": "-100123" }
}
```

Builds a provider from the given type and config without touching the database, then sends the same test event.

### Timeout Behavior

Both test endpoints wrap the provider's `test()` call in a 15-second timeout (`tokio::time::timeout`). Possible outcomes:

| Result | Response |
|--------|----------|
| Success | `{"success": true, "message": "test notification sent successfully"}` |
| Provider error | `{"success": false, "message": "<error details>"}` |
| Timeout | `{"success": false, "message": "test timed out after 15 seconds"}` |
| Bad config | `{"success": false, "message": "failed to build <type> provider from config -- check required fields"}` |

---

## In-App User Notifications

Per-user notifications stored in the `user_notifications` table. These are separate from external providers and are managed through user-scoped API endpoints (require `RequireUser` authentication, not admin).

### Database Schema

```sql
CREATE TABLE user_notifications (
    id          BIGSERIAL PRIMARY KEY,
    user_id     BIGINT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    notification_type TEXT NOT NULL,
    title       TEXT NOT NULL,
    body        TEXT,
    data        JSONB,
    read        BOOLEAN NOT NULL DEFAULT false,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
```

Indexed on `(user_id, read, created_at DESC)` for efficient unread-first queries.

### API Endpoints

#### List Notifications

```
GET /api/v1/user/notifications?unread=true&limit=50&offset=0
```

| Param | Default | Description |
|-------|---------|-------------|
| `unread` | `false` | If `true`, return only unread notifications |
| `limit` | `50` | Max results (capped at 200) |
| `offset` | `0` | Pagination offset |

Returns an array of `UserNotification` objects ordered by `created_at DESC`.

#### Unread Count

```
GET /api/v1/user/notifications/unread-count
```

**Response**: `{"count": 5}`

#### Mark Single as Read

```
PUT /api/v1/user/notifications/{id}/read
```

**Response** `200`: `{"ok": true}`. `404` if the notification does not exist or belongs to another user.

#### Mark All as Read

```
PUT /api/v1/user/notifications/read-all
```

**Response**: `{"marked": 12}` (number of notifications marked read).

#### Clear All Notifications

```
DELETE /api/v1/user/notifications
```

Permanently deletes all notifications for the authenticated user.

**Response**: `{"deleted": 42}`

### Automatic Cleanup

The `delete_old_notifications(days)` DB method deletes notifications older than the specified number of days. This is intended to be called from the scheduler to prevent unbounded table growth.

### Push Subscriptions

The `push_subscriptions` table stores Web Push API subscription credentials per user, enabling browser push notifications.

```sql
CREATE TABLE push_subscriptions (
    id          BIGSERIAL PRIMARY KEY,
    user_id     BIGINT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    endpoint    TEXT NOT NULL UNIQUE,
    p256dh      TEXT NOT NULL,
    auth        TEXT NOT NULL,
    user_agent  TEXT,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
```

#### Save Push Subscription

```
POST /api/v1/user/push-subscription
```

**Request body**:
```json
{
  "endpoint": "https://fcm.googleapis.com/fcm/send/...",
  "p256dh": "BNcRdreALRFXTkOOUHK1...",
  "auth": "tBHItqI7...",
  "userAgent": "Mozilla/5.0..."
}
```

Uses `ON CONFLICT (endpoint) DO UPDATE` so re-subscribing from the same browser replaces the old keys.

#### Remove Push Subscription

```
DELETE /api/v1/user/push-subscription
```

**Request body**:
```json
{ "endpoint": "https://fcm.googleapis.com/fcm/send/..." }
```

**Response** `204` on success. `404` if no matching subscription exists for the user.

---

## Adding a New Provider Type

To add a new external notification provider (e.g., Gotify, Pushover, ntfy):

### 1. Implement the Provider (stackarr-notify/src/lib.rs)

Create a new struct and implement the `NotificationProvider` trait:

```rust
pub struct GotifyProvider {
    client: reqwest::Client,
    url: String,
    token: String,
}

impl GotifyProvider {
    pub fn new(url: String, token: String) -> Self {
        Self { client: reqwest::Client::new(), url, token }
    }
}

#[async_trait::async_trait]
impl NotificationProvider for GotifyProvider {
    fn name(&self) -> &str { "Gotify" }

    async fn send(&self, event: &NotificationEvent) -> Result<()> {
        tracing::debug!("sending gotify notification");
        let body = serde_json::json!({
            "title": "StackArr",
            "message": event.summary(),
            "priority": 5,
        });
        self.client
            .post(format!("{}/message", self.url))
            .header("X-Gotify-Key", &self.token)
            .json(&body)
            .send()
            .await?
            .error_for_status()?;
        Ok(())
    }

    async fn test(&self) -> Result<()> {
        let test_event = NotificationEvent::HealthIssue {
            source: "test".to_string(),
            message: "This is a test notification from StackArr".to_string(),
        };
        self.send(&test_event).await
    }
}
```

### 2. Register in build_provider

Add a match arm in both `NotificationProviderRow::build_provider()` and `build_provider_from_config()`:

```rust
"gotify" => {
    let url = config.get("url")?.as_str()?;
    let token = config.get("token")?.as_str()?;
    Some(Box::new(GotifyProvider::new(url.to_string(), token.to_string())))
}
```

### 3. Add to Valid Types (stackarr-web)

In `crates/stackarr-web/src/routes/notification_providers.rs`, add the type to the constant:

```rust
const VALID_PROVIDER_TYPES: &[&str] = &["webhook", "discord", "telegram", "slack", "email", "gotify"];
```

### 4. Add Config Validation

Add a validation arm in `validate_provider_config()`:

```rust
"gotify" => {
    for field in &["url", "token"] {
        if config.get(*field).and_then(|v| v.as_str()).is_none() {
            return Some(format!("gotify provider requires '{field}' in config"));
        }
    }
}
```

### 5. Add Tests

Add unit tests in the `#[cfg(test)]` module of `lib.rs` using `wiremock` to mock the HTTP endpoint, following the pattern of existing provider tests.

No database migration is needed -- the `provider_type` column is a free-form `TEXT` field and the `config` column is `JSONB`, so new provider types are supported without schema changes.

---

## Source Files

| File | Purpose |
|------|---------|
| `crates/stackarr-notify/src/lib.rs` | Provider trait, 5 provider implementations, `NotificationService`, `dispatch_event` |
| `crates/stackarr-web/src/routes/notification_providers.rs` | Admin CRUD + test endpoints for providers |
| `crates/stackarr-web/src/routes/notifications.rs` | User notification + push subscription endpoints |
| `crates/stackarr-core/src/models/user.rs` | `UserNotification` and `PushSubscription` model structs |
| `crates/stackarr-core/src/db.rs` | DB methods for user notifications and push subscriptions |
| `crates/stackarr-scheduler/src/lib.rs` | Calls `dispatch_event` for Import and DownloadFailure events |
| `crates/stackarr-web/src/routes/releases.rs` | Calls `dispatch_event` for Grab events |
| `migrations/001_initial.sql` | `notification_providers` table schema |
| `migrations/006_users.sql` | `user_notifications` and `push_subscriptions` table schemas |
