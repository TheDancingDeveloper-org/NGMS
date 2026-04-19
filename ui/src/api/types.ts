export interface SystemStatus {
  version: string
  instanceName: string
  firstBoot: boolean
  authMethod: string
  modules: EnabledModules
  indexarrAvailable: boolean
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
  remoteAccess: boolean
  stremioAddon: boolean
  davStreaming: boolean
}

export interface CurrentUser {
  id: number
  username: string
  displayName: string
  role: string
  avatarUrl: string | null
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
  tmdbId?: number
  tvdbId?: number
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
  tmdbId?: number
}

export interface MediaFile {
  id: number
  relativePath: string
  size: number
  quality: string | { quality: string; revision?: unknown }
  dateAdded: string
  mediaType?: string
  sceneName?: string | null
  releaseGroup?: string | null
  releaseHash?: string | null
  edition?: string | null
  languages?: unknown
  mediaInfo?: MediaStreamInfo | null
  indexerFlags?: number
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
  upgradeAllowed: boolean
  minFormatScore: number
  cutoffFormatScore: number
  minUpgradeFormatScore: number
  items: QualityProfileItem[]
  mediaType: string | null
  language: number
  formatItems: ProfileFormatItem[]
}

export interface QualityProfileItem {
  id?: number // Group ID (1000+) — only on group entries
  name?: string // Group name — only on group entries
  quality: {
    id: number
    name: string
  } | null
  allowed: boolean
  items?: QualityProfileItem[]
}

export interface ProfileFormatItem {
  format: number
  name: string
  score: number
}

export interface CustomFormat {
  id: number
  name: string
  specifications: FormatSpecification[]
  includeCustomFormatWhenRenaming: boolean
}

export interface FormatSpecification {
  field: FormatField
  pattern: string
  negate: boolean
  required: boolean
}

export type FormatField = 'releaseName' | 'quality' | 'language' | 'releaseGroup' | 'indexerFlag' | 'size'

export interface MatchedFormat {
  formatId: number
  formatName: string
  score: number
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
  downloadId?: string
  protocol?: 'usenet' | 'torrent'
  errorMessage?: string
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
  downloadClient?: string
  data?: Record<string, unknown>
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

export interface DownloadDecision {
  approved: boolean
  release: {
    guid: string
    title: string
    downloadUrl: string | null
    infoUrl: string | null
    indexerId: number
    indexerName: string
    protocol: string
    size: number
    ageDays: number
    publishDate: string
    infoHash: string | null
    magnetUrl: string | null
    seeders: number | null
    leechers: number | null
    nzbUrl: string | null
    categories: number[]
    indexerFlags: string[]
    indexerPriority: number
  }
  rejections: { reason: string; rejectionType: string }[]
  customFormatScore: number
  matchedFormats: MatchedFormat[]
}

export interface FreehandSearchResult {
  guid: string
  title: string
  downloadUrl: string | null
  infoUrl: string | null
  indexerId: number
  indexerName: string
  protocol: string
  size: number
  ageDays: number
  publishDate: number
  infoHash: string | null
  magnetUrl: string | null
  seeders: number | null
  leechers: number | null
  nzbUrl: string | null
  categories: number[]
  indexerFlags: string[]
  quality: string
}

export interface IndexerConfig {
  id: number
  name: string
  indexerType: string
  protocol: string
  baseUrl: string
  apiKey: string | null
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
  clientType: string
  protocol: string
  config: Record<string, unknown>
  enabled: boolean
  priority: number
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
  episodeId: number
  seriesId: number
  seriesTitle: string
  seasonNumber: number
  episodeNumber: number
  episodeTitle: string | null
  airDateUtc: string | null
  monitored: boolean
  hasFile: boolean
  posterUrl: string | null
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
    stremioAddon: boolean
    davStreaming: boolean
    notifications: boolean
  }
  mediaLibraryFolders?: Array<{ path: string; mediaType: string }>
  pathMappings?: Array<{ from: string; to: string; mediaType?: string }>
  indexarr?: {
    url: string
    apiKey: string
  }
}

// ─── RSS ─────────────────────────────────────────────────────────

export interface RssFeed {
  id: number
  name: string
  url: string
  protocol: 'usenet' | 'torrent'
  pollIntervalSecs: number
  category: string | null
  filterRegex: string | null
  enabled: boolean
  autoDownload: boolean
  createdAt: string
  updatedAt: string
}

export interface RssItem {
  id: string
  feedId: number
  title: string
  url: string | null
  publishedAt: string | null
  firstSeenAt: string
  downloaded: boolean
  downloadedAt: string | null
  category: string | null
  sizeBytes: number | null
}

export interface RssRule {
  id: number
  name: string
  feedIds: number[]
  category: string | null
  priority: number
  matchRegex: string
  enabled: boolean
  createdAt: string
}

// ─── Activities & Notifications ──────────────────────────────────

export interface SystemActivity {
  id: number
  activityType: string
  status: 'running' | 'completed' | 'failed'
  title: string
  detail: string | null
  progress: Record<string, unknown> | null
  result: Record<string, unknown> | null
  error: string | null
  startedAt: string
  updatedAt: string
  completedAt: string | null
}

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

export interface UnifiedSession {
  id: string
  source: 'stackarr' | 'plex'
  title: string | null
  user: string | null
  player: string | null
  state: string
  progressPercent: number | null
  sessionType: string
  startedAt: string | null
  videoCodec: string | null
  audioCodec: string | null
  resolution: string | null
  bitrate: number | null
  videoDecision: string | null
  audioDecision: string | null
  transcodeSpeed: number | null
  platform: string | null
  isLocal: boolean | null
}

export interface PlexEvent {
  id: number
  eventType: string
  plexServerId: number | null
  userName: string | null
  title: string | null
  ratingKey: string | null
  metadata: Record<string, unknown> | null
  thumbUrl: string | null
  receivedAt: string
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

export interface ClaimCodeResponse {
  code: string
  expiresInSecs: number
  clientToken: string
}

export interface RemoteClient {
  id: number
  token: string
  clientName: string | null
  createdAt: string
  lastSeen: string | null
}

export interface MigrationResult {
  seriesImported: number
  moviesImported: number
  episodesImported: number
  mediaFilesImported: number
  qualityProfilesImported: number
  customFormatsImported: number
  formatScoresImported: number
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
  tmdbId: number
  posterUrl: string | null
  seasonCount: number
}

export interface MovieLookup {
  title: string
  year: number
  overview: string
  studio: string
  tmdbId: number
  posterUrl: string | null
}

// ─── TMDB / Discover ─────────────────────────────────────────────

export interface TmdbSearchResults<T> {
  page: number
  total_pages: number
  total_results: number
  results: T[]
}

export interface TmdbTrendingItem {
  id: number
  media_type: string
  title?: string
  name?: string
  overview?: string
  release_date?: string
  first_air_date?: string
  poster_path?: string
  backdrop_path?: string
  genre_ids: number[]
  vote_average: number
  vote_count: number
  popularity: number
  original_language?: string
}

export interface TmdbMovie {
  id: number
  title: string
  overview?: string
  release_date?: string
  poster_path?: string
  backdrop_path?: string
  genre_ids: number[]
  vote_average: number
  vote_count: number
  popularity: number
  original_language?: string
}

export interface TmdbSeries {
  id: number
  name: string
  overview?: string
  first_air_date?: string
  poster_path?: string
  backdrop_path?: string
  genre_ids: number[]
  vote_average: number
  vote_count: number
  popularity: number
  original_language?: string
}

export interface TmdbGenre {
  id: number
  name: string
}

export interface DiscoverSlider {
  id: number
  sliderType: string
  displayOrder: number
  isBuiltIn: boolean
  enabled: boolean
  title?: string
  customData?: Record<string, unknown>
  createdAt: string
  updatedAt: string
}

export interface WatchlistItem {
  id: number
  tmdb_id: number
  media_type: string
  plex_rating_key?: string
  auto_requested: boolean
  created_at: string
}

// ─── Plex ─────────────────────────────────────────────────────────

export interface PlexServer {
  id: number
  name: string
  machineId: string | null
  ip: string
  port: number
  useSsl: boolean
  authToken: string | null
  webAppUrl: string | null
  webhookSecret: string | null
  createdAt: string
  updatedAt: string
}

export interface PlexLibrary {
  id: number
  plexServerId: number
  sectionId: string
  name: string
  enabled: boolean
  libraryType: string
  lastScan: string | null
}

export interface PlexTvUser {
  id: number
  uuid: string
  username: string
  email: string | null
  thumb: string | null
  title: string | null
}

export interface PlexResource {
  name: string
  clientIdentifier: string
  provides: string
  connections: PlexConnection[]
}

export interface PlexConnection {
  uri: string
  local: boolean
  protocol: string
}

// ─── Media Requests ───────────────────────────────────────────────

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

const TMDB_IMAGE_BASE = 'https://image.tmdb.org/t/p'

export function tmdbPosterUrl(path: string | null | undefined, size: 'w185' | 'w342' | 'w500' = 'w342'): string | null {
  if (!path) return null
  return `/api/v1/images/${TMDB_IMAGE_BASE}/${size}${path}`
}

export function tmdbBackdropUrl(path: string | null | undefined, size: 'w780' | 'w1280' | 'original' = 'w1280'): string | null {
  if (!path) return null
  return `/api/v1/images/${TMDB_IMAGE_BASE}/${size}${path}`
}

/** Get display title from a trending item (movie=title, tv=name). */
export function tmdbDisplayTitle(item: TmdbTrendingItem): string {
  return item.title || item.name || 'Unknown'
}

/** Get release year from a trending item. */
export function tmdbYear(item: TmdbTrendingItem | TmdbMovie | TmdbSeries): string {
  const date = 'release_date' in item
    ? (item as TmdbTrendingItem).release_date || (item as TmdbTrendingItem).first_air_date
    : 'first_air_date' in item
      ? (item as TmdbSeries).first_air_date
      : (item as TmdbMovie).release_date
  return date?.substring(0, 4) || ''
}

// ── DAV Streaming types ────────────────────────────────────────────────────

export interface DavItem {
  id: string
  name: string
  path: string
  fileSize: number | null
  isDirectory: boolean
  itemType: number
  subType: number
  createdAt: string
}

export interface DavStreamRequest {
  nzbUrl: string
  name: string
  category?: string
}

export interface DavStreamResponse {
  davPath: string
  itemsCreated: number
  jobDirId: string
}

export interface DavHistoryItem {
  id: string
  createdAt: string
  fileName: string
  jobName: string
  category: string
  downloadStatus: number
  totalSegmentBytes: number
  downloadTimeSeconds: number
  failMessage: string | null
}

export interface DavStatus {
  enabled: boolean
  providerConnections: number
  itemsCount: number
  queueCount: number
  historyCount: number
}
