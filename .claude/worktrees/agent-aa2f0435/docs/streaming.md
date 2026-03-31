# StackArr Streaming Server — Implementation Plan

## Overview

A Plex-like streaming server built into StackArr with two components:
1. **Server** — new Rust crate (`stackarr-stream`) serving media via HTTP with direct play + transcoding
2. **Client** — lightweight web player in the existing React UI using HLS.js

MVP1 scope: no auth/users, full library access. Docker supports Intel QSV hardware transcoding.

---

## Architecture

```
┌────────────────────────────────────────────────────────────┐
│                    Web Player (ui/)                         │
│  HLS.js (transcode) │ native <video> (direct play)        │
│  Audio/subtitle track selection │ Codec auto-detection     │
└──────────────────────┬─────────────────────────────────────┘
                       │ HTTP
┌──────────────────────▼─────────────────────────────────────┐
│              Axum Routes (stackarr-web)                     │
│  /api/v1/stream/{id}/info      — media info (ffprobe)      │
│  /api/v1/stream/{id}/direct    — range-request file serve  │
│  /api/v1/stream/{id}/transcode — start transcode session   │
│  /api/v1/stream/{id}/hls/...   — HLS playlist + segments   │
│  /api/v1/stream/{id}/subtitles — WebVTT subtitle tracks    │
│  /api/v1/stream/sessions       — active session list       │
├────────────────────────────────────────────────────────────┤
│              stackarr-stream crate                          │
│  ffprobe::probe()     — extract media info                 │
│  direct::serve_file() — HTTP range request serving         │
│  ffmpeg::start()      — spawn FFmpeg transcode process     │
│  hls::playlist/segment — manage HLS output files           │
│  subtitle::extract()  — embedded subs → WebVTT             │
│  session::Manager     — track active streams, cleanup      │
├────────────────────────────────────────────────────────────┤
│  FFmpeg (external binary)                                  │
│  QSV: -init_hw_device qsv=hw -c:v h264_qsv               │
│  VAAPI: -vaapi_device /dev/dri/renderD128 -c:v h264_vaapi │
│  Software fallback: -c:v libx264 -preset veryfast          │
└────────────────────────────────────────────────────────────┘
```

---

## Phase 1: Foundation (Config, Module Gate, Database)

### 1.1 EnabledModules — add `streaming: bool`

**File:** `crates/stackarr-core/src/config.rs`
- Add `pub streaming: bool` to `EnabledModules` struct
- Add `StreamingConfig` and `HwAccelConfig` structs to `AppConfig`

```rust
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct StreamingConfig {
    pub enabled: bool,
    pub transcode_dir: Option<PathBuf>,
    pub ffmpeg_path: String,    // default: "ffmpeg"
    pub ffprobe_path: String,   // default: "ffprobe"
    pub hwaccel: HwAccelConfig,
    pub segment_duration_secs: u32, // default: 6
    pub max_concurrent_sessions: usize, // default: 3
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HwAccelConfig {
    pub enabled: bool,
    pub accel_type: String,    // "qsv", "vaapi", "nvenc", "none"
    pub device: Option<String>, // e.g. "/dev/dri/renderD128"
}
```

### 1.2 Database module persistence

**File:** `crates/stackarr-core/src/db.rs`
- Add `"streaming"` match arm in `load_enabled_modules()` and `save_enabled_modules()`

### 1.3 System status / setup routes

**File:** `crates/stackarr-web/src/routes/system.rs`
- Add `streaming: bool` to `EnabledModulesResponse`, `EnabledModulesRequest`, and `init_setup()` module entries

### 1.4 Database migration

**New file:** `migrations/002_streaming.sql`

```sql
CREATE TABLE streaming_sessions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    media_file_id BIGINT NOT NULL REFERENCES media_files(id) ON DELETE CASCADE,
    session_type TEXT NOT NULL,          -- 'direct' or 'transcode'
    status TEXT NOT NULL DEFAULT 'active',
    started_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    last_activity TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    transcode_progress REAL,
    video_codec TEXT,
    audio_codec TEXT,
    resolution TEXT,
    bitrate BIGINT,
    client_info TEXT,
    transcode_dir TEXT
);
CREATE INDEX idx_streaming_sessions_media ON streaming_sessions(media_file_id);
CREATE INDEX idx_streaming_sessions_status ON streaming_sessions(status);
```

### 1.5 Config example

**File:** `config.example.toml` — add `[streaming]` section

---

## Phase 2: Core Streaming Crate (`stackarr-stream`)

### 2.1 Crate skeleton

**New:** `crates/stackarr-stream/Cargo.toml` + `src/lib.rs`

Module structure:
```
src/
├── lib.rs       — pub mod declarations, re-exports
├── error.rs     — StreamError enum (thiserror)
├── types.rs     — MediaInfo, VideoStream, AudioStream, SubtitleStream, etc.
├── ffprobe.rs   — spawn ffprobe, parse JSON → MediaInfo
├── direct.rs    — HTTP range-request file serving
├── ffmpeg.rs    — spawn FFmpeg transcode, progress monitoring
├── hls.rs       — read/rewrite m3u8 playlists, serve .ts segments
├── subtitle.rs  — extract embedded subs → WebVTT via ffmpeg
└── session.rs   — SessionManager (DashMap), cleanup task, DB persistence
```

### 2.2 Types

```rust
pub struct MediaInfo {
    pub container: String,
    pub duration_secs: f64,
    pub bitrate: u64,
    pub video_streams: Vec<VideoStream>,
    pub audio_streams: Vec<AudioStream>,
    pub subtitle_streams: Vec<SubtitleStream>,
}

pub struct VideoStream {
    pub index: usize,
    pub codec: String,
    pub width: u32,
    pub height: u32,
    pub bitrate: u64,
    pub profile: String,
    pub level: u32,
    pub is_hdr: bool,
    pub frame_rate: f64,
}

pub struct AudioStream {
    pub index: usize,
    pub codec: String,
    pub channels: u32,
    pub language: String,
    pub title: String,
    pub bitrate: u64,
    pub is_default: bool,
}

pub struct SubtitleStream {
    pub index: usize,
    pub codec: String,
    pub language: String,
    pub title: String,
    pub forced: bool,
    pub is_default: bool,
}
```

### 2.3 FFprobe

- Spawns `ffprobe -v quiet -print_format json -show_format -show_streams <file>`
- Parses JSON → `MediaInfo`
- HDR detection via `color_transfer`, `color_primaries` fields

### 2.4 Direct Play (highest-value feature)

- Parse `Range: bytes=start-end` header (RFC 7233)
- Seek `tokio::fs::File` to offset, wrap in `ReaderStream`
- Return 206 Partial Content with `Content-Range`, `Accept-Ranges: bytes`
- MIME type from `mime_guess` based on file extension

### 2.5 FFmpeg Transcoding Engine

FFmpeg command construction:

**Intel QSV:**
```
ffmpeg -init_hw_device qsv=hw,child_device=/dev/dri/renderD128
  -filter_hw_device hw -hwaccel qsv -hwaccel_output_format qsv
  -i <source>
  -map 0:v:<idx> -map 0:a:<idx>
  -c:v h264_qsv -preset medium -global_quality 23
  -c:a aac -b:a 192k
  -f hls -hls_time 6 -hls_list_size 0
  -hls_segment_filename <dir>/%04d.ts
  <dir>/master.m3u8
```

**VAAPI:**
```
ffmpeg -vaapi_device /dev/dri/renderD128
  -i <source>
  -vf 'format=nv12|vaapi,hwupload'
  -c:v h264_vaapi -qp 23
  ...
```

**Software fallback:**
```
ffmpeg -i <source>
  -c:v libx264 -preset veryfast -crf 23
  -c:a aac -b:a 192k
  -f hls ...
```

Each transcode session gets a UUID-named temp directory under `transcode_dir`.

### 2.6 HLS Management

- `read_playlist()` — reads m3u8, rewrites segment URLs to API routes
- `read_segment()` — serves .ts file, validates name against path traversal
- `wait_for_segment()` — polls for segment file (ffmpeg writes incrementally)

### 2.7 Subtitle Extraction

- `ffmpeg -i <source> -map 0:s:<track> -f webvtt <output.vtt>`
- Handles SRT, ASS/SSA → WebVTT conversion
- PGS bitmap subs flagged for burn-in (can't convert to text)

### 2.8 Session Manager

```rust
pub struct SessionManager {
    sessions: DashMap<Uuid, Session>,
    config: StreamingConfig,
    pool: PgPool,
}
```

- `create_direct_session()` — record in DB, return ID
- `create_transcode_session()` — check max sessions, create temp dir, spawn ffmpeg, record in DB
- `stop_session()` — kill ffmpeg, clean temp dir, update DB
- `spawn_cleanup_task()` — background task every 60s, kill sessions idle >5min

---

## Phase 3: API Routes

**New file:** `crates/stackarr-web/src/routes/stream.rs`

| Method | Endpoint | Description |
|--------|----------|-------------|
| GET | `/api/v1/stream/{media_file_id}/info` | FFprobe media info (cached in `media_files.media_info`) |
| GET | `/api/v1/stream/{media_file_id}/direct` | Direct play with HTTP range requests |
| POST | `/api/v1/stream/{media_file_id}/transcode` | Start transcode session, returns session ID + HLS URL |
| GET | `/api/v1/stream/{media_file_id}/hls/{session_id}/master.m3u8` | HLS master playlist |
| GET | `/api/v1/stream/{media_file_id}/hls/{session_id}/{segment}.ts` | HLS segment |
| GET | `/api/v1/stream/{media_file_id}/subtitles/{track_index}` | Subtitle track as WebVTT |
| GET | `/api/v1/stream/sessions` | List active streaming sessions |
| DELETE | `/api/v1/stream/sessions/{session_id}` | Stop and clean up session |

### Media file path resolution

Media files need a join to resolve full filesystem path:
```sql
SELECT mlf.path, mf.relative_path, mf.media_type
FROM media_files mf
LEFT JOIN movies m ON m.movie_file_id = mf.id AND mf.media_type = 'movie'
LEFT JOIN (
    SELECT ef.media_file_id, s.media_library_folder_id
    FROM episode_files ef
    JOIN episodes e ON ef.episode_id = e.id
    JOIN series s ON e.series_id = s.id
) es ON es.media_file_id = mf.id AND mf.media_type = 'series'
JOIN media_library_folders mlf
    ON mlf.id = COALESCE(m.media_library_folder_id, es.media_library_folder_id)
WHERE mf.id = $1
```

Full path = `mlf.path + "/" + mf.relative_path`

---

## Phase 4: Web Client (UI)

### 4.1 Dependencies

Add `hls.js` to `ui/package.json`

### 4.2 API Types

Add to `ui/src/api/types.ts`:
- `MediaStreamInfo` — ffprobe output for UI
- `VideoStreamInfo`, `AudioStreamInfo`, `SubtitleStreamInfo`
- `StreamSession`, `TranscodeRequest`, `TranscodeResponse`

### 4.3 Hooks

Add to `ui/src/hooks/useApi.ts`:
- `useStreamInfo(mediaFileId)` — fetch media stream info
- `useStreamSessions()` — list active sessions (5s refetch)
- `useStartTranscode()` — mutation to start transcode

### 4.4 VideoPlayer Component

**New:** `ui/src/components/VideoPlayer.tsx`

- Codec detection via `MediaSource.isTypeSupported()` / `canPlayType()`
- Direct play: `<video src="/api/v1/stream/{id}/direct">`
- Transcode: HLS.js with returned playlist URL
- Audio/subtitle track selectors
- Cleanup: `DELETE /stream/sessions/{id}` on unmount

### 4.5 Player Page

**New:** `ui/src/pages/Player.tsx`
- Route: `/play/:mediaFileId`
- Shows media title, VideoPlayer, stream info

### 4.6 Play Buttons

- `SeriesDetail.tsx` — Play icon on episodes with files
- `MovieDetail.tsx` — Play button when movie has file

### 4.7 Navigation

- `App.tsx` — add `/play/:mediaFileId` route
- `Sidebar.tsx` — add "Streaming" nav item gated by `modules.streaming`
- `types.ts` — add `streaming: boolean` to `EnabledModules`

---

## Phase 5: Docker & QSV Support

### 5.1 Dockerfile

Add to runtime stage:
```dockerfile
RUN apt-get update && apt-get install -y --no-install-recommends \
    ffmpeg \
    intel-media-va-driver-non-free \
    vainfo \
    && rm -rf /var/lib/apt/lists/*
```

### 5.2 Docker Compose

Add to both `docker-compose.yml` and `docker-compose.prod.yml`:
```yaml
devices:
  - /dev/dri:/dev/dri
volumes:
  - stackarr-transcode:/config/transcode
```

---

## Phase 6: FirstBoot Integration

- `FirstBoot.tsx` — add streaming toggle in feature selection (Step 0)
- `types.ts` — add `streaming?: boolean` to `SetupInit.modules`

---

## Implementation Order

| Step | What | Why |
|------|------|-----|
| 1 | Config + module gate (Phase 1.1-1.3) | Everything depends on this |
| 2 | Migration (Phase 1.4) | Session tracking table |
| 3 | Crate skeleton + types + errors (Phase 2.1-2.2) | Foundation for all streaming logic |
| 4 | FFprobe (Phase 2.3) | Needed by info endpoint and codec detection |
| 5 | Direct play (Phase 2.4) | Highest-value feature, simplest path |
| 6 | API routes: info + direct (Phase 3 partial) | Wire up first playable features |
| 7 | AppState + main.rs init (Phase 3.3-3.4) | Connect crate to server |
| 8 | UI types, hooks, deps (Phase 4.1-4.3) | Foundation for player |
| 9 | Player component + page (Phase 4.4-4.7) | First playable UI |
| 10 | FFmpeg transcoding (Phase 2.5-2.7) | Transcode support |
| 11 | Session manager (Phase 2.8) | Lifecycle management |
| 12 | Docker + QSV (Phase 5) | Production readiness |
| 13 | FirstBoot (Phase 6) | Setup integration |
| 14 | Config example (Phase 1.5) | Documentation |

---

## Key Design Decisions

1. **SessionManager as `Option<Arc<SessionManager>>` in AppState** — follows the pattern of `torrent_session`, `usenet_queue`, etc. Route handlers return 503 when None.

2. **Direct play = raw file + range requests; transcode = HLS** — pragmatic split. Modern browsers handle H264/AAC natively. Anything else gets transcoded to HLS.

3. **Per-session temp directories** — each transcode gets `{transcode_dir}/{uuid}/`. Avoids segment naming collisions. Cleanup task removes expired sessions.

4. **FFmpeg as external process** — spawned via `tokio::process::Command`. No Rust FFmpeg bindings needed. Progress parsed from stderr.

5. **Media info cached in `media_files.media_info` JSONB** — already exists in the schema. First probe writes it, subsequent requests read from DB.

6. **No auth for MVP1** — streaming routes sit under the existing `protected_routes` which already handles API key validation.
