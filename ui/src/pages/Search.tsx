import { useState } from 'react'
import { Search as SearchIcon, Loader2, ExternalLink, Magnet, HardDrive } from 'lucide-react'
import { useSearchReleases } from '../hooks/useApi'

export default function Search() {
  const [input, setInput] = useState('')
  const [query, setQuery] = useState('')
  const { data: results, isLoading, error } = useSearchReleases(query)

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault()
    setQuery(input.trim())
  }

  return (
    <div>
      <div className="mb-6">
        <h2 className="text-2xl font-bold mb-4">Search</h2>
        <form onSubmit={handleSubmit} className="flex gap-3">
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
