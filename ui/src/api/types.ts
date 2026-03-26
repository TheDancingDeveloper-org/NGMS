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
  rootFolderPath: string
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
  rootFolderPath: string
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
  quality: string
  dateAdded: string
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
  quality: string
}

export interface HistoryEvent {
  id: number
  date: string
  eventType: string
  sourceTitle: string
  quality: string
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
  protocol: string
  baseUrl: string
  enabled: boolean
  fields: Record<string, string>
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

export interface RootFolder {
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
  }
  rootFolders?: Array<{ path: string; mediaType: string }>
  indexarr?: {
    url: string
    apiKey: string
  }
}

export interface MigrationResult {
  success: boolean
  imported: {
    series: number
    movies: number
    indexers: number
  }
  errors: string[]
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
