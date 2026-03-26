# API Reference

All endpoints are under `/api/v1/`. The API is JSON-based. Responses use standard HTTP status codes.

## Common Patterns

**Success**: `200 OK` with JSON body. `201 Created` for POST where applicable.
**Not Found**: `404` with `{"error": "not found: ..."}`.
**Validation**: `400` with `{"error": "validation error: ..."}`.
**Server Error**: `500` with `{"error": "..."}`.

**Handler pattern** (Axum):
```rust
async fn handler(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    Json(body): Json<RequestBody>,
) -> impl IntoResponse {
    match service.operation(id, body).await {
        Ok(data) => (StatusCode::OK, Json(data)).into_response(),
        Err(Error::NotFound(msg)) => StatusCode::NOT_FOUND.into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))).into_response(),
    }
}
```

---

## System

| Method | Path | Description |
|--------|------|-------------|
| GET | `/health` | Simple health check |
| GET | `/api/v1/system/health` | DB connectivity check |
| GET | `/api/v1/system/status` | Version, instance name, first boot, enabled modules, start time |
| POST | `/api/v1/system/setup` | First-boot initialization (enable modules, set folders) |
| POST | `/api/v1/system/migrate` | Upload *arr backup DBs for migration (multipart, 1GB limit) |
| POST | `/api/v1/command` | Execute commands: `DiskScan`, `RefreshSeries`, `RefreshMovie`, `RefreshAll` |
| GET | `/api/v1/filesystem/browse` | Browse server filesystem (query: `path`) |

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
    "notifications": false
  },
  "startTime": "2026-03-27T00:00:00Z"
}
```

### Setup Request
```json
{
  "enabled_modules": { "tvManagement": true, "movieManagement": true, ... },
  "media_library_folders": [
    { "path": "/media/TV1", "media_type": "series" },
    { "path": "/media/Movies1", "media_type": "movie" }
  ],
  "indexarr_config": { "url": "http://indexarr:8080", "api_key": "...", "mode": "peer" }
}
```

---

## Series

| Method | Path | Description |
|--------|------|-------------|
| GET | `/api/v1/series` | List all series |
| POST | `/api/v1/series` | Create series |
| GET | `/api/v1/series/{id}` | Get series by ID |
| PUT | `/api/v1/series/{id}` | Update series |
| DELETE | `/api/v1/series/{id}` | Delete series (cascades to seasons, episodes, files) |
| GET | `/api/v1/series/lookup` | TMDB search (query: `term`) |

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
| GET | `/api/v1/movies` | List all movies |
| POST | `/api/v1/movies` | Create movie |
| GET | `/api/v1/movies/{id}` | Get movie by ID |
| PUT | `/api/v1/movies/{id}` | Update movie |
| DELETE | `/api/v1/movies/{id}` | Delete movie |
| GET | `/api/v1/movies/lookup` | TMDB movie search (query: `term`) |

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
| PUT | `/api/v1/episode/{id}` | Update episode (monitored flag) |

---

## Calendar

| Method | Path | Description |
|--------|------|-------------|
| GET | `/api/v1/calendar` | Upcoming episodes (query: `start`, `end` as YYYY-MM-DD, default: today + 14 days) |

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
| GET | `/api/v1/history` | Paginated history (query: `page=1`, `page_size=20`) |

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
| GET | `/api/v1/wanted/missing` | Missing (monitored, no file) items, paginated |

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
| POST | `/api/v1/indexer` | Create indexer |
| GET | `/api/v1/indexer/{id}` | Get indexer |
| PUT | `/api/v1/indexer/{id}` | Update indexer |
| DELETE | `/api/v1/indexer/{id}` | Delete indexer |

---

## Download Clients

| Method | Path | Description |
|--------|------|-------------|
| GET | `/api/v1/downloadclient` | List clients |
| POST | `/api/v1/downloadclient` | Create client |
| GET | `/api/v1/downloadclient/{id}` | Get client |
| PUT | `/api/v1/downloadclient/{id}` | Update client |
| DELETE | `/api/v1/downloadclient/{id}` | Delete client |

---

## Torrent Engine

| Method | Path | Description |
|--------|------|-------------|
| GET | `/api/v1/torrent/status` | Engine status, speeds, peers, counters |
| GET | `/api/v1/torrent/list` | List active torrents |
| POST | `/api/v1/torrent/add` | Add torrent from URL |
| POST | `/api/v1/torrent/{id}/pause` | Pause torrent |
| POST | `/api/v1/torrent/{id}/resume` | Resume torrent |
| DELETE | `/api/v1/torrent/{id}` | Remove torrent |

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

---

## Media Library Folders

| Method | Path | Description |
|--------|------|-------------|
| GET | `/api/v1/medialibraryfolder` | List folders |
| POST | `/api/v1/medialibraryfolder` | Create folder |
| PUT | `/api/v1/medialibraryfolder/{id}` | Update folder |
| DELETE | `/api/v1/medialibraryfolder/{id}` | Delete folder |

---

## Naming

| Method | Path | Description |
|--------|------|-------------|
| GET | `/api/v1/naming` | Get naming config |
| PUT | `/api/v1/naming` | Update naming config |

---

## Tags

| Method | Path | Description |
|--------|------|-------------|
| GET | `/api/v1/tag` | List tags |
| POST | `/api/v1/tag` | Create tag |
| PUT | `/api/v1/tag/{id}` | Update tag |
| DELETE | `/api/v1/tag/{id}` | Delete tag |

---

## Import Lists

| Method | Path | Description |
|--------|------|-------------|
| GET | `/api/v1/importlist` | List import lists |
| POST | `/api/v1/importlist` | Create import list |
| GET | `/api/v1/importlist/{id}` | Get import list |
| PUT | `/api/v1/importlist/{id}` | Update import list |
| DELETE | `/api/v1/importlist/{id}` | Delete import list |
| POST | `/api/v1/importlist/{id}/sync` | Manually trigger sync |

---

## Discover

| Method | Path | Description |
|--------|------|-------------|
| GET | `/api/v1/discover` | List discover sliders |
| POST | `/api/v1/discover` | Create slider |
| PUT | `/api/v1/discover/{id}` | Update slider |
| DELETE | `/api/v1/discover/{id}` | Delete slider |
| PUT | `/api/v1/discover/reorder` | Reorder sliders |

---

## Plex

| Method | Path | Description |
|--------|------|-------------|
| GET | `/api/v1/plex/servers` | List Plex servers |
| POST | `/api/v1/plex/servers` | Add Plex server (validates connection) |
| PUT | `/api/v1/plex/servers/{id}` | Update server |
| DELETE | `/api/v1/plex/servers/{id}` | Delete server |
| POST | `/api/v1/plex/servers/{id}/sync-libraries` | Sync library list from Plex |

---

## Indexarr

| Method | Path | Description |
|--------|------|-------------|
| GET | `/api/v1/indexarr/status` | Indexarr sidecar status |
| POST | `/api/v1/indexarr/test` | Test connection |

---

## Releases

| Method | Path | Description |
|--------|------|-------------|
| POST | `/api/v1/releases/search` | Search indexers for releases |
| POST | `/api/v1/releases/grab` | Grab a release for download |
