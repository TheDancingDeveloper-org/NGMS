# Phase 3: Media Requests

## Goal

Allow users to request TV series and movies that aren't in the library yet. Admins can approve/decline requests. Approved requests auto-add media to the library via the existing TMDB + series/movie creation flow.

**Prerequisite:** Phase 1 (user accounts) must be complete.

---

## 1. Database Schema

The `media_requests` table should already exist from migration `006_users.sql`. If not:

```sql
CREATE TABLE media_requests (
    id BIGSERIAL PRIMARY KEY,
    user_id BIGINT NOT NULL REFERENCES users(id),
    media_type TEXT NOT NULL,              -- 'series' | 'movie'
    tmdb_id BIGINT NOT NULL,
    title TEXT NOT NULL,
    year INTEGER,
    poster_url TEXT,
    overview TEXT,
    status TEXT NOT NULL DEFAULT 'pending', -- 'pending' | 'approved' | 'declined' | 'available'
    admin_note TEXT,
    approved_by BIGINT REFERENCES users(id),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(tmdb_id, media_type)
);
CREATE INDEX idx_media_requests_user ON media_requests(user_id);
CREATE INDEX idx_media_requests_status ON media_requests(status);
```

---

## 2. Rust Model

### Add to `crates/stackarr-core/src/models/user.rs` (or new `request.rs`)

```rust
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct MediaRequest {
    pub id: i64,
    pub user_id: i64,
    pub media_type: String,
    pub tmdb_id: i64,
    pub title: String,
    pub year: Option<i32>,
    pub poster_url: Option<String>,
    pub overview: Option<String>,
    pub status: String,
    pub admin_note: Option<String>,
    pub approved_by: Option<i64>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Enriched request with requester info (for admin listing)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaRequestWithUser {
    #[serde(flatten)]
    pub request: MediaRequest,
    pub requested_by: String,  // username
    pub approved_by_name: Option<String>,
}
```

---

## 3. Database Methods (`crates/stackarr-core/src/db.rs`)

```rust
pub async fn create_media_request(
    &self,
    user_id: i64,
    media_type: &str,
    tmdb_id: i64,
    title: &str,
    year: Option<i32>,
    poster_url: Option<&str>,
    overview: Option<&str>,
) -> crate::Result<MediaRequest>
// INSERT ... ON CONFLICT (tmdb_id, media_type) DO NOTHING (or return existing)

pub async fn get_media_request(&self, id: i64) -> crate::Result<Option<MediaRequest>>

pub async fn list_media_requests(
    &self,
    status: Option<&str>,
    user_id: Option<i64>,  // None = all (admin), Some = filter to user
) -> crate::Result<Vec<MediaRequest>>
// If user_id is Some, WHERE user_id = $1
// If status is Some, AND status = $2
// ORDER BY created_at DESC

pub async fn update_request_status(
    &self,
    id: i64,
    status: &str,
    approved_by: Option<i64>,
    admin_note: Option<&str>,
) -> crate::Result<bool>
// UPDATE media_requests SET status = $2, approved_by = $3, admin_note = $4, updated_at = NOW() WHERE id = $1

pub async fn delete_media_request(&self, id: i64) -> crate::Result<bool>

pub async fn check_request_exists(&self, tmdb_id: i64, media_type: &str) -> crate::Result<Option<MediaRequest>>
// For showing "Already Requested" state on discover page

pub async fn mark_request_available(&self, tmdb_id: i64, media_type: &str) -> crate::Result<bool>
// UPDATE media_requests SET status = 'available', updated_at = NOW()
// WHERE tmdb_id = $1 AND media_type = $2 AND status IN ('pending', 'approved')
// Called when media is actually added/imported
```

---

## 4. API Routes

### New file: `crates/stackarr-web/src/routes/requests.rs`

```rust
pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/v1/requests", get(list_requests).post(create_request))
        .route("/api/v1/requests/:id", get(get_request).delete(delete_request))
        .route("/api/v1/requests/:id/approve", put(approve_request))
        .route("/api/v1/requests/:id/decline", put(decline_request))
}
```

**POST /api/v1/requests** (RequireUser)
- Body: `{ mediaType: "series"|"movie", tmdbId: number, title: string, year?: number, posterUrl?: string, overview?: string }`
- Check if already in library (series/movies table by tmdb_id) → return 409 "Already in library"
- Check if already requested → return 409 "Already requested" with existing request
- Create request with status "pending"

**GET /api/v1/requests** (RequireUser)
- Query params: `?status=pending&mine=true`
- If user role is admin AND `mine` is not set → return all requests
- If user role is user OR `mine=true` → return only user's requests
- Include requester username via JOIN

**PUT /api/v1/requests/:id/approve** (RequireAdmin)
- Body: `{ note?: string }`
- Set status = "approved", approved_by = admin user_id
- **Trigger media addition:**
  - For series: call the existing series creation flow (TMDB lookup → create series → trigger search)
  - For movie: call the existing movie creation flow
  - Use existing `SeriesService::add_series()` or `MovieService::add_movie()` patterns
- If auto-add succeeds, set status = "available"
- Create notification for requesting user (Phase 5, or just log for now)

**PUT /api/v1/requests/:id/decline** (RequireAdmin)
- Body: `{ note?: string }`
- Set status = "declined"

**DELETE /api/v1/requests/:id** (RequireAdmin)
- Hard delete

---

## 5. Auto-Availability Hook

When media is imported (episodes/movies added to library), check if there's a matching request and mark it available.

### In `crates/stackarr-import/` or the import route handler:

After successful import, call:
```rust
// After a series/movie is fully imported:
state.db.mark_request_available(tmdb_id, media_type).await?;
```

This should be added to:
- Series creation flow (when admin adds via UI)
- Movie creation flow
- Import scan completion
- Approve handler (immediate if TMDB lookup + add succeeds)

---

## 6. Client App - Discover Page

### New page: `client/src/pages/DiscoverPage.tsx`

The discover page lets users search TMDB for content to request.

```typescript
function DiscoverPage() {
  const [query, setQuery] = useState('')
  const [mediaType, setMediaType] = useState<'series' | 'movie'>('series')

  // Search TMDB via the server's existing search endpoint
  const { data: results } = useQuery({
    queryKey: ['discover', mediaType, query],
    queryFn: () => api.searchTmdb(query, mediaType),
    enabled: query.length >= 2,
  })

  // Fetch existing requests to show status
  const { data: myRequests } = useQuery({
    queryKey: ['my-requests'],
    queryFn: () => api.listRequests({ mine: true }),
  })

  // Fetch library to show "In Library" badge
  const { data: library } = useQuery({
    queryKey: ['library-tmdb-ids', mediaType],
    queryFn: () => api.getLibraryTmdbIds(mediaType),
  })

  return (
    <div>
      <SearchBar value={query} onChange={setQuery} />
      <MediaTypeToggle value={mediaType} onChange={setMediaType} />
      <DiscoverGrid
        results={results}
        requests={myRequests}
        libraryIds={library}
        onRequest={(item) => requestMutation.mutate(item)}
      />
    </div>
  )
}
```

Each result card shows one of:
- "In Library" badge (already have it) → links to detail page
- "Requested" badge (pending/approved) → shows status
- "Request" button → creates request

### TMDB Search API

The server needs a TMDB search endpoint accessible to regular users (not just admins). Check if existing TMDB search in `stackarr-metadata` is exposed via API. If only admin-accessible, add a user-accessible variant:

```
GET /api/v1/discover/search?q=breaking+bad&type=series  (RequireUser)
```

This proxies to TMDB search and returns simplified results:
```json
[{
  "tmdbId": 1396,
  "title": "Breaking Bad",
  "year": 2008,
  "overview": "...",
  "posterUrl": "https://image.tmdb.org/...",
  "inLibrary": false,
  "requestStatus": null
}]
```

Also add:
```
GET /api/v1/discover/trending?type=series  (RequireUser)
```
For the default discover page view (before searching).

---

## 7. Client App - Requests Page

### New page: `client/src/pages/RequestsPage.tsx`

```typescript
function RequestsPage() {
  const { data: requests } = useQuery({
    queryKey: ['my-requests'],
    queryFn: () => api.listRequests({ mine: true }),
  })

  return (
    <div>
      <h1>My Requests</h1>
      {requests?.map(req => (
        <RequestCard key={req.id} request={req} />
      ))}
    </div>
  )
}

function RequestCard({ request }) {
  // Show poster, title, year, status badge (pending/approved/declined/available)
  // Status colors: pending=yellow, approved=blue, declined=red, available=green
}
```

---

## 8. Admin UI - Request Management

### New page or section: `ui/src/pages/Requests.tsx` (admin UI)

- List all requests with requester name
- Filter by status
- Approve button → optional note → triggers add to library
- Decline button → optional note
- Delete button

Add "Requests" to admin sidebar with badge showing pending count.

---

## 9. API Client Updates (`client/src/api.ts`)

```typescript
// Requests
createRequest(data: { mediaType: string; tmdbId: number; title: string; year?: number; posterUrl?: string; overview?: string }): Promise<MediaRequest>
listRequests(params?: { status?: string; mine?: boolean }): Promise<MediaRequest[]>
getRequest(id: number): Promise<MediaRequest>

// Discover
searchTmdb(query: string, type: 'series' | 'movie'): Promise<DiscoverResult[]>
getTrending(type: 'series' | 'movie'): Promise<DiscoverResult[]>
getLibraryTmdbIds(type: 'series' | 'movie'): Promise<number[]>
```

---

## Files to Create
- `crates/stackarr-web/src/routes/requests.rs`
- `crates/stackarr-web/src/routes/discover.rs` (TMDB search for users)
- `client/src/pages/DiscoverPage.tsx`
- `client/src/pages/RequestsPage.tsx`
- `client/src/components/RequestCard.tsx`
- `client/src/components/DiscoverGrid.tsx`
- `ui/src/pages/Requests.tsx` (admin UI)

## Files to Modify
- `crates/stackarr-core/src/models/user.rs` (add MediaRequest model)
- `crates/stackarr-core/src/db.rs` (add request methods)
- `crates/stackarr-web/src/routes/mod.rs` (add requests, discover modules)
- `crates/stackarr-web/src/lib.rs` (register routes)
- `client/src/App.tsx` (add /app/discover and /app/requests routes)
- `client/src/api.ts` (add request + discover API methods)
- Import handlers (add mark_request_available hook)

## Verification
1. `cargo test --workspace --lib` passes
2. User searches TMDB on discover page → sees results with request buttons
3. User requests "Breaking Bad" → status shows "pending"
4. Admin sees request in admin UI → approves → series auto-added to library
5. Request status updates to "available"
6. User tries to request something already in library → gets "Already in library"
7. User tries to request something already requested → gets "Already requested"
8. Declined requests show declined status with admin note
