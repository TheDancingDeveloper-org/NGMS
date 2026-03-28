import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query'
import { api } from '../api'

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

// ── Discover ───────────────────────────────────────────────────────────────

export function useDiscoverSearch(query: string, type: 'movie' | 'series', enabled: boolean) {
  return useQuery({
    queryKey: ['discover', query, type],
    queryFn: () => api.discoverSearch(query, type),
    enabled: enabled && query.length >= 2,
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
