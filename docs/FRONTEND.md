# Frontend

React 19 SPA with TypeScript, Tailwind CSS v4, TanStack React Query v5, and Vite 8.

## Stack

| Tool | Version | Purpose |
|------|---------|---------|
| React | 19.2 | UI framework |
| TypeScript | 5.9 | Type safety (strict mode) |
| React Router DOM | 7.13 | Client-side routing |
| TanStack React Query | 5.95 | Server state management |
| Tailwind CSS | 4.2 (via @tailwindcss/vite) | Utility-first styling |
| Vite | 8.0 | Build tool + dev server |
| Lucide React | 1.7 | Icon library |
| HLS.js | 1.6 | HLS video streaming |

## Directory Structure

```
ui/
├── index.html              # HTML shell (#root mount point)
├── package.json            # Dependencies and scripts
├── vite.config.ts          # Vite config with API proxy
├── tsconfig.json           # TypeScript root config
├── tsconfig.app.json       # App-specific TS config (strict)
├── eslint.config.js        # ESLint with React plugins
└── src/
    ├── main.tsx            # Entry: QueryClient + BrowserRouter + App
    ├── App.tsx             # Route definitions + system status gate
    ├── index.css           # @import "tailwindcss"
    ├── api/
    │   ├── client.ts       # apiFetch<T>() helper
    │   └── types.ts        # All TypeScript interfaces (25+)
    ├── hooks/
    │   └── useApi.ts       # TanStack Query hooks for all endpoints
    ├── components/
    │   ├── Layout.tsx                    # Main layout wrapper with sidebar + header
    │   ├── Sidebar.tsx                   # Navigation with collapsible menu + module gating
    │   ├── MediaCard.tsx                 # Reusable TMDB media card (poster, rating, add button)
    │   ├── MediaSlider.tsx               # Horizontal scrollable carousel with chevron nav
    │   ├── VideoPlayer.tsx               # HLS video player with direct play / transcode fallback
    │   ├── ActivityNotificationBell.tsx   # Bell icon in header with combined badge counter
    │   ├── ActivityNotificationPopup.tsx  # Tabbed popup (Events, Activity, Notifications)
    │   ├── ActivityTab.tsx               # System activity list with progress bars
    │   ├── EventsTab.tsx                 # History event stream with quality badges
    │   ├── InteractiveSearchModal.tsx      # Sortable release search with CF score + matched format badges
    │   └── NotificationTab.tsx           # User notification list with unread indicators
    ├── utils/
    │   ├── date.ts         # formatDate, formatDateTime, formatTime, formatAirDate
    │   └── clipboard.ts    # copyToClipboard() with navigator.clipboard + legacy fallback
    └── pages/
        ├── Discover.tsx    # Trending/popular/upcoming content from TMDB
        ├── SeriesList.tsx  # Grid view with search + add modal
        ├── SeriesDetail.tsx# Detail page with season accordion
        ├── MovieList.tsx   # Movie grid view
        ├── MovieDetail.tsx # Movie detail page
        ├── Calendar.tsx    # Upcoming episodes grouped by date
        ├── Search.tsx      # Freehand indexer search with multi-select filter
        ├── Queue.tsx       # Download queue table (5s auto-refresh)
        ├── History.tsx     # Paginated history table
        ├── Wanted.tsx      # Missing/cutoff tabs with pagination
        ├── Watchlist.tsx   # Plex watchlist items
        ├── Requests.tsx    # Media request management with status tabs
        ├── Users.tsx       # Admin user/invite management
        ├── Torrents.tsx    # Torrent engine management
        ├── Usenet.tsx      # Usenet engine management
        ├── Player.tsx      # HLS video player page
        ├── Streaming.tsx   # Active stream sessions (5s auto-refresh)
        ├── Settings.tsx    # 7-tab settings page
        ├── ServerConnect.tsx # Remote server connection (claim code / direct URL)
        └── FirstBoot.tsx   # Multi-step setup wizard
```

## Scripts

```bash
npm run dev       # Vite dev server on :3000 (proxies /api → :8989)
npm run build     # tsc type-check + vite production build → dist/
npm run lint      # ESLint
npm run preview   # Preview production build locally
```

## API Client

`api/client.ts` handles both local and remote server connections:

```typescript
// Connection management (persisted in localStorage)
getConnection()        // Retrieve saved server connection
saveConnection(conn)   // Save remote server URL + auth token
clearConnection()      // Reset to local mode
redeemClaimCode(code)  // Bootstrap discovery via claim codes

// API layer
apiFetch<T>(path, options)  // Generic fetch with auth headers (Bearer token)
```

- **Local mode**: Base URL is `/api/v1`, Vite proxies to `:8989` in dev.
- **Remote mode**: Base URL is the remote server URL with auth token from bootstrap pairing.
- The client probes local and public IPs to find the fastest endpoint for remote connections.
- `ServerConnection.clientToken` is now **optional** — session-based auth is used for login mode (no token needed).

### ServerConnect Modes

The `ServerConnect` page provides two connection modes (toggled via mode buttons):

1. **Claim Code** — Enter a client name and 4-character claim code. Bootstrap discovery resolves the server via `redeemClaimCode()`. Advanced section allows overriding the bootstrap URL.
2. **Direct URL** — Enter a server URL and API key/client token manually. Validates by probing `/api/v1/system/status`, then saves the connection.

## State Management

**TanStack React Query** is the only state management:

```typescript
// main.tsx
const queryClient = new QueryClient({
    defaultOptions: {
        queries: {
            staleTime: 30_000,   // 30s cache
            retry: 1,
        },
    },
})
```

All data fetching hooks live in `hooks/useApi.ts`:

```typescript
// Query hooks (GET)
export function useSeries() {
    return useQuery({ queryKey: ['series'], queryFn: () => apiFetch<Series[]>('/series') })
}

// Mutation hooks (POST/PUT/DELETE)
export function useAddSeries() {
    const qc = useQueryClient()
    return useMutation({
        mutationFn: (data: CreateSeriesInput) => apiFetch<Series>('/series', { method: 'POST', body: JSON.stringify(data) }),
        onSuccess: () => qc.invalidateQueries({ queryKey: ['series'] }),
    })
}
```

### Key Hooks

| Hook | Endpoint | Notes |
|------|----------|-------|
| **System** | | |
| `useSystemStatus()` | GET /system/status | Gates first-boot vs main app |
| `useSetupInit()` | POST /setup/init | Mutation — first boot |
| `useMigrate()` | POST /system/migrate | Multipart mutation |
| **Media** | | |
| `useSeries()` | GET /series | |
| `useSeriesDetail(id)` | GET /series/{id} | |
| `useEpisodes(seriesId)` | GET /series/{id}/episodes | Enabled when seriesId is set |
| `useSeriesLookup(term)` | GET /series/lookup | TMDB search |
| `useMovies()` | GET /movies | |
| `useMovieDetail(id)` | GET /movies/{id} | |
| `useMovieLookup(term)` | GET /movies/lookup | TMDB search |
| `useAddSeries()` | POST /series | Mutation, invalidates series |
| `useDeleteSeries()` | DELETE /series/{id} | Mutation |
| `useAddMovie()` | POST /movies | Mutation, invalidates movies |
| `useDeleteMovie()` | DELETE /movies/{id} | Mutation |
| `useToggleSeriesMonitor()` | PUT /series/{id} | Mutation |
| `useToggleEpisodeMonitor()` | PUT /episode/{id} | Mutation |
| `useSearchEpisode()` | POST /command | Mutation (EpisodeSearch) |
| `useSearchMovie()` | POST /command | Mutation (MovieSearch) |
| **Operations** | | |
| `useQueue()` | GET /queue | refetchInterval: 5000 |
| `useHistory(page)` | GET /history | Paginated |
| `useCalendar(start, end)` | GET /calendar | Date range |
| **Config** | | |
| `useQualityProfiles()` | GET /qualityprofile | |
| `useIndexers()` | GET /indexer | |
| `useDownloadClients()` | GET /downloadclient | |
| `useNamingConfig()` | GET /config/naming | |
| `useMediaLibraryFolders()` | GET /medialibraryfolder | |
| `useTags()` | GET /tag | |
| **Discover** | | |
| `useTrending()` | GET /discover/trending | |
| `usePopularMovies()` | GET /discover/movies | |
| `usePopularTv()` | GET /discover/tv | |
| `useUpcomingMovies()` | GET /discover/movies/upcoming | |
| `useUpcomingTv()` | GET /discover/tv/upcoming | |
| `useMovieRecommendations(id)` | GET /discover/movies/{id}/recommendations | |
| `useTvRecommendations(id)` | GET /discover/tv/{id}/recommendations | |
| `useMovieSimilar(id)` | GET /discover/movies/{id}/similar | |
| `useTvSimilar(id)` | GET /discover/tv/{id}/similar | |
| `useMovieGenres()` | GET /discover/genres/movie | |
| `useTvGenres()` | GET /discover/genres/tv | |
| `useDiscoverSliders()` | GET /discover/sliders | |
| **Streaming** | | |
| `useStreamInfo(id)` | GET /stream/{id}/info | |
| `useStreamSessions()` | GET /stream/sessions | refetchInterval: 5000 |
| `useStartTranscode()` | POST /stream/{id}/transcode | Mutation |
| `useStopStreamSession()` | DELETE /stream/sessions/{id} | Mutation |
| **Freehand Search** | | |
| `useSearchReleases(query, indexerIds?)` | GET /search | Enabled when query non-empty |
| **Plex** | | |
| `useWatchlist()` | GET /plex/watchlist | |
| `useSyncWatchlist()` | POST /plex/watchlist/sync | Mutation |
| `usePlexServers()` | GET /plex/servers | |
| `useAddPlexServer()` | POST /plex/servers | Mutation, invalidates plex servers |
| `useUpdatePlexServer()` | PUT /plex/servers/{id} | Mutation, invalidates plex servers |
| `useDeletePlexServer()` | DELETE /plex/servers/{id} | Mutation, invalidates plex servers |
| `usePlexLibraries(serverId)` | GET /plex/servers/{id}/libraries | Enabled when serverId > 0 |
| `useTogglePlexLibrary()` | PUT /plex/libraries/{id} | Mutation, invalidates plex libraries |
| `usePlexFullScan()` | POST /plex/scan/full | Mutation |
| `usePlexRecentScan()` | POST /plex/scan/recent | Mutation |
| `useValidatePlexToken()` | POST /plex/auth/validate | Mutation, returns `{ valid, user }` |
| `useDiscoverPlexServers()` | POST /plex/auth/servers | Mutation, returns `PlexResource[]` |
| **Media Requests** | | |
| `useMediaRequests(status?)` | GET /requests | refetchInterval: 15000 |
| `usePendingRequestCount()` | GET /requests/pending/count | refetchInterval: 30000 |
| `useApproveRequest()` | PUT /requests/{id}/approve | Mutation, invalidates requests |
| `useDeclineRequest()` | PUT /requests/{id}/decline | Mutation, invalidates requests |
| `useDeleteRequest()` | DELETE /requests/{id} | Mutation, invalidates requests |
| **Activities** | | |
| `useActivities(enabled?)` | GET /activities | refetchInterval: 5000 (when enabled) |
| `useRunningActivityCount()` | GET /activities/running | refetchInterval: 10000 |
| **Notifications** | | |
| `useNotifications(enabled?)` | GET /user/notifications | Enabled flag controls fetching |
| `useUnreadNotificationCount()` | GET /user/notifications/unread-count | refetchInterval: 30000 |
| `useMarkNotificationRead()` | PUT /user/notifications/{id}/read | Mutation, invalidates notifications |
| `useMarkAllNotificationsRead()` | PUT /user/notifications/read-all | Mutation, invalidates notifications |
| **Event Stream** | | |
| `useEventStream(enabled?)` | GET /history/stream | refetchInterval: 5000 (when enabled), limit=30 |
| **User** | | |
| `useCurrentUser()` | GET /auth/me | staleTime: 5 min |

## Routing

Defined in `App.tsx`. The app shows a loading spinner while checking system status, offers `ServerConnect` when the API is unreachable, and redirects to `FirstBoot` when `firstBoot` is `true`.

```typescript
<Routes>
    {/* First boot redirects to setup wizard */}
    <Route path="/setup" element={<FirstBoot />} />
    {firstBoot && <Route path="*" element={<Navigate to="/setup" replace />} />}

    {/* Main app routes */}
    <Route element={<Layout />}>
        <Route path="/discover" element={<Discover />} />
        <Route path="/series" element={<SeriesList />} />
        <Route path="/series/:id" element={<SeriesDetail />} />
        <Route path="/movies" element={<MovieList />} />
        <Route path="/movies/:id" element={<MovieDetail />} />
        <Route path="/calendar" element={<Calendar />} />
        <Route path="/search" element={<Search />} />
        <Route path="/queue" element={<Queue />} />
        <Route path="/torrents" element={<Torrents />} />
        <Route path="/usenet" element={<Usenet />} />
        <Route path="/history" element={<History />} />
        <Route path="/wanted/missing" element={<Wanted />} />
        <Route path="/watchlist" element={<Watchlist />} />
        <Route path="/play/:mediaFileId" element={<Player />} />
        <Route path="/streaming" element={<Streaming />} />
        <Route path="/requests" element={<Requests />} />
        <Route path="/users" element={<Users />} />
        <Route path="/settings" element={<Settings />} />
        <Route path="/migrate" element={<Navigate to="/settings" replace />} />
        <Route path="/" element={<Navigate to="/discover" replace />} />
        <Route path="*" element={<Navigate to="/discover" replace />} />
    </Route>
</Routes>
```

## Module Gating

Navigation items in `Sidebar.tsx` are defined as a typed array of `NavItem` objects with an optional `gate` function that receives `EnabledModules`. Items without a gate are always visible. Items are filtered via `navItems.filter(item => !item.gate || !modules || item.gate(modules))`.

| Route | Label | Gate |
|-------|-------|------|
| `/discover` | Discover | _always visible_ |
| `/series` | Series | `tvManagement` |
| `/movies` | Movies | `movieManagement` |
| `/search` | Search | `externalIndexers \|\| indexarrSidecar` |
| `/calendar` | Calendar | _always visible_ |
| `/queue` | Queue | _always visible_ |
| `/torrents` | Torrents | `torrentEmbedded` |
| `/usenet` | Usenet | `usenetEmbedded` |
| `/streaming` | Streaming | `streaming` |
| `/history` | History | _always visible_ |
| `/wanted/missing` | Wanted | _always visible_ |
| `/watchlist` | Watchlist | `plexIntegration` |
| `/requests` | Requests | _always visible_ |
| `/users` | Users | _always visible_ |
| `/settings` | Settings | _always visible_ |

The **Requests** nav item shows a yellow pending-count badge sourced from `usePendingRequestCount()` (polls every 30s).

## Styling Patterns

**Dark theme** with slate color palette:

| Element | Classes |
|---------|---------|
| Page background | `bg-slate-900` |
| Card/container | `bg-slate-800 rounded-lg` |
| Hover state | `hover:bg-slate-700` |
| Primary text | `text-white` |
| Secondary text | `text-slate-400` |
| Border | `border-slate-700` |
| Primary accent | `bg-blue-600 hover:bg-blue-700` |
| Danger | `bg-red-600 hover:bg-red-700` |

**Status badges**:
```typescript
const colors: Record<string, string> = {
    downloading: 'bg-blue-500/20 text-blue-400',
    paused:      'bg-yellow-500/20 text-yellow-400',
    completed:   'bg-green-500/20 text-green-400',
    failed:      'bg-red-500/20 text-red-400',
}
```

**Grid layouts**: `grid grid-cols-2 sm:grid-cols-3 md:grid-cols-4 lg:grid-cols-5 gap-4`

**Modals**: Fixed overlay with centered card
```tsx
<div className="fixed inset-0 z-50 bg-black/60 flex items-center justify-center">
    <div className="rounded-xl bg-slate-800 shadow-2xl max-w-lg w-full p-6">
        {/* content */}
    </div>
</div>
```

## Component Patterns

### Loading/Error States
```tsx
if (isLoading) return <div className="flex justify-center p-8"><Loader2 className="animate-spin" /></div>
if (error) return <div className="text-red-400 p-4">Error: {error.message}</div>
if (!data?.length) return <EmptyState message="No series found" />
```

### Tables
```tsx
<table className="w-full text-sm">
    <thead>
        <tr className="border-b border-slate-700 text-left text-slate-400">
            <th className="px-4 py-2">Title</th>
            ...
        </tr>
    </thead>
    <tbody>
        {data.map(item => (
            <tr key={item.id} className="border-b border-slate-700/50 hover:bg-slate-700/30">
                <td className="px-4 py-2">{item.title}</td>
                ...
            </tr>
        ))}
    </tbody>
</table>
```

### Forms
Direct `fetch()` for file uploads (migration page). `useMutation` + `JSON.stringify` for JSON forms. No form library — controlled components with `useState`.

## Key Pages

### FirstBoot (Setup Wizard)
Multi-step flow:
1. **Admin account creation** — `POST /api/v1/auth/setup` (username, password, displayName). Replaces the legacy auto-generated API key approach.
2. Select features (TV, Movies, Torrent, Usenet, Plex, Streaming, etc.)
3. Optional migration import
4. Indexarr sidecar config
5. Media library folder selection
6. Completion

The setup screen is gated by `GET /api/v1/auth/status` returning `setupRequired: true`.

### Discover
Landing page with horizontal carousels (MediaSlider + MediaCard). Configurable sliders: Trending, Popular Movies/TV, Upcoming, Genre-specific. Each card shows poster, rating badge, media type, and an "add to library" button.

### Settings
13 tabs across two groups (**Settings**: General, Modules, Quality Profiles, Custom Formats, Indexers, Download Clients, Naming, Media Folders, Tags, Plex, Remote Access; **Data**: Backup / Restore, Migration). Each tab has CRUD forms. Sidebar navigation groups tabs by category.

#### Custom Formats Tab
Custom format management via direct `fetch()` against `/api/v1/customformat`. No `useApi.ts` hooks -- uses local `useState` + `useCallback` for loading, editing, and CRUD state.

**List view** -- responsive card grid (`grid-cols-1 sm:grid-cols-2 lg:grid-cols-3`). Each card shows the format name, condition count (e.g. "3 conditions"), and a delete button. Clicking a card opens the edit form.

**Create/Edit form** -- fields:
- **Name** (text input) and **Include in renaming** (toggle).
- **Conditions** -- dynamic list of `FormatSpecification` rows. Each row has:
  - Field type selector (Release Title, Quality, Language, Release Group, Indexer Flag, Size).
  - Pattern input -- for most fields, a regex text input with monospace font. For the `size` field, two numeric inputs (Min GB / Max GB) that store the value in bytes as a `min-max` string pattern.
  - **Negate** checkbox -- inverts the match.
  - **Required** checkbox -- condition must match (AND logic vs OR).
  - Remove button per row.
- Validation: name required, at least one condition required.

**Live test** -- separated by a border below the conditions. Enter a release title, click Test (sends `POST /api/v1/customformat/test` with `{ releaseTitle, specifications }`), and see a green "Matched" or red "No match" result inline.

**API endpoints**:
- `GET /api/v1/customformat` -- list all custom formats.
- `POST /api/v1/customformat` -- create a new custom format.
- `PUT /api/v1/customformat/{id}` -- update an existing custom format.
- `DELETE /api/v1/customformat/{id}` -- delete a custom format.
- `POST /api/v1/customformat/test` -- test specifications against a release title.

#### Quality Profiles Tab (Enhanced)
Quality profile editor with expanded inline editing. Profiles are grouped by media type (Series, Movie, Any) with a summary table showing name, type badge, cutoff, and item count. Clicking a row expands inline editing.

**Expanded editor fields**:
- **Row 1** (4-column grid): Name, Cutoff (numeric), Media Type (Any/Series/Movies select), Language (select: Any, Original, English, French, Spanish, German, Italian, Portuguese, Japanese, Korean, Chinese, Russian).
- **Row 2** (flex wrap): Upgrade Allowed (toggle), Min Format Score (numeric), Cutoff Format Score (numeric), Min Upgrade Score (numeric).
- **Qualities** -- checkbox list of all quality levels with allowed/disallowed toggle per item.
- **Custom Format Scores** -- shown when `formatItems` is non-empty. Scrollable table (max-height 256px) with sticky header listing each custom format name and an editable numeric score input. Scores are persisted as `ProfileFormatItem[]` in the `formatItems` array on the `PUT /api/v1/qualityprofile/{id}` payload.

### SeriesDetail
Poster + metadata header, stat badges (episode count, file count, size), collapsible season sections with episode rows showing air date, quality badge, monitored toggle, search button.

### Player
HLS.js-based video player page at `/play/:mediaFileId`. Auto-detects codec compatibility via `MediaSource.isTypeSupported()` — uses direct play for compatible codecs (h264/aac), falls back to transcoding for others. Subtitle track selection.

### Search
Freehand indexer search at `/search`. Features:
- **Indexer filter dropdown** — multi-select from enabled indexers with "All Indexers" default. When Indexarr sidecar is enabled, it is shown as "always active" in the dropdown. Selected indexers display as removable chip badges below the search bar.
- **URL parameter support** — reads initial query from `?q=` search param.
- **Result table** — columns: Title, Indexer, Type (protocol badge), Size, Age, Peers (seeders/leechers for torrents), Links (info page + download/magnet).
- **Protocol badges** — orange `Torrent` or purple `Usenet` rounded badges.
- Uses `useSearchReleases(query, indexerIds?)` hook, `useIndexers()` for the dropdown, and `useSystemStatus()` to detect Indexarr.

### InteractiveSearchModal
Reusable search modal component used by `SeriesDetail`, `MovieDetail`, and `Wanted` pages. Searches for releases via `useInteractiveSearch()` and displays results as `DownloadDecision[]` in a sortable table.

- **Sortable columns** — Title, Indexer, Protocol, Size, Age, Seeders, CF Score. Clicking a column header toggles ascending/descending sort. Default sort: size descending.
- **Rejected releases** — collapsed by default, toggled via a "Show Rejected" button. Rejected rows display rejection reasons with warning icons.
- **Grab action** — download button per row (uses `useGrabRelease()` mutation). Grabbed releases show a green checkmark.
- **Custom Format Score column** — numeric score displayed with color: green for positive, red for negative, muted gray for zero.
- **Matched format badges** — below the CF score, matched custom formats are rendered as small colored badges (`text-[9px]`). Badge colors: green (`bg-green-500/15 text-green-400`) for positive score, red (`bg-red-500/15 text-red-400`) for negative, gray (`bg-slate-500/15 text-slate-400`) for zero. Each badge has a `title` tooltip showing the format name and its individual score (e.g. "HEVC: +10").

### Requests
Media request management at `/requests`. Admin interface for handling user media requests:
- **Status tab bar** — All, Pending, Approved, Declined, Available. Filters via `useMediaRequests(status?)`.
- **Request cards** — each shows poster thumbnail (TMDB), title, year, media type badge (TV/Movie), status badge (color-coded: yellow=pending, green=approved, red=declined, blue=available), overview excerpt, request date, and admin note if present.
- **Actions** — Pending requests show Approve, Decline, Note (with inline text input), and Delete buttons. Non-pending show only Delete.
- **Pending count badge** — shown in the page header when viewing non-pending filters.
- Uses `useApproveRequest()`, `useDeclineRequest()`, `useDeleteRequest()` mutations.

### Users
Admin user management at `/users`. Two sections:
- **Users table** — displays avatar initial, display name, username, role badge (admin=amber, user=blue with Shield/User icons), enabled status, creation date, and delete action.
- **Invite Codes table** — shows code (monospace), role, claimed status, expiration date, copy-to-clipboard button (uses `utils/clipboard.ts`), and delete action.
- **Create User modal** — form with username, display name, password, and role (user/admin) select.
- **Create Invite modal** — form with role select and optional expiry in hours.
- Uses inline `useQuery`/`useMutation` calls against `/admin/users` and `/admin/invites` endpoints (not in `useApi.ts`).

### ServerConnect
Remote server connection page rendered by `App.tsx` when the API is unreachable. Props: `onConnected` callback. Two connection modes:
- **Claim Code** — enter a client name and 4-character claim code (uppercased). Advanced section allows overriding the bootstrap URL (default: `https://streambootstrap.indexarr.net`). Uses `redeemClaimCode()` from `api/client.ts`.
- **Direct URL** — enter server URL and API key/client token manually. Validates by fetching `/api/v1/system/status` with a 5s timeout, then saves the connection via `saveConnection()`.

## Activity & Notification System

The activity/notification system provides a unified bell icon in the header for monitoring server events, background tasks, and user notifications.

### ActivityNotificationBell
Renders a `Bell` icon button in the `Layout` header. Displays a combined badge count of running activities + unread notifications (capped at "99+"). Clicking toggles the `ActivityNotificationPopup`. Closes on outside click.

**Data sources** (all from `useApi.ts`):
- `useRunningActivityCount()` — polls every 10s for running task count.
- `useUnreadNotificationCount()` — polls every 30s for unread notification count.
- `useActivities(open)` — fetches full activity list only when popup is open (polls 5s).
- `useEventStream(open)` — fetches event stream only when popup is open (polls 5s).
- `useNotifications(open)` — fetches notifications only when popup is open.

### ActivityNotificationPopup
Fixed-position dropdown (400px wide) with three tabs: **Events**, **Activity**, **Notifications**. Each tab shows its own badge count — Activity shows running count (blue), Notifications shows unread count (red). Renders the corresponding tab component.

### ActivityTab
Displays `SystemActivity[]` items. Each activity shows:
- **Icon** with color-coded background: blue (running), green (completed), red (failed).
- **Running spinner** — animated border ring around the icon for running tasks.
- **Progress bar** — based on `progress.folders_done / progress.folders_total`. Green fill for completed, red for failed.
- **Meta row** — status label with icon (Loader2 spinning for running, Check for completed, X for failed) and relative timestamp.
- Max height 380px with scroll overflow.

### EventsTab
Displays `HistoryEvent[]` items as a compact stream. Each event shows:
- **Type icon** — Download (grabbed), Upload (imported), Trash2 (fileDeleted), ArrowUpCircle (upgraded), XCircle (downloadFailed), FileText (fileRenamed), Eye (downloadIgnored).
- **Color-coded label** — Grabbed (blue), Imported (green), Upgraded (cyan/orange), Failed (red), Renamed (purple), Ignored (yellow).
- **Quality badge** — blue rounded badge showing quality name (via `qualityName()` utility).
- **Upgrade context** — for upgrade/delete events, shows replacement quality and whether the file was recycled or permanently deleted.
- **Source title** and relative timestamp.

### NotificationTab
Displays `UserNotification[]` items. Features:
- **"Mark all read" button** — shown at the top when any unread notifications exist.
- **Unread indicator** — blue dot and subtle blue background tint for unread items. Clicking an unread notification marks it as read.
- Each notification shows title (bold when unread), optional body text, and relative timestamp.

## Utilities

### `utils/date.ts`
Date formatting helpers:
- `formatDate(value)` — locale date string (e.g. "3/27/2026")
- `formatDateTime(value)` — locale date + time (e.g. "3/27/2026 . 14:30")
- `formatTime(value)` — locale time only (e.g. "14:30")
- `formatAirDate(dateStr)` — parses YYYY-MM-DD as local midnight (not UTC), formatted with month/day/year (e.g. "Mar 27, 2026")

### `utils/clipboard.ts`
`copyToClipboard(text)` — async function that attempts `navigator.clipboard.writeText()` first, falling back to a legacy textarea + `document.execCommand('copy')` approach for non-HTTPS contexts. Returns `Promise<boolean>`.

## Types

All in `api/types.ts`. Key interfaces:

- **System**: `SystemStatus`, `EnabledModules`, `CurrentUser`
- **Media**: `Series`, `Episode`, `Movie`, `MediaFile`, `MediaStreamInfo`
- **Config**: `QualityProfile` (with `upgradeAllowed`, `minFormatScore`, `cutoffFormatScore`, `minUpgradeFormatScore`, `language`, `formatItems`), `QualityProfileItem`, `ProfileFormatItem`, `IndexerConfig`, `AvailableIndexer`, `AvailableSetting`, `DownloadClientConfig`, `NamingConfig`, `Tag`, `MediaLibraryFolder`
- **Custom Formats**: `CustomFormat`, `FormatSpecification`, `FormatField` (union: `releaseName | quality | language | releaseGroup | indexerFlag | size`), `MatchedFormat`
- **Operations**: `QueueItem`, `HistoryEvent`, `HistoryResponse`, `CalendarEntry`, `ReleaseInfo`, `DownloadDecision` (with `customFormatScore`, `matchedFormats`), `FreehandSearchResult`
- **Activities & Notifications**: `SystemActivity`, `UserNotification`
- **Streaming**: `StreamSession`, `TranscodeRequest`, `TranscodeResponse`, `VideoStreamInfo`, `AudioStreamInfo`, `SubtitleStreamInfo`
- **Discover**: `TmdbTrendingItem`, `TmdbMovie`, `TmdbSeries`, `TmdbGenre`, `DiscoverSlider`, `WatchlistItem`
- **Plex**: `PlexServer`, `PlexLibrary`, `PlexTvUser`, `PlexResource`, `PlexConnection`
- **Media Requests**: `MediaRequest`
- **Remote Access**: `ClaimCodeResponse`, `RemoteClient`
- **Lookup**: `SeriesLookup`, `MovieLookup`
- **Setup**: `SetupInit`, `MigrationResult`

**Utility functions** (also in `types.ts`):
- `qualityName(quality)` — human-readable quality string from raw string or JSONB object
- `tmdbPosterUrl(path, size?)` / `tmdbBackdropUrl(path, size?)` — TMDB image URLs proxied through `/api/v1/images/`
- `tmdbDisplayTitle(item)` — resolve title vs name for movie/TV
- `tmdbYear(item)` — extract year from release_date or first_air_date
