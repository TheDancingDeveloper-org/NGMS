# Phase 2: Watch Progress + Continue Watching

## Goal

Track per-user watch progress for all media files. Display a "Continue Watching" row on the client home page. Report progress from the video player automatically.

**Prerequisite:** Phase 1 (user accounts + auth) must be complete.

---

## 1. Database Schema

The `watch_progress` table should already exist from migration `006_users.sql`. If not included there, add as `007_watch_progress.sql`:

```sql
CREATE TABLE watch_progress (
    id BIGSERIAL PRIMARY KEY,
    user_id BIGINT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    media_file_id BIGINT NOT NULL REFERENCES media_files(id) ON DELETE CASCADE,
    media_type TEXT NOT NULL,              -- 'series' | 'movie'
    media_id BIGINT NOT NULL,             -- series.id or movies.id
    episode_id BIGINT,                    -- episodes.id (NULL for movies)
    position_secs REAL NOT NULL DEFAULT 0,
    duration_secs REAL NOT NULL DEFAULT 0,
    completed BOOLEAN NOT NULL DEFAULT false,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(user_id, media_file_id)
);
CREATE INDEX idx_watch_progress_user ON watch_progress(user_id, updated_at DESC);
CREATE INDEX idx_watch_progress_continue ON watch_progress(user_id, completed, updated_at DESC);
CREATE INDEX idx_watch_progress_media ON watch_progress(media_type, media_id);
```

Also add `user_id` to streaming sessions if not done in Phase 1:
```sql
ALTER TABLE streaming_sessions ADD COLUMN IF NOT EXISTS user_id BIGINT REFERENCES users(id);
```

---

## 2. Rust Model

### Add to `crates/stackarr-core/src/models/user.rs` (or new `progress.rs`)

```rust
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct WatchProgress {
    pub id: i64,
    pub user_id: i64,
    pub media_file_id: i64,
    pub media_type: String,
    pub media_id: i64,
    pub episode_id: Option<i64>,
    pub position_secs: f32,
    pub duration_secs: f32,
    pub completed: bool,
    pub updated_at: DateTime<Utc>,
}

/// Enriched progress entry for Continue Watching display
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContinueWatchingItem {
    pub progress: WatchProgress,
    pub title: String,           // series title or movie title
    pub subtitle: Option<String>, // e.g. "S02E05 - Episode Title"
    pub poster_url: Option<String>,
    pub backdrop_url: Option<String>,
    pub percent_complete: f32,
}
```

---

## 3. Database Methods (`crates/stackarr-core/src/db.rs`)

```rust
/// Upsert watch progress (INSERT ... ON CONFLICT UPDATE)
pub async fn upsert_watch_progress(
    &self,
    user_id: i64,
    media_file_id: i64,
    media_type: &str,
    media_id: i64,
    episode_id: Option<i64>,
    position_secs: f32,
    duration_secs: f32,
    completed: bool,
) -> crate::Result<()>
// INSERT INTO watch_progress (...) VALUES (...)
// ON CONFLICT (user_id, media_file_id) DO UPDATE SET
//   position_secs = EXCLUDED.position_secs,
//   duration_secs = EXCLUDED.duration_secs,
//   completed = EXCLUDED.completed,
//   updated_at = NOW()

/// Get progress for a specific file
pub async fn get_watch_progress(
    &self,
    user_id: i64,
    media_file_id: i64,
) -> crate::Result<Option<WatchProgress>>

/// Get Continue Watching list (incomplete, sorted by most recent)
pub async fn get_continue_watching(
    &self,
    user_id: i64,
    limit: i64,  // default 20
) -> crate::Result<Vec<WatchProgress>>
// SELECT * FROM watch_progress
// WHERE user_id = $1 AND completed = false AND position_secs > 0
// ORDER BY updated_at DESC LIMIT $2

/// Get all progress for a series (to show watched indicators on episodes)
pub async fn get_series_progress(
    &self,
    user_id: i64,
    series_id: i64,
) -> crate::Result<Vec<WatchProgress>>
// SELECT * FROM watch_progress WHERE user_id = $1 AND media_type = 'series' AND media_id = $2

/// Get all progress for a movie
pub async fn get_movie_progress(
    &self,
    user_id: i64,
    movie_id: i64,
) -> crate::Result<Option<WatchProgress>>

/// Clear progress for a file
pub async fn delete_watch_progress(
    &self,
    user_id: i64,
    media_file_id: i64,
) -> crate::Result<bool>

/// Mark all episodes of a series as completed up to a point
pub async fn mark_series_watched(
    &self,
    user_id: i64,
    series_id: i64,
) -> crate::Result<u64>
// UPDATE watch_progress SET completed = true WHERE user_id = $1 AND media_type = 'series' AND media_id = $2
```

---

## 4. API Routes

### New file: `crates/stackarr-web/src/routes/progress.rs`

```rust
pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/v1/user/progress/continue", get(continue_watching))
        .route("/api/v1/user/progress/:mediaFileId", get(get_progress).put(update_progress).delete(clear_progress))
        .route("/api/v1/user/progress/series/:seriesId", get(series_progress))
        .route("/api/v1/user/progress/movie/:movieId", get(movie_progress))
}
```

All routes use `RequireUser` extractor.

**PUT /api/v1/user/progress/:mediaFileId**
- Body: `{ positionSecs: f32, durationSecs: f32, completed?: bool }`
- Needs to resolve `media_file_id` → `media_type`, `media_id`, `episode_id` from DB
- Auto-complete if `positionSecs / durationSecs > 0.9`
- Upsert into `watch_progress`

**GET /api/v1/user/progress/continue**
- Query params: `?limit=20`
- Returns enriched `ContinueWatchingItem[]` with title, poster, percent
- JOIN with series/movies/episodes for metadata
- The enrichment query:
```sql
SELECT wp.*,
    COALESCE(s.title, m.title) as title,
    CASE WHEN wp.media_type = 'series' THEN
        'S' || LPAD(e.season_number::text, 2, '0') || 'E' || LPAD(e.episode_number::text, 2, '0') || ' - ' || e.title
    END as subtitle,
    COALESCE(s.images, m.images) as images
FROM watch_progress wp
LEFT JOIN series s ON wp.media_type = 'series' AND wp.media_id = s.id
LEFT JOIN movies m ON wp.media_type = 'movie' AND wp.media_id = m.id
LEFT JOIN episodes e ON wp.episode_id = e.id
WHERE wp.user_id = $1 AND wp.completed = false AND wp.position_secs > 0
ORDER BY wp.updated_at DESC
LIMIT $2
```

**GET /api/v1/user/progress/series/:seriesId**
- Returns all progress entries for that series (for episode watched indicators)

**GET /api/v1/user/progress/:mediaFileId**
- Returns single progress entry (for player resume)

---

## 5. Streaming Integration

### Update `crates/stackarr-web/src/routes/stream.rs`

When a stream starts (direct play or transcode), the `user_id` from `RequireUser` should be stored in the streaming session so progress can be attributed.

Update stream routes to use `RequireUser` instead of `RequireAuth`:
```rust
async fn direct_play(
    State(state): State<Arc<AppState>>,
    RequireUser(user): RequireUser,  // changed from RequireAuth
    Path(file_id): Path<i64>,
) -> impl IntoResponse {
    // ... existing logic
    // Store user.user_id in streaming session if applicable
}
```

---

## 6. Client App Changes

### New component: `client/src/components/ProgressReporter.tsx`

```typescript
interface Props {
  mediaFileId: number
  videoRef: React.RefObject<HTMLVideoElement>
}

function ProgressReporter({ mediaFileId, videoRef }: Props) {
  const reportMutation = useMutation({
    mutationFn: (data: { positionSecs: number; durationSecs: number; completed: boolean }) =>
      api.updateProgress(mediaFileId, data),
  })

  useEffect(() => {
    const video = videoRef.current
    if (!video) return

    let lastReported = 0
    const INTERVAL = 10 // seconds

    const onTimeUpdate = () => {
      const now = video.currentTime
      if (Math.abs(now - lastReported) >= INTERVAL) {
        lastReported = now
        reportMutation.mutate({
          positionSecs: now,
          durationSecs: video.duration,
          completed: now / video.duration > 0.9,
        })
      }
    }

    const onPause = () => {
      reportMutation.mutate({
        positionSecs: video.currentTime,
        durationSecs: video.duration,
        completed: video.currentTime / video.duration > 0.9,
      })
    }

    video.addEventListener('timeupdate', onTimeUpdate)
    video.addEventListener('pause', onPause)
    video.addEventListener('ended', onPause)
    return () => {
      // Report final position on unmount
      if (video.currentTime > 0) {
        api.updateProgress(mediaFileId, {
          positionSecs: video.currentTime,
          durationSecs: video.duration,
          completed: video.currentTime / video.duration > 0.9,
        })
      }
      video.removeEventListener('timeupdate', onTimeUpdate)
      video.removeEventListener('pause', onPause)
      video.removeEventListener('ended', onPause)
    }
  }, [mediaFileId])

  return null // invisible component
}
```

### Update `client/src/pages/Player.tsx`
- On mount, check `GET /api/v1/user/progress/:mediaFileId` for resume position
- If position exists, prompt "Resume from X:XX?" or auto-seek
- Render `<ProgressReporter>` alongside video element

### New page: `client/src/pages/HomePage.tsx`
```typescript
function HomePage() {
  const { data: continueWatching } = useQuery({
    queryKey: ['continue-watching'],
    queryFn: () => api.getContinueWatching(20),
  })
  const { data: recentSeries } = useQuery({
    queryKey: ['recent-series'],
    queryFn: () => api.listSeries({ sort: 'added', limit: 20 }),
  })
  const { data: recentMovies } = useQuery({
    queryKey: ['recent-movies'],
    queryFn: () => api.listMovies({ sort: 'added', limit: 20 }),
  })

  return (
    <div>
      {continueWatching?.length > 0 && (
        <MediaRow title="Continue Watching" items={continueWatching} />
      )}
      <MediaRow title="Recently Added Series" items={recentSeries} />
      <MediaRow title="Recently Added Movies" items={recentMovies} />
    </div>
  )
}
```

### New component: `client/src/components/MediaRow.tsx`
- Horizontal scrollable row of poster cards
- Each card shows: poster image, title, subtitle, progress bar (if applicable)
- Click navigates to detail page or directly to player (for continue watching)

### Update `client/src/api.ts`
Add methods:
```typescript
updateProgress(mediaFileId: number, data: { positionSecs: number; durationSecs: number; completed: boolean }): Promise<void>
getProgress(mediaFileId: number): Promise<WatchProgress | null>
getContinueWatching(limit?: number): Promise<ContinueWatchingItem[]>
getSeriesProgress(seriesId: number): Promise<WatchProgress[]>
```

### Update series/movie detail pages
- Show watched indicator (checkmark) on completed episodes
- Show progress bar on in-progress episodes
- Fetch progress via `getSeriesProgress(id)` or `getMovieProgress(id)`

---

## Files to Create
- `crates/stackarr-web/src/routes/progress.rs`
- `client/src/pages/HomePage.tsx`
- `client/src/components/ProgressReporter.tsx`
- `client/src/components/MediaRow.tsx`

## Files to Modify
- `crates/stackarr-core/src/models/user.rs` (add WatchProgress, ContinueWatchingItem)
- `crates/stackarr-core/src/db.rs` (add progress methods)
- `crates/stackarr-web/src/routes/mod.rs` (add progress module)
- `crates/stackarr-web/src/lib.rs` (register progress routes)
- `crates/stackarr-web/src/routes/stream.rs` (use RequireUser, pass user_id)
- `client/src/App.tsx` (add HomePage route as default, /app/home)
- `client/src/pages/Player.tsx` (add ProgressReporter, resume logic)
- `client/src/pages/SeriesView.tsx` (watched indicators)
- `client/src/pages/MovieView.tsx` (progress indicator)
- `client/src/api.ts` (add progress API methods)

## Verification
1. `cargo test --workspace --lib` passes
2. Play a video → pause at 5 min → check DB has progress row
3. Navigate to /app/home → "Continue Watching" shows the paused video
4. Click continue → player resumes at 5 min mark
5. Watch to 90%+ → marked as completed → disappears from Continue Watching
6. View series detail → completed episodes show checkmark
7. Different users see only their own progress
