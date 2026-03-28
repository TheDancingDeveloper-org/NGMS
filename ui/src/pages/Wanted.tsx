import { useState, useEffect, useCallback } from 'react'
import { Link, useNavigate } from 'react-router-dom'
import { Search, Loader2, AlertCircle, FileQuestion, SearchCheck } from 'lucide-react'
import { formatAirDate } from '../utils/date'

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

const API = '/api/v1'

type WantedTab = 'missing' | 'cutoff'

interface WantedMissingItem {
  id: number
  title: string
  mediaType: 'series' | 'movie'
  mediaId: number
  seasonNumber?: number | null
  episodeNumber?: number | null
  episodeTitle?: string | null
  qualityProfile?: string | null
  monitored: boolean
  airDate?: string | null
  currentQuality?: string | null
  cutoffQuality?: string | null
}

interface WantedResponse {
  page: number
  pageSize: number
  totalRecords: number
  records: WantedMissingItem[]
}

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

export default function Wanted() {
  const navigate = useNavigate()
  const [activeTab, setActiveTab] = useState<WantedTab>('missing')
  const [records, setRecords] = useState<WantedMissingItem[]>([])
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<string | null>(null)
  const [totalRecords, setTotalRecords] = useState(0)
  const [page, setPage] = useState(1)
  const [searchingId, setSearchingId] = useState<number | null>(null)
  const [searchingAll, setSearchingAll] = useState(false)

  const pageSize = 25

  const load = useCallback(async () => {
    setLoading(true)
    setError(null)
    try {
      const endpoint = activeTab === 'missing' ? 'wanted/missing' : 'wanted/cutoff'
      const res = await fetch(`${API}/${endpoint}?page=${page}&pageSize=${pageSize}`)
      if (res.status === 404) {
        setRecords([])
        setTotalRecords(0)
        setError(null)
        return
      }
      if (!res.ok) throw new Error(`API error: ${res.status}`)
      const data: WantedResponse = await res.json()
      setRecords(data.records)
      setTotalRecords(data.totalRecords)
    } catch (err) {
      const msg = err instanceof Error ? err.message : 'Unknown error'
      if (!msg.includes('404')) {
        setError(msg)
      }
      setRecords([])
      setTotalRecords(0)
    } finally {
      setLoading(false)
    }
  }, [activeTab, page])

  useEffect(() => {
    void load()
  }, [load])

  useEffect(() => {
    setPage(1)
  }, [activeTab])

  const triggerSearch = async (item: WantedMissingItem) => {
    setSearchingId(item.id)
    try {
      const body =
        item.mediaType === 'series'
          ? { name: 'EpisodeSearch', episodeIds: [item.id] }
          : { name: 'MoviesSearch', movieIds: [item.id] }
      const res = await fetch(`${API}/command`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(body),
      })
      if (!res.ok) throw new Error('Search command failed')
    } catch {
      /* Silently fail — the user sees the button reset */
    } finally {
      setSearchingId(null)
    }
  }

  const triggerSearchAll = async () => {
    if (!confirm(`Search for all ${totalRecords} ${activeTab === 'missing' ? 'missing' : 'cutoff unmet'} items? This may take a while.`)) return
    setSearchingAll(true)
    try {
      const commandName = activeTab === 'missing' ? 'MissingSearch' : 'CutoffSearch'
      const res = await fetch(`${API}/command`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ name: commandName }),
      })
      if (!res.ok) throw new Error('Search all command failed')
    } catch {
      /* Silently fail */
    } finally {
      setSearchingAll(false)
    }
  }

  const totalPages = Math.max(1, Math.ceil(totalRecords / pageSize))

  return (
    <div>
      <div>
        {/* Header */}
        <div className="mb-6 flex items-center justify-between">
          <div className="flex items-center gap-3">
            <FileQuestion className="h-6 w-6 text-blue-400" />
            <h1 className="text-2xl font-bold text-white">Wanted</h1>
            {!loading && (
              <span className="ml-2 rounded-full bg-slate-700 px-2.5 py-0.5 text-xs font-medium text-slate-300">
                {totalRecords}
              </span>
            )}
          </div>
          {!loading && totalRecords > 0 && (
            <button
              onClick={() => void triggerSearchAll()}
              disabled={searchingAll}
              className="flex items-center gap-2 rounded-lg bg-blue-600 px-4 py-2 text-sm font-medium text-white hover:bg-blue-700 transition-colors disabled:opacity-50"
            >
              {searchingAll ? (
                <Loader2 className="h-4 w-4 animate-spin" />
              ) : (
                <SearchCheck className="h-4 w-4" />
              )}
              Search All {activeTab === 'missing' ? 'Missing' : 'Cutoff Unmet'}
            </button>
          )}
        </div>

        {/* Tabs */}
        <div className="mb-6 flex gap-2">
          <button
            onClick={() => setActiveTab('missing')}
            className={`rounded-full px-4 py-2 text-sm font-medium transition-colors ${
              activeTab === 'missing'
                ? 'bg-blue-600 text-white'
                : 'bg-slate-800 text-slate-300 hover:bg-slate-700 hover:text-white'
            }`}
          >
            Missing
          </button>
          <button
            onClick={() => setActiveTab('cutoff')}
            className={`rounded-full px-4 py-2 text-sm font-medium transition-colors ${
              activeTab === 'cutoff'
                ? 'bg-blue-600 text-white'
                : 'bg-slate-800 text-slate-300 hover:bg-slate-700 hover:text-white'
            }`}
          >
            Cutoff Unmet
          </button>
        </div>

        {/* Content */}
        <div className="rounded-xl border border-slate-700 bg-slate-800 p-6">
          {loading ? (
            <div className="flex items-center justify-center gap-2 py-12 text-slate-400">
              <Loader2 className="h-5 w-5 animate-spin" />
              <span>Loading...</span>
            </div>
          ) : error ? (
            <div className="flex flex-col items-center justify-center gap-3 py-12 text-slate-400">
              <AlertCircle className="h-8 w-8 text-red-400" />
              <p className="text-sm">{error}</p>
              <button
                onClick={() => void load()}
                className="rounded-lg bg-blue-600 px-4 py-2 text-sm font-medium text-white hover:bg-blue-700 transition-colors"
              >
                Retry
              </button>
            </div>
          ) : records.length === 0 ? (
            <div className="flex flex-col items-center justify-center gap-2 py-12 text-slate-400">
              <FileQuestion className="h-8 w-8" />
              <p className="text-sm">
                {activeTab === 'missing'
                  ? 'No missing media found. Everything is up to date.'
                  : 'No cutoff unmet media found.'}
              </p>
            </div>
          ) : (
            <>
              <table className="w-full text-left text-sm">
                <thead>
                  <tr className="border-b border-slate-700 text-slate-400">
                    <th className="pb-3 pr-4 font-medium">Title</th>
                    <th className="pb-3 pr-4 font-medium">Episode</th>
                    {activeTab === 'cutoff' ? (
                      <>
                        <th className="pb-3 pr-4 font-medium">Current Quality</th>
                        <th className="pb-3 pr-4 font-medium">Wanted Quality</th>
                      </>
                    ) : (
                      <th className="pb-3 pr-4 font-medium">Quality Profile</th>
                    )}
                    <th className="pb-3 pr-4 font-medium">Air Date</th>
                    <th className="pb-3 font-medium" />
                  </tr>
                </thead>
                <tbody>
                  {records.map((item) => (
                    <tr
                      key={item.id}
                      className="border-b border-slate-700/50 hover:bg-slate-700/50 transition-colors"
                    >
                      <td className="py-3 pr-4">
                        <div className="flex items-center gap-2">
                          <Link
                            to={item.mediaType === 'series' ? `/series/${item.mediaId}` : `/movies/${item.mediaId}`}
                            className="text-white font-medium hover:text-blue-400 transition-colors"
                          >
                            {item.title}
                          </Link>
                          <span
                            className={`rounded px-1.5 py-0.5 text-xs font-medium ${
                              item.mediaType === 'series'
                                ? 'bg-blue-500/20 text-blue-300'
                                : 'bg-purple-500/20 text-purple-300'
                            }`}
                          >
                            {item.mediaType === 'series' ? 'TV' : 'Movie'}
                          </span>
                        </div>
                      </td>
                      <td className="py-3 pr-4 text-slate-300">
                        {item.mediaType === 'series' && item.seasonNumber != null && item.episodeNumber != null ? (
                          <Link
                            to={`/series/${item.mediaId}`}
                            className="hover:text-blue-400 transition-colors"
                          >
                            S{String(item.seasonNumber).padStart(2, '0')}E
                            {String(item.episodeNumber).padStart(2, '0')}
                            {item.episodeTitle && (
                              <span className="ml-1 text-slate-400">- {item.episodeTitle}</span>
                            )}
                          </Link>
                        ) : (
                          <span className="text-slate-500">-</span>
                        )}
                      </td>
                      {activeTab === 'cutoff' ? (
                        <>
                          <td className="py-3 pr-4">
                            <span className="rounded bg-yellow-500/20 px-2 py-0.5 text-xs font-medium text-yellow-400">
                              {item.currentQuality ?? '-'}
                            </span>
                          </td>
                          <td className="py-3 pr-4">
                            <span className="rounded bg-green-500/20 px-2 py-0.5 text-xs font-medium text-green-400">
                              {item.cutoffQuality ?? '-'}
                            </span>
                          </td>
                        </>
                      ) : (
                        <td className="py-3 pr-4 text-slate-300">{item.qualityProfile ?? '-'}</td>
                      )}
                      <td className="py-3 pr-4 text-slate-300">
                        {item.airDate ? (
                          <span>{formatAirDate(item.airDate)}</span>
                        ) : (
                          <span className="text-slate-500">-</span>
                        )}
                      </td>
                      <td className="py-3 text-right">
                        <div className="flex items-center justify-end gap-1.5">
                          <button
                            onClick={() => void triggerSearch(item)}
                            disabled={searchingId === item.id}
                            className="inline-flex items-center gap-1.5 rounded-lg bg-blue-600 px-3 py-1.5 text-xs font-medium text-white hover:bg-blue-700 transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
                            title="Automatic search"
                          >
                            {searchingId === item.id ? (
                              <Loader2 className="h-3.5 w-3.5 animate-spin" />
                            ) : (
                              <Search className="h-3.5 w-3.5" />
                            )}
                            Search
                          </button>
                          <button
                            onClick={() => {
                              const query = item.mediaType === 'series'
                                ? `${item.title} S${String(item.seasonNumber ?? 0).padStart(2, '0')}E${String(item.episodeNumber ?? 0).padStart(2, '0')}`
                                : item.title
                              navigate(`/search?q=${encodeURIComponent(query)}`)
                            }}
                            className="inline-flex items-center gap-1.5 rounded-lg bg-slate-700 px-3 py-1.5 text-xs font-medium text-slate-300 hover:bg-slate-600 hover:text-white transition-colors"
                            title="Manual search on search page"
                          >
                            <SearchCheck className="h-3.5 w-3.5" />
                            Manual
                          </button>
                        </div>
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>

              {/* Pagination */}
              {totalPages > 1 && (
                <div className="mt-4 flex items-center justify-between border-t border-slate-700 pt-4">
                  <span className="text-sm text-slate-400">
                    Page {page} of {totalPages} ({totalRecords} total)
                  </span>
                  <div className="flex gap-2">
                    <button
                      onClick={() => setPage((p) => Math.max(1, p - 1))}
                      disabled={page <= 1}
                      className="rounded-lg bg-slate-700 px-3 py-1.5 text-sm font-medium text-slate-300 hover:bg-slate-600 transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
                    >
                      Previous
                    </button>
                    <button
                      onClick={() => setPage((p) => Math.min(totalPages, p + 1))}
                      disabled={page >= totalPages}
                      className="rounded-lg bg-slate-700 px-3 py-1.5 text-sm font-medium text-slate-300 hover:bg-slate-600 transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
                    >
                      Next
                    </button>
                  </div>
                </div>
              )}
            </>
          )}
        </div>
      </div>
    </div>
  )
}
