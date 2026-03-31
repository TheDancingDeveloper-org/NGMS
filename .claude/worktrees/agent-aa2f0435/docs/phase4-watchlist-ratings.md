# Phase 4: Watchlist + Ratings

## Goal

Per-user watchlist (bookmark media to watch later) and ratings (1-10 score). These are personal to each user and visible on media detail pages and a dedicated watchlist page.

**Prerequisite:** Phase 1 (user accounts) must be complete.

---

## 1. Database Schema

From migration `006_users.sql` (or add as separate migration if needed):

```sql
CREATE TABLE user_watchlist (
    id BIGSERIAL PRIMARY KEY,
    user_id BIGINT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    media_type TEXT NOT NULL,              -- 'series' | 'movie'
    media_id BIGINT NOT NULL,             -- series.id or movies.id (local library ID)
    tmdb_id BIGINT NOT NULL,
    added_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(user_id, media_type, media_id)
);
CREATE INDEX idx_user_watchlist_user ON user_watchlist(user_id, added_at DESC);

CREATE TABLE user_ratings (
    id BIGSERIAL PRIMARY KEY,
    user_id BIGINT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    media_type TEXT NOT NULL,
    media_id BIGINT NOT NULL,
    rating SMALLINT NOT NULL CHECK (rating >= 1 AND rating <= 10),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(user_id, media_type, media_id)
);
CREATE INDEX idx_user_ratings_user ON user_ratings(user_id);
CREATE INDEX idx_user_ratings_media ON user_ratings(media_type, media_id);
```

---

## 2. Rust Models

### Add to `crates/stackarr-core/src/models/user.rs`

```rust
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct UserWatchlistItem {
    pub id: i64,
    pub user_id: i64,
    pub media_type: String,
    pub media_id: i64,
    pub tmdb_id: i64,
    pub added_at: DateTime<Utc>,
}

/// Enriched watchlist item with media metadata for display
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WatchlistItemWithMedia {
    pub id: i64,
    pub media_type: String,
    pub media_id: i64,
    pub tmdb_id: i64,
    pub title: String,
    pub year: Option<i32>,
    pub poster_url: Option<String>,
    pub overview: Option<String>,
    pub added_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct UserRating {
    pub id: i64,
    pub user_id: i64,
    pub media_type: String,
    pub media_id: i64,
    pub rating: i16,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
```

---

## 3. Database Methods (`crates/stackarr-core/src/db.rs`)

### Watchlist
```rust
pub async fn add_to_watchlist(
    &self,
    user_id: i64,
    media_type: &str,
    media_id: i64,
    tmdb_id: i64,
) -> crate::Result<UserWatchlistItem>
// INSERT ... ON CONFLICT DO NOTHING RETURNING *
// If conflict, SELECT existing

pub async fn remove_from_watchlist(
    &self,
    user_id: i64,
    media_type: &str,
    media_id: i64,
) -> crate::Result<bool>

pub async fn get_watchlist(
    &self,
    user_id: i64,
    media_type: Option<&str>,  // filter by type, or all
) -> crate::Result<Vec<UserWatchlistItem>>
// ORDER BY added_at DESC

pub async fn is_on_watchlist(
    &self,
    user_id: i64,
    media_type: &str,
    media_id: i64,
) -> crate::Result<bool>
```

### Ratings
```rust
pub async fn set_rating(
    &self,
    user_id: i64,
    media_type: &str,
    media_id: i64,
    rating: i16,
) -> crate::Result<UserRating>
// INSERT ... ON CONFLICT (user_id, media_type, media_id)
// DO UPDATE SET rating = EXCLUDED.rating, updated_at = NOW()
// RETURNING *

pub async fn get_rating(
    &self,
    user_id: i64,
    media_type: &str,
    media_id: i64,
) -> crate::Result<Option<UserRating>>

pub async fn delete_rating(
    &self,
    user_id: i64,
    media_type: &str,
    media_id: i64,
) -> crate::Result<bool>

pub async fn get_user_ratings(
    &self,
    user_id: i64,
    media_type: Option<&str>,
) -> crate::Result<Vec<UserRating>>
// ORDER BY updated_at DESC

pub async fn get_average_rating(
    &self,
    media_type: &str,
    media_id: i64,
) -> crate::Result<Option<f32>>
// SELECT AVG(rating)::REAL FROM user_ratings WHERE media_type = $1 AND media_id = $2
```

---

## 4. API Routes

### Extend `crates/stackarr-web/src/routes/user.rs` (or new files)

```rust
// Add to user.rs router or create routes/watchlist.rs:
.route("/api/v1/user/watchlist", get(list_watchlist))
.route("/api/v1/user/watchlist/:mediaType/:mediaId", put(add_to_watchlist).delete(remove_from_watchlist))

.route("/api/v1/user/ratings", get(list_ratings))
.route("/api/v1/user/ratings/:mediaType/:mediaId", put(set_rating).get(get_rating).delete(delete_rating))
```

All routes use `RequireUser`.

**GET /api/v1/user/watchlist**
- Query: `?type=series` (optional filter)
- Returns enriched items with media metadata (JOIN series/movies)
- The enrichment query:
```sql
SELECT w.id, w.media_type, w.media_id, w.tmdb_id, w.added_at,
    COALESCE(s.title, m.title) as title,
    COALESCE(s.year, m.year) as year,
    COALESCE(s.images, m.images) as images,
    COALESCE(s.overview, m.overview) as overview
FROM user_watchlist w
LEFT JOIN series s ON w.media_type = 'series' AND w.media_id = s.id
LEFT JOIN movies m ON w.media_type = 'movie' AND w.media_id = m.id
WHERE w.user_id = $1
ORDER BY w.added_at DESC
```

**PUT /api/v1/user/watchlist/:mediaType/:mediaId**
- No body needed (the path has all info)
- Look up tmdb_id from series/movies table
- Return the created watchlist item

**DELETE /api/v1/user/watchlist/:mediaType/:mediaId**
- Returns 204

**PUT /api/v1/user/ratings/:mediaType/:mediaId**
- Body: `{ rating: number }` (1-10)
- Validates range
- Returns the rating

**GET /api/v1/user/ratings/:mediaType/:mediaId**
- Returns user's rating + average rating from all users

---

## 5. Client App - Watchlist Page

### New page: `client/src/pages/WatchlistPage.tsx`

```typescript
function WatchlistPage() {
  const [filter, setFilter] = useState<'all' | 'series' | 'movie'>('all')
  const { data: items } = useQuery({
    queryKey: ['watchlist', filter],
    queryFn: () => api.getWatchlist(filter === 'all' ? undefined : filter),
  })

  return (
    <div>
      <h1>My Watchlist</h1>
      <FilterTabs value={filter} onChange={setFilter} />
      <MediaGrid items={items} onRemove={(item) => removeMutation.mutate(item)} />
    </div>
  )
}
```

---

## 6. Client App - Watchlist/Rating on Detail Pages

### Update `client/src/pages/SeriesView.tsx` and `MovieView.tsx`

Add two interactive elements to detail pages:

**Watchlist toggle button:**
```typescript
function WatchlistButton({ mediaType, mediaId }: Props) {
  const { data: isOnWatchlist } = useQuery({
    queryKey: ['watchlist-check', mediaType, mediaId],
    queryFn: () => api.isOnWatchlist(mediaType, mediaId),
  })

  const toggleMutation = useMutation({
    mutationFn: () => isOnWatchlist
      ? api.removeFromWatchlist(mediaType, mediaId)
      : api.addToWatchlist(mediaType, mediaId),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ['watchlist-check', mediaType, mediaId] }),
  })

  return (
    <button onClick={() => toggleMutation.mutate()}>
      {isOnWatchlist ? <BookmarkFilledIcon /> : <BookmarkIcon />}
      {isOnWatchlist ? 'On Watchlist' : 'Add to Watchlist'}
    </button>
  )
}
```

**Star rating component:**
```typescript
function RatingStars({ mediaType, mediaId }: Props) {
  const { data } = useQuery({
    queryKey: ['rating', mediaType, mediaId],
    queryFn: () => api.getRating(mediaType, mediaId),
  })

  const setMutation = useMutation({
    mutationFn: (rating: number) => api.setRating(mediaType, mediaId, rating),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ['rating', mediaType, mediaId] }),
  })

  // Render 10 stars (or 5 with halves), highlight up to user's rating
  // Show average rating from other users below
  // Click to set rating, click current rating to remove
}
```

Use `lucide-react` icons (already in dependencies): `Bookmark`, `Star`.

---

## 7. API Client Updates (`client/src/api.ts`)

```typescript
// Watchlist
getWatchlist(type?: string): Promise<WatchlistItemWithMedia[]>
addToWatchlist(mediaType: string, mediaId: number): Promise<void>
removeFromWatchlist(mediaType: string, mediaId: number): Promise<void>
isOnWatchlist(mediaType: string, mediaId: number): Promise<boolean>

// Ratings
getRating(mediaType: string, mediaId: number): Promise<{ userRating: number | null, averageRating: number | null }>
setRating(mediaType: string, mediaId: number, rating: number): Promise<void>
deleteRating(mediaType: string, mediaId: number): Promise<void>
getUserRatings(type?: string): Promise<UserRating[]>
```

---

## Files to Create
- `client/src/pages/WatchlistPage.tsx`
- `client/src/components/WatchlistButton.tsx`
- `client/src/components/RatingStars.tsx`

## Files to Modify
- `crates/stackarr-core/src/models/user.rs` (add watchlist + rating models)
- `crates/stackarr-core/src/db.rs` (add watchlist + rating methods)
- `crates/stackarr-web/src/routes/user.rs` (add watchlist + rating routes, or new module)
- `crates/stackarr-web/src/routes/mod.rs` (if new module)
- `crates/stackarr-web/src/lib.rs` (register routes if new module)
- `client/src/App.tsx` (add /app/watchlist route)
- `client/src/pages/SeriesView.tsx` (add WatchlistButton + RatingStars)
- `client/src/pages/MovieView.tsx` (add WatchlistButton + RatingStars)
- `client/src/api.ts` (add watchlist + rating API methods)

## Verification
1. `cargo test --workspace --lib` passes
2. View series detail → click bookmark → icon fills, shows "On Watchlist"
3. Navigate to /app/watchlist → series appears
4. Click bookmark again → removed from watchlist
5. Rate a movie 8/10 → stars fill up to 8
6. Different user rates same movie 6/10 → average shows 7.0
7. Click current rating → removed, average recalculates
8. Watchlist page shows correct posters and metadata
