# API Reference

All endpoints are under `/api/v1/` unless noted. The API is JSON-based with `camelCase` field names.

## Authentication

Most endpoints require an API key. The key is generated during first-boot setup and can be provided via:

- **Header**: `X-Api-Key: <key>`
- **Bearer token**: `Authorization: Bearer <key>`
- **Query parameter**: `?apikey=<key>`

Some endpoints also accept a **remote client token** (UUID) as an alternative to the API key (used by bootstrap-paired clients).

**Public routes** (no auth): `/health`, `GET /api/v1/system/status`, `POST /api/v1/setup/init`, `GET /api/v1/auth/status`, `POST /api/v1/auth/setup`.

## Common Patterns

**Success**: `200 OK` with JSON body. `201 Created` for POST where applicable.
**Not Found**: `404` with `{"error": "not found: ..."}`.
**Validation**: `400` with `{"error": "validation error: ..."}`.
**Unauthorized**: `401` if API key is missing or invalid.
**Service Unavailable**: `503` if a required engine/module is not initialized.
**Server Error**: `500` with `{"error": "..."}`.

**Pagination** (where applicable):
- Query: `?page=1&page_size=20` (defaults)
- Response: `{ "page": 1, "pageSize": 20, "totalRecords": N, "records": [...] }`

---

## System

| Method | Path | Auth | Description |
|--------|------|------|-------------|
| GET | `/health` | No | Simple health check (`{"status": "ok"}`) |
| GET | `/api/v1/system/status` | No | Version, instance name, first boot, enabled modules, start time |
| GET | `/api/v1/system/health` | Yes | Detailed health (DB, disk, indexers, download clients) |
| POST | `/api/v1/setup/init` | No | First-boot initialization (enable modules, set folders, returns API key) |
| POST | `/api/v1/system/migrate` | Yes | Upload *arr backup DBs for migration (multipart, 1GB limit) |
| POST | `/api/v1/command` | Yes | Execute commands: `DiskScan`, `RefreshSeries`, `RefreshMovie`, `RefreshAll` |
| PUT | `/api/v1/modules` | Yes | Update enabled modules configuration |
| GET | `/api/v1/filesystem/browse` | Yes | Browse server filesystem (query: `path`) |
| GET | `/metrics` | Yes | Prometheus-compatible metrics (text/plain) |

### Status Response
```json
{
  "version": "0.1.0",
  "instanceName": "StackArr",
  "firstBoot": false,
  "modules": {
    "tvManagement": true,
    "movieManagement": true,
    "torrentEmbedded": true,
    "usenetEmbedded": false,
    "torrentExternal": false,
    "usenetExternal": false,
    "indexarrSidecar": true,
    "externalIndexers": false,
    "plexIntegration": true,
    "notifications": false,
    "streaming": false,
    "remoteAccess": false
  },
  "startTime": "2026-03-27T00:00:00Z"
}
```

### Setup Request
```json
{
  "enabled_modules": { "tvManagement": true, "movieManagement": true, "..." : true },
  "media_library_folders": [
    { "path": "/media/TV1", "media_type": "series" },
    { "path": "/media/Movies1", "media_type": "movie" }
  ],
  "indexarr_config": { "url": "http://indexarr:8080", "api_key": "...", "mode": "peer" }
}
```

### Setup Response
```json
{
  "apiKey": "generated-api-key-string"
}
```

---

## Auth

| Method | Path | Auth | Description |
|--------|------|------|-------------|
| GET | `/api/v1/auth/status` | No | Returns `{ setupRequired, registrationEnabled }` — whether first-boot setup is needed |
| POST | `/api/v1/auth/setup` | No | First-boot admin account creation. Rejects if any users exist. |
| POST | `/api/v1/auth/login` | No | Login with username + password. Accepts optional `deviceName` field. |
| POST | `/api/v1/auth/logout` | Yes | Invalidate current session |
| POST | `/api/v1/auth/register` | No | Register account with invite code |
| GET | `/api/v1/auth/me` | Yes | Get current authenticated user |

### Auth Status Response
```json
{
  "setupRequired": true,
  "registrationEnabled": false
}
```

### Setup Request
```json
{
  "username": "admin",
  "password": "...",
  "displayName": "Admin"
}
```
Guards: rejects if any users already exist in the database.

### Login Request / Response
```json
// Request
{
  "username": "admin",
  "password": "...",
  "deviceName": "iPhone 15"  // optional — if provided, returns deviceToken
}

// Response
{
  "user": { "id": 1, "username": "admin", "role": "admin", "displayName": "Admin" },
  "deviceToken": "uuid-string"  // only present if deviceName was sent
}
```

---

## Admin

### User & Invite Management

| Method | Path | Auth | Description |
|--------|------|------|-------------|
| GET | `/api/v1/admin/users` | Admin | List all users |
| POST | `/api/v1/admin/users` | Admin | Create user |
| PUT | `/api/v1/admin/users/{id}` | Admin | Update user |
| DELETE | `/api/v1/admin/users/{id}` | Admin | Delete user |
| GET | `/api/v1/admin/invites` | Admin | List invite codes |
| POST | `/api/v1/admin/invites` | Admin | Create invite code |
| DELETE | `/api/v1/admin/invites/{id}` | Admin | Delete invite code |

### Bootstrap Name Management

| Method | Path | Auth | Description |
|--------|------|------|-------------|
| GET | `/api/v1/admin/bootstrap/status` | Admin | Returns `{ enabled, nameRegistered, serverName }` |
| POST | `/api/v1/admin/bootstrap/register-name` | Admin | Register server name with bootstrap, returns 12-word BIP39 recovery phrase |
| POST | `/api/v1/admin/bootstrap/recover-name` | Admin | Recover server name with BIP39 recovery phrase after rebuild |

### Register Name Response
```json
{
  "serverName": "my-server",
  "recoveryPhrase": "abandon ability able about above absent absorb abstract absurd abuse access accident"
}
```

### Bootstrap Status Response
```json
{
  "enabled": true,
  "nameRegistered": true,
  "serverName": "my-server"
}
```

When bootstrap is enabled, invite codes are auto-registered with the bootstrap service as unified claim codes. A single 8-char code handles both server discovery (via bootstrap) and account creation (via invite).

---

## Series

| Method | Path | Description |
|--------|------|-------------|
| GET | `/api/v1/series` | List all series (includes season/episode/file counts) |
| POST | `/api/v1/series` | Create series |
| GET | `/api/v1/series/{id}` | Get series by ID |
| PUT | `/api/v1/series/{id}` | Update series |
| DELETE | `/api/v1/series/{id}` | Delete series (cascades to seasons, episodes, files) |
| GET | `/api/v1/series/lookup?term=<query>` | TMDB search for series |

### Create Series
```json
{
  "title": "Breaking Bad",
  "path": "/media/TV1/Breaking Bad",
  "qualityProfileId": 2,
  "monitored": true,
  "tvdbId": 81189,
  "tmdbId": 1396,
  "imdbId": "tt0903747"
}
```

### Series Response
```json
{
  "id": 1,
  "title": "Breaking Bad",
  "cleanTitle": "breaking bad",
  "sortTitle": "breaking bad",
  "status": "ended",
  "seriesType": "standard",
  "year": 2008,
  "path": "/media/TV1/Breaking Bad",
  "qualityProfileId": 2,
  "monitored": true,
  "tvdbId": 81189,
  "tmdbId": 1396,
  "imdbId": "tt0903747",
  "images": [...],
  "genres": ["Drama", "Crime"],
  "tags": [1, 3],
  "addedAt": "2026-03-27T00:00:00Z",
  "seasonCount": 5,
  "episodeCount": 62,
  "episodeFileCount": 62
}
```

---

## Movies

| Method | Path | Description |
|--------|------|-------------|
| GET | `/api/v1/movies` | List all movies (includes file info) |
| POST | `/api/v1/movies` | Create movie |
| GET | `/api/v1/movies/{id}` | Get movie by ID |
| PUT | `/api/v1/movies/{id}` | Update movie |
| DELETE | `/api/v1/movies/{id}` | Delete movie |
| GET | `/api/v1/movies/lookup?term=<query>` | TMDB movie search |

### Create Movie
```json
{
  "title": "Inception",
  "path": "/media/Movies1/Inception (2010)",
  "qualityProfileId": 3,
  "monitored": true,
  "tmdbId": 27205,
  "imdbId": "tt1375666",
  "year": 2010
}
```

---

## Episodes

| Method | Path | Description |
|--------|------|-------------|
| GET | `/api/v1/series/{seriesId}/episodes` | List episodes for series |
| GET | `/api/v1/episode/{id}` | Get episode details |
| PUT | `/api/v1/episode/{id}` | Update episode (body: `{ "monitored": bool }`) |
| PUT | `/api/v1/episode/monitor` | Bulk monitor toggle (body: `{ "episodeIds": [...], "monitored": bool }`) |

---

## Calendar

| Method | Path | Description |
|--------|------|-------------|
| GET | `/api/v1/calendar?start=<YYYY-MM-DD>&end=<YYYY-MM-DD>` | Episodes airing in date range (default: today + 14 days) |

### Response
```json
[
  {
    "episodeId": 42,
    "seriesId": 1,
    "seriesTitle": "Show Name",
    "seasonNumber": 3,
    "episodeNumber": 7,
    "episodeTitle": "Episode Title",
    "airDateUtc": "2026-04-01T20:00:00Z",
    "hasFile": false,
    "monitored": true
  }
]
```

---

## Queue

| Method | Path | Description |
|--------|------|-------------|
| GET | `/api/v1/queue` | List in-progress downloads |
| DELETE | `/api/v1/queue/{id}` | Remove queue item |

### Queue Item
```json
{
  "id": 1,
  "mediaType": "series",
  "mediaId": 5,
  "episodeId": 42,
  "title": "Show.Name.S03E07.720p.HDTV.x264-GROUP",
  "quality": { "quality": {"id": 6, "name": "HDTV-720p"}, "revision": {"version": 1} },
  "size": 1073741824,
  "status": "downloading",
  "downloadId": "abc123",
  "protocol": "torrent",
  "addedAt": "2026-03-27T00:00:00Z"
}
```

---

## History

| Method | Path | Description |
|--------|------|-------------|
| GET | `/api/v1/history?page=1&page_size=20` | Paginated history events (includes quality resolution and indexer names) |

### Response
```json
{
  "page": 1,
  "pageSize": 20,
  "totalRecords": 150,
  "records": [
    {
      "id": 1,
      "mediaType": "series",
      "mediaId": 5,
      "episodeId": 42,
      "eventType": "grabbed",
      "quality": {...},
      "sourceTitle": "Show.Name.S03E07.720p.HDTV.x264-GROUP",
      "downloadId": "abc123",
      "occurredAt": "2026-03-27T00:00:00Z"
    }
  ]
}
```

---

## Wanted

| Method | Path | Description |
|--------|------|-------------|
| GET | `/api/v1/wanted/missing?page=1&page_size=20` | Missing items (monitored, aired, no file), paginated |
| GET | `/api/v1/wanted/cutoff?page=1&page_size=20` | Items with files below quality cutoff, paginated |

---

## Quality Profiles

| Method | Path | Description |
|--------|------|-------------|
| GET | `/api/v1/qualityprofile` | List profiles |
| POST | `/api/v1/qualityprofile` | Create profile |
| GET | `/api/v1/qualityprofile/{id}` | Get profile |
| PUT | `/api/v1/qualityprofile/{id}` | Update profile |
| DELETE | `/api/v1/qualityprofile/{id}` | Delete profile |

---

## Indexers

| Method | Path | Description |
|--------|------|-------------|
| GET | `/api/v1/indexer` | List indexers |
| POST | `/api/v1/indexer` | Create indexer (Cardigann or Newznab/Torznab) |
| PUT | `/api/v1/indexer/{id}` | Update indexer |
| DELETE | `/api/v1/indexer/{id}` | Delete indexer |
| POST | `/api/v1/indexer/{id}/test` | Test indexer connection |
| GET | `/api/v1/indexer/available` | List all available Cardigann definitions (query: `?privacy=public`) |
| GET | `/api/v1/indexer/available/{id}` | Get single Cardigann definition with settings |

---

## Download Clients

| Method | Path | Description |
|--------|------|-------------|
| GET | `/api/v1/downloadclient` | List clients |
| POST | `/api/v1/downloadclient` | Create client |
| PUT | `/api/v1/downloadclient/{id}` | Update client |
| DELETE | `/api/v1/downloadclient/{id}` | Delete client |
| POST | `/api/v1/downloadclient/{id}/test` | Test client connection |

---

## Releases & Search

| Method | Path | Description |
|--------|------|-------------|
| GET | `/api/v1/release?term=<query>&quality_profile_id=<id>&media_type=series\|movie` | Search releases with decision engine ranking |
| POST | `/api/v1/release` | Grab/download release |
| GET | `/api/v1/search?query=<text>&categories=<csv>` | Freehand text search across all indexers |

### Grab Request
```json
{
  "guid": "release-guid",
  "indexerId": 1,
  "title": "Show.Name.S01E01.720p.HDTV",
  "downloadUrl": "https://...",
  "protocol": "torrent",
  "size": 1073741824,
  "mediaId": 5,
  "mediaType": "series"
}
```

---

## Blocklist

| Method | Path | Description |
|--------|------|-------------|
| GET | `/api/v1/blocklist?page=1&pageSize=20` | Paginated blocklist |
| POST | `/api/v1/blocklist` | Add blocklist entry |
| DELETE | `/api/v1/blocklist/{id}` | Delete entry |
| DELETE | `/api/v1/blocklist/bulk` | Bulk delete (body: `{ "ids": [...] }`) |

---

## Torrent Engine

| Method | Path | Description |
|--------|------|-------------|
| GET | `/api/v1/torrent/status` | Session stats (speeds, peers, counters) |
| GET | `/api/v1/torrent/list` | List all torrents |
| POST | `/api/v1/torrent/add` | Add torrent by URL |
| GET | `/api/v1/torrent/{id}` | Get torrent details |
| GET | `/api/v1/torrent/{id}/stats` | Get torrent statistics |
| POST | `/api/v1/torrent/{id}/pause` | Pause torrent |
| POST | `/api/v1/torrent/{id}/resume` | Resume torrent |
| POST | `/api/v1/torrent/{id}/delete?deleteFiles=true\|false` | Delete torrent |

Returns `503 Service Unavailable` if engine not initialized.

---

## Usenet Engine

| Method | Path | Description |
|--------|------|-------------|
| GET | `/api/v1/usenet/status` | Engine status, queue size, active downloads |
| GET | `/api/v1/usenet/queue` | List NZB jobs |
| POST | `/api/v1/usenet/add` | Add NZB from URL |
| POST | `/api/v1/usenet/{jobId}/pause` | Pause job |
| POST | `/api/v1/usenet/{jobId}/resume` | Resume job |
| DELETE | `/api/v1/usenet/{jobId}` | Delete job |
| PUT | `/api/v1/usenet/{jobId}/category` | Update category |
| PUT | `/api/v1/usenet/{jobId}/priority` | Update priority |

Returns `503 Service Unavailable` if engine not initialized.

---

## Media Library Folders

| Method | Path | Description |
|--------|------|-------------|
| GET | `/api/v1/medialibraryfolder` | List folders (includes disk space info) |
| POST | `/api/v1/medialibraryfolder` | Create folder (validates path exists; body: `{ "path": "...", "mediaType": "series\|movie" }`) |
| DELETE | `/api/v1/medialibraryfolder/{id}` | Delete folder |

---

## Naming

| Method | Path | Description |
|--------|------|-------------|
| GET | `/api/v1/config/naming` | Get naming config for series and movies |
| PUT | `/api/v1/config/naming` | Update naming config |

---

## Tags

| Method | Path | Description |
|--------|------|-------------|
| GET | `/api/v1/tag` | List tags |
| POST | `/api/v1/tag` | Create tag |
| PUT | `/api/v1/tag/{id}` | Update tag label |
| DELETE | `/api/v1/tag/{id}` | Delete tag |

---

## Import Lists

| Method | Path | Description |
|--------|------|-------------|
| GET | `/api/v1/importlist` | List import lists |
| POST | `/api/v1/importlist` | Create import list |
| PUT | `/api/v1/importlist/{id}` | Update import list |
| DELETE | `/api/v1/importlist/{id}` | Delete import list |
| POST | `/api/v1/importlist/{id}/sync` | Sync single list |
| POST | `/api/v1/importlist/sync` | Sync all lists |

---

## Discover

### Slider Management (UI carousels)

| Method | Path | Description |
|--------|------|-------------|
| GET | `/api/v1/discover/sliders` | List discover sliders |
| POST | `/api/v1/discover/sliders` | Reorder sliders (body: `{ "sliderIds": [...] }`) |
| POST | `/api/v1/discover/sliders/add` | Create custom slider |
| PUT | `/api/v1/discover/sliders/{id}` | Update slider |
| DELETE | `/api/v1/discover/sliders/{id}` | Delete custom slider (built-ins protected) |
| POST | `/api/v1/discover/sliders/reset` | Reset sliders to defaults |

### TMDB Browse & Discovery

| Method | Path | Description |
|--------|------|-------------|
| GET | `/api/v1/discover/trending?media_type=all\|movie\|tv&time_window=day\|week&page=1&language=en` | Trending content |
| GET | `/api/v1/discover/movies` | Discover movies with filters |
| GET | `/api/v1/discover/movies/upcoming` | Upcoming movies |
| GET | `/api/v1/discover/movies/genre/{genre_id}` | Movies by genre |
| GET | `/api/v1/discover/movies/studio/{studio_id}` | Movies by studio |
| GET | `/api/v1/discover/tv` | Discover TV with filters |
| GET | `/api/v1/discover/tv/upcoming` | Upcoming TV shows |
| GET | `/api/v1/discover/tv/genre/{genre_id}` | TV shows by genre |
| GET | `/api/v1/discover/tv/network/{network_id}` | TV shows by network |

### Recommendations & Similar

| Method | Path | Description |
|--------|------|-------------|
| GET | `/api/v1/discover/movies/{id}/recommendations` | Movie recommendations |
| GET | `/api/v1/discover/movies/{id}/similar` | Similar movies |
| GET | `/api/v1/discover/tv/{id}/recommendations` | TV recommendations |
| GET | `/api/v1/discover/tv/{id}/similar` | Similar TV shows |

### Genres, Languages & Keywords

| Method | Path | Description |
|--------|------|-------------|
| GET | `/api/v1/discover/genres/movie` | Movie genres list |
| GET | `/api/v1/discover/genres/tv` | TV genres list |
| GET | `/api/v1/discover/languages` | Available languages |
| GET | `/api/v1/discover/keyword/{keyword_id}` | Get keyword info |
| GET | `/api/v1/discover/keyword/{keyword_id}/movies` | Movies with keyword |

---

## Streaming

Requires `streaming` module enabled. Uses `RequireAuth` (API key or client token).

| Method | Path | Description |
|--------|------|-------------|
| GET | `/api/v1/stream/{mediaFileId}/info` | ffprobe media info (video/audio/subtitle streams) |
| GET | `/api/v1/stream/{mediaFileId}/direct` | Direct play with HTTP range request support |
| POST | `/api/v1/stream/{mediaFileId}/transcode` | Start transcode session, returns HLS URL |
| GET | `/api/v1/stream/{mediaFileId}/hls/{sessionId}/master.m3u8` | HLS master playlist |
| GET | `/api/v1/stream/{mediaFileId}/hls/{sessionId}/{segment}` | HLS segment (TS) |
| GET | `/api/v1/stream/{mediaFileId}/subtitles/{trackIndex}` | Extract subtitle as WebVTT |
| GET | `/api/v1/stream/sessions` | List active streaming sessions |
| DELETE | `/api/v1/stream/sessions/{sessionId}` | Stop streaming session |

### Transcode Request
```json
{
  "videoCodec": "h264",
  "audioCodec": "aac",
  "maxWidth": 1920,
  "maxHeight": 1080
}
```

---

## Plex

| Method | Path | Description |
|--------|------|-------------|
| GET | `/api/v1/plex/servers` | List Plex servers |
| POST | `/api/v1/plex/servers` | Add Plex server (validates connection) |
| PUT | `/api/v1/plex/servers/{id}` | Update server |
| DELETE | `/api/v1/plex/servers/{id}` | Delete server |
| GET | `/api/v1/plex/servers/{id}/libraries` | Sync and list libraries |
| PUT | `/api/v1/plex/libraries/{id}` | Enable/disable library |
| POST | `/api/v1/plex/scan/full` | Full library scan (background) |
| POST | `/api/v1/plex/scan/recent` | Recent items scan (background) |
| POST | `/api/v1/plex/auth/validate` | Validate Plex auth token |
| POST | `/api/v1/plex/auth/servers` | Discover Plex servers by token |
| GET | `/api/v1/plex/watchlist` | List watchlist entries |
| POST | `/api/v1/plex/watchlist/sync` | Sync watchlist (background) |

---

## Indexarr Sidecar

Proxies requests to the Indexarr sidecar with API key injection.

| Method | Path | Description |
|--------|------|-------------|
| GET | `/api/v1/indexarr/status` | Check Indexarr reachability |
| GET | `/api/v1/indexarr/search?q=<query>` | Proxy search |
| GET | `/api/v1/indexarr/recent` | Recent torrents |
| GET | `/api/v1/indexarr/trending` | Trending torrents |
| GET | `/api/v1/indexarr/torrent/{infoHash}` | Torrent details |
| GET | `/api/v1/indexarr/identity/status` | Identity status |
| POST | `/api/v1/indexarr/identity/acknowledge` | Acknowledge identity |
| GET | `/api/v1/indexarr/sync/preferences` | Sync preferences |
| POST | `/api/v1/indexarr/sync/preferences` | Update sync preferences |

---

## Remote Access

Requires `remote_access` module and bootstrap config.

| Method | Path | Description |
|--------|------|-------------|
| POST | `/api/v1/remote/claim` | Generate claim code (admin API key required) |
| POST | `/api/v1/remote/register` | Self-register client (client token auth) |
| GET | `/api/v1/remote/clients` | List remote clients (admin only) |
| DELETE | `/api/v1/remote/clients/{id}` | Revoke client (admin only) |

## Bootstrap Discovery Service

These endpoints run on the standalone `stackarr-bootstrap` binary (not the main StackArr server).

| Method | Path | Auth | Description |
|--------|------|------|-------------|
| GET | `/api/v1/servers/by-name/{name}` | No | Public server name resolution — returns connection details |
| POST | `/api/v1/servers/register-name` | Bearer | Register a server name, returns BIP39 12-word recovery phrase |
| POST | `/api/v1/servers/recover-name` | Bearer | Verify recovery phrase and transfer server name ownership |
| POST | `/api/v1/claims` | Bearer | Create claim code. Accepts server-provided 8-char codes with `claimType` and `inviteCode` metadata |
| POST | `/api/v1/claims/{code}/redeem` | No | Redeem claim code, returns server connection details |

### Server Name Resolution Response
```json
{
  "name": "my-server",
  "localIp": "192.168.1.100",
  "publicIp": "203.0.113.50",
  "port": 9111
}
```

### Claim Creation (with unified invite code)
```json
{
  "serverId": "uuid",
  "code": "AB12CD34",
  "claimType": "invite",
  "inviteCode": "AB12CD34",
  "localIp": "192.168.1.100",
  "publicIp": "203.0.113.50",
  "port": 9111
}
```

Bootstrap persistence uses SQLite with `server_names` and `pending_claims` tables.

---

## Backup & Restore

| Method | Path | Description |
|--------|------|-------------|
| GET | `/api/v1/system/backup` | Export database tables as JSON |
| POST | `/api/v1/system/restore` | Import configuration from backup JSON |

---

## Logs

| Method | Path | Description |
|--------|------|-------------|
| GET | `/api/v1/log` | Get logs (directs to container log mechanism) |
| GET | `/api/v1/log/file` | List available log files |
