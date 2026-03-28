import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query'
import { apiFetch } from '../api/client'
import type {
  SystemStatus,
  Series,
  Movie,
  Episode,
  QueueItem,
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
} from '../api/types'

// ─── System ───────────────────────────────────────────────────────

export function useSystemStatus() {
  return useQuery({
    queryKey: ['system', 'status'],
    queryFn: () => apiFetch<SystemStatus>('/system/status'),
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
  })
}

// ─── History ──────────────────────────────────────────────────────

export function useHistory(page: number, pageSize = 20) {
  return useQuery({
    queryKey: ['history', page, pageSize],
    queryFn: () => apiFetch<HistoryResponse>(`/history?page=${page}&pageSize=${pageSize}`),
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

// ─── Freehand Search ──────────────────────────────────────────────

export function useSearchReleases(query: string, indexerIds?: number[]) {
  const params = new URLSearchParams({ query })
  if (indexerIds?.length) params.set('indexerIds', indexerIds.join(','))
  return useQuery({
    queryKey: ['search', query, indexerIds],
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
      apiFetch<{ success: boolean; apiKey: string }>('/setup/init', {
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
  })
}

export function usePendingRequestCount() {
  return useQuery({
    queryKey: ['requests', 'pending', 'count'],
    queryFn: () => apiFetch<{ count: number }>('/requests/pending/count'),
    refetchInterval: 30000,
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
