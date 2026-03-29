# NGMS Streaming Server

## Overview

A Plex-like streaming server built into NGMS with two components:
1. **Server** — `ngms-stream` crate serving media via HTTP with direct play + HLS transcoding
2. **Client** — web player in the React UI using HLS.js, plus a standalone Tauri client app (`client/`)

Docker ships with jellyfin-ffmpeg7 for full QSV/VAAPI hardware transcoding support.

---

## Architecture

```
┌────────────────────────────────────────────────────────────┐
│            Web Player (ui/) / Client App (client/)          │
│  HLS.js (transcode) │ native <video> (direct play)        │
│  Audio/subtitle track selection │ Codec auto-detection     │
│  Bandwidth test → adaptive quality tier selection          │
└──────────────────────┬─────────────────────────────────────┘
                       │ HTTP
┌──────────────────────▼─────────────────────────────────────┐
│              Axum Routes (ngms-web)                     │
│  /api/v1/stream/{id}/info        — media info (ffprobe)    │
│  /api/v1/stream/{id}/direct      — range-request file serve│
│  /api/v1/stream/{id}/transcode   — start transcode session │
│  /api/v1/stream/{id}/hls/...     — HLS playlist + segments │
│  /api/v1/stream/{id}/subtitles   — WebVTT subtitle tracks  │
│  /api/v1/stream/sessions         — active session list     │
│  /api/v1/stream/bandwidth-test   — zero-fill payload       │
├────────────────────────────────────────────────────────────┤
│              ngms-stream crate                          │
│  provision::ensure_ffmpeg()  — find or download ffmpeg     │
│  ffprobe::probe()            — extract media info          │
│  direct::serve_file()        — HTTP range request serving  │
│  ffmpeg::start_transcode()   — single-rendition HLS       │
│  ffmpeg::start_multi_rendition_transcode() — ABR streaming │
│  hls::read_playlist/segment  — manage HLS output files    │
│  subtitle::extract()         — embedded subs → WebVTT     │
│  session::SessionManager     — track streams, cleanup     │
│  session::probe_hwaccel()    — detect GPU capabilities    │
├────────────────────────────────────────────────────────────┤
│  FFmpeg (external binary — jellyfin-ffmpeg7 preferred)     │
│  QSV: -init_hw_device qsv=hw -c:v h264_qsv               │
│  VAAPI: -vaapi_device /dev/dri/renderD128 -c:v h264_vaapi │
│  NVENC: -hwaccel cuda -c:v h264_nvenc                     │
│  Software fallback: -c:v libx264 -preset veryfast          │
└────────────────────────────────────────────────────────────┘
```

---

## Phase 1: Foundation (Config, Module Gate, Database)

### 1.1 Config structs

**File:** `crates/ngms-core/src/config.rs`

```rust
pub struct StreamingConfig {
    pub enabled: bool,
    pub transcode_dir: Option<PathBuf>,
    pub ffmpeg_path: String,             // auto-detects jellyfin-ffmpeg
    pub ffprobe_path: String,            // auto-detects jellyfin-ffmpeg
    pub hwaccel: HwAccelConfig,
    pub segment_duration_secs: u32,      // default: 6
    pub max_concurrent_sessions: usize,  // default: 3
    pub quality_tiers: Vec<QualityTierConfig>,  // ABR tiers (see below)
}

pub struct HwAccelConfig {
    pub enabled: bool,
    pub accel_type: String,    // "vaapi" (default), "qsv", "nvenc"
    pub device: Option<String>, // e.g. "/dev/dri/renderD128"
}

pub struct QualityTierConfig {
    pub name: String,           // e.g. "1080p"
    pub max_width: u32,
    pub max_height: u32,
    pub video_bitrate: u64,     // bits per second
    pub audio_bitrate: u64,     // bits per second
}
```

Default quality tiers: 4K (40 Mbps), 4K Low (20 Mbps), 1080p (8 Mbps), 1080p Low (4 Mbps), 720p (2.5 Mbps), 480p (1.5 Mbps).

### 1.2 Database module persistence

**File:** `crates/ngms-core/src/db.rs`
- Add `"streaming"` match arm in `load_enabled_modules()` and `save_enabled_modules()`

### 1.3 System status / setup routes

**File:** `crates/ngms-web/src/routes/system.rs`
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

## Phase 2: Core Streaming Crate (`ngms-stream`)

### 2.1 Crate structure

**Crate:** `crates/ngms-stream/`

Module structure:
```
src/
├── lib.rs        — pub mod declarations, re-exports
├── error.rs      — StreamError enum (thiserror)
├── types.rs      — MediaInfo, VideoStream, AudioStream, SubtitleStream, etc.
├── provision.rs  — ensure_ffmpeg(), FfmpegPaths (find or download ffmpeg)
├── ffprobe.rs    — spawn ffprobe, parse JSON → MediaInfo
├── direct.rs     — HTTP range-request file serving
├── ffmpeg.rs     — single + multi-rendition transcode, hwaccel flags
├── hls.rs        — read/rewrite m3u8 playlists, serve .ts segments
├── subtitle.rs   — extract embedded subs → WebVTT via ffmpeg
└── session.rs    — SessionManager, probe_hwaccel(), DetectedAccel, cleanup
```

Public re-exports from `lib.rs`:
```rust
pub use error::{StreamError, StreamResult};
pub use provision::{ensure_ffmpeg, FfmpegPaths};
pub use session::{probe_hwaccel, DetectedAccel, SessionManager};
pub use types::*;
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

### 2.3 FFmpeg Provisioning (`provision.rs`)

`ensure_ffmpeg()` finds or downloads ffmpeg/ffprobe at startup:

1. Check configured paths (`streaming.ffmpeg_path`, `streaming.ffprobe_path`)
2. Check well-known paths: `/usr/lib/jellyfin-ffmpeg/ffmpeg`, `/usr/bin/ffmpeg`
3. Check `{data_dir}/ffmpeg/` for previous download
4. Download platform-specific jellyfin-ffmpeg portable build to `{data_dir}/ffmpeg/`

Returns `FfmpegPaths { ffmpeg: String, ffprobe: String }` with resolved absolute paths.

Supported download platforms: linux-x86_64, linux-aarch64, windows-x86_64. Other platforms must install ffmpeg manually.

### 2.4 Hardware Acceleration Detection (`session.rs`)

`probe_hwaccel()` runs a minimal ffmpeg test encode at startup to detect GPU capabilities:

```rust
pub enum DetectedAccel {
    Hardware { accel_type: String, device: String },
    Software,
}
```

Probing strategy:
- If `hwaccel.enabled = true`, try the configured `accel_type` first, then fallback chain
- VAAPI and QSV are probed as fallback candidates for each other
- NVENC is only tried if explicitly configured
- If `hwaccel.enabled = false`, still probes to log available capabilities but returns `Software`

Each probe runs `ffmpeg` with a 1-frame test encode using the target encoder (h264_vaapi, h264_qsv, h264_nvenc) with a 10-second timeout.

### 2.5 FFprobe

- Spawns `ffprobe -v quiet -print_format json -show_format -show_streams <file>`
- Parses JSON → `MediaInfo`
- HDR detection via `color_transfer` field (smpte2084, arib-std-b67, bt2020-10, bt2020-12)
- Filters out attached pictures (album art) from video streams

### 2.6 Direct Play (highest-value feature)

- Parse `Range: bytes=start-end` header (RFC 7233)
- Seek `tokio::fs::File` to offset, wrap in `ReaderStream`
- Return 206 Partial Content with `Content-Range`, `Accept-Ranges: bytes`
- MIME type from `mime_guess` based on file extension

### 2.7 FFmpeg Transcoding Engine (`ffmpeg.rs`)

Two modes:

**Single-rendition** (`start_transcode()`) — one ffmpeg process, one HLS output:
- Hardware accel attempted first; if ffmpeg dies within 2 seconds, automatically falls back to software
- VAAPI pipeline includes `tonemap_vaapi` for HDR-to-SDR tone mapping (passthrough for SDR)
- Audio always transcoded to AAC stereo for browser compatibility
- Optional subtitle burn-in via `-vf subtitles=...` (software) or in the hw filter chain

**Multi-rendition ABR** (`start_multi_rendition_transcode()`) — one ffmpeg process per quality tier:
- Each tier gets its own subdirectory `{session_dir}/v{n}/`
- Aligned keyframes across all renditions (`-g 48 -keyint_min 48 -force_key_frames expr:gte(t,n_forced*2)`)
- Master playlist generated with `#EXT-X-STREAM-INF` entries for each tier
- Process limit: `max_concurrent_sessions * 4` total ffmpeg processes across all sessions

FFmpeg command construction:

**VAAPI:**
```
ffmpeg -vaapi_device /dev/dri/renderD128
  -hwaccel vaapi -hwaccel_output_format vaapi
  -i <source>
  -map 0:v:<idx> -map 0:a:<idx>
  -c:v h264_vaapi -b:v 8000000
  -vf 'tonemap_vaapi=format=nv12:t=bt709:m=bt709:p=bt709,scale_vaapi=...'
  -c:a aac -b:a 192k -ac 2
  -f hls -hls_time 6 -hls_list_size 0
  -hls_segment_filename <dir>/%04d.ts
  <dir>/master.m3u8
```

**Intel QSV:**
```
ffmpeg -init_hw_device qsv=hw,child_device=/dev/dri/renderD128
  -filter_hw_device hw -hwaccel qsv -hwaccel_output_format qsv
  -i <source>
  -c:v h264_qsv -preset medium -b:v 8000000
  -c:a aac -b:a 192k -ac 2
  -f hls ...
```

**NVENC:**
```
ffmpeg -hwaccel cuda -hwaccel_output_format cuda
  -i <source>
  -c:v h264_nvenc -preset p4 -b:v 8000000
  -c:a aac -b:a 192k -ac 2
  -f hls ...
```

**Software fallback:**
```
ffmpeg -i <source>
  -c:v libx264 -preset veryfast -crf 23 -level 4.1 -pix_fmt yuv420p
  -c:a aac -b:a 192k -ac 2
  -f hls ...
```

Each transcode session gets a UUID-named temp directory under `transcode_dir`.

### 2.8 HLS Management (`hls.rs`)

- `read_playlist()` — reads master.m3u8, rewrites segment/sub-playlist URLs to API routes
- `read_sub_playlist()` — reads rendition sub-playlist (`v{n}/stream.m3u8`), rewrites segment URLs
- `read_segment()` — serves .ts file, validates name against path traversal (rejects `..`, `/`, `\`, non-.ts)
- `wait_for_segment()` — polls for segment file with configurable timeout (ffmpeg writes incrementally)

### 2.9 Subtitle Extraction

- `ffmpeg -i <source> -map 0:s:<track> -f webvtt <output.vtt>`
- Handles SRT, ASS/SSA → WebVTT conversion
- PGS bitmap subs flagged for burn-in (can't convert to text)

### 2.10 Session Manager (`session.rs`)

```rust
pub struct SessionManager {
    sessions: DashMap<Uuid, Session>,
    config: StreamingConfig,
    detected_accel: DetectedAccel,
    pool: PgPool,
}
```

- `create_direct_session()` — record in DB, return session ID
- `create_transcode_session()` — check max sessions, create temp dir, spawn ffmpeg (with hw fallback), record in DB
- `create_multi_rendition_session()` — spawn one ffmpeg per quality tier, generate master playlist, record in DB
- `stop_session()` — kill all ffmpeg processes, clean temp dir, update DB
- `spawn_cleanup_task()` — background task every 60s, kill sessions idle >5min
- `heartbeat()` — update last activity timestamp (prevents idle cleanup)

### 2.11 Bandwidth Test

`GET /api/v1/stream/bandwidth-test?size={bytes}` returns a zero-filled payload for client-side bandwidth measurement. The client uses this to select the appropriate quality tier for ABR streaming.

---

## Phase 3: API Routes

**New file:** `crates/ngms-web/src/routes/stream.rs`

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

The runtime stage installs jellyfin-ffmpeg7 (FFmpeg 7.1.x with full QSV/VAAPI/oneVPL + bundled Intel drivers):
```dockerfile
RUN apt-get update \
    && curl -fsSL https://repo.jellyfin.org/jellyfin_team.gpg.key \
       | gpg --dearmor -o /usr/share/keyrings/jellyfin.gpg \
    && echo "deb [signed-by=/usr/share/keyrings/jellyfin.gpg] https://repo.jellyfin.org/debian bookworm main" \
       > /etc/apt/sources.list.d/jellyfin.list \
    && apt-get install -y --no-install-recommends jellyfin-ffmpeg7 xz-utils \
    && rm -rf /var/lib/apt/lists/*
```

Environment: `LIBVA_DRIVER_NAME=iHD` is set for Intel GPU driver selection.

### 5.2 Docker Compose

Add to both `docker-compose.yml` and `docker-compose.prod.yml`:
```yaml
devices:
  - /dev/dri:/dev/dri
volumes:
  - ngms-transcode:/config/transcode
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
