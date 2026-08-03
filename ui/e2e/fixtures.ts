// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 The StackArr Authors

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
  usenetEmbedded: true,
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

// ─── Usenet / Torrent mock data ────────────────────────────────

export const mockUsenetStatus = {
  enabled: true,
  downloadSpeed: 5_242_880,
  queueSize: 2,
  activeDownloads: 1,
  maxActiveDownloads: 1,
  speedLimit: 0,
  paused: false,
  pauseRemainingSecs: null,
}

export const mockUsenetQueue = {
  jobs: [
    {
      id: 'job-1',
      name: 'Breaking.Bad.S01E01.720p.BluRay.x264-DEMAND',
      size: 1_500_000_000,
      progress: 45.2,
      speed: 5_242_880,
      status: 'downloading',
      eta: 180,
      errorMessage: null,
      category: '',
      priority: 1,
      totalArticles: 2000,
      downloadedArticles: 904,
    },
    {
      id: 'job-2',
      name: 'Inception.2010.2160p.UHD.BluRay.x265-DEMAND',
      size: 9_000_000_000,
      progress: 0,
      speed: 0,
      status: 'queued',
      eta: 0,
      errorMessage: null,
      category: '',
      priority: 1,
      totalArticles: 11000,
      downloadedArticles: 0,
    },
  ],
}

export const mockUsenetServers = {
  servers: [
    {
      id: 'srv-uuid-1',
      dbId: 5,
      name: 'news.example.com',
      host: 'news.example.com',
      port: 563,
      ssl: true,
      username: 'user1',
      password: '********',
      connections: 20,
      priority: 0,
      optional: false,
      enabled: true,
    },
    {
      id: 'srv-uuid-2',
      dbId: 6,
      name: 'backup.example.com',
      host: 'backup.example.com',
      port: 563,
      ssl: true,
      username: 'user2',
      password: '********',
      connections: 10,
      priority: 5,
      optional: true,
      enabled: false,
    },
  ],
}

export const mockUsenetSettings = {
  maxActiveDownloads: 1,
  speedLimit: 0,
  historyRetention: null,
  incompleteDir: '/downloads/usenet/incomplete',
  completeDir: '/downloads/usenet/complete',
}

export const mockUsenetHistory = { items: [], total: 0 }

export const mockTorrentStatus = {
  enabled: true,
  downloadSpeed: 10_485_760,
  uploadSpeed: 1_048_576,
  sessionUptime: 86400,
  peers: { connecting: 5, liveTcp: 20, liveUtp: 10, dead: 3, queued: 2, seen: 100 },
  counters: { fetchedBytes: 5_000_000_000, uploadedBytes: 1_000_000_000 },
}

export const mockTorrentList = {
  torrents: [
    {
      id: 1,
      info_hash: 'abc123def456abc123def456abc123def456abcd',
      name: 'Ubuntu.24.04.iso',
      output_folder: '/downloads/torrent',
      total_pieces: 1000,
      stats: {
        state: 'live',
        error: null,
        progress_bytes: 750_000_000,
        total_bytes: 1_000_000_000,
        finished: false,
        file_progress: [750_000_000],
        live: {
          snapshot: {
            have_bytes: 750_000_000,
            downloaded_and_checked_bytes: 750_000_000,
            downloaded_and_checked_pieces: 750,
            fetched_bytes: 800_000_000,
            uploaded_bytes: 250_000_000,
            initially_needed_bytes: 1_000_000_000,
            remaining_bytes: 250_000_000,
            total_bytes: 1_000_000_000,
            total_piece_download_ms: 5000,
            peer_stats: { queued: 2, connecting: 1, live: 15, seen: 50, dead: 3, not_needed: 0 },
          },
          download_speed: { mbps: 10.0, human_readable: '10 MB/s' },
          upload_speed: { mbps: 1.0, human_readable: '1 MB/s' },
          time_remaining: { duration: { secs: 25, nanos: 0 }, human_readable: '25s' },
        },
      },
    },
  ],
  total: 1,
}

export const mockTorrentSettings = {
  downloadFolder: '/downloads/torrent',
  completedFolder: '/downloads/torrent/complete',
  uploadLimitBps: 0,
  downloadLimitBps: 0,
  peerLimit: 200,
  concurrentInitLimit: 3,
  dhtEnabled: true,
}

export const mockDownloadClients = [
  { id: -1, name: 'Embedded Torrent Client', clientType: 'embedded_torrent', protocol: 'torrent', config: {}, enabled: true, priority: 0 },
  { id: -2, name: 'Embedded Usenet Client', clientType: 'embedded_usenet_engine', protocol: 'usenet', config: {}, enabled: true, priority: 0 },
]

// ─── Logs mock data ────────────────────────────────────────────

export const mockLogResponse = {
  entries: [
    { timestamp: '2026-03-30T00:00:01Z', level: 'INFO', target: 'stackarr_web', message: 'Server started on port 8989', seq: 1 },
    { timestamp: '2026-03-30T00:00:02Z', level: 'WARN', target: 'stackarr_indexer', message: 'Indexer NZBGeek returned 0 results', seq: 2 },
    { timestamp: '2026-03-30T00:00:03Z', level: 'ERROR', target: 'stackarr_download', message: 'Connection refused: SABnzbd at localhost:8080', seq: 3 },
    { timestamp: '2026-03-30T00:00:04Z', level: 'DEBUG', target: 'stackarr_scheduler', message: 'RSS sync task completed in 1.2s', seq: 4 },
    { timestamp: '2026-03-30T00:00:05Z', level: 'TRACE', target: 'stackarr_core', message: 'Database query executed: SELECT * FROM series', seq: 5 },
  ],
  latestSeq: 5,
}

// ─── Users / Invites mock data ─────────────────────────────────

export const mockUsers = [
  { id: 1, username: 'admin', displayName: 'Admin', role: 'admin', enabled: true, avatarUrl: null, createdAt: '2026-01-01T00:00:00Z', updatedAt: '2026-03-30T00:00:00Z' },
  { id: 2, username: 'viewer', displayName: 'Viewer', role: 'user', enabled: true, avatarUrl: null, createdAt: '2026-02-15T00:00:00Z', updatedAt: '2026-03-30T00:00:00Z' },
  { id: 3, username: 'disabled_user', displayName: 'Disabled', role: 'user', enabled: false, avatarUrl: null, createdAt: '2026-03-01T00:00:00Z', updatedAt: '2026-03-30T00:00:00Z' },
]

export const mockInvites = [
  { id: 1, code: 'INVITE-ABC-123', createdBy: 1, claimedBy: null, role: 'user', expiresAt: '2026-04-30T00:00:00Z', createdAt: '2026-03-28T00:00:00Z' },
  { id: 2, code: 'INVITE-DEF-456', createdBy: 1, claimedBy: 2, role: 'admin', expiresAt: null, createdAt: '2026-02-10T00:00:00Z' },
]

// ─── Watchlist mock data ───────────────────────────────────────

export const mockWatchlistItems = [
  { id: 1, tmdb_id: 27205, media_type: 'movie', auto_requested: true, created_at: '2026-03-25T00:00:00Z' },
  { id: 2, tmdb_id: 1396, media_type: 'tv', auto_requested: false, created_at: '2026-03-26T00:00:00Z' },
]

// ─── Search / Indexer mock data ────────────────────────────────

export const mockIndexerConfigs = [
  { id: 1, name: 'NZBGeek', indexerType: 'newznab', protocol: 'usenet', baseUrl: 'https://nzbgeek.info', apiKey: null, enabled: true, priority: 0, config: null, fields: {} },
  { id: 2, name: '1337x', indexerType: 'torznab', protocol: 'torrent', baseUrl: 'https://1337x.to', apiKey: null, enabled: true, priority: 1, config: null, fields: {} },
]

export const mockSearchResults = [
  {
    guid: 'result-1', title: 'Breaking.Bad.S01E01.720p.BluRay.x264-DEMAND', downloadUrl: 'https://example.com/nzb/1',
    infoUrl: 'https://example.com/details/1', indexerId: 1, indexerName: 'NZBGeek', protocol: 'usenet',
    size: 1_500_000_000, ageDays: 30, publishDate: 1709222400, infoHash: null, magnetUrl: null,
    seeders: null, leechers: null, nzbUrl: 'https://example.com/nzb/1', categories: [5030], indexerFlags: [], quality: 'Bluray-720p',
  },
  {
    guid: 'result-2', title: 'Breaking.Bad.S01E01.1080p.WEB-DL.DD5.1.H.264', downloadUrl: 'https://example.com/torrent/2',
    infoUrl: 'https://example.com/details/2', indexerId: 2, indexerName: '1337x', protocol: 'torrent',
    size: 2_200_000_000, ageDays: 5, publishDate: 1711382400, infoHash: 'abc123', magnetUrl: 'magnet:?xt=urn:btih:abc123',
    seeders: 42, leechers: 8, nzbUrl: null, categories: [5030], indexerFlags: [], quality: 'WEBDL-1080p',
  },
]

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
  // Usenet endpoints
  await page.route('**/api/v1/usenet/status', (route) => json(route, mockUsenetStatus))
  await page.route('**/api/v1/usenet/queue', (route) => json(route, mockUsenetQueue))
  await page.route('**/api/v1/usenet/servers', (route) => json(route, mockUsenetServers))
  await page.route('**/api/v1/usenet/settings', (route) => {
    if (route.request().method() === 'PUT') return json(route, mockUsenetSettings)
    return json(route, mockUsenetSettings)
  })
  await page.route('**/api/v1/usenet/history**', (route) => json(route, mockUsenetHistory))
  await page.route('**/api/v1/usenet/servers/*/test', (route) => json(route, { success: true, message: 'Connection successful' }))
  await page.route('**/api/v1/usenet/servers/test', (route) => json(route, { success: true, message: 'Connection successful' }))

  // Torrent endpoints
  await page.route('**/api/v1/torrent/status', (route) => json(route, mockTorrentStatus))
  await page.route('**/api/v1/torrent/list', (route) => json(route, mockTorrentList))
  await page.route('**/api/v1/torrent/settings', (route) => {
    if (route.request().method() === 'PUT') return json(route, mockTorrentSettings)
    return json(route, mockTorrentSettings)
  })

  // Download clients
  await page.route('**/api/v1/downloadclient', (route) => json(route, mockDownloadClients))

  // Logs endpoint
  await page.route('**/api/v1/log**', (route) => json(route, mockLogResponse))

  // Admin users / invites
  await page.route('**/api/v1/admin/users', (route) => json(route, mockUsers))
  await page.route('**/api/v1/admin/invites', (route) => json(route, mockInvites))

  // Watchlist
  await page.route('**/api/v1/plex/watchlist', (route) => json(route, mockWatchlistItems))

  // Search releases
  await page.route('**/api/v1/search**', (route) => json(route, mockSearchResults))

  // Indexer configs (override catch-all empty array)
  await page.route('**/api/v1/indexer', (route) => json(route, mockIndexerConfigs))

  // Release grab
  await page.route('**/api/v1/release', (route) => {
    if (route.request().method() === 'POST') return json(route, { ok: true })
    return json(route, [])
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
