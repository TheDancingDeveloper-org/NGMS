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
  IndexerConfig,
  DownloadClientConfig,
  NamingConfig,
  Tag,
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
