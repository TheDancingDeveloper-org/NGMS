import { useState, useEffect, useCallback } from 'react'
import { Search, Loader2, AlertCircle, FileQuestion } from 'lucide-react'

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

const API = '/api/v1'

type WantedTab = 'missing' | 'cutoff'

interface WantedMissingItem {
  id: number
  title: string
  mediaType: 'series' | 'movie'
  seasonNumber?: number
  episodeNumber?: number
  episodeTitle?: string
  qualityProfile: string
  monitored: boolean
  airDate?: string
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
  const [activeTab, setActiveTab] = useState<WantedTab>('missing')
  const [records, setRecords] = useState<WantedMissingItem[]>([])
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<string | null>(null)
  const [totalRecords, setTotalRecords] = useState(0)
  const [page, setPage] = useState(1)
  const [searchingId, setSearchingId] = useState<number | null>(null)

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
      const endpoint =
        item.mediaType === 'series'
          ? `${API}/command`
          : `${API}/command`
      const body =
        item.mediaType === 'series'
          ? { name: 'EpisodeSearch', episodeIds: [item.id] }
          : { name: 'MoviesSearch', movieIds: [item.id] }
      const res = await fetch(endpoint, {
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

  const totalPages = Math.max(1, Math.ceil(totalRecords / pageSize))

  return (
    <div className="min-h-screen bg-slate-900 p-6">
      <div>
        {/* Header */}
        <div className="mb-6 flex items-center gap-3">
          <FileQuestion className="h-6 w-6 text-blue-400" />
          <h1 className="text-2xl font-bold text-white">Wanted</h1>
          {!loading && (
            <span className="ml-2 rounded-full bg-slate-700 px-2.5 py-0.5 text-xs font-medium text-slate-300">
              {totalRecords}
            </span>
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
                    <th className="pb-3 pr-4 font-medium">Quality Profile</th>
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
                          <span className="text-white font-medium">{item.title}</span>
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
                          <span>
                            S{String(item.seasonNumber).padStart(2, '0')}E
                            {String(item.episodeNumber).padStart(2, '0')}
                            {item.episodeTitle && (
                              <span className="ml-1 text-slate-400">- {item.episodeTitle}</span>
                            )}
                          </span>
                        ) : (
                          <span className="text-slate-500">-</span>
                        )}
                      </td>
                      <td className="py-3 pr-4 text-slate-300">{item.qualityProfile}</td>
                      <td className="py-3 pr-4 text-slate-300">
                        {item.airDate ? (
                          <span>{item.airDate}</span>
                        ) : (
                          <span className="text-slate-500">-</span>
                        )}
                      </td>
                      <td className="py-3 text-right">
                        <button
                          onClick={() => void triggerSearch(item)}
                          disabled={searchingId === item.id}
                          className="inline-flex items-center gap-1.5 rounded-lg bg-blue-600 px-3 py-1.5 text-xs font-medium text-white hover:bg-blue-700 transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
                        >
                          {searchingId === item.id ? (
                            <Loader2 className="h-3.5 w-3.5 animate-spin" />
                          ) : (
                            <Search className="h-3.5 w-3.5" />
                          )}
                          Search
                        </button>
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
