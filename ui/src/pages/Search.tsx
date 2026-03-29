import { useState, useRef, useEffect } from 'react'
import { useSearchParams } from 'react-router-dom'
import { Search as SearchIcon, Loader2, ExternalLink, Magnet, HardDrive, ChevronDown, X } from 'lucide-react'
import { useSearchReleases, useIndexers, useSystemStatus } from '../hooks/useApi'

export default function Search() {
  const [searchParams] = useSearchParams()
  const initialQuery = searchParams.get('q') ?? ''
  const [input, setInput] = useState(initialQuery)
  const [query, setQuery] = useState(initialQuery)
  const [selectedIndexerIds, setSelectedIndexerIds] = useState<number[]>([])
  const [indexarrOnly, setIndexarrOnly] = useState(false)
  const [dropdownOpen, setDropdownOpen] = useState(false)
  const dropdownRef = useRef<HTMLDivElement>(null)

  const { data: indexers } = useIndexers()
  const { data: status } = useSystemStatus()
  const indexarrEnabled = status?.modules?.indexarrSidecar === true
  const enabledIndexers = (indexers ?? []).filter(i => i.enabled)
  const { data: results, isLoading, error } = useSearchReleases(query, selectedIndexerIds, indexarrOnly)

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault()
    setQuery(input.trim())
  }

  const toggleIndexer = (id: number) => {
    setIndexarrOnly(false)
    setSelectedIndexerIds(prev =>
      prev.includes(id) ? prev.filter(i => i !== id) : [...prev, id]
    )
  }

  // Close dropdown on outside click
  useEffect(() => {
    const handler = (e: MouseEvent) => {
      if (dropdownRef.current && !dropdownRef.current.contains(e.target as Node)) {
        setDropdownOpen(false)
      }
    }
    document.addEventListener('mousedown', handler)
    return () => document.removeEventListener('mousedown', handler)
  }, [])

  const indexerLabel = indexarrOnly
    ? 'Indexarr Only'
    : selectedIndexerIds.length === 0
      ? 'All Indexers'
      : selectedIndexerIds.length === 1
        ? enabledIndexers.find(i => i.id === selectedIndexerIds[0])?.name ?? '1 indexer'
        : `${selectedIndexerIds.length} indexers`

  return (
    <div>
      <div className="mb-6">
        <h2 className="text-2xl font-bold mb-4">Search</h2>
        <form onSubmit={handleSubmit} className="flex gap-3">
          {/* Indexer dropdown */}
          <div className="relative" ref={dropdownRef}>
            <button
              type="button"
              onClick={() => setDropdownOpen(!dropdownOpen)}
              className="flex items-center gap-2 rounded-lg border border-slate-600 bg-slate-800 px-3 py-2 text-sm text-slate-300 hover:border-slate-500 transition-colors whitespace-nowrap"
            >
              {indexerLabel}
              <ChevronDown size={14} className={`transition-transform ${dropdownOpen ? 'rotate-180' : ''}`} />
            </button>
            {dropdownOpen && enabledIndexers.length > 0 && (
              <div className="absolute top-full left-0 z-20 mt-1 w-64 rounded-lg border border-slate-600 bg-slate-800 py-1 shadow-xl">
                <button
                  type="button"
                  onClick={() => { setSelectedIndexerIds([]); setIndexarrOnly(false) }}
                  className={`w-full px-3 py-2 text-left text-sm transition-colors ${
                    selectedIndexerIds.length === 0 && !indexarrOnly ? 'bg-blue-600/20 text-blue-400' : 'text-slate-300 hover:bg-slate-700'
                  }`}
                >
                  All Indexers
                </button>
                <div className="border-t border-slate-700 my-1" />
                {indexarrEnabled && (
                  <button
                    type="button"
                    onClick={() => {
                      setIndexarrOnly(!indexarrOnly)
                      if (!indexarrOnly) setSelectedIndexerIds([])
                    }}
                    className={`w-full px-3 py-2 text-left text-sm transition-colors flex items-center justify-between ${
                      indexarrOnly ? 'bg-green-600/20 text-green-400' : 'text-green-400/70 hover:bg-slate-700'
                    }`}
                  >
                    <span>Indexarr</span>
                    <span className="text-[10px] rounded bg-green-500/20 px-1.5 py-0.5 font-medium">
                      {indexarrOnly ? 'selected' : 'always active'}
                    </span>
                  </button>
                )}
                {enabledIndexers.map(idx => (
                  <button
                    key={idx.id}
                    type="button"
                    onClick={() => toggleIndexer(idx.id)}
                    className={`w-full px-3 py-2 text-left text-sm transition-colors flex items-center justify-between ${
                      selectedIndexerIds.includes(idx.id) ? 'bg-blue-600/20 text-blue-400' : 'text-slate-300 hover:bg-slate-700'
                    }`}
                  >
                    <span>{idx.name}</span>
                    <span className="text-xs text-slate-500">{idx.protocol}</span>
                  </button>
                ))}
              </div>
            )}
          </div>

          {/* Search input */}
          <div className="relative flex-1">
            <SearchIcon size={16} className="absolute left-3 top-1/2 -translate-y-1/2 text-slate-400" />
            <input
              type="text"
              value={input}
              onChange={e => setInput(e.target.value)}
              placeholder="Search indexers for releases..."
              className="w-full rounded-lg border border-slate-600 bg-slate-800 py-2 pl-10 pr-4 text-sm text-white placeholder-slate-400 focus:border-blue-500 focus:outline-none"
              autoFocus
            />
          </div>
          <button
            type="submit"
            disabled={!input.trim() || isLoading}
            className="rounded-lg bg-blue-600 px-5 py-2 text-sm font-medium text-white hover:bg-blue-500 disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
          >
            Search
          </button>
        </form>

        {/* Selected indexer chips */}
        {selectedIndexerIds.length > 0 && (
          <div className="mt-2 flex flex-wrap gap-1.5">
            {selectedIndexerIds.map(id => {
              const idx = enabledIndexers.find(i => i.id === id)
              if (!idx) return null
              return (
                <span key={id} className="inline-flex items-center gap-1 rounded-full bg-blue-600/20 px-2.5 py-0.5 text-xs text-blue-400">
                  {idx.name}
                  <button type="button" onClick={() => toggleIndexer(id)} className="hover:text-blue-300">
                    <X size={12} />
                  </button>
                </span>
              )
            })}
            <button
              type="button"
              onClick={() => setSelectedIndexerIds([])}
              className="text-xs text-slate-500 hover:text-slate-400"
            >
              Clear all
            </button>
          </div>
        )}
      </div>

      {isLoading && (
        <div className="flex items-center justify-center py-20">
          <Loader2 size={32} className="animate-spin text-blue-500" />
        </div>
      )}

      {error && (
        <div className="rounded-lg border border-red-500/30 bg-red-500/10 p-4 text-red-400">
          Search failed: {error.message}
        </div>
      )}

      {!isLoading && !error && query && results?.length === 0 && (
        <div className="flex flex-col items-center justify-center py-20 text-slate-400">
          <SearchIcon size={48} className="mb-4 text-slate-600" />
          <p>No results found for "{query}"</p>
        </div>
      )}

      {results && results.length > 0 && (
        <div className="overflow-x-auto rounded-lg bg-slate-800">
          <table className="w-full text-sm">
            <thead>
              <tr className="border-b border-slate-700 text-left text-xs uppercase text-slate-400">
                <th className="px-4 py-3 font-medium">Title</th>
                <th className="px-4 py-3 font-medium">Quality</th>
                <th className="px-4 py-3 font-medium">Indexer</th>
                <th className="px-4 py-3 font-medium">Type</th>
                <th className="px-4 py-3 font-medium">Size</th>
                <th className="px-4 py-3 font-medium">Age</th>
                <th className="px-4 py-3 font-medium">Peers</th>
                <th className="px-4 py-3 font-medium w-20">Links</th>
              </tr>
            </thead>
            <tbody>
              {results.map((r) => (
                <tr key={r.guid} className="border-b border-slate-700/50 hover:bg-slate-700/30 transition-colors">
                  <td className="px-4 py-3 font-medium text-white max-w-md truncate" title={r.title}>
                    {r.title}
                  </td>
                  <td className="px-4 py-3">
                    {r.quality && r.quality !== 'Unknown' ? (
                      <span className="rounded bg-blue-500/20 px-1.5 py-0.5 text-xs font-medium text-blue-400">
                        {r.quality}
                      </span>
                    ) : (
                      <span className="text-slate-500 text-xs">-</span>
                    )}
                  </td>
                  <td className="px-4 py-3 text-slate-300">{r.indexerName}</td>
                  <td className="px-4 py-3">
                    <ProtocolBadge protocol={r.protocol} />
                  </td>
                  <td className="px-4 py-3 text-slate-300 whitespace-nowrap">{formatSize(r.size)}</td>
                  <td className="px-4 py-3 text-slate-300 whitespace-nowrap">{formatAge(r.ageDays)}</td>
                  <td className="px-4 py-3 text-slate-300">
                    {r.protocol === 'torrent' && r.seeders != null
                      ? <span><span className="text-green-400">{r.seeders}</span> / <span className="text-red-400">{r.leechers ?? 0}</span></span>
                      : '-'}
                  </td>
                  <td className="px-4 py-3">
                    <div className="flex items-center gap-2">
                      {r.infoUrl && (
                        <a href={r.infoUrl} target="_blank" rel="noopener noreferrer" className="text-slate-400 hover:text-blue-400 transition-colors" title="Info page">
                          <ExternalLink size={14} />
                        </a>
                      )}
                      {r.downloadUrl && (
                        <a href={r.downloadUrl} target="_blank" rel="noopener noreferrer" className="text-slate-400 hover:text-green-400 transition-colors" title="Download">
                          {r.protocol === 'torrent' ? <Magnet size={14} /> : <HardDrive size={14} />}
                        </a>
                      )}
                    </div>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
    </div>
  )
}

function ProtocolBadge({ protocol }: { protocol: string }) {
  const isTorrent = protocol === 'torrent'
  return (
    <span className={`rounded-full px-2 py-0.5 text-xs font-medium ${
      isTorrent ? 'bg-orange-500/20 text-orange-400' : 'bg-purple-500/20 text-purple-400'
    }`}>
      {isTorrent ? 'Torrent' : 'Usenet'}
    </span>
  )
}

function formatSize(bytes: number): string {
  if (!bytes || !isFinite(bytes) || bytes <= 0) return '0 B'
  const units = ['B', 'KB', 'MB', 'GB', 'TB']
  const i = Math.floor(Math.log(bytes) / Math.log(1024))
  return `${(bytes / Math.pow(1024, i)).toFixed(1)} ${units[i]}`
}

function formatAge(days: number): string {
  if (days <= 0) return 'Today'
  if (days === 1) return '1 day'
  if (days < 30) return `${days}d`
  if (days < 365) return `${Math.floor(days / 30)}mo`
  return `${(days / 365).toFixed(1)}y`
}
