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
    │   ├── Layout.tsx      # Main layout wrapper with sidebar
    │   └── Sidebar.tsx     # Navigation with collapsible menu
    └── pages/
        ├── SeriesList.tsx  # Grid view with search + add modal
        ├── SeriesDetail.tsx# Detail page with season accordion
        ├── MovieList.tsx   # Movie grid view
        ├── MovieDetail.tsx # Movie detail page
        ├── Calendar.tsx    # Upcoming episodes grouped by date
        ├── Queue.tsx       # Download queue table (5s auto-refresh)
        ├── History.tsx     # Paginated history table
        ├── Wanted.tsx      # Missing/cutoff tabs with pagination
        ├── Torrents.tsx    # Torrent engine management
        ├── Usenet.tsx      # Usenet engine management
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

`api/client.ts`:
```typescript
const API_BASE = '/api/v1'

export async function apiFetch<T>(path: string, options?: RequestInit): Promise<T> {
    const res = await fetch(`${API_BASE}${path}`, {
        headers: { 'Content-Type': 'application/json' },
        ...options,
    })
    if (!res.ok) throw new Error(`API error: ${res.status}`)
    return res.json()
}
```

In development, Vite proxies `/api` to `http://localhost:8989` (configured in `vite.config.ts`).

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
| `useSystemStatus()` | GET /system/status | Gates first-boot vs main app |
| `useSeries()` | GET /series | |
| `useSeriesDetail(id)` | GET /series/{id} | |
| `useEpisodes(seriesId)` | GET /series/{id}/episodes | enabled when seriesId is set |
| `useSeriesLookup(term)` | GET /series/lookup | TMDB search |
| `useMovies()` | GET /movies | |
| `useMovieDetail(id)` | GET /movies/{id} | |
| `useQueue()` | GET /queue | refetchInterval: 5000 |
| `useHistory(page)` | GET /history | paginated |
| `useCalendar(start, end)` | GET /calendar | date range |
| `useQualityProfiles()` | GET /qualityprofile | |
| `useIndexers()` | GET /indexer | |
| `useDownloadClients()` | GET /downloadclient | |
| `useNamingConfig()` | GET /naming | |
| `useMediaLibraryFolders()` | GET /medialibraryfolder | |
| `useTags()` | GET /tag | |
| `useSetupInit()` | POST /system/setup | mutation |
| `useMigrate()` | POST /system/migrate | multipart mutation |

## Routing

Defined in `App.tsx`:

```typescript
<Routes>
    {/* First boot redirects to setup wizard */}
    {systemStatus?.firstBoot && <Route path="*" element={<Navigate to="/setup" />} />}
    <Route path="/setup" element={<FirstBoot />} />

    {/* Main app routes */}
    <Route element={<Layout />}>
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
        <Route path="/settings" element={<Settings />} />
        <Route path="/migrate" element={<Migrate />} />
        <Route path="/" element={<Navigate to="/series" />} />
    </Route>
</Routes>
```

## Module Gating

Navigation items in `Sidebar.tsx` are filtered based on `EnabledModules`:

```typescript
// Only show "Torrents" nav item if torrentEmbedded is enabled
{modules.torrentEmbedded && <NavLink to="/torrents">Torrents</NavLink>}
{modules.usenetEmbedded && <NavLink to="/usenet">Usenet</NavLink>}
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
1. Select features (TV, Movies, Torrent, Usenet)
2. Optional migration import
3. Indexarr sidecar config
4. Media library folder selection
5. Completion

### Settings
7 tabs: General, Quality Profiles, Indexers, Download Clients, Naming, Media Folders, Tags. Each tab has CRUD forms.

### SeriesDetail
Poster + metadata header, stat badges (episode count, file count, size), collapsible season sections with episode rows showing air date, quality badge, monitored toggle, search button.

## Types

All in `api/types.ts`. Key interfaces:

- `SystemStatus` — version, instanceName, firstBoot, modules
- `EnabledModules` — boolean flags for each feature
- `Series` — full series entity with computed fields (seasonCount, episodeCount, episodeFileCount)
- `Episode` — episode with quality and file info
- `Movie` — full movie entity
- `QueueItem` — download in progress
- `HistoryEvent` — history record
- `CalendarEntry` — upcoming episode
- `QualityProfile` — quality config
- `IndexerConfig` — indexer settings
- `DownloadClientConfig` — client settings
- `MediaLibraryFolder` — storage path
- `SetupInit` — first boot request
- `MigrationResult` — import results
