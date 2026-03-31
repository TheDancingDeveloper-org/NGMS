# StackArr User Journeys

Test scenarios covering all user-facing functionality organized by feature area.

---

## First Boot & Auth

| # | Journey |
|---|---------|
| 1 | User installs app and goes through first boot flow |
| 2 | User logs in with credentials |
| 3 | User logs out |
| 4 | Admin creates an invite code and shares it |
| 5 | User registers with an invite code |
| 6 | Admin creates a new user account directly |
| 7 | Admin deletes a user account |
| 8 | User updates their profile (display name, avatar, password) |
| 9 | User connects from a mobile/Tauri device (device token auth) |

## Module Configuration

| # | Journey |
|---|---------|
| 10 | Admin enables/disables feature modules (TV, Movies, Torrent, Usenet, Plex, etc.) |
| 11 | User navigates UI and sees only module-gated pages relevant to enabled modules |

## Usenet

| # | Journey |
|---|---------|
| 12 | User adds a usenet server |
| 13 | User tests a usenet server connection |
| 14 | User edits a usenet server |
| 15 | User disables/re-enables a usenet server |
| 16 | User deletes a usenet server |
| 17 | User adds an NZB by URL |
| 18 | User uploads an NZB file |
| 19 | User pauses a usenet download |
| 20 | User resumes a usenet download |
| 21 | User deletes a usenet download |
| 22 | User pauses all usenet downloads |
| 23 | User resumes all usenet downloads |
| 24 | User sets a usenet speed limit |
| 25 | User changes usenet engine settings |

## Indexers

| # | Journey |
|---|---------|
| 26 | User adds a Newznab (usenet) indexer |
| 27 | User adds a Torznab (torrent) indexer |
| 28 | User adds a Cardigann indexer from the catalog |
| 29 | User browses the available Cardigann indexer catalog |
| 30 | User tests an indexer connection |
| 31 | User edits an indexer |
| 32 | User deletes an indexer |
| 33 | User performs a freehand search across all indexers |
| 34 | User searches with specific category filters |
| 35 | User searches with Indexarr-only flag |

## TV Series

| # | Journey |
|---|---------|
| 36 | User adds a TV series (search, select, configure quality/path) |
| 37 | User views the series list |
| 38 | User views a series detail page (seasons, episodes, metadata) |
| 39 | User monitors/unmonitors specific episodes |
| 40 | User monitors/unmonitors an entire season |
| 41 | User searches for a specific episode |
| 42 | User manually grabs a release for an episode |
| 43 | User searches for all missing episodes of a series (Wanted > Search) |
| 44 | User edits series settings (quality profile, path, monitoring) |
| 45 | User deletes a TV series |
| 46 | User views episode files/quality info |

## Movies

| # | Journey |
|---|---------|
| 47 | User adds a movie (search, select, configure quality/path) |
| 48 | User views the movie list |
| 49 | User views a movie detail page |
| 50 | User searches for a specific movie release |
| 51 | User manually grabs a release for a movie |
| 52 | User edits movie settings (quality profile, path, monitoring) |
| 53 | User deletes a movie |

## Discover

| # | Journey |
|---|---------|
| 54 | User browses trending content on the Discover page |
| 55 | User browses upcoming movies |
| 56 | User browses upcoming TV shows |
| 57 | User filters discover by genre |
| 58 | User filters discover by studio/network |
| 59 | User views recommendations for a movie |
| 60 | User views similar content for a TV show |
| 61 | User searches from the Discover page and sees in-library status |
| 62 | User adds a movie/series directly from Discover |
| 63 | User customizes Discover sliders (add, reorder, delete, reset) |

## Requests

| # | Journey |
|---|---------|
| 64 | User requests a movie from Discover |
| 65 | User requests a TV series from Discover |
| 66 | Admin views pending requests |
| 67 | Admin approves a request |
| 68 | Admin declines a request with a note |
| 69 | User views their own request history |

## Quality Profiles

| # | Journey |
|---|---------|
| 70 | User creates a quality profile with cutoff and allowed qualities |
| 71 | User edits a quality profile |
| 72 | User deletes a quality profile |
| 73 | User configures custom format scoring rules |
| 74 | User sets a minimum custom format score |

## Download Clients

| # | Journey |
|---|---------|
| 75 | User adds an external download client |
| 76 | User tests an external download client connection |
| 77 | User edits a download client |
| 78 | User deletes a download client |
| 79 | User views embedded engine status alongside external clients |
| 80 | User sets download client priority |

## Queue & Wanted

| # | Journey |
|---|---------|
| 81 | User views the active download queue |
| 82 | User removes an item from the queue |
| 83 | User views Wanted > Missing episodes/movies |
| 84 | User views Wanted > Cutoff Unmet items |
| 85 | User triggers "Search All" for all missing content |
| 86 | User triggers search for a single missing item from Wanted |

## Torrent Engine

| # | Journey |
|---|---------|
| 87 | User views torrent engine status/stats |
| 88 | User adds a torrent by magnet link |
| 89 | User uploads a .torrent file |
| 90 | User pauses a torrent |
| 91 | User resumes a torrent |
| 92 | User deletes a torrent (with/without files) |
| 93 | User changes torrent engine settings (speed limits, DHT, peer limits) |

## RSS Feeds

| # | Journey |
|---|---------|
| 94 | User adds an RSS feed |
| 95 | User edits an RSS feed |
| 96 | User deletes an RSS feed |
| 97 | User manually triggers a feed check |
| 98 | User manually downloads an item from a feed |
| 99 | User creates an RSS auto-download rule with regex matching |
| 100 | User edits/deletes an RSS rule |

## Calendar

| # | Journey |
|---|---------|
| 101 | User views the calendar for upcoming episodes |
| 102 | User changes the calendar date range |

## History

| # | Journey |
|---|---------|
| 103 | User views download history (grabs, imports, upgrades, failures) |
| 104 | User paginates through history |

## Blocklist

| # | Journey |
|---|---------|
| 105 | User views the blocklist |
| 106 | User adds a release to the blocklist |
| 107 | User removes a release from the blocklist |
| 108 | User bulk-deletes blocklist entries |

## Plex Integration

| # | Journey |
|---|---------|
| 109 | User adds a Plex server via PIN-based OAuth |
| 110 | User validates Plex server connection |
| 111 | User syncs and views Plex libraries |
| 112 | User enables/disables specific Plex libraries |
| 113 | User triggers a full Plex library scan |
| 114 | User triggers a recent Plex scan |
| 115 | User generates and copies the Plex webhook URL |
| 116 | Plex sends a webhook event (play/pause/stop/scrobble) and user views it in the event log |
| 117 | User syncs Plex watchlist |
| 118 | User configures watchlist auto-request settings |
| 119 | User views unified streaming sessions (StackArr + Plex combined) |
| 120 | User deletes a Plex server |

## Streaming

| # | Journey |
|---|---------|
| 121 | User plays a media file via direct play in the built-in player |
| 122 | User starts an HLS transcode session |
| 123 | User switches quality tiers mid-playback |
| 124 | User views active streaming sessions |
| 125 | User stops another user's streaming session (admin) |
| 126 | User runs a bandwidth test |
| 127 | User views subtitles extracted from an embedded track |

## Stremio Addon

| # | Journey |
|---|---------|
| 128 | User adds the StackArr Stremio addon via manifest URL |
| 129 | User browses their library from Stremio |
| 130 | User plays a stream from Stremio |

## User Watchlist & Ratings

| # | Journey |
|---|---------|
| 131 | User adds a title to their personal watchlist |
| 132 | User removes a title from their watchlist |
| 133 | User rates a movie or series |
| 134 | User views their ratings |

## Notifications

| # | Journey |
|---|---------|
| 135 | Admin configures a Discord notification provider |
| 136 | Admin configures a Telegram notification provider |
| 137 | Admin configures a Slack notification provider |
| 138 | Admin configures an Email notification provider |
| 139 | Admin configures a generic webhook notification provider |
| 140 | Admin selects which events trigger each notification (grab, import, upgrade, health, failure) |
| 141 | System fires a notification on grab/import/failure and user receives it |

## Settings — General & Naming

| # | Journey |
|---|---------|
| 142 | User changes the instance name |
| 143 | User updates the TMDB API key |
| 144 | User changes bind address/port |
| 145 | User configures file naming templates (standard, daily, anime, season folder) |
| 146 | User configures movie naming templates |

## Media Folders & Tags

| # | Journey |
|---|---------|
| 147 | User adds a media library root folder |
| 148 | User edits/deletes a media library folder |
| 149 | User creates a tag |
| 150 | User applies tags to series/movies |
| 151 | User deletes a tag |

## Import & Migration

| # | Journey |
|---|---------|
| 152 | User migrates from Sonarr (SQLite import) |
| 153 | User migrates from Radarr (SQLite import) |
| 154 | User migrates from Prowlarr (SQLite import) |
| 155 | User exports a full config backup (JSON) |
| 156 | User restores from a config backup (JSON) |

## Import Lists

| # | Journey |
|---|---------|
| 157 | User adds an import list (Trakt, IMDB, etc.) |
| 158 | User triggers a manual import list sync |
| 159 | User edits/deletes an import list |

## File Browser

| # | Journey |
|---|---------|
| 160 | User browses server filesystem via the file browser |

## Logs & Health

| # | Journey |
|---|---------|
| 161 | User views live logs in the log viewer |
| 162 | User views log files |
| 163 | System health check detects a failing indexer and auto-disables it |
| 164 | System health check detects recovery and auto-re-enables an indexer |

## Remote Access (Bootstrap)

| # | Journey |
|---|---------|
| 165 | Admin generates a claim code for remote access |
| 166 | Remote client registers using the claim code |
| 167 | Admin views and manages connected remote clients |
| 168 | Admin revokes a remote client |

## Background Tasks (Implicit Journeys)

| # | Journey |
|---|---------|
| 169 | RSS sync runs automatically and auto-downloads a matching release |
| 170 | Import scan detects a completed download and imports it to the library |
| 171 | Metadata refresh updates series/movie info from TMDB |
| 172 | Auto-search finds and grabs missing content automatically |
| 173 | Plex watchlist sync creates auto-requests for new watchlist items |
| 174 | Recycle bin cleanup removes old files |

## Multi-User Scenarios

| # | Journey |
|---|---------|
| 175 | Non-admin user attempts an admin-only action (delete series) and gets denied |
| 176 | Two users request the same movie — second gets "already requested" |
| 177 | User adds a series that already exists — gets "already exists" error |

## Error & Edge Cases

| # | Journey |
|---|---------|
| 178 | User adds an indexer with invalid credentials — test fails |
| 179 | User grabs a release that gets blocklisted — system skips it on next search |
| 180 | Download fails — appears in history as failure, notification fires |
| 181 | User attempts to stream a file that doesn't exist on disk |
| 182 | User hits rate limit on login attempts |
