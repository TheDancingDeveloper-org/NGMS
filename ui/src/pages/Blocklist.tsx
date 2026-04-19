import { useState, useCallback, useEffect } from 'react'
import { Loader2, Ban, Trash2, CheckSquare, Square } from 'lucide-react'
import { formatDateTime } from '../utils/date'

const API = '/api/v1'

interface BlocklistEntry {
  id: number
  mediaType: string
  mediaId: number
  sourceTitle: string
  quality: Record<string, unknown>
  languages: Record<string, unknown> | null
  indexerId: number | null
  infoHash: string | null
  message: string | null
  addedAt: string
}

interface BlocklistResponse {
  page: number
  pageSize: number
  totalRecords: number
  records: BlocklistEntry[]
}

export default function Blocklist() {
  const [data, setData] = useState<BlocklistResponse | null>(null)
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<string | null>(null)
  const [page, setPage] = useState(1)
  const [selected, setSelected] = useState<Set<number>>(new Set())
  const [deleting, setDeleting] = useState(false)
  const pageSize = 25

  const load = useCallback(async () => {
    setLoading(true)
    setError(null)
    try {
      const res = await fetch(`${API}/blocklist?page=${page}&pageSize=${pageSize}`)
      if (!res.ok) throw new Error('Failed to load blocklist')
      const json: BlocklistResponse = await res.json()
      setData(json)
    } catch (e) {
      setError(e instanceof Error ? e.message : 'Unknown error')
    } finally {
      setLoading(false)
    }
  }, [page])

  useEffect(() => {
    void load()
  }, [load])

  const deleteEntry = async (id: number) => {
    await fetch(`${API}/blocklist/${id}`, { method: 'DELETE' })
    setSelected((s) => { const n = new Set(s); n.delete(id); return n })
    void load()
  }

  const bulkDelete = async () => {
    if (selected.size === 0) return
    setDeleting(true)
    await fetch(`${API}/blocklist/bulk`, {
      method: 'DELETE',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ ids: [...selected] }),
    })
    setSelected(new Set())
    setDeleting(false)
    void load()
  }

  const clearAll = async () => {
    if (!data || data.totalRecords === 0) return
    if (!confirm(`Delete all ${data.totalRecords} blocklist entries?`)) return
    setDeleting(true)
    await fetch(`${API}/blocklist/clear`, { method: 'DELETE' })
    setSelected(new Set())
    setDeleting(false)
    setPage(1)
    void load()
  }

  const toggleSelect = (id: number) => {
    setSelected((s) => {
      const n = new Set(s)
      if (n.has(id)) n.delete(id)
      else n.add(id)
      return n
    })
  }

  const toggleSelectAll = () => {
    if (!data) return
    const ids = data.records.map((r) => r.id)
    const allSelected = ids.every((id) => selected.has(id))
    if (allSelected) {
      setSelected((s) => { const n = new Set(s); ids.forEach((id) => n.delete(id)); return n })
    } else {
      setSelected((s) => { const n = new Set(s); ids.forEach((id) => n.add(id)); return n })
    }
  }

  const allOnPageSelected = data ? data.records.length > 0 && data.records.every((r) => selected.has(r.id)) : false

  return (
    <div>
      <div className="mb-6 flex items-center justify-between">
        <h2 className="text-2xl font-bold">Blocklist</h2>
        <div className="flex gap-2">
          {selected.size > 0 && (
            <button
              onClick={bulkDelete}
              disabled={deleting}
              className="flex items-center gap-1.5 rounded-lg bg-red-600 px-3 py-1.5 text-sm font-medium text-white hover:bg-red-500 disabled:opacity-50 transition-colors"
            >
              <Trash2 size={14} />
              Delete Selected ({selected.size})
            </button>
          )}
          {data && data.totalRecords > 0 && (
            <button
              onClick={clearAll}
              disabled={deleting}
              className="flex items-center gap-1.5 rounded-lg bg-slate-700 px-3 py-1.5 text-sm font-medium text-slate-300 hover:bg-slate-600 disabled:opacity-50 transition-colors"
            >
              <Trash2 size={14} />
              Clear All
            </button>
          )}
        </div>
      </div>

      {loading && page === 1 && (
        <div className="flex items-center justify-center py-20">
          <Loader2 size={32} className="animate-spin text-blue-500" />
        </div>
      )}

      {error && (
        <div className="rounded-lg border border-red-500/30 bg-red-500/10 p-4 text-red-400">
          Failed to load blocklist: {error}
        </div>
      )}

      {!loading && !error && data && data.records.length === 0 && page === 1 && (
        <div className="flex flex-col items-center justify-center py-20 text-slate-400">
          <Ban size={48} className="mb-4 text-slate-600" />
          <p>No blocklist entries</p>
        </div>
      )}

      {data && data.records.length > 0 && (
        <>
          <div className="overflow-x-auto rounded-lg bg-slate-800">
            <table className="w-full text-sm">
              <thead>
                <tr className="border-b border-slate-700 text-left text-xs uppercase text-slate-400">
                  <th className="px-4 py-3 font-medium w-10">
                    <button onClick={toggleSelectAll} className="text-slate-400 hover:text-white transition-colors">
                      {allOnPageSelected ? <CheckSquare size={16} /> : <Square size={16} />}
                    </button>
                  </th>
                  <th className="px-4 py-3 font-medium">Title</th>
                  <th className="px-4 py-3 font-medium">Type</th>
                  <th className="px-4 py-3 font-medium">Reason</th>
                  <th className="px-4 py-3 font-medium">Date</th>
                  <th className="px-4 py-3 font-medium w-10" />
                </tr>
              </thead>
              <tbody>
                {data.records.map((entry) => (
                  <tr
                    key={entry.id}
                    className="border-b border-slate-700/50 hover:bg-slate-700/30 transition-colors"
                  >
                    <td className="px-4 py-3">
                      <button onClick={() => toggleSelect(entry.id)} className="text-slate-400 hover:text-white transition-colors">
                        {selected.has(entry.id) ? <CheckSquare size={16} className="text-blue-400" /> : <Square size={16} />}
                      </button>
                    </td>
                    <td className="px-4 py-3">
                      <div className="font-medium text-white truncate max-w-md" title={entry.sourceTitle}>
                        {entry.sourceTitle}
                      </div>
                    </td>
                    <td className="px-4 py-3">
                      <span className={`rounded-full px-2 py-0.5 text-xs font-medium capitalize ${
                        entry.mediaType === 'series'
                          ? 'bg-blue-500/20 text-blue-400'
                          : 'bg-purple-500/20 text-purple-400'
                      }`}>
                        {entry.mediaType}
                      </span>
                    </td>
                    <td className="px-4 py-3 text-slate-400 max-w-xs truncate" title={entry.message ?? ''}>
                      {entry.message || '-'}
                    </td>
                    <td className="px-4 py-3 text-slate-300 whitespace-nowrap">
                      {formatDateTime(entry.addedAt)}
                    </td>
                    <td className="px-4 py-3">
                      <button
                        onClick={() => deleteEntry(entry.id)}
                        className="text-slate-400 hover:text-red-400 transition-colors"
                        title="Remove from blocklist"
                      >
                        <Trash2 size={14} />
                      </button>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>

          {/* Pagination */}
          <div className="mt-4 flex items-center justify-between">
            <span className="text-sm text-slate-400">
              Showing {(page - 1) * pageSize + 1}-{Math.min(page * pageSize, data.totalRecords)} of{' '}
              {data.totalRecords}
            </span>
            <div className="flex gap-2">
              <button
                onClick={() => setPage((p) => Math.max(1, p - 1))}
                disabled={page === 1}
                className="rounded-lg bg-slate-700 px-3 py-1.5 text-sm font-medium text-slate-300 hover:bg-slate-600 disabled:opacity-50 transition-colors"
              >
                Previous
              </button>
              <button
                onClick={() => setPage((p) => p + 1)}
                disabled={page * pageSize >= data.totalRecords}
                className="rounded-lg bg-slate-700 px-3 py-1.5 text-sm font-medium text-slate-300 hover:bg-slate-600 disabled:opacity-50 transition-colors"
              >
                Next
              </button>
            </div>
          </div>
        </>
      )}
    </div>
  )
}
