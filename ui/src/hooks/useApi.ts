import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query'
import { apiFetch } from '../api/client'
import type {
  CurrentUser,
  SystemStatus,
  Series,
  Movie,
  Episode,
  QueueItem,
  HistoryEvent,
  HistoryResponse,
  QualityProfile,
  CalendarEntry,
  SetupInit,
  MigrationResult,
  SeriesLookup,
  MovieLookup,
  MediaLibraryFolder,
  FreehandSearchResult,
  IndexerConfig,
  DownloadClientConfig,
  NamingConfig,
  Tag,
  MediaStreamInfo,
  StreamSession,
  TranscodeRequest,
  TranscodeResponse,
  UnifiedSession,
  PlexEvent,
  TmdbSearchResults,
  TmdbTrendingItem,
  TmdbMovie,
  TmdbSeries,
  TmdbGenre,
  DiscoverSlider,
  WatchlistItem,
  PlexServer,
  PlexLibrary,
  PlexTvUser,
  PlexResource,
  MediaRequest,
  SystemActivity,
  UserNotification,
  RssFeed,
  RssItem,
  RssRule,
  DownloadDecision,
  DavItem,
  DavStatus,
  DavHistoryItem,
  DavStreamRequest,
  DavStreamResponse,
} from '../api/types'

// ─── System ───────────────────────────────────────────────────────

export function useSystemStatus() {
  return useQuery({
    queryKey: ['system', 'status'],
    queryFn: () => apiFetch<SystemStatus>('/system/status'),
  })
}

export function useCurrentUser() {
  return useQuery({
    queryKey: ['auth', 'me'],
    queryFn: () => apiFetch<CurrentUser>('/auth/me'),
    staleTime: 5 * 60 * 1000,
  })
}

// ─── Series ───────────────────────────────────────────────────────

export function useSeries() {
  return useQuery({
    queryKey: ['series'],
    queryFn: () => apiFetch<Series[]>('/series'),
  })
}

export function useSeriesDetail(id: number) {
  return useQuery({
    queryKey: ['series', id],
    queryFn: () => apiFetch<Series>(`/series/${id}`),
    enabled: id > 0,
  })
}

export function useEpisodes(seriesId: number) {
  return useQuery({
    queryKey: ['episodes', seriesId],
    queryFn: () => apiFetch<Episode[]>(`/series/${seriesId}/episodes`),
    enabled: seriesId > 0,
  })
}

export function useSeriesLookup(term: string) {
  return useQuery({
    queryKey: ['series', 'lookup', term],
    queryFn: () => apiFetch<SeriesLookup[]>(`/series/lookup?term=${encodeURIComponent(term)}`),
    enabled: term.length >= 2,
  })
}

export function useAddSeries() {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: (data: Partial<Series>) =>
      apiFetch<Series>('/series', { method: 'POST', body: JSON.stringify(data) }),
    onSuccess: () => { void qc.invalidateQueries({ queryKey: ['series'] }) },
  })
}

export function useDeleteSeries() {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: (id: number) =>
      apiFetch<void>(`/series/${id}`, { method: 'DELETE' }),
    onSuccess: () => { void qc.invalidateQueries({ queryKey: ['series'] }) },
  })
}

export function useToggleSeriesMonitor() {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: ({ id, monitored }: { id: number; monitored: boolean }) =>
      apiFetch<Series>(`/series/${id}`, {
        method: 'PUT',
        body: JSON.stringify({ monitored }),
      }),
    onSuccess: (_data, vars) => {
      void qc.invalidateQueries({ queryKey: ['series', vars.id] })
      void qc.invalidateQueries({ queryKey: ['series'] })
    },
  })
}

export function useUpdateSeries() {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: ({ id, ...data }: { id: number; qualityProfileId?: number; monitored?: boolean }) =>
      apiFetch<Series>(`/series/${id}`, {
        method: 'PUT',
        body: JSON.stringify(data),
      }),
    onSuccess: (_data, vars) => {
      void qc.invalidateQueries({ queryKey: ['series', vars.id] })
      void qc.invalidateQueries({ queryKey: ['series'] })
    },
  })
}

export function useBulkUpdateSeries() {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: (data: { seriesIds: number[]; qualityProfileId?: number; monitored?: boolean }) =>
      apiFetch<{ updated: number }>('/series/bulk', {
        method: 'PUT',
        body: JSON.stringify(data),
      }),
    onSuccess: () => {
      void qc.invalidateQueries({ queryKey: ['series'] })
    },
  })
}

export function useToggleEpisodeMonitor() {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: ({ id, monitored }: { id: number; monitored: boolean }) =>
      apiFetch<Episode>(`/episode/${id}`, {
        method: 'PUT',
        body: JSON.stringify({ monitored }),
      }),
    onSuccess: (_data, vars) => {
      void qc.invalidateQueries({ queryKey: ['episodes', vars.id] })
    },
  })
}

export function useSearchEpisode() {
  return useMutation({
    mutationFn: (episodeId: number) =>
      apiFetch<void>('/command', {
        method: 'POST',
        body: JSON.stringify({ name: 'EpisodeSearch', episodeIds: [episodeId] }),
      }),
  })
}

export function useSeriesMissingSearch() {
  return useMutation({
    mutationFn: (seriesId: number) =>
      apiFetch<void>('/command', {
        method: 'POST',
        body: JSON.stringify({ name: 'SeriesMissingSearch', seriesId }),
      }),
  })
}

export function useSeriesCutoffSearch() {
  return useMutation({
    mutationFn: (seriesId: number) =>
      apiFetch<void>('/command', {
        method: 'POST',
        body: JSON.stringify({ name: 'SeriesCutoffSearch', seriesId }),
      }),
  })
}

export type MonitorStrategy = 'all' | 'latestSeason' | 'firstSeason' | 'upcoming' | 'none'

export function useSetSeasonMonitor() {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: ({ seriesId, seasonNumber, monitored }: { seriesId: number; seasonNumber: number; monitored: boolean }) =>
      apiFetch<void>(`/series/${seriesId}/seasons/${seasonNumber}/monitor`, {
        method: 'PUT',
        body: JSON.stringify({ monitored }),
      }),
    onSuccess: (_data, vars) => {
      void qc.invalidateQueries({ queryKey: ['episodes', vars.seriesId] })
    },
  })
}

export function useApplyMonitorStrategy() {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: ({ seriesId, monitorStrategy }: { seriesId: number; monitorStrategy: MonitorStrategy }) =>
      apiFetch<void>(`/series/${seriesId}/monitor`, {
        method: 'PUT',
        body: JSON.stringify({ monitorStrategy }),
      }),
    onSuccess: (_data, vars) => {
      void qc.invalidateQueries({ queryKey: ['episodes', vars.seriesId] })
      void qc.invalidateQueries({ queryKey: ['series', vars.seriesId] })
    },
  })
}

// ─── Movies ───────────────────────────────────────────────────────

export function useMovies() {
  return useQuery({
    queryKey: ['movies'],
    queryFn: () => apiFetch<Movie[]>('/movies'),
  })
}

export function useMovieDetail(id: number) {
  return useQuery({
    queryKey: ['movie', id],
    queryFn: () => apiFetch<Movie>(`/movies/${id}`),
    enabled: id > 0,
  })
}

export function useMovieLookup(term: string) {
  return useQuery({
    queryKey: ['movie', 'lookup', term],
    queryFn: () => apiFetch<MovieLookup[]>(`/movies/lookup?term=${encodeURIComponent(term)}`),
    enabled: term.length >= 2,
  })
}

export function useAddMovie() {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: (data: Partial<Movie>) =>
      apiFetch<Movie>('/movies', { method: 'POST', body: JSON.stringify(data) }),
    onSuccess: () => { void qc.invalidateQueries({ queryKey: ['movies'] }) },
  })
}

export function useDeleteMovie() {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: (id: number) =>
      apiFetch<void>(`/movies/${id}`, { method: 'DELETE' }),
    onSuccess: () => { void qc.invalidateQueries({ queryKey: ['movies'] }) },
  })
}

export function useUpdateMovie() {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: ({ id, ...data }: { id: number; qualityProfileId?: number; monitored?: boolean }) =>
      apiFetch<Movie>(`/movies/${id}`, {
        method: 'PUT',
        body: JSON.stringify(data),
      }),
    onSuccess: (_data, vars) => {
      void qc.invalidateQueries({ queryKey: ['movie', vars.id] })
      void qc.invalidateQueries({ queryKey: ['movies'] })
    },
  })
}

export function useBulkUpdateMovies() {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: (data: { movieIds: number[]; qualityProfileId?: number; monitored?: boolean }) =>
      apiFetch<{ updated: number }>('/movies/bulk', {
        method: 'PUT',
        body: JSON.stringify(data),
      }),
    onSuccess: () => {
      void qc.invalidateQueries({ queryKey: ['movies'] })
    },
  })
}

export function useSearchMovie() {
  return useMutation({
    mutationFn: (movieId: number) =>
      apiFetch<void>('/command', {
        method: 'POST',
        body: JSON.stringify({ name: 'MovieSearch', movieIds: [movieId] }),
      }),
  })
}

// ─── Queue ────────────────────────────────────────────────────────

export function useQueue() {
  return useQuery({
    queryKey: ['queue'],
    queryFn: () => apiFetch<QueueItem[]>('/queue'),
    refetchInterval: 5000,
    refetchIntervalInBackground: false,
  })
}

// ─── History ──────────────────────────────────────────────────────

export function useHistory(page: number, pageSize = 20) {
  return useQuery({
    queryKey: ['history', page, pageSize],
    queryFn: () => apiFetch<HistoryResponse>(`/history?page=${page}&pageSize=${pageSize}`),
  })
}

export function useEventStream(enabled = true) {
  return useQuery({
    queryKey: ['history', 'stream'],
    queryFn: () => apiFetch<HistoryEvent[]>('/history/stream?limit=30'),
    refetchInterval: enabled ? 5000 : false,
    refetchIntervalInBackground: false,
  })
}

// ─── Calendar ─────────────────────────────────────────────────────

export function useCalendar(start: string, end: string) {
  return useQuery({
    queryKey: ['calendar', start, end],
    queryFn: () => apiFetch<CalendarEntry[]>(`/calendar?start=${start}&end=${end}`),
  })
}

// ─── Quality Profiles ─────────────────────────────────────────────

export function useQualityProfiles() {
  return useQuery({
    queryKey: ['qualityprofile'],
    queryFn: () => apiFetch<QualityProfile[]>('/qualityprofile'),
  })
}

// ─── Media Library Folders ────────────────────────────────────────

export function useMediaLibraryFolders() {
  return useQuery({
    queryKey: ['medialibraryfolder'],
    queryFn: () => apiFetch<MediaLibraryFolder[]>('/medialibraryfolder'),
  })
}

// ─── Interactive Release Search ───────────────────────────────────

export function useInteractiveSearch(params: {
  term: string
  mediaType?: string
  qualityProfileId?: number
  seriesId?: number
  movieId?: number
  episodeId?: number
} | null) {
  const qs = new URLSearchParams()
  if (params) {
    qs.set('term', params.term)
    if (params.mediaType) qs.set('mediaType', params.mediaType)
    if (params.qualityProfileId) qs.set('qualityProfileId', String(params.qualityProfileId))
    if (params.seriesId) qs.set('seriesId', String(params.seriesId))
    if (params.movieId) qs.set('movieId', String(params.movieId))
    if (params.episodeId) qs.set('episodeId', String(params.episodeId))
  }
  return useQuery({
    queryKey: ['release', 'search', params],
    queryFn: () => apiFetch<DownloadDecision[]>(`/release?${qs}`),
    enabled: !!params && params.term.length > 0,
  })
}

export function useGrabRelease() {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: (data: {
      guid: string
      indexerId: number
      title: string
      downloadUrl: string
      protocol: string
      size: number
      mediaId?: number
      mediaType?: string
      episodeId?: number
    }) =>
      apiFetch<{ success: boolean; downloadClientId: number; downloadId: string }>('/release', {
        method: 'POST',
        body: JSON.stringify(data),
      }),
    onSuccess: () => {
      void qc.invalidateQueries({ queryKey: ['queue'] })
    },
  })
}

// ─── Freehand Search ──────────────────────────────────────────────

export function useSearchReleases(query: string, indexerIds?: number[], indexarrOnly = false) {
  const params = new URLSearchParams({ query })
  if (indexerIds?.length) params.set('indexerIds', indexerIds.join(','))
  if (indexarrOnly) params.set('indexarrOnly', 'true')
  return useQuery({
    queryKey: ['search', query, indexerIds, indexarrOnly],
    queryFn: () => apiFetch<FreehandSearchResult[]>(`/search?${params}`),
    enabled: query.length > 0,
  })
}

// ─── Indexers ─────────────────────────────────────────────────────

export function useIndexers() {
  return useQuery({
    queryKey: ['indexer'],
    queryFn: () => apiFetch<IndexerConfig[]>('/indexer'),
  })
}

// ─── Download Clients ─────────────────────────────────────────────

export function useDownloadClients() {
  return useQuery({
    queryKey: ['downloadclient'],
    queryFn: () => apiFetch<DownloadClientConfig[]>('/downloadclient'),
  })
}

// ─── Tags ─────────────────────────────────────────────────────────

export function useTags() {
  return useQuery({
    queryKey: ['tag'],
    queryFn: () => apiFetch<Tag[]>('/tag'),
  })
}

// ─── Naming ───────────────────────────────────────────────────────

export function useNamingConfig() {
  return useQuery({
    queryKey: ['naming'],
    queryFn: () => apiFetch<NamingConfig>('/config/naming'),
  })
}

// ─── Setup ────────────────────────────────────────────────────────

export function useSetupInit() {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: (data: SetupInit) =>
      apiFetch<{ success: boolean; apiKey: string; recoveryPhrase?: string }>('/setup/init', {
        method: 'POST',
        body: JSON.stringify(data),
      }),
    onSuccess: () => { void qc.invalidateQueries({ queryKey: ['system', 'status'] }) },
  })
}

// ─── Streaming ────────────────────────────────────────────────────

export function useStreamInfo(mediaFileId: number) {
  return useQuery({
    queryKey: ['stream', 'info', mediaFileId],
    queryFn: () => apiFetch<MediaStreamInfo>(`/stream/${mediaFileId}/info`),
    enabled: mediaFileId > 0,
  })
}

export function useStreamSessions() {
  return useQuery({
    queryKey: ['stream', 'sessions'],
    queryFn: () => apiFetch<StreamSession[]>('/stream/sessions'),
    refetchInterval: 5000,
    refetchIntervalInBackground: false,
  })
}

export function useStartTranscode() {
  return useMutation({
    mutationFn: ({ mediaFileId, ...body }: TranscodeRequest & { mediaFileId: number }) =>
      apiFetch<TranscodeResponse>(`/stream/${mediaFileId}/transcode`, {
        method: 'POST',
        body: JSON.stringify(body),
      }),
  })
}

export function useStopStreamSession() {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: (sessionId: string) =>
      apiFetch<void>(`/stream/sessions/${sessionId}`, { method: 'DELETE' }),
    onSuccess: () => {
      void qc.invalidateQueries({ queryKey: ['stream', 'sessions'] })
    },
  })
}

export function useUnifiedSessions() {
  return useQuery({
    queryKey: ['stream', 'sessions', 'unified'],
    queryFn: () => apiFetch<UnifiedSession[]>('/stream/sessions/unified'),
    refetchInterval: 5000,
    refetchIntervalInBackground: false,
  })
}

// ─── Discover ────────────────────────────────────────────────────

export function useTrending(mediaType = 'all', timeWindow = 'day') {
  return useQuery({
    queryKey: ['discover', 'trending', mediaType, timeWindow],
    queryFn: () =>
      apiFetch<TmdbSearchResults<TmdbTrendingItem>>(
        `/discover/trending?mediaType=${mediaType}&timeWindow=${timeWindow}`,
      ),
    staleTime: 60 * 60 * 1000, // 1 hour
  })
}

export function usePopularMovies(page = 1) {
  return useQuery({
    queryKey: ['discover', 'popular-movies', page],
    queryFn: () =>
      apiFetch<TmdbSearchResults<TmdbMovie>>(
        `/discover/movies?sortBy=popularity.desc&page=${page}`,
      ),
    staleTime: 60 * 60 * 1000,
  })
}

export function usePopularTv(page = 1) {
  return useQuery({
    queryKey: ['discover', 'popular-tv', page],
    queryFn: () =>
      apiFetch<TmdbSearchResults<TmdbSeries>>(
        `/discover/tv?sortBy=popularity.desc&page=${page}`,
      ),
    staleTime: 60 * 60 * 1000,
  })
}

export function useUpcomingMovies() {
  return useQuery({
    queryKey: ['discover', 'upcoming-movies'],
    queryFn: () => apiFetch<TmdbSearchResults<TmdbMovie>>('/discover/movies/upcoming'),
    staleTime: 60 * 60 * 1000,
  })
}

export function useUpcomingTv() {
  return useQuery({
    queryKey: ['discover', 'upcoming-tv'],
    queryFn: () => apiFetch<TmdbSearchResults<TmdbSeries>>('/discover/tv/upcoming'),
    staleTime: 60 * 60 * 1000,
  })
}

export function useTopRatedMovies(page = 1) {
  return useQuery({
    queryKey: ['discover', 'top-rated-movies', page],
    queryFn: () =>
      apiFetch<TmdbSearchResults<TmdbMovie>>(
        `/discover/movies?sortBy=vote_average.desc&voteCountGte=1000&page=${page}`,
      ),
    staleTime: 60 * 60 * 1000,
  })
}

export function useTopRatedTv(page = 1) {
  return useQuery({
    queryKey: ['discover', 'top-rated-tv', page],
    queryFn: () =>
      apiFetch<TmdbSearchResults<TmdbSeries>>(
        `/discover/tv?sortBy=vote_average.desc&voteCountGte=500&page=${page}`,
      ),
    staleTime: 60 * 60 * 1000,
  })
}

export function useRecentMovies(page = 1) {
  return useQuery({
    queryKey: ['discover', 'recent-movies', page],
    queryFn: () =>
      apiFetch<TmdbSearchResults<TmdbMovie>>(
        `/discover/movies?sortBy=primary_release_date.desc&voteCountGte=50&page=${page}`,
      ),
    staleTime: 60 * 60 * 1000,
  })
}

export function useRecentTv(page = 1) {
  return useQuery({
    queryKey: ['discover', 'recent-tv', page],
    queryFn: () =>
      apiFetch<TmdbSearchResults<TmdbSeries>>(
        `/discover/tv?sortBy=first_air_date.desc&voteCountGte=20&page=${page}`,
      ),
    staleTime: 60 * 60 * 1000,
  })
}

export function useMoviesByGenre(genreId: number, page = 1) {
  return useQuery({
    queryKey: ['discover', 'movies-genre', genreId, page],
    queryFn: () =>
      apiFetch<TmdbSearchResults<TmdbMovie>>(
        `/discover/movies/genre/${genreId}?sortBy=popularity.desc&page=${page}`,
      ),
    enabled: genreId > 0,
    staleTime: 60 * 60 * 1000,
  })
}

export function useTvByGenre(genreId: number, page = 1) {
  return useQuery({
    queryKey: ['discover', 'tv-genre', genreId, page],
    queryFn: () =>
      apiFetch<TmdbSearchResults<TmdbSeries>>(
        `/discover/tv/genre/${genreId}?sortBy=popularity.desc&page=${page}`,
      ),
    enabled: genreId > 0,
    staleTime: 60 * 60 * 1000,
  })
}

export function useMovieRecommendations(tmdbId: number) {
  return useQuery({
    queryKey: ['discover', 'movie-recs', tmdbId],
    queryFn: () =>
      apiFetch<TmdbSearchResults<TmdbMovie>>(`/discover/movies/${tmdbId}/recommendations`),
    enabled: tmdbId > 0,
    staleTime: 60 * 60 * 1000,
  })
}

export function useMovieSimilar(tmdbId: number) {
  return useQuery({
    queryKey: ['discover', 'movie-similar', tmdbId],
    queryFn: () =>
      apiFetch<TmdbSearchResults<TmdbMovie>>(`/discover/movies/${tmdbId}/similar`),
    enabled: tmdbId > 0,
    staleTime: 60 * 60 * 1000,
  })
}

export function useTvRecommendations(tmdbId: number) {
  return useQuery({
    queryKey: ['discover', 'tv-recs', tmdbId],
    queryFn: () =>
      apiFetch<TmdbSearchResults<TmdbSeries>>(`/discover/tv/${tmdbId}/recommendations`),
    enabled: tmdbId > 0,
    staleTime: 60 * 60 * 1000,
  })
}

export function useTvSimilar(tmdbId: number) {
  return useQuery({
    queryKey: ['discover', 'tv-similar', tmdbId],
    queryFn: () =>
      apiFetch<TmdbSearchResults<TmdbSeries>>(`/discover/tv/${tmdbId}/similar`),
    enabled: tmdbId > 0,
    staleTime: 60 * 60 * 1000,
  })
}

export function useMovieGenres() {
  return useQuery({
    queryKey: ['discover', 'genres', 'movie'],
    queryFn: () => apiFetch<{ genres: TmdbGenre[] }>('/discover/genres/movie'),
    staleTime: 24 * 60 * 60 * 1000,
  })
}

export function useTvGenres() {
  return useQuery({
    queryKey: ['discover', 'genres', 'tv'],
    queryFn: () => apiFetch<{ genres: TmdbGenre[] }>('/discover/genres/tv'),
    staleTime: 24 * 60 * 60 * 1000,
  })
}

export function useDiscoverSliders() {
  return useQuery({
    queryKey: ['discover', 'sliders'],
    queryFn: () => apiFetch<DiscoverSlider[]>('/discover/sliders'),
  })
}

export function useWatchlist() {
  return useQuery({
    queryKey: ['watchlist'],
    queryFn: () => apiFetch<WatchlistItem[]>('/plex/watchlist'),
  })
}

export function useSyncWatchlist() {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: () =>
      apiFetch<void>('/plex/watchlist/sync', { method: 'POST' }),
    onSuccess: () => { void qc.invalidateQueries({ queryKey: ['watchlist'] }) },
  })
}

// ─── Plex ─────────────────────────────────────────────────────────

export function usePlexServers() {
  return useQuery({
    queryKey: ['plex', 'servers'],
    queryFn: () => apiFetch<PlexServer[]>('/plex/servers'),
  })
}

export function useAddPlexServer() {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: (data: { name?: string; ip: string; port?: number; useSsl?: boolean; authToken: string; webAppUrl?: string }) =>
      apiFetch<PlexServer>('/plex/servers', { method: 'POST', body: JSON.stringify(data) }),
    onSuccess: () => { void qc.invalidateQueries({ queryKey: ['plex', 'servers'] }) },
  })
}

export function useUpdatePlexServer() {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: ({ id, ...data }: { id: number; name?: string; ip?: string; port?: number; useSsl?: boolean; authToken?: string; webAppUrl?: string }) =>
      apiFetch<PlexServer>(`/plex/servers/${id}`, { method: 'PUT', body: JSON.stringify(data) }),
    onSuccess: () => { void qc.invalidateQueries({ queryKey: ['plex', 'servers'] }) },
  })
}

export function useDeletePlexServer() {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: (id: number) =>
      apiFetch<void>(`/plex/servers/${id}`, { method: 'DELETE' }),
    onSuccess: () => { void qc.invalidateQueries({ queryKey: ['plex', 'servers'] }) },
  })
}

export function usePlexLibraries(serverId: number) {
  return useQuery({
    queryKey: ['plex', 'libraries', serverId],
    queryFn: () => apiFetch<PlexLibrary[]>(`/plex/servers/${serverId}/libraries`),
    enabled: serverId > 0,
  })
}

export function useTogglePlexLibrary() {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: ({ id, enabled }: { id: number; enabled: boolean }) =>
      apiFetch<PlexLibrary>(`/plex/libraries/${id}`, { method: 'PUT', body: JSON.stringify({ enabled }) }),
    onSuccess: () => { void qc.invalidateQueries({ queryKey: ['plex', 'libraries'] }) },
  })
}

export function usePlexFullScan() {
  return useMutation({
    mutationFn: () =>
      apiFetch<{ status: string }>('/plex/scan/full', { method: 'POST' }),
  })
}

export function usePlexRecentScan() {
  return useMutation({
    mutationFn: () =>
      apiFetch<{ status: string }>('/plex/scan/recent', { method: 'POST' }),
  })
}

export function usePlexEvents(eventType?: string) {
  const params = eventType ? `?eventType=${eventType}&limit=100` : '?limit=100'
  return useQuery({
    queryKey: ['plex', 'events', eventType],
    queryFn: () => apiFetch<PlexEvent[]>(`/plex/events${params}`),
    refetchInterval: 10000,
    refetchIntervalInBackground: false,
  })
}

export function useClearPlexEvents() {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: () => apiFetch<{ deleted: number }>('/plex/events', { method: 'DELETE' }),
    onSuccess: () => { void qc.invalidateQueries({ queryKey: ['plex', 'events'] }) },
  })
}

export function useValidatePlexToken() {
  return useMutation({
    mutationFn: (authToken: string) =>
      apiFetch<{ valid: boolean; user: PlexTvUser }>('/plex/auth/validate', {
        method: 'POST',
        body: JSON.stringify({ authToken }),
      }),
  })
}

export function useDiscoverPlexServers() {
  return useMutation({
    mutationFn: (authToken: string) =>
      apiFetch<PlexResource[]>('/plex/auth/servers', {
        method: 'POST',
        body: JSON.stringify({ authToken }),
      }),
  })
}

// ─── Media Requests ───────────────────────────────────────────────

export function useMediaRequests(status?: string) {
  const params = status ? `?status=${status}` : ''
  return useQuery({
    queryKey: ['requests', status],
    queryFn: () => apiFetch<MediaRequest[]>(`/requests${params}`),
    refetchInterval: 15000,
    refetchIntervalInBackground: false,
  })
}

export function usePendingRequestCount() {
  return useQuery({
    queryKey: ['requests', 'pending', 'count'],
    queryFn: () => apiFetch<{ count: number }>('/requests/pending/count'),
    refetchInterval: 30000,
    refetchIntervalInBackground: false,
  })
}

export function useApproveRequest() {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: ({ id, note }: { id: number; note?: string }) =>
      apiFetch<MediaRequest>(`/requests/${id}/approve`, {
        method: 'PUT',
        body: JSON.stringify({ note }),
      }),
    onSuccess: () => {
      void qc.invalidateQueries({ queryKey: ['requests'] })
    },
  })
}

export function useDeclineRequest() {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: ({ id, note }: { id: number; note?: string }) =>
      apiFetch<MediaRequest>(`/requests/${id}/decline`, {
        method: 'PUT',
        body: JSON.stringify({ note }),
      }),
    onSuccess: () => {
      void qc.invalidateQueries({ queryKey: ['requests'] })
    },
  })
}

export function useDeleteRequest() {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: (id: number) =>
      apiFetch<void>(`/requests/${id}`, { method: 'DELETE' }),
    onSuccess: () => {
      void qc.invalidateQueries({ queryKey: ['requests'] })
    },
  })
}

// ─── Activities ──────────────────────────────────────────────────

export function useActivities(enabled = true) {
  return useQuery({
    queryKey: ['activities'],
    queryFn: () => apiFetch<SystemActivity[]>('/activities?includeCompleted=true&limit=20'),
    refetchInterval: enabled ? 5000 : false,
    refetchIntervalInBackground: false,
  })
}

export function useRunningActivityCount() {
  return useQuery({
    queryKey: ['activities', 'running'],
    queryFn: () => apiFetch<{ count: number }>('/activities/running'),
    refetchInterval: 10000,
    refetchIntervalInBackground: false,
  })
}

// ─── Notifications ───────────────────────────────────────────────

export function useNotifications(enabled = true) {
  return useQuery({
    queryKey: ['notifications'],
    queryFn: () => apiFetch<UserNotification[]>('/user/notifications?limit=50'),
    enabled,
  })
}

export function useUnreadNotificationCount() {
  return useQuery({
    queryKey: ['notifications', 'unread-count'],
    queryFn: () => apiFetch<{ count: number }>('/user/notifications/unread-count'),
    refetchInterval: 30000,
    refetchIntervalInBackground: false,
  })
}

export function useMarkNotificationRead() {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: (id: number) =>
      apiFetch<{ ok: boolean }>(`/user/notifications/${id}/read`, { method: 'PUT' }),
    onSuccess: () => {
      void qc.invalidateQueries({ queryKey: ['notifications'] })
    },
  })
}

export function useMarkAllNotificationsRead() {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: () =>
      apiFetch<{ marked: number }>('/user/notifications/read-all', { method: 'PUT' }),
    onSuccess: () => {
      void qc.invalidateQueries({ queryKey: ['notifications'] })
    },
  })
}

// ─── RSS ─────────────────────────────────────────────────────────

export function useRssFeeds() {
  return useQuery({
    queryKey: ['rss', 'feeds'],
    queryFn: () => apiFetch<RssFeed[]>('/rss/feed'),
  })
}

export function useRssItems(feedId?: number, limit = 500) {
  const params = new URLSearchParams()
  if (feedId) params.set('feedId', String(feedId))
  params.set('limit', String(limit))
  return useQuery({
    queryKey: ['rss', 'items', feedId, limit],
    queryFn: () => apiFetch<RssItem[]>(`/rss/item?${params}`),
  })
}

export function useRssRules() {
  return useQuery({
    queryKey: ['rss', 'rules'],
    queryFn: () => apiFetch<RssRule[]>('/rss/rule'),
  })
}

export function useCreateRssFeed() {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: (data: Partial<RssFeed>) =>
      apiFetch<RssFeed>('/rss/feed', { method: 'POST', body: JSON.stringify(data) }),
    onSuccess: () => { void qc.invalidateQueries({ queryKey: ['rss', 'feeds'] }) },
  })
}

export function useUpdateRssFeed() {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: ({ id, ...data }: { id: number } & Partial<RssFeed>) =>
      apiFetch<RssFeed>(`/rss/feed/${id}`, { method: 'PUT', body: JSON.stringify(data) }),
    onSuccess: () => { void qc.invalidateQueries({ queryKey: ['rss', 'feeds'] }) },
  })
}

export function useDeleteRssFeed() {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: (id: number) =>
      apiFetch<void>(`/rss/feed/${id}`, { method: 'DELETE' }),
    onSuccess: () => {
      void qc.invalidateQueries({ queryKey: ['rss', 'feeds'] })
      void qc.invalidateQueries({ queryKey: ['rss', 'items'] })
    },
  })
}

export function useCheckRssFeed() {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: (id: number) =>
      apiFetch<{ newItems: number; downloaded: number }>(`/rss/feed/${id}/check`, { method: 'POST' }),
    onSuccess: () => { void qc.invalidateQueries({ queryKey: ['rss', 'items'] }) },
  })
}

export function useDownloadRssItem() {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: (id: string) =>
      apiFetch<{ success: boolean }>(`/rss/item/${id}/download`, { method: 'POST' }),
    onSuccess: () => { void qc.invalidateQueries({ queryKey: ['rss', 'items'] }) },
  })
}

export function useCreateRssRule() {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: (data: Partial<RssRule>) =>
      apiFetch<RssRule>('/rss/rule', { method: 'POST', body: JSON.stringify(data) }),
    onSuccess: () => { void qc.invalidateQueries({ queryKey: ['rss', 'rules'] }) },
  })
}

export function useUpdateRssRule() {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: ({ id, ...data }: { id: number } & Partial<RssRule>) =>
      apiFetch<RssRule>(`/rss/rule/${id}`, { method: 'PUT', body: JSON.stringify(data) }),
    onSuccess: () => { void qc.invalidateQueries({ queryKey: ['rss', 'rules'] }) },
  })
}

export function useDeleteRssRule() {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: (id: number) =>
      apiFetch<void>(`/rss/rule/${id}`, { method: 'DELETE' }),
    onSuccess: () => { void qc.invalidateQueries({ queryKey: ['rss', 'rules'] }) },
  })
}

// ─── Migration ────────────────────────────────────────────────────

export function useMigrate() {
  return useMutation({
    mutationFn: (formData: FormData) =>
      fetch('/api/v1/system/migrate', { method: 'POST', body: formData }).then(async (res) => {
        if (!res.ok) throw new Error(`API error: ${res.status} ${res.statusText}`)
        return res.json() as Promise<MigrationResult>
      }),
  })
}

// ── DAV Streaming hooks ────────────────────────────────────────────────────

export function useDavItems(path: string) {
  return useQuery<DavItem[]>({
    queryKey: ['dav-items', path],
    queryFn: () => apiFetch(`/dav/items?path=${encodeURIComponent(path)}`),
  })
}

export function useDavStatus() {
  return useQuery<DavStatus>({
    queryKey: ['dav-status'],
    queryFn: () => apiFetch('/dav/status'),
  })
}

export function useDavHistory(offset = 0, limit = 50) {
  return useQuery<DavHistoryItem[]>({
    queryKey: ['dav-history', offset, limit],
    queryFn: () => apiFetch(`/dav/history?offset=${offset}&limit=${limit}`),
  })
}

export function useDavStream() {
  const qc = useQueryClient()
  return useMutation<DavStreamResponse, Error, DavStreamRequest>({
    mutationFn: (req) =>
      apiFetch('/dav/stream', {
        method: 'POST',
        body: JSON.stringify(req),
      }),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ['dav-items'] })
      qc.invalidateQueries({ queryKey: ['dav-status'] })
      qc.invalidateQueries({ queryKey: ['dav-history'] })
    },
  })
}

export function useDavDeleteItem() {
  const qc = useQueryClient()
  return useMutation<void, Error, string>({
    mutationFn: (id) => apiFetch(`/dav/items/${id}`, { method: 'DELETE' }),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ['dav-items'] })
      qc.invalidateQueries({ queryKey: ['dav-status'] })
    },
  })
}
