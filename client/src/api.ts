// ── Connection management ───────────────────────────────────────────────────

export interface ServerConnection {
  serverUrl: string
  serverName: string
  serverId: string
  clientToken: string
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

// ── Bootstrap discovery ─────────────────────────────────────────────────────

interface ClaimRedeemResponse {
  serverId: string
  serverName: string
  publicIp: string
  localIps: string[]
  port: number
  clientToken: string
  version: string
}

export async function redeemClaimCode(
  code: string,
  clientName: string,
  bootstrapUrl: string,
): Promise<ServerConnection> {
  const res = await fetch(`${bootstrapUrl}/api/v1/claims/${code.toUpperCase()}/redeem`, {
    method: 'POST',
  })
  if (!res.ok) {
    if (res.status === 404) throw new Error('Invalid or expired claim code')
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
          clientToken: data.clientToken,
        }
        saveConnection(conn)

        // Register client name with the server
        try {
          await fetch(`${url}/api/v1/remote/register`, {
            method: 'POST',
            headers: {
              'Content-Type': 'application/json',
              Authorization: `Bearer ${data.clientToken}`,
            },
            body: JSON.stringify({ clientName }),
          })
        } catch {
          // Non-fatal — name registration is best-effort
        }

        return conn
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
  episodeFileId: number | null
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
}
