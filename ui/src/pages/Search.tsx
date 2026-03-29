import { useState, useRef, useEffect, useMemo } from 'react'
import { useSearchParams } from 'react-router-dom'
import {
  Search as SearchIcon,
  Loader2,
  ExternalLink,
  Download,
  CheckCircle,
  ChevronDown,
  ChevronUp,
  ArrowUpDown,
  X,
} from 'lucide-react'
import { useSearchReleases, useIndexers, useSystemStatus, useGrabRelease } from '../hooks/useApi'
import type { FreehandSearchResult } from '../api/types'

type SortField = 'title' | 'quality' | 'indexer' | 'protocol' | 'size' | 'age' | 'seeders'
type SortDir = 'asc' | 'desc'

function sortResults(results: FreehandSearchResult[], field: SortField, dir: SortDir): FreehandSearchResult[] {
  const sorted = [...results]
  const m = dir === 'asc' ? 1 : -1
  sorted.sort((a, b) => {
    switch (field) {
      case 'title': return m * a.title.localeCompare(b.title)
      case 'quality': return m * (a.quality ?? '').localeCompare(b.quality ?? '')
      case 'indexer': return m * a.indexerName.localeCompare(b.indexerName)
      case 'protocol': return m * a.protocol.localeCompare(b.protocol)
      case 'size': return m * (a.size - b.size)
      case 'age': return m * (a.ageDays - b.ageDays)
      case 'seeders': return m * ((a.seeders ?? -1) - (b.seeders ?? -1))
    }
  })
  return sorted
}

export default function Search() {
  const [searchParams] = useSearchParams()
  const initialQuery = searchParams.get('q') ?? ''
  const [input, setInput] = useState(initialQuery)
  const [query, setQuery] = useState(initialQuery)
  const [selectedIndexerIds, setSelectedIndexerIds] = useState<number[]>([])
  const [indexarrOnly, setIndexarrOnly] = useState(false)
  const [dropdownOpen, setDropdownOpen] = useState(false)
  const [sortField, setSortField] = useState<SortField>('size')
  const [sortDir, setSortDir] = useState<SortDir>('desc')
  const [grabbedGuids, setGrabbedGuids] = useState<Set<string>>(new Set())
  const dropdownRef = useRef<HTMLDivElement>(null)

  const { data: indexers } = useIndexers()
  const { data: status } = useSystemStatus()
  const indexarrEnabled = status?.modules?.indexarrSidecar === true
  const enabledIndexers = (indexers ?? []).filter(i => i.enabled)
  const { data: results, isLoading, error } = useSearchReleases(query, selectedIndexerIds, indexarrOnly)
  const grabMutation = useGrabRelease()

  const sorted = useMemo(
    () => results ? sortResults(results, sortField, sortDir) : [],
    [results, sortField, sortDir],
  )

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault()
    setQuery(input.trim())
  }

  const toggleSort = (field: SortField) => {
    if (sortField === field) {
      setSortDir(d => d === 'asc' ? 'desc' : 'asc')
    } else {
      setSortField(field)
      setSortDir(field === 'title' || field === 'indexer' ? 'asc' : 'desc')
    }
  }

  const toggleIndexer = (id: number) => {
    setIndexarrOnly(false)
    setSelectedIndexerIds(prev =>
      prev.includes(id) ? prev.filter(i => i !== id) : [...prev, id]
    )
  }

  const handleGrab = (r: FreehandSearchResult) => {
    if (!r.downloadUrl) return
    grabMutation.mutate(
      {
        guid: r.guid,
        indexerId: r.indexerId,
        title: r.title,
        downloadUrl: r.downloadUrl,
        protocol: r.protocol,
        size: r.size,
      },
      {
        onSuccess: () => {
          setGrabbedGuids(prev => new Set(prev).add(r.guid))
        },
      },
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

      {sorted.length > 0 && (
        <div className="overflow-x-auto rounded-lg bg-slate-800">
          <table className="w-full text-sm">
            <thead>
              <tr className="border-b border-slate-700 text-left text-[11px] uppercase tracking-wide text-slate-500">
                <SortHeader field="title" label="Title" current={sortField} dir={sortDir} onClick={toggleSort} className="pl-4 w-[40%]" />
                <SortHeader field="quality" label="Quality" current={sortField} dir={sortDir} onClick={toggleSort} />
                <SortHeader field="indexer" label="Indexer" current={sortField} dir={sortDir} onClick={toggleSort} />
                <SortHeader field="protocol" label="Type" current={sortField} dir={sortDir} onClick={toggleSort} />
                <SortHeader field="size" label="Size" current={sortField} dir={sortDir} onClick={toggleSort} />
                <SortHeader field="age" label="Age" current={sortField} dir={sortDir} onClick={toggleSort} />
                <SortHeader field="seeders" label="Peers" current={sortField} dir={sortDir} onClick={toggleSort} />
                <th className="px-3 py-3 font-medium text-right pr-4">Actions</th>
              </tr>
            </thead>
            <tbody>
              {sorted.map((r) => (
                <tr key={r.guid} className="border-b border-slate-700/40 hover:bg-slate-700/30 transition-colors">
                  <td className="py-2.5 pl-4 pr-3">
                    <div className="text-xs text-white leading-snug break-all line-clamp-2" title={r.title}>
                      {r.title}
                    </div>
                  </td>
                  <td className="px-3 py-2.5">
                    {r.quality && r.quality !== 'Unknown' ? (
                      <span className="rounded bg-blue-500/20 px-1.5 py-0.5 text-[10px] font-medium text-blue-400">
                        {r.quality}
                      </span>
                    ) : (
                      <span className="text-slate-600 text-xs">-</span>
                    )}
                  </td>
                  <td className="px-3 py-2.5 text-xs text-blue-400 whitespace-nowrap">{r.indexerName}</td>
                  <td className="px-3 py-2.5">
                    <ProtocolBadge protocol={r.protocol} />
                  </td>
                  <td className="px-3 py-2.5 text-xs text-slate-300 whitespace-nowrap">{formatSize(r.size)}</td>
                  <td className="px-3 py-2.5 text-xs text-slate-400 whitespace-nowrap">{formatAge(r.ageDays)}</td>
                  <td className="px-3 py-2.5 text-xs">
                    {r.protocol === 'torrent' && r.seeders != null ? (
                      <span>
                        <span className={r.seeders > 0 ? 'text-green-400' : 'text-red-400'}>{r.seeders}</span>
                        <span className="text-slate-600"> / </span>
                        <span className="text-slate-400">{r.leechers ?? 0}</span>
                      </span>
                    ) : (
                      <span className="text-slate-600">-</span>
                    )}
                  </td>
                  <td className="px-3 py-2.5 pr-4">
                    <div className="flex items-center justify-end gap-1.5">
                      {r.infoUrl && (
                        <a
                          href={r.infoUrl}
                          target="_blank"
                          rel="noopener noreferrer"
                          className="rounded p-1 text-slate-500 hover:text-blue-400 hover:bg-slate-700 transition-colors"
                          title="Info page"
                        >
                          <ExternalLink size={13} />
                        </a>
                      )}
                      {grabbedGuids.has(r.guid) ? (
                        <span className="flex items-center gap-1 rounded-md bg-green-500/15 px-2 py-1 text-[11px] text-green-400">
                          <CheckCircle size={12} /> Grabbed
                        </span>
                      ) : r.downloadUrl ? (
                        <button
                          onClick={() => handleGrab(r)}
                          disabled={grabMutation.isPending && grabMutation.variables?.guid === r.guid}
                          className="flex items-center gap-1 rounded-md bg-blue-600 px-2.5 py-1 text-[11px] font-medium text-white hover:bg-blue-500 disabled:opacity-40 transition-colors"
                          title="Download this release"
                        >
                          {grabMutation.isPending && grabMutation.variables?.guid === r.guid ? (
                            <Loader2 size={12} className="animate-spin" />
                          ) : (
                            <Download size={12} />
                          )}
                          Grab
                        </button>
                      ) : (
                        <span className="text-slate-600 text-xs">-</span>
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

function SortHeader({
  field,
  label,
  current,
  dir,
  onClick,
  className = '',
}: {
  field: SortField
  label: string
  current: SortField
  dir: SortDir
  onClick: (f: SortField) => void
  className?: string
}) {
  const active = current === field
  return (
    <th className={`px-3 py-3 font-medium ${className}`}>
      <button
        onClick={() => onClick(field)}
        className={`inline-flex items-center gap-1 hover:text-slate-300 transition-colors ${active ? 'text-blue-400' : ''}`}
      >
        {label}
        {active ? (
          dir === 'asc' ? <ChevronUp size={12} /> : <ChevronDown size={12} />
        ) : (
          <ArrowUpDown size={10} className="opacity-40" />
        )}
      </button>
    </th>
  )
}

function ProtocolBadge({ protocol }: { protocol: string }) {
  const isTorrent = protocol === 'torrent'
  return (
    <span className={`rounded-full px-2 py-0.5 text-[10px] font-medium ${
      isTorrent ? 'bg-orange-500/20 text-orange-400' : 'bg-purple-500/20 text-purple-400'
    }`}>
      {isTorrent ? 'Torrent' : 'Usenet'}
    </span>
  )
}

function formatSize(bytes: number): string {
  if (!bytes || !isFinite(bytes) || bytes <= 0) return '-'
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
