import { test as base, type Page, type Route } from '@playwright/test'
import type {
  SystemStatus,
  EnabledModules,
  CurrentUser,
  Series,
  Movie,
  QueueItem,
  CalendarEntry,
  QualityProfile,
  MediaLibraryFolder,
  HistoryResponse,
  TmdbSearchResults,
  TmdbTrendingItem,
  TmdbMovie,
  TmdbSeries,
} from '../src/api/types'

// ─── Default mock data ──────────────────────────────────────────

const defaultModules: EnabledModules = {
  tvManagement: true,
  movieManagement: true,
  torrentEmbedded: true,
  usenetEmbedded: false,
  torrentExternal: false,
  usenetExternal: false,
  indexarrSidecar: true,
  externalIndexers: true,
  plexIntegration: false,
  notifications: true,
  streaming: false,
  remoteAccess: false,
  stremioAddon: false,
}

export const mockStatus: SystemStatus = {
  version: '0.1.0-test',
  instanceName: 'Test NGMS',
  firstBoot: false,
  modules: defaultModules,
  indexarrAvailable: true,
  startTime: '2026-03-30T00:00:00Z',
}

export const mockUser: CurrentUser = {
  id: 1,
  username: 'admin',
  displayName: 'Admin',
  role: 'admin',
  avatarUrl: null,
}

export const mockSeries: Series[] = [
  {
    id: 1,
    title: 'Breaking Bad',
    sortTitle: 'breaking bad',
    status: 'ended',
    overview: 'A chemistry teacher turns to manufacturing methamphetamine.',
    network: 'AMC',
    year: 2008,
    seasonCount: 5,
    episodeCount: 62,
    episodeFileCount: 62,
    totalEpisodeCount: 62,
    monitored: true,
    qualityProfileId: 1,
    mediaLibraryFolderPath: '/tv',
    path: '/tv/Breaking Bad',
    posterUrl: '',
    fanartUrl: '',
    added: '2026-01-01T00:00:00Z',
    tags: [],
    tmdbId: 1396,
  },
  {
    id: 2,
    title: 'The Office',
    sortTitle: 'office',
    status: 'ended',
    overview: 'A mockumentary sitcom about office workers.',
    network: 'NBC',
    year: 2005,
    seasonCount: 9,
    episodeCount: 201,
    episodeFileCount: 180,
    totalEpisodeCount: 201,
    monitored: true,
    qualityProfileId: 1,
    mediaLibraryFolderPath: '/tv',
    path: '/tv/The Office',
    posterUrl: '',
    fanartUrl: '',
    added: '2026-01-15T00:00:00Z',
    tags: [],
    tmdbId: 2316,
  },
]

export const mockMovies: Movie[] = [
  {
    id: 1,
    title: 'Inception',
    sortTitle: 'inception',
    status: 'released',
    overview: 'A thief who steals secrets through dream-sharing technology.',
    studio: 'Warner Bros.',
    year: 2010,
    monitored: true,
    qualityProfileId: 1,
    mediaLibraryFolderPath: '/movies',
    path: '/movies/Inception (2010)',
    posterUrl: '',
    fanartUrl: '',
    hasFile: true,
    added: '2026-02-01T00:00:00Z',
    tags: [],
    tmdbId: 27205,
  },
]

export const mockQueue: QueueItem[] = [
  {
    id: 1,
    title: 'Breaking.Bad.S01E01.720p.BluRay.x264',
    status: 'downloading',
    progress: 45.2,
    size: 1_500_000_000,
    sizeLeft: 822_000_000,
    estimatedCompletionTime: '2026-03-30T02:00:00Z',
    downloadClient: 'librtbit',
    mediaType: 'series',
    seriesId: 1,
    quality: 'Bluray-720p',
  },
]

export const mockCalendar: CalendarEntry[] = [
  {
    episodeId: 101,
    seriesId: 1,
    seriesTitle: 'Breaking Bad',
    seasonNumber: 1,
    episodeNumber: 1,
    episodeTitle: 'Pilot',
    airDateUtc: '2026-03-30T02:00:00Z',
    monitored: true,
    hasFile: false,
    posterUrl: null,
  },
]

export const mockProfiles: QualityProfile[] = [
  {
    id: 1,
    name: 'HD-1080p',
    cutoff: 7,
    items: [],
    mediaType: null,
  },
]

export const mockFolders: MediaLibraryFolder[] = [
  { id: 1, path: '/tv', freeSpace: 500_000_000_000, totalSpace: 1_000_000_000_000, mediaType: 'tv' },
  { id: 2, path: '/movies', freeSpace: 500_000_000_000, totalSpace: 1_000_000_000_000, mediaType: 'movie' },
]

export const mockHistory: HistoryResponse = {
  page: 1,
  pageSize: 20,
  totalRecords: 0,
  records: [],
}

const mockTrending: TmdbSearchResults<TmdbTrendingItem> = {
  page: 1,
  total_pages: 1,
  total_results: 1,
  results: [
    {
      id: 27205,
      media_type: 'movie',
      title: 'Inception',
      overview: 'A thief who steals secrets through dream-sharing technology.',
      release_date: '2010-07-16',
      poster_path: '/poster.jpg',
      backdrop_path: '/backdrop.jpg',
      genre_ids: [28, 878],
      vote_average: 8.4,
      vote_count: 30000,
      popularity: 100,
    },
  ],
}

const mockPopularMovies: TmdbSearchResults<TmdbMovie> = {
  page: 1,
  total_pages: 1,
  total_results: 1,
  results: [
    {
      id: 27205,
      title: 'Inception',
      overview: 'Dream heist.',
      release_date: '2010-07-16',
      poster_path: '/poster.jpg',
      backdrop_path: '/backdrop.jpg',
      genre_ids: [28],
      vote_average: 8.4,
      vote_count: 30000,
      popularity: 100,
    },
  ],
}

const mockPopularTv: TmdbSearchResults<TmdbSeries> = {
  page: 1,
  total_pages: 1,
  total_results: 1,
  results: [
    {
      id: 1396,
      name: 'Breaking Bad',
      overview: 'Chemistry teacher turns criminal.',
      first_air_date: '2008-01-20',
      poster_path: '/poster.jpg',
      backdrop_path: '/backdrop.jpg',
      genre_ids: [18],
      vote_average: 8.9,
      vote_count: 10000,
      popularity: 80,
    },
  ],
}

// ─── API mock helper ────────────────────────────────────────────

type ApiOverrides = {
  status?: Partial<SystemStatus>
  series?: Series[]
  movies?: Movie[]
  queue?: QueueItem[]
  calendar?: CalendarEntry[]
  profiles?: QualityProfile[]
  folders?: MediaLibraryFolder[]
  history?: HistoryResponse
}

/** Install API route mocks on a page. Intercepts /api/v1/* and returns mock JSON.
 *
 * IMPORTANT: Playwright matches routes in LIFO order (last registered wins).
 * The catch-all must be registered FIRST so specific routes take priority.
 */
export async function mockApi(page: Page, overrides: ApiOverrides = {}) {
  const status = { ...mockStatus, ...overrides.status }
  const series = overrides.series ?? mockSeries
  const movies = overrides.movies ?? mockMovies
  const queue = overrides.queue ?? mockQueue
  const calendar = overrides.calendar ?? mockCalendar
  const profiles = overrides.profiles ?? mockProfiles
  const folders = overrides.folders ?? mockFolders
  const history = overrides.history ?? mockHistory

  const json = (route: Route, body: unknown) =>
    route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify(body) })

  // Catch-all FIRST (lowest priority) — any unmatched /api/ request returns empty JSON
  await page.route('**/api/v1/**', (route) => json(route, []))

  // Image proxy — return a 1x1 transparent PNG
  await page.route('**/api/v1/images/**', (route) =>
    route.fulfill({
      status: 200,
      contentType: 'image/png',
      body: Buffer.from('iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNk+M9QDwADhgGAWjR9awAAAABJRU5ErkJggg==', 'base64'),
    }),
  )

  // Discover / TMDB endpoints
  await page.route('**/api/v1/discover/trending**', (route) => json(route, mockTrending))
  await page.route('**/api/v1/discover/movie/popular**', (route) => json(route, mockPopularMovies))
  await page.route('**/api/v1/discover/tv/popular**', (route) => json(route, mockPopularTv))
  await page.route('**/api/v1/discover/movie/upcoming**', (route) => json(route, mockPopularMovies))
  await page.route('**/api/v1/discover/tv/upcoming**', (route) => json(route, mockPopularTv))

  // Specific routes (highest priority — registered last)
  await page.route('**/api/v1/naming', (route) => json(route, {}))
  await page.route('**/api/v1/downloadclient', (route) => json(route, []))
  await page.route('**/api/v1/indexer', (route) => json(route, []))
  await page.route('**/api/v1/tag', (route) => json(route, []))
  await page.route('**/api/v1/requests/pending/count', (route) => json(route, { count: 0 }))
  await page.route('**/api/v1/user/notifications/unread-count', (route) => json(route, { count: 0 }))
  await page.route('**/api/v1/activities/running', (route) => json(route, { count: 0 }))
  await page.route('**/api/v1/history**', (route) => json(route, history))
  await page.route('**/api/v1/history/stream**', (route) => json(route, []))
  await page.route('**/api/v1/medialibraryfolder', (route) => json(route, folders))
  await page.route('**/api/v1/qualityprofile', (route) => json(route, profiles))
  await page.route('**/api/v1/calendar**', (route) => json(route, calendar))
  await page.route('**/api/v1/queue', (route) => json(route, queue))
  await page.route('**/api/v1/movies/*', (route) => {
    const id = Number(route.request().url().split('/').pop())
    const found = movies.find((m) => m.id === id)
    return found ? json(route, found) : route.fulfill({ status: 404 })
  })
  await page.route('**/api/v1/movies', (route) => json(route, movies))
  await page.route('**/api/v1/series/*', (route) => {
    const id = Number(route.request().url().split('/').pop())
    const found = series.find((s) => s.id === id)
    return found ? json(route, found) : route.fulfill({ status: 404 })
  })
  await page.route('**/api/v1/series', (route) => {
    if (route.request().method() === 'GET') return json(route, series)
    return json(route, { id: 99, ...series[0] })
  })
  await page.route('**/api/v1/auth/me', (route) => json(route, mockUser))
  await page.route('**/api/v1/system/status', (route) => json(route, status))
}

// ─── Extended test fixture ──────────────────────────────────────

export const test = base.extend<{ mockPage: Page }>({
  mockPage: async ({ page }, use) => {
    await mockApi(page)
    await use(page)
  },
})

export { expect } from '@playwright/test'
