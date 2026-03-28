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

interface Image {
  coverType: string
  remoteUrl: string
}

export interface Series {
  id: number
  title: string
  sortTitle: string
  overview: string | null
  status: string
  network: string | null
  year: number | null
  images: Image[] | null
}

export interface Episode {
  id: number
  seriesId: number
  seasonNumber: number
  episodeNumber: number
  title: string | null
  overview: string | null
  monitored: boolean
  hasFile: boolean
  episodeFile: { id: number } | null
}

export interface Movie {
  id: number
  title: string
  sortTitle: string
  overview: string | null
  year: number | null
  studio: string | null
  movieFileId: number | null
  images: Image[] | null
}

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

// ── Media Requests ──────────────────────────────────────────────────────────

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

// ── Watchlist ───────────────────────────────────────────────────────────────

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

// ── Ratings ─────────────────────────────────────────────────────────────────

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

// ── Notifications ────────────────────────────────────────────────────────────

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

// Helpers

export function imageUrl(images: Image[] | null, type: 'poster' | 'fanart' | 'banner'): string | null {
  if (!images) return null
  const img = images.find((i) => i.coverType === type)
  return img?.remoteUrl ?? null
}

export const api = {
  listSeries: () => get<Series[]>('/series'),
  getSeries: (id: number) => get<Series>(`/series/${id}`),
  getEpisodes: (seriesId: number) => get<Episode[]>(`/series/${seriesId}/episodes`),
  listMovies: () => get<Movie[]>('/movies'),
  getMovie: (id: number) => get<Movie>(`/movies/${id}`),
  streamInfo: (fileId: number) => get<StreamInfo>(`/stream/${fileId}/info`),
  startTranscode: (fileId: number, opts?: Record<string, unknown>) =>
    post<TranscodeResponse>(`/stream/${fileId}/transcode`, opts ?? {}),
  directPlayUrl: (fileId: number) => `${getApiBase()}/stream/${fileId}/direct`,
  hlsUrl: (fileId: number, sessionId: string) =>
    `${getApiBase()}/stream/${fileId}/hls/${sessionId}/master.m3u8`,
  subtitleUrl: (fileId: number, trackIndex: number) =>
    `${getApiBase()}/stream/${fileId}/subtitles/${trackIndex}`,

  // Progress
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

  // Discover (enriched search)
  discoverSearch: (q: string, type: 'movie' | 'series' = 'movie') =>
    get<DiscoverSearchResults>(`/discover/search?q=${encodeURIComponent(q)}&type=${type}`),

  // Requests
  listMyRequests: () => get<MediaRequest[]>('/requests?mine=true'),
  createRequest: (body: {
    mediaType: string; tmdbId: number; title: string;
    year?: number; posterUrl?: string; overview?: string;
  }) => post<MediaRequest>('/requests', body),

  // Watchlist
  getWatchlist: (mediaType?: string) =>
    get<WatchlistItem[]>(`/user/watchlist${mediaType ? `?mediaType=${mediaType}` : ''}`),
  addToWatchlist: (mediaType: string, mediaId: number) =>
    put<WatchlistItem>(`/user/watchlist/${mediaType}/${mediaId}`),
  removeFromWatchlist: (mediaType: string, mediaId: number) =>
    del(`/user/watchlist/${mediaType}/${mediaId}`),

  // Ratings
  getUserRatings: (mediaType?: string) =>
    get<UserRating[]>(`/user/ratings${mediaType ? `?mediaType=${mediaType}` : ''}`),
  setRating: (mediaType: string, mediaId: number, rating: number) =>
    put<UserRating>(`/user/ratings/${mediaType}/${mediaId}`, { rating }),
  getRating: (mediaType: string, mediaId: number) =>
    get<RatingInfo>(`/user/ratings/${mediaType}/${mediaId}`),
  deleteRating: (mediaType: string, mediaId: number) =>
    del(`/user/ratings/${mediaType}/${mediaId}`),

  // Notifications
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
}
