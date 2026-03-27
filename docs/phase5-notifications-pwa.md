# Phase 5: Notifications + PWA

## Goal

In-app notification system that alerts users when new content arrives, request status changes, or system events occur. Make the client web app installable as a PWA with optional push notifications.

**Prerequisite:** Phase 1 (user accounts). Phases 2-4 are optional but notifications will reference their features.

---

## 1. Database Schema

From migration `006_users.sql` (or add as separate migration):

```sql
CREATE TABLE user_notifications (
    id BIGSERIAL PRIMARY KEY,
    user_id BIGINT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    notification_type TEXT NOT NULL,       -- see types below
    title TEXT NOT NULL,
    body TEXT,
    data JSONB,                           -- structured payload for deep linking
    read BOOLEAN NOT NULL DEFAULT false,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX idx_user_notifications_user ON user_notifications(user_id, read, created_at DESC);
CREATE INDEX idx_user_notifications_created ON user_notifications(created_at);

-- Optional: push subscription storage for web push
CREATE TABLE push_subscriptions (
    id BIGSERIAL PRIMARY KEY,
    user_id BIGINT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    endpoint TEXT NOT NULL UNIQUE,
    p256dh TEXT NOT NULL,
    auth TEXT NOT NULL,
    user_agent TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX idx_push_subscriptions_user ON push_subscriptions(user_id);
```

### Notification Types
| Type | Trigger | Data |
|------|---------|------|
| `new_episode` | Episode imported | `{ seriesId, seasonNumber, episodeNumber, title }` |
| `new_movie` | Movie imported | `{ movieId, title }` |
| `request_approved` | Admin approves request | `{ requestId, title, mediaType }` |
| `request_declined` | Admin declines request | `{ requestId, title, note }` |
| `request_available` | Requested media now in library | `{ requestId, mediaType, mediaId, title }` |
| `system` | Server announcements | `{ message }` |

---

## 2. Rust Models

### Add to `crates/stackarr-core/src/models/user.rs`

```rust
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct UserNotification {
    pub id: i64,
    pub user_id: i64,
    pub notification_type: String,
    pub title: String,
    pub body: Option<String>,
    pub data: Option<serde_json::Value>,
    pub read: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct PushSubscription {
    pub id: i64,
    pub user_id: i64,
    pub endpoint: String,
    pub p256dh: String,
    pub auth: String,
    pub user_agent: Option<String>,
    pub created_at: DateTime<Utc>,
}
```

---

## 3. Database Methods (`crates/stackarr-core/src/db.rs`)

### Notifications
```rust
pub async fn create_notification(
    &self,
    user_id: i64,
    notification_type: &str,
    title: &str,
    body: Option<&str>,
    data: Option<&serde_json::Value>,
) -> crate::Result<UserNotification>

/// Create notification for ALL users (e.g., new content)
pub async fn create_notification_for_all_users(
    &self,
    notification_type: &str,
    title: &str,
    body: Option<&str>,
    data: Option<&serde_json::Value>,
) -> crate::Result<u64>
// INSERT INTO user_notifications (user_id, ...)
// SELECT id, $1, $2, $3, $4 FROM users WHERE enabled = true

pub async fn list_notifications(
    &self,
    user_id: i64,
    unread_only: bool,
    limit: i64,
    offset: i64,
) -> crate::Result<Vec<UserNotification>>
// ORDER BY created_at DESC LIMIT $3 OFFSET $4

pub async fn unread_notification_count(
    &self,
    user_id: i64,
) -> crate::Result<i64>
// SELECT COUNT(*) FROM user_notifications WHERE user_id = $1 AND read = false

pub async fn mark_notification_read(
    &self,
    notification_id: i64,
    user_id: i64,
) -> crate::Result<bool>
// UPDATE ... WHERE id = $1 AND user_id = $2

pub async fn mark_all_notifications_read(
    &self,
    user_id: i64,
) -> crate::Result<u64>
// UPDATE ... WHERE user_id = $1 AND read = false

pub async fn delete_old_notifications(
    &self,
    older_than_days: i32,
) -> crate::Result<u64>
// DELETE FROM user_notifications WHERE created_at < NOW() - INTERVAL '$1 days'
// Called by scheduler periodically
```

### Push subscriptions (optional, for web push)
```rust
pub async fn save_push_subscription(
    &self,
    user_id: i64,
    endpoint: &str,
    p256dh: &str,
    auth: &str,
    user_agent: Option<&str>,
) -> crate::Result<()>
// INSERT ... ON CONFLICT (endpoint) DO UPDATE SET user_id, p256dh, auth

pub async fn get_push_subscriptions(
    &self,
    user_id: i64,
) -> crate::Result<Vec<PushSubscription>>

pub async fn delete_push_subscription(
    &self,
    endpoint: &str,
) -> crate::Result<bool>
```

---

## 4. Notification Service

### New file: `crates/stackarr-core/src/notifications.rs`

A helper module that provides high-level notification creation. This is called from import hooks, request handlers, etc.

```rust
use crate::db::Database;

pub struct NotificationService {
    db: Database,
}

impl NotificationService {
    pub fn new(db: Database) -> Self { Self { db } }

    /// New episode imported - notify all users
    pub async fn notify_new_episode(
        &self,
        series_title: &str,
        season: i32,
        episode: i32,
        episode_title: &str,
        series_id: i64,
    ) -> crate::Result<()> {
        let title = format!("New Episode: {series_title}");
        let body = format!("S{season:02}E{episode:02} - {episode_title}");
        let data = serde_json::json!({
            "seriesId": series_id,
            "seasonNumber": season,
            "episodeNumber": episode,
            "title": episode_title
        });
        self.db.create_notification_for_all_users(
            "new_episode", &title, Some(&body), Some(&data)
        ).await?;
        Ok(())
    }

    /// New movie imported - notify all users
    pub async fn notify_new_movie(
        &self,
        movie_title: &str,
        movie_id: i64,
    ) -> crate::Result<()> {
        let data = serde_json::json!({ "movieId": movie_id, "title": movie_title });
        self.db.create_notification_for_all_users(
            "new_movie", &format!("New Movie: {movie_title}"), None, Some(&data)
        ).await?;
        Ok(())
    }

    /// Request status changed - notify the requesting user
    pub async fn notify_request_update(
        &self,
        user_id: i64,
        request_id: i64,
        title: &str,
        status: &str,
        note: Option<&str>,
    ) -> crate::Result<()> {
        let notif_title = match status {
            "approved" => format!("Request Approved: {title}"),
            "declined" => format!("Request Declined: {title}"),
            "available" => format!("Now Available: {title}"),
            _ => format!("Request Update: {title}"),
        };
        let data = serde_json::json!({ "requestId": request_id, "title": title, "status": status });
        self.db.create_notification(
            user_id, "request_update", &notif_title, note, Some(&data)
        ).await?;
        Ok(())
    }
}
```

### Integration points

Add `NotificationService` to `AppState` (or create on-demand from `state.db`).

**In import pipeline** (`crates/stackarr-import/`):
- After episode file imported → `notify_new_episode()`
- After movie file imported → `notify_new_movie()`

**In request handlers** (`crates/stackarr-web/src/routes/requests.rs`):
- After approve → `notify_request_update(user_id, ..., "approved")`
- After decline → `notify_request_update(user_id, ..., "declined")`
- After mark_request_available → `notify_request_update(user_id, ..., "available")`

**In scheduler** (`crates/stackarr-scheduler/`):
- Add periodic cleanup task: `delete_old_notifications(30)` — remove notifications older than 30 days

---

## 5. API Routes

### Add to `crates/stackarr-web/src/routes/user.rs` or new `notifications.rs`

```rust
.route("/api/v1/user/notifications", get(list_notifications))
.route("/api/v1/user/notifications/unread-count", get(unread_count))
.route("/api/v1/user/notifications/:id/read", put(mark_read))
.route("/api/v1/user/notifications/read-all", put(mark_all_read))
// Optional: push subscription management
.route("/api/v1/user/push-subscription", post(subscribe_push).delete(unsubscribe_push))
```

All routes use `RequireUser`.

**GET /api/v1/user/notifications**
- Query: `?unread=true&limit=50&offset=0`
- Returns paginated notifications

**GET /api/v1/user/notifications/unread-count**
- Returns `{ count: number }`
- Called frequently (on page load, on focus) for badge updates

**PUT /api/v1/user/notifications/:id/read**
- Mark single notification as read
- Returns 204

**PUT /api/v1/user/notifications/read-all**
- Mark all as read
- Returns `{ marked: number }`

---

## 6. Client App - Notification Bell

### New component: `client/src/components/NotificationBell.tsx`

```typescript
function NotificationBell() {
  const { data: unreadCount } = useQuery({
    queryKey: ['notifications-unread-count'],
    queryFn: () => api.getUnreadNotificationCount(),
    refetchInterval: 30_000,  // poll every 30s
    refetchOnWindowFocus: true,
  })

  const [open, setOpen] = useState(false)

  return (
    <div className="relative">
      <button onClick={() => setOpen(!open)}>
        <BellIcon />
        {unreadCount > 0 && (
          <span className="absolute -top-1 -right-1 bg-red-500 text-white text-xs rounded-full w-5 h-5 flex items-center justify-center">
            {unreadCount > 99 ? '99+' : unreadCount}
          </span>
        )}
      </button>
      {open && <NotificationDropdown onClose={() => setOpen(false)} />}
    </div>
  )
}
```

### New component: `client/src/components/NotificationDropdown.tsx`

```typescript
function NotificationDropdown({ onClose }) {
  const { data: notifications } = useQuery({
    queryKey: ['notifications'],
    queryFn: () => api.listNotifications({ limit: 20 }),
  })

  const markAllRead = useMutation({
    mutationFn: () => api.markAllNotificationsRead(),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['notifications'] })
      queryClient.invalidateQueries({ queryKey: ['notifications-unread-count'] })
    },
  })

  return (
    <div className="absolute right-0 top-10 w-80 bg-slate-800 rounded-lg shadow-xl border border-slate-700 max-h-96 overflow-y-auto">
      <div className="flex justify-between items-center p-3 border-b border-slate-700">
        <h3>Notifications</h3>
        <button onClick={() => markAllRead.mutate()}>Mark all read</button>
      </div>
      {notifications?.map(notif => (
        <NotificationItem key={notif.id} notification={notif} />
      ))}
      {notifications?.length === 0 && (
        <div className="p-4 text-center text-slate-400">No notifications</div>
      )}
    </div>
  )
}
```

### NotificationItem
- Click notification → navigate to relevant page (using `data` field for deep link)
- Mark as read on click
- Unread notifications have a blue dot indicator
- Show relative time ("2 hours ago")

### Integration into layout
Add `<NotificationBell />` to the top nav/header in the authenticated layout (`client/src/App.tsx` or layout component).

---

## 7. PWA Setup

### New file: `client/public/manifest.json`

```json
{
  "name": "StackArr",
  "short_name": "StackArr",
  "description": "Media streaming and management",
  "start_url": "/app/",
  "scope": "/app/",
  "display": "standalone",
  "background_color": "#0f172a",
  "theme_color": "#3b82f6",
  "icons": [
    { "src": "/app/icons/icon-192.png", "sizes": "192x192", "type": "image/png" },
    { "src": "/app/icons/icon-512.png", "sizes": "512x512", "type": "image/png" },
    { "src": "/app/icons/icon-maskable-512.png", "sizes": "512x512", "type": "image/png", "purpose": "maskable" }
  ]
}
```

### Update `client/index.html`
```html
<link rel="manifest" href="/app/manifest.json">
<meta name="theme-color" content="#3b82f6">
<meta name="apple-mobile-web-app-capable" content="yes">
<meta name="apple-mobile-web-app-status-bar-style" content="black-translucent">
<link rel="apple-touch-icon" href="/app/icons/icon-192.png">
```

### Service Worker: `client/public/sw.js`

Minimal service worker for app shell caching:

```javascript
const CACHE_NAME = 'stackarr-v1'
const SHELL_URLS = ['/app/', '/app/index.html']

self.addEventListener('install', (event) => {
  event.waitUntil(
    caches.open(CACHE_NAME).then((cache) => cache.addAll(SHELL_URLS))
  )
  self.skipWaiting()
})

self.addEventListener('activate', (event) => {
  event.waitUntil(
    caches.keys().then((keys) =>
      Promise.all(keys.filter((k) => k !== CACHE_NAME).map((k) => caches.delete(k)))
    )
  )
  self.clients.claim()
})

self.addEventListener('fetch', (event) => {
  // Network-first for API calls
  if (event.request.url.includes('/api/')) return

  // Cache-first for app shell assets
  event.respondWith(
    caches.match(event.request).then((cached) => cached || fetch(event.request))
  )
})

// Push notification handler (for future web push)
self.addEventListener('push', (event) => {
  const data = event.data?.json() || {}
  event.waitUntil(
    self.registration.showNotification(data.title || 'StackArr', {
      body: data.body,
      icon: '/app/icons/icon-192.png',
      badge: '/app/icons/icon-96.png',
      data: data.data, // for click handling
    })
  )
})

self.addEventListener('notificationclick', (event) => {
  event.notification.close()
  const data = event.notification.data || {}
  // Deep link based on notification data
  let url = '/app/'
  if (data.seriesId) url = `/app/series/${data.seriesId}`
  else if (data.movieId) url = `/app/movie/${data.movieId}`
  event.waitUntil(clients.openWindow(url))
})
```

### Register service worker in `client/src/main.tsx`
```typescript
if ('serviceWorker' in navigator && !('__TAURI_INTERNALS__' in window)) {
  navigator.serviceWorker.register('/app/sw.js')
}
```

Only register in web builds (not Tauri — Tauri has its own update mechanism).

---

## 8. Icons

Create PNG icons for the PWA manifest. These can be simple StackArr-branded icons:
- `client/public/icons/icon-192.png`
- `client/public/icons/icon-512.png`
- `client/public/icons/icon-maskable-512.png` (with safe zone padding)
- `client/public/icons/icon-96.png` (notification badge)

These can be placeholder colored squares initially and replaced with proper branding later.

---

## 9. API Client Updates (`client/src/api.ts`)

```typescript
// Notifications
listNotifications(params?: { unread?: boolean; limit?: number; offset?: number }): Promise<UserNotification[]>
getUnreadNotificationCount(): Promise<number>
markNotificationRead(id: number): Promise<void>
markAllNotificationsRead(): Promise<void>

// Push subscriptions (optional)
subscribePush(subscription: PushSubscriptionJSON): Promise<void>
unsubscribePush(endpoint: string): Promise<void>
```

---

## Files to Create
- `crates/stackarr-core/src/notifications.rs`
- `crates/stackarr-web/src/routes/notifications.rs` (or extend user.rs)
- `client/src/components/NotificationBell.tsx`
- `client/src/components/NotificationDropdown.tsx`
- `client/src/components/NotificationItem.tsx`
- `client/public/manifest.json`
- `client/public/sw.js`
- `client/public/icons/` (4 PNG files)

## Files to Modify
- `crates/stackarr-core/src/models/user.rs` (add notification + push subscription models)
- `crates/stackarr-core/src/db.rs` (add notification + push methods)
- `crates/stackarr-core/src/lib.rs` (pub mod notifications)
- `crates/stackarr-web/src/routes/mod.rs` (add notifications module)
- `crates/stackarr-web/src/lib.rs` (register notification routes)
- `crates/stackarr-web/src/state.rs` (optionally add NotificationService)
- `crates/stackarr-web/src/routes/requests.rs` (add notification calls on approve/decline)
- Import pipeline handlers (add notification calls on episode/movie import)
- `crates/stackarr-scheduler/` (add notification cleanup task)
- `client/index.html` (add manifest + meta tags)
- `client/src/main.tsx` (register service worker)
- `client/src/App.tsx` or layout (add NotificationBell to header)
- `client/src/api.ts` (add notification API methods)

## Verification
1. `cargo test --workspace --lib` passes
2. Import an episode → all users get "New Episode" notification
3. Client shows bell icon with unread count badge
4. Click bell → dropdown shows notifications
5. Click notification → navigates to series/movie detail
6. "Mark all read" → count resets to 0
7. Admin approves request → requesting user gets notification
8. Open client in mobile browser → "Add to Home Screen" prompt appears (PWA)
9. Installed PWA opens in standalone mode with correct theme
10. Service worker caches app shell (works offline for cached pages)
