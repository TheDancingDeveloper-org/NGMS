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
    │   ├── Layout.tsx      # Main layout wrapper with sidebar + header
    │   ├── Sidebar.tsx     # Navigation with collapsible menu + module gating
    │   ├── MediaCard.tsx   # Reusable TMDB media card (poster, rating, add button)
    │   ├── MediaSlider.tsx # Horizontal scrollable carousel with chevron nav
    │   └── VideoPlayer.tsx # HLS video player with direct play / transcode fallback
    └── pages/
        ├── Discover.tsx    # Trending/popular/upcoming content from TMDB
        ├── SeriesList.tsx  # Grid view with search + add modal
        ├── SeriesDetail.tsx# Detail page with season accordion
        ├── MovieList.tsx   # Movie grid view
        ├── MovieDetail.tsx # Movie detail page
        ├── Calendar.tsx    # Upcoming episodes grouped by date
        ├── Queue.tsx       # Download queue table (5s auto-refresh)
        ├── History.tsx     # Paginated history table
        ├── Wanted.tsx      # Missing/cutoff tabs with pagination
        ├── Watchlist.tsx   # Plex watchlist items
        ├── Torrents.tsx    # Torrent engine management
        ├── Usenet.tsx      # Usenet engine management
        ├── Player.tsx      # HLS video player page
        ├── Streaming.tsx   # Active stream sessions (5s auto-refresh)
        ├── Settings.tsx    # 7-tab settings page
        ├── Migrate.tsx     # *arr database import wizard
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

The `ServerConnect` page provides three connection modes:

1. **Invite Code** — Enter an 8-char unified code. Bootstrap resolves the server, then redirects to `RegisterPage` with the invite code pre-filled for account creation.
2. **Sign In** — Enter a server name + existing credentials. Bootstrap resolves the server name to connection details via `GET /api/v1/servers/by-name/{name}`, then logs in directly. No admin involvement required.
3. **Direct URL** — Enter a server URL manually (for LAN or non-bootstrap setups).

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
| `useAddSeries()` | POST /series | Mutation, invalidates series |
| `useDeleteSeries()` | DELETE /series/{id} | Mutation |
| `useAddMovie()` | POST /movies | Mutation, invalidates movies |
| `useDeleteMovie()` | DELETE /movies/{id} | Mutation |
| `useToggleSeriesMonitor()` | PUT /series/{id} | Mutation |
| `useToggleEpisodeMonitor()` | PUT /episode/{id} | Mutation |
| `useSearchEpisode()` | POST /release | Mutation |
| `useSearchMovie()` | POST /release | Mutation |
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
| **Plex** | | |
| `useWatchlist()` | GET /plex/watchlist | |
| `useSyncWatchlist()` | POST /plex/watchlist/sync | Mutation |

## Routing

Defined in `App.tsx`:

```typescript
<Routes>
    {/* First boot redirects to setup wizard */}
    {systemStatus?.firstBoot && <Route path="*" element={<Navigate to="/setup" />} />}
    <Route path="/setup" element={<FirstBoot />} />

    {/* Main app routes */}
    <Route element={<Layout />}>
        <Route path="/discover" element={<Discover />} />
        <Route path="/series" element={<SeriesList />} />
        <Route path="/series/:id" element={<SeriesDetail />} />
        <Route path="/movies" element={<MovieList />} />
        <Route path="/movies/:id" element={<MovieDetail />} />
        <Route path="/calendar" element={<Calendar />} />
        <Route path="/queue" element={<Queue />} />
        <Route path="/torrents" element={<Torrents />} />
        <Route path="/usenet" element={<Usenet />} />
        <Route path="/history" element={<History />} />
        <Route path="/wanted/missing" element={<Wanted />} />
        <Route path="/watchlist" element={<Watchlist />} />
        <Route path="/play/:mediaFileId" element={<Player />} />
        <Route path="/streaming" element={<Streaming />} />
        <Route path="/settings" element={<Settings />} />
        <Route path="/migrate" element={<Migrate />} />
        <Route path="/" element={<Navigate to="/discover" />} />
    </Route>
</Routes>
```

## Module Gating

Navigation items in `Sidebar.tsx` are filtered based on `EnabledModules`:

```typescript
// Only show nav items if the corresponding module is enabled
{modules.torrentEmbedded && <NavLink to="/torrents">Torrents</NavLink>}
{modules.usenetEmbedded && <NavLink to="/usenet">Usenet</NavLink>}
{modules.plexIntegration && <NavLink to="/watchlist">Watchlist</NavLink>}
{modules.streaming && <NavLink to="/streaming">Streaming</NavLink>}
```

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
7 tabs: General, Quality Profiles, Indexers, Download Clients, Naming, Media Folders, Tags. Each tab has CRUD forms.

### SeriesDetail
Poster + metadata header, stat badges (episode count, file count, size), collapsible season sections with episode rows showing air date, quality badge, monitored toggle, search button.

### Player
HLS.js-based video player page at `/play/:mediaFileId`. Auto-detects codec compatibility via `MediaSource.isTypeSupported()` — uses direct play for compatible codecs (h264/aac), falls back to transcoding for others. Subtitle track selection.

## Types

All in `api/types.ts`. Key interfaces:

- **System**: `SystemStatus`, `EnabledModules`
- **Media**: `Series`, `Episode`, `Movie`, `MediaFile`, `MediaStreamInfo`
- **Config**: `QualityProfile`, `IndexerConfig`, `DownloadClientConfig`, `NamingConfig`, `Tag`, `MediaLibraryFolder`
- **Operations**: `QueueItem`, `HistoryEvent`, `CalendarEntry`, `ReleaseInfo`
- **Streaming**: `StreamSession`, `TranscodeRequest`, `TranscodeResponse`, `VideoStreamInfo`, `AudioStreamInfo`, `SubtitleStreamInfo`
- **Discover**: `TmdbTrendingItem`, `TmdbMovie`, `TmdbSeries`, `TmdbGenre`, `DiscoverSlider`, `WatchlistItem`
- **Setup**: `SetupInit`, `MigrationResult`

**Utility functions** (also in `types.ts`):
- `qualityName(quality)` — human-readable quality string
- `tmdbPosterUrl(path)` / `tmdbBackdropUrl(path)` — TMDB image URLs
- `tmdbDisplayTitle(item)` — resolve title vs name for movie/TV
- `tmdbYear(item)` — extract year from release_date or first_air_date
