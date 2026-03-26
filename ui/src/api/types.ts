export interface SystemStatus {
  version: string
  instanceName: string
  firstBoot: boolean
  modules: EnabledModules
  startTime: string
}

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
}

export interface Series {
  id: number
  title: string
  sortTitle: string
  status: string
  overview: string
  network: string
  year: number
  seasonCount: number
  episodeCount: number
  episodeFileCount: number
  totalEpisodeCount: number
  monitored: boolean
  qualityProfileId: number
  mediaLibraryFolderPath: string
  path: string
  posterUrl: string
  fanartUrl: string
  added: string
  tags: number[]
}

export interface Episode {
  id: number
  seriesId: number
  seasonNumber: number
  episodeNumber: number
  title: string
  airDate: string
  overview: string
  hasFile: boolean
  monitored: boolean
  absoluteEpisodeNumber: number
  episodeFile?: MediaFile
}

export interface Movie {
  id: number
  title: string
  sortTitle: string
  status: string
  overview: string
  studio: string
  year: number
  monitored: boolean
  qualityProfileId: number
  mediaLibraryFolderPath: string
  path: string
  posterUrl: string
  fanartUrl: string
  hasFile: boolean
  movieFile?: MediaFile
  added: string
  tags: number[]
}

export interface MediaFile {
  id: number
  relativePath: string
  size: number
  quality: string | { quality: string; revision?: unknown }
  dateAdded: string
}

/** Extract a display string from a quality value that may be a raw string or a JSONB object. */
export function qualityName(q: unknown): string {
  if (typeof q === 'string') return q
  if (q && typeof q === 'object' && 'quality' in q) return String((q as Record<string, unknown>).quality)
  return 'Unknown'
}

export interface QualityProfile {
  id: number
  name: string
  cutoff: number
  items: QualityProfileItem[]
}

export interface QualityProfileItem {
  id: number
  name: string
  quality: {
    id: number
    name: string
  }
  allowed: boolean
}

export interface QueueItem {
  id: number
  title: string
  status: string
  progress: number
  size: number
  sizeLeft: number
  estimatedCompletionTime: string
  downloadClient: string
  mediaType: 'series' | 'movie'
  seriesId?: number
  movieId?: number
  episodeId?: number
  quality: string | { quality: string; revision?: unknown }
}

export interface HistoryEvent {
  id: number
  date: string
  eventType: string
  sourceTitle: string
  quality: string | { quality: string; revision?: unknown }
  indexer: string
  mediaType: 'series' | 'movie'
  seriesId?: number
  movieId?: number
  episodeId?: number
}

export interface HistoryResponse {
  page: number
  pageSize: number
  totalRecords: number
  records: HistoryEvent[]
}

export interface ReleaseInfo {
  id: number
  title: string
  indexer: string
  size: number
  quality: string
  seeders: number
  leechers: number
  protocol: string
  age: number
  approved: boolean
  rejections: string[]
}

export interface IndexerConfig {
  id: number
  name: string
  indexerType: string
  protocol: string
  baseUrl: string
  enabled: boolean
  priority: number
  config: Record<string, unknown> | null
  fields: Record<string, string>
}

export interface AvailableIndexer {
  id: string
  name: string
  description: string | null
  privacy: string
  language: string | null
  protocol: string
  urls: string[]
  settings: AvailableSetting[]
}

export interface AvailableSetting {
  name: string
  fieldType: string
  label: string | null
  default: string | null
  options: { value: string; label: string }[] | null
}

export interface DownloadClientConfig {
  id: number
  name: string
  protocol: string
  implementation: string
  host: string
  port: number
  enabled: boolean
  fields: Record<string, string>
}

export interface MediaLibraryFolder {
  id: number
  path: string
  freeSpace: number
  totalSpace: number
  mediaType: 'tv' | 'movie'
}

export interface Tag {
  id: number
  label: string
}

export interface NamingConfig {
  id: number
  renameEpisodes: boolean
  replaceIllegalCharacters: boolean
  standardEpisodeFormat: string
  dailyEpisodeFormat: string
  animeEpisodeFormat: string
  seriesFolderFormat: string
  seasonFolderFormat: string
  movieFolderFormat: string
  movieFileFormat: string
}

export interface CalendarEntry {
  id: number
  seriesId: number
  seriesTitle: string
  seasonNumber: number
  episodeNumber: number
  title: string
  airDate: string
  monitored: boolean
  hasFile: boolean
}

export interface SetupInit {
  modules: {
    tvManagement: boolean
    movieManagement: boolean
    torrentEmbedded: boolean
    usenetEmbedded: boolean
    indexarrSidecar: boolean
    plexIntegration: boolean
    streaming: boolean
  }
  mediaLibraryFolders?: Array<{ path: string; mediaType: string }>
  indexarr?: {
    url: string
    apiKey: string
  }
}

// ─── Streaming ────────────────────────────────────────────────────

export interface MediaStreamInfo {
  container: string
  durationSecs: number
  bitrate: number
  videoStreams: VideoStreamInfo[]
  audioStreams: AudioStreamInfo[]
  subtitleStreams: SubtitleStreamInfo[]
}

export interface VideoStreamInfo {
  index: number
  codec: string
  width: number
  height: number
  bitrate: number
  profile: string
  level: number
  isHdr: boolean
  frameRate: number
}

export interface AudioStreamInfo {
  index: number
  codec: string
  channels: number
  language: string
  title: string
  bitrate: number
  isDefault: boolean
}

export interface SubtitleStreamInfo {
  index: number
  codec: string
  language: string
  title: string
  forced: boolean
  isDefault: boolean
}

export interface StreamSession {
  sessionId: string
  mediaFileId: number
  sessionType: 'direct' | 'transcode'
  status: string
  startedAt: string
  lastActivity: string
  transcodeProgress: number | null
}

export interface TranscodeRequest {
  videoStreamIndex: number
  audioStreamIndex: number
  subtitleStreamIndex?: number
  maxWidth?: number
  maxHeight?: number
  videoBitrate?: number
}

export interface TranscodeResponse {
  sessionId: string
  playlistUrl: string
}

export interface MigrationResult {
  seriesImported: number
  moviesImported: number
  episodesImported: number
  mediaFilesImported: number
  qualityProfilesImported: number
  indexersImported: number
  downloadClientsImported: number
  historyEventsImported: number
  blocklistEntriesImported: number
  warnings: string[]
  dryRun: boolean
}

export interface SeriesLookup {
  title: string
  year: number
  overview: string
  network: string
  tvdbId: number
  posterUrl: string
  seasonCount: number
}

export interface MovieLookup {
  title: string
  year: number
  overview: string
  studio: string
  tmdbId: number
  posterUrl: string
}
