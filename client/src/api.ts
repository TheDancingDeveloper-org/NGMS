const BASE = '/api/v1'

async function get<T>(path: string): Promise<T> {
  const res = await fetch(`${BASE}${path}`)
  if (!res.ok) throw new Error(`${res.status} ${res.statusText}`)
  return res.json()
}

async function post<T>(path: string, body?: unknown): Promise<T> {
  const res = await fetch(`${BASE}${path}`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: body ? JSON.stringify(body) : undefined,
  })
  if (!res.ok) throw new Error(`${res.status} ${res.statusText}`)
  return res.json()
}

// Types — matches the actual StackArr API response shapes

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
  directPlayUrl: (fileId: number) => `${BASE}/stream/${fileId}/direct`,
  hlsUrl: (fileId: number, sessionId: string) =>
    `${BASE}/stream/${fileId}/hls/${sessionId}/master.m3u8`,
  subtitleUrl: (fileId: number, trackIndex: number) =>
    `${BASE}/stream/${fileId}/subtitles/${trackIndex}`,
}
