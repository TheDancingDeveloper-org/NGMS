import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query'
import { api } from '../api'

// ── System ────────────────────────────────────────────────────────────────

export function useSystemStatus() {
  return useQuery({
    queryKey: ['system-status'],
    queryFn: () => api.getSystemStatus(),
    staleTime: 60_000,
  })
}

// ── Library ────────────────────────────────────────────────────────────────

export function useSeries() {
  return useQuery({ queryKey: ['series'], queryFn: () => api.listSeries() })
}

export function useMovies() {
  return useQuery({ queryKey: ['movies'], queryFn: () => api.listMovies() })
}

export function useSeriesDetail(id: number) {
  return useQuery({
    queryKey: ['series', id],
    queryFn: () => api.getSeries(id),
    enabled: id > 0,
  })
}

export function useMovieDetail(id: number) {
  return useQuery({
    queryKey: ['movie', id],
    queryFn: () => api.getMovie(id),
    enabled: id > 0,
  })
}

export function useEpisodes(seriesId: number) {
  return useQuery({
    queryKey: ['episodes', seriesId],
    queryFn: () => api.getEpisodes(seriesId),
    enabled: seriesId > 0,
  })
}

// ── Continue Watching ──────────────────────────────────────────────────────

export function useContinueWatching(limit = 20) {
  return useQuery({
    queryKey: ['continue-watching', limit],
    queryFn: () => api.getContinueWatching(limit),
  })
}

// ── Watchlist ──────────────────────────────────────────────────────────────

export function useWatchlist(filter?: string) {
  return useQuery({
    queryKey: ['watchlist', filter],
    queryFn: () => api.getWatchlist(filter),
  })
}

export function useRemoveFromWatchlist() {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: ({ mediaType, mediaId }: { mediaType: string; mediaId: number }) =>
      api.removeFromWatchlist(mediaType, mediaId),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ['watchlist'] })
    },
  })
}

// ── Ratings ────────────────────────────────────────────────────────────────

export function useRating(mediaType: string, mediaId: number) {
  return useQuery({
    queryKey: ['rating', mediaType, mediaId],
    queryFn: () => api.getRating(mediaType, mediaId),
    enabled: mediaId > 0,
  })
}

export function useSetRating() {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: ({ mediaType, mediaId, rating }: { mediaType: string; mediaId: number; rating: number }) =>
      api.setRating(mediaType, mediaId, rating),
    onSuccess: (_d, vars) => {
      qc.invalidateQueries({ queryKey: ['rating', vars.mediaType, vars.mediaId] })
    },
  })
}

export function useDeleteRating() {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: ({ mediaType, mediaId }: { mediaType: string; mediaId: number }) =>
      api.deleteRating(mediaType, mediaId),
    onSuccess: (_d, vars) => {
      qc.invalidateQueries({ queryKey: ['rating', vars.mediaType, vars.mediaId] })
    },
  })
}

// ── Requests ───────────────────────────────────────────────────────────────

export function useMyRequests() {
  return useQuery({
    queryKey: ['my-requests'],
    queryFn: () => api.listMyRequests(),
  })
}

export function useCreateRequest() {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: api.createRequest,
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ['my-requests'] })
    },
  })
}

export function usePendingRequestCount() {
  return useQuery({
    queryKey: ['requests', 'pending-count'],
    queryFn: () => api.getPendingRequestCount(),
    refetchInterval: 60_000,
  })
}

// ── Discover ───────────────────────────────────────────────────────────────

export function useDiscoverSearch(query: string, type: 'movie' | 'series', enabled: boolean) {
  return useQuery({
    queryKey: ['discover', query, type],
    queryFn: () => api.discoverSearch(query, type),
    enabled: enabled && query.length >= 2,
  })
}

export function useDiscoverSliders() {
  return useQuery({
    queryKey: ['discover', 'sliders'],
    queryFn: () => api.getSliders(),
    staleTime: 60_000,
  })
}

export function useTrending(params?: { mediaType?: string; timeWindow?: string; page?: number }) {
  return useQuery({
    queryKey: ['discover', 'trending', params],
    queryFn: () => api.getTrending(params),
    staleTime: 5 * 60_000,
  })
}

export function useUpcomingMovies() {
  return useQuery({
    queryKey: ['discover', 'movies', 'upcoming'],
    queryFn: () => api.getUpcomingMovies(),
    staleTime: 5 * 60_000,
  })
}

export function useUpcomingTv() {
  return useQuery({
    queryKey: ['discover', 'tv', 'upcoming'],
    queryFn: () => api.getUpcomingTv(),
    staleTime: 5 * 60_000,
  })
}

export function useMovieGenres() {
  return useQuery({
    queryKey: ['discover', 'genres', 'movie'],
    queryFn: () => api.getMovieGenres(),
    staleTime: 24 * 60 * 60_000,
  })
}

export function useTvGenres() {
  return useQuery({
    queryKey: ['discover', 'genres', 'tv'],
    queryFn: () => api.getTvGenres(),
    staleTime: 24 * 60 * 60_000,
  })
}

export function useMovieRecommendations(tmdbId: number, enabled = true) {
  return useQuery({
    queryKey: ['discover', 'movies', tmdbId, 'recommendations'],
    queryFn: () => api.getMovieRecommendations(tmdbId),
    enabled: enabled && tmdbId > 0,
    staleTime: 5 * 60_000,
  })
}

export function useTvRecommendations(tmdbId: number, enabled = true) {
  return useQuery({
    queryKey: ['discover', 'tv', tmdbId, 'recommendations'],
    queryFn: () => api.getTvRecommendations(tmdbId),
    enabled: enabled && tmdbId > 0,
    staleTime: 5 * 60_000,
  })
}

// ── Notifications ──────────────────────────────────────────────────────────

export function useNotifications(enabled = true) {
  return useQuery({
    queryKey: ['notifications'],
    queryFn: () => api.getNotifications(false, 50, 0),
    refetchInterval: 30_000,
    enabled,
  })
}

export function useUnreadCount() {
  return useQuery({
    queryKey: ['notifications', 'unread-count'],
    queryFn: () => api.getUnreadCount(),
    refetchInterval: 30_000,
  })
}

export function useMarkNotificationRead() {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: (id: number) => api.markNotificationRead(id),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ['notifications'] })
    },
  })
}

export function useMarkAllNotificationsRead() {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: () => api.markAllNotificationsRead(),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ['notifications'] })
    },
  })
}

// ── User profile / devices / sessions ──────────────────────────────────────

export function useUpdateProfile() {
  return useMutation({
    mutationFn: (body: { displayName?: string; avatarUrl?: string | null }) =>
      api.updateProfile(body),
  })
}

export function useDevices() {
  return useQuery({
    queryKey: ['user', 'devices'],
    queryFn: () => api.getDevices(),
  })
}

export function useDeleteDevice() {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: (id: number) => api.deleteDevice(id),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ['user', 'devices'] })
    },
  })
}

export function useSessions() {
  return useQuery({
    queryKey: ['user', 'sessions'],
    queryFn: () => api.getSessions(),
  })
}

// ── Calendar ───────────────────────────────────────────────────────────────

export function useCalendar(start?: string, end?: string) {
  return useQuery({
    queryKey: ['calendar', start, end],
    queryFn: () => api.getCalendar(start, end),
  })
}

// ── Queue ──────────────────────────────────────────────────────────────────

export function useQueue() {
  return useQuery({
    queryKey: ['queue'],
    queryFn: () => api.getQueue(),
    refetchInterval: 5_000,
  })
}

// ── History ────────────────────────────────────────────────────────────────

export function useHistory(page = 1, pageSize = 20) {
  return useQuery({
    queryKey: ['history', page, pageSize],
    queryFn: () => api.getHistory(page, pageSize),
  })
}

export function useHistoryStream(limit = 30) {
  return useQuery({
    queryKey: ['history', 'stream', limit],
    queryFn: () => api.getHistoryStream(limit),
    refetchInterval: 15_000,
  })
}

// ── Activities ─────────────────────────────────────────────────────────────

export function useActivities(limit = 20) {
  return useQuery({
    queryKey: ['activities', limit],
    queryFn: () => api.getActivities(limit),
    refetchInterval: 5_000,
  })
}

export function useRunningActivities() {
  return useQuery({
    queryKey: ['activities', 'running'],
    queryFn: () => api.getRunningActivities(),
    refetchInterval: 5_000,
  })
}
