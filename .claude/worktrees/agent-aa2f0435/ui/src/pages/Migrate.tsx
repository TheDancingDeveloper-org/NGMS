import { useState, useRef } from 'react'
import { Database, Upload, Loader2, CheckCircle, XCircle, FileUp, Server, Globe, Plus, Trash2, ArrowRight } from 'lucide-react'
import { useMigrate } from '../hooks/useApi'
import type { MigrationResult } from '../api/types'

interface PathMapping {
  from: string
  to: string
}

export default function Migrate() {
  return (
    <div>
      <h2 className="mb-6 text-2xl font-bold">Migration</h2>

      <div className="mx-auto max-w-2xl space-y-6">
        <ArrMigration />
        <SabnzbdImport />
      </div>
    </div>
  )
}

// ── Sonarr / Radarr / Prowlarr Migration ──────────────────────────────────

function ArrMigration() {
  const mutation = useMigrate()
  const [sonarrFile, setSonarrFile] = useState<File | null>(null)
  const [radarrFile, setRadarrFile] = useState<File | null>(null)
  const [prowlarrFile, setProwlarrFile] = useState<File | null>(null)
  const [pathMappings, setPathMappings] = useState<PathMapping[]>([])
  const formRef = useRef<HTMLFormElement>(null)

  const hasAnyFile = sonarrFile || radarrFile || prowlarrFile

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault()
    if (!hasAnyFile) return

    const formData = new FormData()
    if (sonarrFile) formData.append('sonarr_db', sonarrFile)
    if (radarrFile) formData.append('radarr_db', radarrFile)
    if (prowlarrFile) formData.append('prowlarr_db', prowlarrFile)

    const validMappings = pathMappings.filter(m => m.from && m.to)
    if (validMappings.length > 0) {
      formData.append('path_mappings', new Blob([JSON.stringify(validMappings)], { type: 'application/json' }))
    }

    mutation.mutate(formData)
  }

  const handleReset = () => {
    setSonarrFile(null)
    setRadarrFile(null)
    setProwlarrFile(null)
    setPathMappings([])
    mutation.reset()
    formRef.current?.reset()
  }

  const addMapping = () => setPathMappings([...pathMappings, { from: '', to: '' }])
  const removeMapping = (i: number) => setPathMappings(pathMappings.filter((_, idx) => idx !== i))
  const updateMapping = (i: number, field: 'from' | 'to', value: string) => {
    const updated = [...pathMappings]
    updated[i] = { ...updated[i], [field]: value }
    setPathMappings(updated)
  }

  return (
    <div className="rounded-xl bg-slate-800 p-8">
      <div className="mb-6 flex items-center gap-3">
        <Database size={24} className="text-blue-400" />
        <div>
          <h3 className="text-lg font-semibold">Import from Sonarr / Radarr / Prowlarr</h3>
          <p className="text-sm text-slate-400">
            Upload database files to migrate your existing library data into StackArr.
          </p>
        </div>
      </div>

      {!mutation.isSuccess && (
        <form ref={formRef} onSubmit={handleSubmit}>
          <div className="space-y-4">
            <FileInput label="sonarr.db" description="Sonarr database file" file={sonarrFile} onFileChange={setSonarrFile} accept=".db" />
            <FileInput label="radarr.db" description="Radarr database file" file={radarrFile} onFileChange={setRadarrFile} accept=".db" />
            <FileInput label="prowlarr.db" description="Prowlarr database file (indexers)" file={prowlarrFile} onFileChange={setProwlarrFile} accept=".db" />
          </div>

          {/* Path Mappings */}
          <div className="mt-6">
            <div className="mb-3 flex items-center justify-between">
              <div>
                <h4 className="text-sm font-semibold text-slate-300">Path Mappings</h4>
                <p className="text-xs text-slate-500">Remap root folder paths from your old *arr containers to StackArr mount points</p>
              </div>
              <button
                type="button"
                onClick={addMapping}
                className="flex items-center gap-1.5 rounded-lg bg-slate-700 px-3 py-1.5 text-xs font-medium text-slate-300 hover:bg-slate-600 transition-colors"
              >
                <Plus size={14} /> Add Mapping
              </button>
            </div>
            {pathMappings.length > 0 && (
              <div className="space-y-2">
                {pathMappings.map((m, i) => (
                  <div key={i} className="flex items-center gap-2">
                    <input
                      type="text"
                      value={m.from}
                      onChange={(e) => updateMapping(i, 'from', e.target.value)}
                      placeholder="/mnt/movies1/"
                      className="flex-1 rounded-lg border border-slate-600 bg-slate-700 px-3 py-2 text-sm text-white placeholder:text-slate-500 focus:border-blue-500 focus:outline-none"
                    />
                    <ArrowRight size={16} className="shrink-0 text-slate-500" />
                    <input
                      type="text"
                      value={m.to}
                      onChange={(e) => updateMapping(i, 'to', e.target.value)}
                      placeholder="/media/Movies1/"
                      className="flex-1 rounded-lg border border-slate-600 bg-slate-700 px-3 py-2 text-sm text-white placeholder:text-slate-500 focus:border-blue-500 focus:outline-none"
                    />
                    <button type="button" onClick={() => removeMapping(i)} className="shrink-0 text-slate-500 hover:text-red-400 transition-colors">
                      <Trash2 size={16} />
                    </button>
                  </div>
                ))}
              </div>
            )}
          </div>

          {mutation.isError && (
            <div className="mt-4 rounded-lg border border-red-500/30 bg-red-500/10 p-3 text-sm text-red-400">
              Migration failed: {mutation.error.message}
            </div>
          )}

          <div className="mt-6 flex gap-3">
            <button
              type="submit"
              disabled={!hasAnyFile || mutation.isPending}
              className="flex items-center gap-2 rounded-lg bg-blue-600 px-6 py-2.5 font-medium text-white hover:bg-blue-700 disabled:opacity-50 transition-colors"
            >
              {mutation.isPending ? (
                <><Loader2 size={16} className="animate-spin" /> Migrating...</>
              ) : (
                <><Upload size={16} /> Start Migration</>
              )}
            </button>
            {hasAnyFile && !mutation.isPending && (
              <button type="button" onClick={handleReset} className="rounded-lg bg-slate-700 px-4 py-2.5 font-medium text-slate-300 hover:bg-slate-600 transition-colors">
                Clear
              </button>
            )}
          </div>
        </form>
      )}

      {mutation.isSuccess && mutation.data && (
        <ArrMigrationReport result={mutation.data} onReset={handleReset} />
      )}
    </div>
  )
}

// ── SABnzbd Config Import ─────────────────────────────────────────────────

interface SabnzbdPreview {
  servers: Array<{
    name: string
    host: string
    port: number
    ssl: boolean
    username: string
    connections: number
    priority: number
    enabled: boolean
    password_masked: boolean
  }>
  categories: Array<{ name: string; output_dir: string | null; post_processing: number }>
  general: { api_key: string | null; complete_dir: string | null; incomplete_dir: string | null; speed_limit_bps: number }
  rss_feeds: Array<{ name: string; url: string; enabled: boolean }>
  warnings: string[]
  skipped_fields: string[]
}

interface SabnzbdApplyResult {
  success: boolean
  serversAdded: number
  categoriesAdded: number
  rssFeedsAdded: number
  warnings: string[]
  skippedFields: string[]
}

function SabnzbdImport() {
  const [mode, setMode] = useState<'ini' | 'api'>('ini')
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [preview, setPreview] = useState<SabnzbdPreview | null>(null)
  const [applyResult, setApplyResult] = useState<SabnzbdApplyResult | null>(null)

  // INI file upload
  const [iniFile, setIniFile] = useState<File | null>(null)

  // API mode
  const [sabUrl, setSabUrl] = useState('')
  const [sabApiKey, setSabApiKey] = useState('')

  const handleImportIni = async () => {
    if (!iniFile) return
    setLoading(true)
    setError(null)
    try {
      const formData = new FormData()
      formData.append('file', iniFile)
      const res = await fetch('/api/v1/usenet/import-sabnzbd', { method: 'POST', body: formData })
      if (!res.ok) {
        const err = await res.json().catch(() => ({ error: res.statusText }))
        throw new Error(err.error || res.statusText)
      }
      setPreview(await res.json())
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e))
    } finally {
      setLoading(false)
    }
  }

  const handleImportApi = async () => {
    if (!sabUrl) return
    setLoading(true)
    setError(null)
    try {
      const res = await fetch('/api/v1/usenet/import-sabnzbd-api', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ url: sabUrl, apiKey: sabApiKey }),
      })
      if (!res.ok) {
        const err = await res.json().catch(() => ({ error: res.statusText }))
        throw new Error(err.error || res.statusText)
      }
      setPreview(await res.json())
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e))
    } finally {
      setLoading(false)
    }
  }

  const handleApply = async () => {
    if (!preview) return
    setLoading(true)
    setError(null)
    try {
      const res = await fetch('/api/v1/usenet/import-sabnzbd/apply', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(preview),
      })
      if (!res.ok) {
        const err = await res.json().catch(() => ({ error: res.statusText }))
        throw new Error(err.error || res.statusText)
      }
      setApplyResult(await res.json())
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e))
    } finally {
      setLoading(false)
    }
  }

  const handleReset = () => {
    setPreview(null)
    setApplyResult(null)
    setError(null)
    setIniFile(null)
    setSabUrl('')
    setSabApiKey('')
  }

  return (
    <div className="rounded-xl bg-slate-800 p-8">
      <div className="mb-6 flex items-center gap-3">
        <Server size={24} className="text-orange-400" />
        <div>
          <h3 className="text-lg font-semibold">Import from SABnzbd</h3>
          <p className="text-sm text-slate-400">
            Import NNTP servers, categories, and settings from an existing SABnzbd installation.
          </p>
        </div>
      </div>

      {/* Applied result */}
      {applyResult && (
        <div>
          <div className="mb-4 flex items-center gap-2">
            <CheckCircle size={24} className="text-green-500" />
            <span className="text-lg font-semibold text-green-400">SABnzbd Import Applied</span>
          </div>
          <div className="space-y-2 mb-4">
            <ResultRow label="NNTP servers added" value={applyResult.serversAdded} />
            <ResultRow label="Categories added" value={applyResult.categoriesAdded} />
            <ResultRow label="RSS feeds added" value={applyResult.rssFeedsAdded} />
          </div>
          {applyResult.warnings.length > 0 && (
            <div className="mb-4 rounded-lg border border-yellow-500/30 bg-yellow-500/10 p-3">
              <h4 className="mb-1 text-sm font-medium text-yellow-400">Warnings</h4>
              {applyResult.warnings.map((w, i) => <div key={i} className="text-xs text-yellow-300">{w}</div>)}
            </div>
          )}
          <button onClick={handleReset} className="rounded-lg bg-slate-700 px-4 py-2 text-sm font-medium text-slate-300 hover:bg-slate-600 transition-colors">
            Done
          </button>
        </div>
      )}

      {/* Preview */}
      {!applyResult && preview && (
        <div>
          <h4 className="mb-3 text-sm font-semibold text-slate-300 uppercase tracking-wider">Import Preview</h4>

          {preview.servers.length > 0 && (
            <div className="mb-4">
              <h5 className="mb-2 text-sm font-medium text-slate-400">NNTP Servers ({preview.servers.length})</h5>
              <div className="space-y-2">
                {preview.servers.map((s, i) => (
                  <div key={i} className="flex items-center gap-3 rounded-lg bg-slate-700/50 px-4 py-2.5 text-sm">
                    <Server size={14} className="text-orange-400 shrink-0" />
                    <span className="font-medium text-white">{s.name}</span>
                    <span className="text-slate-400">{s.host}:{s.port}</span>
                    {s.ssl && <span className="rounded bg-green-500/20 px-1.5 py-0.5 text-[10px] text-green-400">SSL</span>}
                    <span className="text-slate-500">{s.connections} conn</span>
                    {s.password_masked && <span className="rounded bg-red-500/20 px-1.5 py-0.5 text-[10px] text-red-400">password masked</span>}
                    {!s.enabled && <span className="rounded bg-slate-600 px-1.5 py-0.5 text-[10px] text-slate-400">disabled</span>}
                  </div>
                ))}
              </div>
            </div>
          )}

          {preview.categories.length > 0 && (
            <div className="mb-4">
              <h5 className="mb-2 text-sm font-medium text-slate-400">Categories ({preview.categories.length})</h5>
              <div className="flex flex-wrap gap-2">
                {preview.categories.map((c, i) => (
                  <span key={i} className="rounded-lg bg-slate-700 px-3 py-1.5 text-sm text-white">{c.name}</span>
                ))}
              </div>
            </div>
          )}

          {preview.rss_feeds.length > 0 && (
            <div className="mb-4">
              <h5 className="mb-2 text-sm font-medium text-slate-400">RSS Feeds ({preview.rss_feeds.length})</h5>
              <div className="space-y-1">
                {preview.rss_feeds.map((f, i) => (
                  <div key={i} className="text-sm text-slate-300">{f.name}</div>
                ))}
              </div>
            </div>
          )}

          {preview.warnings.length > 0 && (
            <div className="mb-4 rounded-lg border border-yellow-500/30 bg-yellow-500/10 p-3">
              <h5 className="mb-1 text-sm font-medium text-yellow-400">Warnings</h5>
              {preview.warnings.map((w, i) => <div key={i} className="text-xs text-yellow-300">{w}</div>)}
            </div>
          )}

          {error && (
            <div className="mb-4 rounded-lg border border-red-500/30 bg-red-500/10 p-3 text-sm text-red-400">{error}</div>
          )}

          <div className="flex gap-3">
            <button
              onClick={handleApply}
              disabled={loading}
              className="flex items-center gap-2 rounded-lg bg-orange-600 px-6 py-2.5 font-medium text-white hover:bg-orange-700 disabled:opacity-50 transition-colors"
            >
              {loading ? <><Loader2 size={16} className="animate-spin" /> Applying...</> : <><CheckCircle size={16} /> Apply Import</>}
            </button>
            <button onClick={handleReset} className="rounded-lg bg-slate-700 px-4 py-2.5 font-medium text-slate-300 hover:bg-slate-600 transition-colors">
              Cancel
            </button>
          </div>
        </div>
      )}

      {/* Import form (no preview yet) */}
      {!applyResult && !preview && (
        <div>
          {/* Mode toggle */}
          <div className="mb-5 flex rounded-lg bg-slate-700/50 p-1">
            <button
              onClick={() => setMode('ini')}
              className={`flex-1 rounded-md px-4 py-2 text-sm font-medium transition-colors ${mode === 'ini' ? 'bg-slate-600 text-white' : 'text-slate-400 hover:text-white'}`}
            >
              <FileUp size={14} className="mr-1.5 inline" /> Upload INI File
            </button>
            <button
              onClick={() => setMode('api')}
              className={`flex-1 rounded-md px-4 py-2 text-sm font-medium transition-colors ${mode === 'api' ? 'bg-slate-600 text-white' : 'text-slate-400 hover:text-white'}`}
            >
              <Globe size={14} className="mr-1.5 inline" /> Connect to SABnzbd
            </button>
          </div>

          {mode === 'ini' && (
            <div>
              <FileInput label="sabnzbd.ini" description="SABnzbd configuration file" file={iniFile} onFileChange={setIniFile} accept=".ini,.conf,.cfg" />
              {error && <div className="mt-3 rounded-lg border border-red-500/30 bg-red-500/10 p-3 text-sm text-red-400">{error}</div>}
              <button
                onClick={handleImportIni}
                disabled={!iniFile || loading}
                className="mt-4 flex items-center gap-2 rounded-lg bg-orange-600 px-6 py-2.5 font-medium text-white hover:bg-orange-700 disabled:opacity-50 transition-colors"
              >
                {loading ? <><Loader2 size={16} className="animate-spin" /> Parsing...</> : <><Upload size={16} /> Parse Config</>}
              </button>
            </div>
          )}

          {mode === 'api' && (
            <div className="space-y-4">
              <div>
                <label className="mb-1 block text-sm text-slate-400">SABnzbd URL</label>
                <input
                  type="url"
                  value={sabUrl}
                  onChange={(e) => setSabUrl(e.target.value)}
                  placeholder="http://192.168.0.30:8080"
                  className="w-full rounded-lg border border-slate-600 bg-slate-700 px-4 py-2.5 text-white placeholder:text-slate-500 focus:border-orange-500 focus:outline-none focus:ring-1 focus:ring-orange-500"
                />
              </div>
              <div>
                <label className="mb-1 block text-sm text-slate-400">API Key</label>
                <input
                  type="text"
                  value={sabApiKey}
                  onChange={(e) => setSabApiKey(e.target.value)}
                  placeholder="Your SABnzbd API key"
                  className="w-full rounded-lg border border-slate-600 bg-slate-700 px-4 py-2.5 text-white placeholder:text-slate-500 focus:border-orange-500 focus:outline-none focus:ring-1 focus:ring-orange-500"
                />
                <p className="mt-1 text-xs text-slate-500">Note: passwords may be masked when importing via API. Use INI file for full credentials.</p>
              </div>
              {error && <div className="rounded-lg border border-red-500/30 bg-red-500/10 p-3 text-sm text-red-400">{error}</div>}
              <button
                onClick={handleImportApi}
                disabled={!sabUrl || loading}
                className="flex items-center gap-2 rounded-lg bg-orange-600 px-6 py-2.5 font-medium text-white hover:bg-orange-700 disabled:opacity-50 transition-colors"
              >
                {loading ? <><Loader2 size={16} className="animate-spin" /> Connecting...</> : <><Globe size={16} /> Fetch Config</>}
              </button>
            </div>
          )}
        </div>
      )}
    </div>
  )
}

// ── Shared Components ─────────────────────────────────────────────────────

function FileInput({ label, description, file, onFileChange, accept }: {
  label: string; description: string; file: File | null; onFileChange: (f: File | null) => void; accept: string
}) {
  const inputRef = useRef<HTMLInputElement>(null)

  return (
    <div
      onClick={() => inputRef.current?.click()}
      className="flex cursor-pointer items-center gap-4 rounded-lg border border-dashed border-slate-600 p-4 hover:border-blue-500 transition-colors"
    >
      <FileUp size={24} className={file ? 'text-blue-400' : 'text-slate-500'} />
      <div className="flex-1 min-w-0">
        <div className="font-medium text-white">{label}</div>
        <div className="text-xs text-slate-400">
          {file ? (
            <span className="text-blue-400">{file.name} ({(file.size / 1048576).toFixed(1)} MB)</span>
          ) : description}
        </div>
      </div>
      <input ref={inputRef} type="file" accept={accept} className="hidden" onChange={(e) => onFileChange(e.target.files?.[0] ?? null)} />
      {file && (
        <button type="button" onClick={(e) => { e.stopPropagation(); onFileChange(null); if (inputRef.current) inputRef.current.value = '' }} className="text-slate-400 hover:text-white">
          <XCircle size={16} />
        </button>
      )}
    </div>
  )
}

function ResultRow({ label, value }: { label: string; value: number | string }) {
  return (
    <div className="flex justify-between rounded-lg bg-slate-700/50 px-4 py-3">
      <span className="text-slate-400">{label}</span>
      <span className="font-medium text-white">{value}</span>
    </div>
  )
}

function ArrMigrationReport({ result, onReset }: { result: MigrationResult; onReset: () => void }) {
  return (
    <div>
      <div className="mb-4 flex items-center gap-2">
        {result.warnings.length === 0 ? (
          <><CheckCircle size={24} className="text-green-500" /><span className="text-lg font-semibold text-green-400">Migration Complete</span></>
        ) : (
          <><XCircle size={24} className="text-yellow-500" /><span className="text-lg font-semibold text-yellow-400">Migration completed with warnings</span></>
        )}
      </div>

      <div className="mb-4 space-y-2">
        <ResultRow label="Series imported" value={result.seriesImported} />
        <ResultRow label="Movies imported" value={result.moviesImported} />
        <ResultRow label="Indexers imported" value={result.indexersImported} />
      </div>

      {result.warnings.length > 0 && (
        <div className="mb-4">
          <h4 className="mb-2 text-sm font-medium text-yellow-400">Warnings</h4>
          <div className="max-h-40 overflow-y-auto rounded-lg bg-slate-900 p-3">
            {result.warnings.map((err, i) => <div key={i} className="text-xs text-yellow-300">{err}</div>)}
          </div>
        </div>
      )}

      <button onClick={onReset} className="rounded-lg bg-slate-700 px-4 py-2 text-sm font-medium text-slate-300 hover:bg-slate-600 transition-colors">
        Start Over
      </button>
    </div>
  )
}
