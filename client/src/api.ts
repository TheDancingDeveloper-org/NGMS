// ── Connection management ───────────────────────────────────────────────────

export interface ServerConnection {
  serverUrl: string
  serverName: string
  serverId: string
  clientToken?: string
}

const STORAGE_KEY = 'stackarr_server'

export function getConnection(): ServerConnection | null {
  try {
    const stored = localStorage.getItem(STORAGE_KEY)
    return stored ? JSON.parse(stored) : null
  } catch {
    return null
  }
}

export function saveConnection(conn: ServerConnection) {
  localStorage.setItem(STORAGE_KEY, JSON.stringify(conn))
}

export function clearConnection() {
  localStorage.removeItem(STORAGE_KEY)
}

function getApiBase(): string {
  const conn = getConnection()
  return conn ? `${conn.serverUrl}/api/v1` : '/api/v1'
}

function authHeaders(): Record<string, string> {
  const conn = getConnection()
  if (conn?.clientToken) {
    return { Authorization: `Bearer ${conn.clientToken}` }
  }
  return {}
}

// ── HTTP helpers ────────────────────────────────────────────────────────────

async function get<T>(path: string): Promise<T> {
  const res = await fetch(`${getApiBase()}${path}`, {
    headers: authHeaders(),
    credentials: 'include',
  })
  if (!res.ok) throw new Error(`${res.status} ${res.statusText}`)
  return res.json()
}

async function post<T>(path: string, body?: unknown): Promise<T> {
  const res = await fetch(`${getApiBase()}${path}`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json', ...authHeaders() },
    credentials: 'include',
    body: body ? JSON.stringify(body) : undefined,
  })
  if (!res.ok) throw new Error(`${res.status} ${res.statusText}`)
  return res.json()
}

async function put<T>(path: string, body?: unknown): Promise<T> {
  const res = await fetch(`${getApiBase()}${path}`, {
    method: 'PUT',
    headers: { 'Content-Type': 'application/json', ...authHeaders() },
    credentials: 'include',
    body: body ? JSON.stringify(body) : undefined,
  })
  if (!res.ok) throw new Error(`${res.status} ${res.statusText}`)
  return res.json()
}

async function del(path: string, body?: unknown): Promise<void> {
  const res = await fetch(`${getApiBase()}${path}`, {
    method: 'DELETE',
    headers: body ? { 'Content-Type': 'application/json', ...authHeaders() } : authHeaders(),
    credentials: 'include',
    body: body ? JSON.stringify(body) : undefined,
  })
  if (!res.ok) throw new Error(`${res.status} ${res.statusText}`)
}

// ── Bootstrap discovery ─────────────────────────────────────────────────────

interface ClaimRedeemResponse {
  serverId: string
  serverName: string
  publicIp: string
  localIps: string[]
  port: number
  version: string
  claimType: string        // "invite" or "device"
  inviteCode?: string      // present if claimType == "invite"
  clientToken?: string     // legacy, may not be present
}

export interface ClaimResult extends ServerConnection {
  claimType: string
  inviteCode?: string
}

export async function redeemClaimCode(
  code: string,
  bootstrapUrl: string,
): Promise<ClaimResult> {
  const res = await fetch(`${bootstrapUrl}/api/v1/claims/${code.toUpperCase()}/redeem`, {
    method: 'POST',
  })
  if (!res.ok) {
    if (res.status === 404) throw new Error('Invalid or expired invite code')
    throw new Error(`Bootstrap error: ${res.status}`)
  }
  const data: ClaimRedeemResponse = await res.json()

  // Try local IPs first, then public IP
  const urls = [
    ...data.localIps.map((ip: string) => `http://${ip}:${data.port}`),
    `http://${data.publicIp}:${data.port}`,
  ]

  for (const url of urls) {
    try {
      const probe = await fetch(`${url}/api/v1/system/status`, {
        signal: AbortSignal.timeout(3000),
      })
      if (probe.ok) {
        const conn: ServerConnection = {
          serverUrl: url,
          serverName: data.serverName,
          serverId: data.serverId,
          clientToken: data.clientToken ?? '',
        }
        saveConnection(conn)

        return {
          ...conn,
          claimType: data.claimType,
          inviteCode: data.inviteCode,
        }
      }
    } catch {
      // Try next URL
    }
  }

  throw new Error('Server found but unreachable. Check your network/firewall.')
}

// ── Types ───────────────────────────────────────────────────────────────────

// -- System --

export interface EnabledModules {
  tvManagement: boolean
  movieManagement: boolean
  torrentEmbedded: boolean
  usenetEmbedded: boolean
  torrentExternal: boolean
  usenetExternal: boolean
  indexarrSidecar: boolean
  externalIndexers: boolean
  plexIntegration: boolean
  notifications: boolean
  streaming: boolean
  remoteAccess: boolean
  stremioAddon: boolean
  davStreaming: boolean
}

export interface SystemStatus {
  version: string
  instanceName: string
  firstBoot: boolean
  authMethod: string
  modules: EnabledModules
  indexarrAvailable: boolean
  startTime: string
}

// -- Media --

export interface Image {
  coverType: string
  remoteUrl: string
}

export interface MediaFile {
  id: number
  mediaType: 'series' | 'movie'
  relativePath: string
  size: number
  dateAdded: string
  quality: Record<string, unknown>
  languages: Record<string, unknown>
  sceneName: string | null
  releaseGroup: string | null
  releaseHash: string | null
  edition: string | null
  mediaInfo: Record<string, unknown> | null
  indexerFlags: number
}

export interface Series {
  id: number
  title: string
  cleanTitle: string
  sortTitle: string
  overview: string | null
  status: 'continuing' | 'ended' | 'upcoming' | 'deleted'
  seriesType: 'standard' | 'daily' | 'anime'
  network: string | null
  airTime: string | null
  firstAired: string | null
  year: number | null
  runtime: number | null
  path: string
  mediaLibraryFolderId: number | null
  qualityProfileId: number
  seasonFolder: boolean
  monitored: boolean
  useSceneNumbering: boolean
  tvdbId: number | null
  imdbId: string | null
  tmdbId: number | null
  tvmazeId: number | null
  malId: number | null
  images: Image[] | null
  genres: string[] | null
  tags: number[] | null
  addedAt: string
  lastInfoSync: string | null
  plexRatingKey: string | null
  plexRatingKey4k: string | null
  mediaAddedAt: string | null
  // Enriched fields from SeriesResponse
  posterUrl: string | null
  fanartUrl: string | null
  episodeCount: number
  episodeFileCount: number
  totalEpisodeCount: number
  seasonCount: number
}

export interface Episode {
  id: number
  seriesId: number
  seasonNumber: number
  episodeNumber: number
  absoluteNumber: number | null
  sceneSeasonNumber: number | null
  sceneEpisodeNumber: number | null
  sceneAbsoluteNumber: number | null
  title: string | null
  overview: string | null
  airDate: string | null
  airDateUtc: string | null
  runtime: number | null
  monitored: boolean
  hasFile: boolean
  episodeFile: MediaFile | null
}

export interface Movie {
  id: number
  title: string
  cleanTitle: string
  sortTitle: string
  overview: string | null
  year: number | null
  studio: string | null
  path: string
  mediaLibraryFolderId: number | null
  qualityProfileId: number
  monitored: boolean
  minimumAvailability: 'announced' | 'inCinemas' | 'released'
  movieFileId: number | null
  tmdbId: number | null
  imdbId: string | null
  inCinemas: string | null
  physicalRelease: string | null
  digitalRelease: string | null
  images: Image[] | null
  genres: string[] | null
  tags: number[] | null
  collectionTmdbId: number | null
  addedAt: string
  lastInfoSync: string | null
  plexRatingKey: string | null
  plexRatingKey4k: string | null
  mediaAddedAt: string | null
  // Enriched fields from MovieResponse
  posterUrl: string | null
  fanartUrl: string | null
  hasFile: boolean
  movieFile: MediaFile | null
}

// -- Streaming --

export interface StreamInfo {
  container: string
  durationSecs: number
  bitrate: number
  videoStreams: {
    index: number; codec: string; width: number; height: number
    bitrate: number; profile: string; isHdr: boolean; frameRate: number
  }[]
  audioStreams: {
    index: number; codec: string; channels: number; language: string
    title: string; bitrate: number; isDefault: boolean
  }[]
  subtitleStreams: {
    index: number; codec: string; language: string; title: string
    forced: boolean; isDefault: boolean
  }[]
}

export interface TranscodeResponse {
  sessionId: string
  playlistUrl: string
  encoder: string
}

export interface QualityTier {
  name: string
  maxWidth: number
  maxHeight: number
  videoBitrate: number
  audioBitrate: number
}

export interface StreamSession {
  id: string
  mediaFileId: number
  sessionType: string
  status: string
  startedAt: string
  lastActivity: string
}

// -- Watch Progress --

export interface WatchProgress {
  id: number
  userId: number
  mediaFileId: number
  mediaType: string
  mediaId: number
  episodeId: number | null
  positionSecs: number
  durationSecs: number
  completed: boolean
  updatedAt: string
}

export interface ContinueWatchingItem extends WatchProgress {
  title: string | null
  posterUrl: string | null
  backdropUrl: string | null
  episodeTitle: string | null
  seasonNumber: number | null
  episodeNumber: number | null
  year: number | null
}

// -- Media Requests --

export interface MediaRequest {
  id: number
  userId: number
  mediaType: string
  tmdbId: number
  title: string
  year: number | null
  posterUrl: string | null
  overview: string | null
  status: string
  adminNote: string | null
  approvedBy: number | null
  createdAt: string
  updatedAt: string
}

export interface PendingCount {
  count: number
}

// -- Discover --

export interface DiscoverResult {
  id: number
  title?: string
  name?: string
  overview?: string
  releaseDate?: string
  firstAirDate?: string
  posterPath?: string | null
  backdropPath?: string | null
  voteAverage: number
  mediaType: string
  inLibrary: boolean
  requestStatus: string | null
}

export interface DiscoverSearchResults {
  page: number
  totalPages: number
  totalResults: number
  results: DiscoverResult[]
}

export type DiscoverSliderType =
  | 'trending'
  | 'popular_movies'
  | 'popular_tv'
  | 'upcoming_movies'
  | 'upcoming_tv'
  | 'recently_added'
  | 'movie_genres'
  | 'tv_genres'
  | 'tmdb_movie_genre'
  | 'tmdb_tv_genre'
  | 'tmdb_movie_keyword'
  | 'tmdb_tv_keyword'
  | 'tmdb_search'
  | 'tmdb_studio'
  | 'tmdb_network'
  | 'tmdb_movie_streaming_services'
  | 'tmdb_tv_streaming_services'

export interface DiscoverSlider {
  id: number
  sliderType: DiscoverSliderType
  displayOrder: number
  isBuiltIn: boolean
  enabled: boolean
  title: string | null
  customData: Record<string, unknown> | null
  createdAt: string
  updatedAt: string
}

export interface TmdbGenre {
  id: number
  name: string
}

export interface TmdbTrendingItem {
  id: number
  mediaType: string
  title?: string
  name?: string
  overview?: string
  releaseDate?: string
  firstAirDate?: string
  posterPath?: string | null
  backdropPath?: string | null
  genreIds: number[]
  voteAverage: number
  voteCount: number
  popularity: number
  originalLanguage?: string
}

export interface TmdbSearchResults<T> {
  page: number
  totalPages: number
  totalResults: number
  results: T[]
}

// -- Watchlist --

export interface WatchlistItem {
  id: number
  userId: number
  mediaType: string
  mediaId: number
  tmdbId: number
  addedAt: string
  title: string | null
  posterUrl: string | null
  year: number | null
}

// -- Ratings --

export interface UserRating {
  id: number
  userId: number
  mediaType: string
  mediaId: number
  rating: number
  createdAt: string
  updatedAt: string
}

export interface RatingInfo {
  userRating: number | null
  averageRating: number
  ratingCount: number
}

// -- Notifications --

export interface UserNotification {
  id: number
  userId: number
  notificationType: string
  title: string
  body: string | null
  data: Record<string, unknown> | null
  read: boolean
  createdAt: string
}

export interface UnreadCount {
  count: number
}

// -- User --

export interface UserDevice {
  id: number
  userId: number
  deviceName: string
  lastActive: string
  createdAt: string
}

export interface UserSession {
  id: string
  userId: number
  createdAt: string
  lastActive: string
  userAgent: string | null
}

// -- Calendar --

export interface CalendarEpisode {
  id: number
  seriesId: number
  seasonNumber: number
  episodeNumber: number
  title: string | null
  airDateUtc: string | null
  seriesTitle: string
  posterUrl: string | null
}

// -- Queue --

export interface QueueItem {
  id: number
  title: string
  status: string
  protocol: string
  size: number
  sizeleft: number
  timeleft: string | null
  trackedDownloadState: string | null
  trackedDownloadStatus: string | null
  downloadClient: string | null
  outputPath: string | null
}

// -- History --

export interface HistoryEvent {
  id: number
  eventType: string
  sourceTitle: string | null
  date: string
  data: Record<string, unknown> | null
  mediaType: string | null
  mediaId: number | null
  episodeId: number | null
}

export interface PaginatedResponse<T> {
  page: number
  pageSize: number
  totalRecords: number
  records: T[]
}

// -- Activities --

export interface Activity {
  id: string
  name: string
  status: string
  message: string | null
  progress: number | null
  startedAt: string
  completedAt: string | null
}

// ── Helpers ─────────────────────────────────────────────────────────────────

export function imageUrl(images: Image[] | null, type: 'poster' | 'fanart' | 'banner'): string | null {
  if (!images) return null
  const img = images.find((i) => i.coverType === type)
  return img?.remoteUrl ?? null
}

// ── API ─────────────────────────────────────────────────────────────────────

export const api = {
  // -- System --
  getSystemStatus: () => get<SystemStatus>('/system/status'),

  // -- Series --
  listSeries: () => get<Series[]>('/series'),
  getSeries: (id: number) => get<Series>(`/series/${id}`),
  getEpisodes: (seriesId: number) => get<Episode[]>(`/series/${seriesId}/episodes`),

  // -- Movies --
  listMovies: () => get<Movie[]>('/movies'),
  getMovie: (id: number) => get<Movie>(`/movies/${id}`),

  // -- Streaming --
  streamInfo: (fileId: number) => get<StreamInfo>(`/stream/${fileId}/info`),
  qualityTiers: (fileId: number) => get<QualityTier[]>(`/stream/${fileId}/quality-tiers`),
  startTranscode: (fileId: number, opts?: Record<string, unknown>) =>
    post<TranscodeResponse>(`/stream/${fileId}/transcode`, opts ?? {}),

  bandwidthTest: async (): Promise<number> => {
    const conn = getConnection()
    const base = conn ? conn.serverUrl : ''
    const size = 2_000_000
    const start = performance.now()
    const res = await fetch(`${base}/api/v1/stream/bandwidth-test?size=${size}`, {
      headers: conn?.clientToken ? { Authorization: `Bearer ${conn.clientToken}` } : {},
      credentials: 'include',
      cache: 'no-store',
    })
    if (!res.ok) throw new Error(`Bandwidth test failed: ${res.status}`)
    await res.arrayBuffer()
    const elapsed = (performance.now() - start) / 1000
    return Math.round((size * 8) / elapsed)
  },
  directPlayUrl: (fileId: number) => `${getApiBase()}/stream/${fileId}/direct`,
  hlsUrl: (fileId: number, sessionId: string) =>
    `${getApiBase()}/stream/${fileId}/hls/${sessionId}/master.m3u8`,
  subtitleUrl: (fileId: number, trackIndex: number) =>
    `${getApiBase()}/stream/${fileId}/subtitles/${trackIndex}`,

  // Stream sessions
  listStreamSessions: () => get<StreamSession[]>('/stream/sessions'),
  stopStreamSession: (sessionId: string) => del(`/stream/sessions/${sessionId}`),

  // -- Progress --
  getContinueWatching: (limit = 20) =>
    get<ContinueWatchingItem[]>(`/user/progress/continue?limit=${limit}`),
  getProgress: (mediaFileId: number) =>
    get<WatchProgress>(`/user/progress/${mediaFileId}`),
  getProgressSafe: async (mediaFileId: number): Promise<WatchProgress | null> => {
    try {
      return await get<WatchProgress>(`/user/progress/${mediaFileId}`)
    } catch {
      return null
    }
  },
  updateProgress: (mediaFileId: number, positionSecs: number, durationSecs: number) =>
    put<WatchProgress>(`/user/progress/${mediaFileId}`, { positionSecs, durationSecs }),
  deleteProgress: (mediaFileId: number) =>
    del(`/user/progress/${mediaFileId}`),
  getSeriesProgress: (seriesId: number) =>
    get<WatchProgress[]>(`/user/progress/series/${seriesId}`),
  getMovieProgress: (movieId: number) =>
    get<WatchProgress>(`/user/progress/movie/${movieId}`),

  // -- Discover --
  discoverSearch: (q: string, type: 'movie' | 'series' = 'movie') =>
    get<DiscoverSearchResults>(`/discover/search?q=${encodeURIComponent(q)}&type=${type}`),

  // Sliders
  getSliders: () => get<DiscoverSlider[]>('/discover/sliders'),
  reorderSliders: (sliderIds: number[]) =>
    post<DiscoverSlider[]>('/discover/sliders', { sliderIds }),
  addSlider: (input: { sliderType: DiscoverSliderType; title?: string; customData?: Record<string, unknown>; enabled?: boolean }) =>
    post<DiscoverSlider>('/discover/sliders/add', input),
  updateSlider: (id: number, input: { title?: string; enabled?: boolean; customData?: Record<string, unknown> }) =>
    put<DiscoverSlider>(`/discover/sliders/${id}`, input),
  deleteSlider: (id: number) => del(`/discover/sliders/${id}`),
  resetSliders: () => post<DiscoverSlider[]>('/discover/sliders/reset'),

  // Trending & browse
  getTrending: (params?: { mediaType?: string; timeWindow?: string; page?: number; language?: string }) => {
    const qs = new URLSearchParams()
    if (params?.mediaType) qs.set('mediaType', params.mediaType)
    if (params?.timeWindow) qs.set('timeWindow', params.timeWindow)
    if (params?.page) qs.set('page', String(params.page))
    if (params?.language) qs.set('language', params.language)
    const q = qs.toString()
    return get<TmdbSearchResults<TmdbTrendingItem>>(`/discover/trending${q ? `?${q}` : ''}`)
  },

  getDiscoverMovies: (params?: Record<string, string>) => {
    const qs = new URLSearchParams(params)
    const q = qs.toString()
    return get<TmdbSearchResults<TmdbTrendingItem>>(`/discover/movies${q ? `?${q}` : ''}`)
  },
  getUpcomingMovies: (params?: Record<string, string>) => {
    const qs = new URLSearchParams(params)
    const q = qs.toString()
    return get<TmdbSearchResults<TmdbTrendingItem>>(`/discover/movies/upcoming${q ? `?${q}` : ''}`)
  },
  getMoviesByGenre: (genreId: number, params?: Record<string, string>) => {
    const qs = new URLSearchParams(params)
    const q = qs.toString()
    return get<TmdbSearchResults<TmdbTrendingItem>>(`/discover/movies/genre/${genreId}${q ? `?${q}` : ''}`)
  },
  getMoviesByStudio: (studioId: number, params?: Record<string, string>) => {
    const qs = new URLSearchParams(params)
    const q = qs.toString()
    return get<TmdbSearchResults<TmdbTrendingItem>>(`/discover/movies/studio/${studioId}${q ? `?${q}` : ''}`)
  },

  getDiscoverTv: (params?: Record<string, string>) => {
    const qs = new URLSearchParams(params)
    const q = qs.toString()
    return get<TmdbSearchResults<TmdbTrendingItem>>(`/discover/tv${q ? `?${q}` : ''}`)
  },
  getUpcomingTv: (params?: Record<string, string>) => {
    const qs = new URLSearchParams(params)
    const q = qs.toString()
    return get<TmdbSearchResults<TmdbTrendingItem>>(`/discover/tv/upcoming${q ? `?${q}` : ''}`)
  },
  getTvByGenre: (genreId: number, params?: Record<string, string>) => {
    const qs = new URLSearchParams(params)
    const q = qs.toString()
    return get<TmdbSearchResults<TmdbTrendingItem>>(`/discover/tv/genre/${genreId}${q ? `?${q}` : ''}`)
  },
  getTvByNetwork: (networkId: number, params?: Record<string, string>) => {
    const qs = new URLSearchParams(params)
    const q = qs.toString()
    return get<TmdbSearchResults<TmdbTrendingItem>>(`/discover/tv/network/${networkId}${q ? `?${q}` : ''}`)
  },

  // Recommendations & similar
  getMovieRecommendations: (movieId: number) =>
    get<TmdbSearchResults<TmdbTrendingItem>>(`/discover/movies/${movieId}/recommendations`),
  getSimilarMovies: (movieId: number) =>
    get<TmdbSearchResults<TmdbTrendingItem>>(`/discover/movies/${movieId}/similar`),
  getTvRecommendations: (tvId: number) =>
    get<TmdbSearchResults<TmdbTrendingItem>>(`/discover/tv/${tvId}/recommendations`),
  getSimilarTv: (tvId: number) =>
    get<TmdbSearchResults<TmdbTrendingItem>>(`/discover/tv/${tvId}/similar`),

  // Genre & language lists
  getMovieGenres: () => get<{ genres: TmdbGenre[] }>('/discover/genres/movie'),
  getTvGenres: () => get<{ genres: TmdbGenre[] }>('/discover/genres/tv'),
  getLanguages: () => get<{ iso_639_1: string; english_name: string; name: string }[]>('/discover/languages'),

  // -- Requests --
  listMyRequests: () => get<MediaRequest[]>('/requests?mine=true'),
  listAllRequests: (status?: string) => {
    const qs = status ? `?status=${status}` : ''
    return get<MediaRequest[]>(`/requests${qs}`)
  },
  getRequest: (id: number) => get<MediaRequest>(`/requests/${id}`),
  createRequest: (body: {
    mediaType: string; tmdbId: number; title: string;
    year?: number; posterUrl?: string; overview?: string;
  }) => post<MediaRequest>('/requests', body),
  deleteRequest: (id: number) => del(`/requests/${id}`),
  approveRequest: (id: number) => put<MediaRequest>(`/requests/${id}/approve`),
  declineRequest: (id: number) => put<MediaRequest>(`/requests/${id}/decline`),
  getPendingRequestCount: () => get<PendingCount>('/requests/pending/count'),

  // -- Watchlist --
  getWatchlist: (mediaType?: string) =>
    get<WatchlistItem[]>(`/user/watchlist${mediaType ? `?mediaType=${mediaType}` : ''}`),
  addToWatchlist: (mediaType: string, mediaId: number) =>
    put<WatchlistItem>(`/user/watchlist/${mediaType}/${mediaId}`),
  removeFromWatchlist: (mediaType: string, mediaId: number) =>
    del(`/user/watchlist/${mediaType}/${mediaId}`),

  // -- Ratings --
  getUserRatings: (mediaType?: string) =>
    get<UserRating[]>(`/user/ratings${mediaType ? `?mediaType=${mediaType}` : ''}`),
  setRating: (mediaType: string, mediaId: number, rating: number) =>
    put<UserRating>(`/user/ratings/${mediaType}/${mediaId}`, { rating }),
  getRating: (mediaType: string, mediaId: number) =>
    get<RatingInfo>(`/user/ratings/${mediaType}/${mediaId}`),
  deleteRating: (mediaType: string, mediaId: number) =>
    del(`/user/ratings/${mediaType}/${mediaId}`),

  // -- Notifications --
  getNotifications: (unread = false, limit = 50, offset = 0) =>
    get<UserNotification[]>(
      `/user/notifications?unread=${unread}&limit=${limit}&offset=${offset}`,
    ),
  getUnreadCount: () => get<UnreadCount>('/user/notifications/unread-count'),
  markNotificationRead: (id: number) =>
    put<{ ok: boolean }>(`/user/notifications/${id}/read`),
  markAllNotificationsRead: () =>
    put<{ marked: number }>('/user/notifications/read-all'),
  savePushSubscription: (endpoint: string, p256dh: string, auth: string) =>
    post<unknown>('/user/push-subscription', { endpoint, p256dh, auth }),
  removePushSubscription: (endpoint: string) =>
    del('/user/push-subscription', { endpoint }),

  // -- User profile / devices / sessions --
  updateProfile: (body: { displayName?: string; avatarUrl?: string | null }) =>
    put<unknown>('/user/profile', body),
  getDevices: () => get<UserDevice[]>('/user/devices'),
  deleteDevice: (id: number) => del(`/user/devices/${id}`),
  getSessions: () => get<UserSession[]>('/user/sessions'),
  deleteAllSessions: () => del('/user/sessions'),

  // -- Calendar --
  getCalendar: (start?: string, end?: string) => {
    const qs = new URLSearchParams()
    if (start) qs.set('start', start)
    if (end) qs.set('end', end)
    const q = qs.toString()
    return get<CalendarEpisode[]>(`/calendar${q ? `?${q}` : ''}`)
  },

  // -- Queue --
  getQueue: () => get<QueueItem[]>('/queue'),
  removeQueueItem: (id: number) => del(`/queue/${id}`),

  // -- History --
  getHistory: (page = 1, pageSize = 20) =>
    get<PaginatedResponse<HistoryEvent>>(`/history?page=${page}&page_size=${pageSize}`),
  getHistoryStream: (limit = 30) =>
    get<HistoryEvent[]>(`/history/stream?limit=${limit}`),

  // -- Activities --
  getActivities: (limit = 20, includeCompleted = true) =>
    get<Activity[]>(`/activities?limit=${limit}&includeCompleted=${includeCompleted}`),
  getRunningActivities: () => get<{ count: number }>('/activities/running'),
}
