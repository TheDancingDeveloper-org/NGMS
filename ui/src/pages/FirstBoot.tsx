import { useState, useMemo, useRef } from 'react'
import { useNavigate } from 'react-router-dom'
import {
  CheckCircle,
  ChevronRight,
  ChevronLeft,
  Tv,
  Film,
  Loader2,
  Magnet,
  HardDrive,
  Globe,
  MonitorPlay,
  FileUp,
  XCircle,
  Upload,
  Database,
  Server,
  Key,
  Copy,
  Check,
  FolderOpen,
  SkipForward,
} from 'lucide-react'
import { useSetupInit } from '../hooks/useApi'
import type { SetupInit, MigrationResult } from '../api/types'

// ── Step definitions ─────────────────────────────────────────────────────────

type StepName = 'Features' | 'Import' | 'Indexarr' | 'Media Libraries' | 'Complete'

export default function FirstBoot() {
  const navigate = useNavigate()
  const setupMutation = useSetupInit()
  const [step, setStep] = useState(0)

  // Step 0: Feature selections
  const [enableTv, setEnableTv] = useState(true)
  const [enableMovies, setEnableMovies] = useState(true)
  const [enableTorrent, setEnableTorrent] = useState(false)
  const [enableUsenet, setEnableUsenet] = useState(false)
  const [enableIndexarr, setEnableIndexarr] = useState(false)
  const [enablePlex, setEnablePlex] = useState(false)

  // Step 1: Import state
  const [sonarrFile, setSonarrFile] = useState<File | null>(null)
  const [radarrFile, setRadarrFile] = useState<File | null>(null)
  const [prowlarrFile, setProwlarrFile] = useState<File | null>(null)
  const [sabnzbdFile, setSabnzbdFile] = useState<File | null>(null)
  const [importRunning, setImportRunning] = useState(false)
  const [importResult, setImportResult] = useState<MigrationResult | null>(null)
  const [importError, setImportError] = useState<string | null>(null)
  const [sabApplied, setSabApplied] = useState(false)

  // Step 2: Indexarr config
  const [indexarrUrl, setIndexarrUrl] = useState('http://indexarr:8080')
  const [indexarrApiKey] = useState(() => {
    if (typeof crypto.randomUUID === 'function') return crypto.randomUUID()
    const bytes = crypto.getRandomValues(new Uint8Array(16))
    bytes[6] = (bytes[6] & 0x0f) | 0x40
    bytes[8] = (bytes[8] & 0x3f) | 0x80
    const hex = Array.from(bytes, b => b.toString(16).padStart(2, '0')).join('')
    return `${hex.slice(0, 8)}-${hex.slice(8, 12)}-${hex.slice(12, 16)}-${hex.slice(16, 20)}-${hex.slice(20)}`
  })
  const [keyCopied, setKeyCopied] = useState(false)

  // Step 3: Media library folders
  const [tvLibraryFolder, setTvLibraryFolder] = useState('/media/tv')
  const [movieLibraryFolder, setMovieLibraryFolder] = useState('/media/movies')

  // Step 4: Complete
  const [done, setDone] = useState(false)

  // Compute active steps based on feature selections
  const steps = useMemo<StepName[]>(() => {
    const s: StepName[] = ['Features', 'Import']
    if (enableIndexarr) s.push('Indexarr')
    s.push('Media Libraries', 'Complete')
    return s
  }, [enableIndexarr])

  const currentStep = steps[step]

  const canNext = () => {
    if (currentStep === 'Features') return enableTv || enableMovies
    return true
  }

  const handleNext = () => {
    if (step < steps.length - 1) setStep((s) => s + 1)
  }

  const handleBack = () => {
    if (step > 0) setStep((s) => s - 1)
  }

  // ── Import handlers ──────────────────────────────────────────────────────

  const handleRunImport = async () => {
    setImportRunning(true)
    setImportError(null)
    setImportResult(null)

    try {
      // Import Sonarr / Radarr / Prowlarr
      if (sonarrFile || radarrFile || prowlarrFile) {
        const formData = new FormData()
        if (sonarrFile) formData.append('sonarr_db', sonarrFile)
        if (radarrFile) formData.append('radarr_db', radarrFile)
        if (prowlarrFile) formData.append('prowlarr_db', prowlarrFile)

        const res = await fetch('/api/v1/system/migrate', {
          method: 'POST',
          body: formData,
        })
        if (!res.ok) {
          const err = await res.json().catch(() => ({ error: res.statusText }))
          throw new Error(err.error || `Migration failed: ${res.statusText}`)
        }
        const result = (await res.json()) as MigrationResult
        setImportResult(result)
      }

      // Import SABnzbd config
      if (sabnzbdFile) {
        const sabForm = new FormData()
        sabForm.append('file', sabnzbdFile)
        const sabRes = await fetch('/api/v1/usenet/import-sabnzbd', {
          method: 'POST',
          body: sabForm,
        })
        if (sabRes.ok) {
          const preview = await sabRes.json()
          // Auto-apply the SABnzbd import
          const applyRes = await fetch('/api/v1/usenet/import-sabnzbd/apply', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify(preview),
          })
          if (applyRes.ok) setSabApplied(true)
        }
      }
    } catch (e) {
      setImportError(e instanceof Error ? e.message : String(e))
    } finally {
      setImportRunning(false)
    }
  }

  // ── Setup finalization ──────────────────────────────────────────────────

  const handleFinish = () => {
    const payload: SetupInit = {
      modules: {
        tvManagement: enableTv,
        movieManagement: enableMovies,
        torrentEmbedded: enableTorrent,
        usenetEmbedded: enableUsenet,
        indexarrSidecar: enableIndexarr,
        plexIntegration: enablePlex,
      },
      mediaLibraryFolders: [
        ...(enableTv ? [{ path: tvLibraryFolder, mediaType: 'tv' }] : []),
        ...(enableMovies ? [{ path: movieLibraryFolder, mediaType: 'movie' }] : []),
      ],
      ...(enableIndexarr ? { indexarr: { url: indexarrUrl, apiKey: indexarrApiKey } } : {}),
    }

    setupMutation.mutate(payload, {
      onSuccess: () => setDone(true),
    })
  }

  const handleCopyKey = () => {
    navigator.clipboard.writeText(indexarrApiKey)
    setKeyCopied(true)
    setTimeout(() => setKeyCopied(false), 2000)
  }

  // ── Render ──────────────────────────────────────────────────────────────

  return (
    <div className="flex min-h-screen items-center justify-center bg-slate-900 p-4">
      <div className="w-full max-w-lg">
        {/* Step indicator */}
        <div className="mb-8 flex items-center justify-center gap-2">
          {steps.map((label, i) => (
            <div key={label} className="flex items-center gap-2">
              <div
                className={`flex h-8 w-8 items-center justify-center rounded-full text-sm font-medium ${
                  i < step
                    ? 'bg-blue-600 text-white'
                    : i === step
                      ? 'bg-blue-500 text-white ring-2 ring-blue-400 ring-offset-2 ring-offset-slate-900'
                      : 'bg-slate-700 text-slate-400'
                }`}
              >
                {i < step ? <CheckCircle size={16} /> : i + 1}
              </div>
              {i < steps.length - 1 && (
                <div className={`h-0.5 w-8 ${i < step ? 'bg-blue-600' : 'bg-slate-700'}`} />
              )}
            </div>
          ))}
        </div>

        {/* Card */}
        <div className="rounded-xl bg-slate-800 p-8 shadow-xl">
          {/* ── Step: Features ───────────────────────────────────────── */}
          {currentStep === 'Features' && (
            <div>
              <h2 className="mb-2 text-2xl font-bold text-white">Welcome to StackArr</h2>
              <p className="mb-6 text-slate-400">
                Your unified media management stack. Choose which modules to enable.
              </p>

              <div className="mb-4 text-xs font-semibold uppercase tracking-wider text-slate-500">
                Core — at least one required
              </div>
              <div className="space-y-3">
                <FeatureToggle
                  icon={<Tv size={24} className="text-blue-400" />}
                  label="TV Series"
                  desc="Track and manage TV shows"
                  checked={enableTv}
                  onChange={setEnableTv}
                />
                <FeatureToggle
                  icon={<Film size={24} className="text-purple-400" />}
                  label="Movies"
                  desc="Track and manage movies"
                  checked={enableMovies}
                  onChange={setEnableMovies}
                />
              </div>

              <div className="mb-4 mt-6 text-xs font-semibold uppercase tracking-wider text-slate-500">
                Optional Integrations
              </div>
              <div className="space-y-3">
                <FeatureToggle
                  icon={<Magnet size={24} className="text-orange-400" />}
                  label="RustTorrent"
                  desc="Built-in torrent download client"
                  checked={enableTorrent}
                  onChange={setEnableTorrent}
                />
                <FeatureToggle
                  icon={<HardDrive size={24} className="text-emerald-400" />}
                  label="RustNZB"
                  desc="Built-in usenet download client"
                  checked={enableUsenet}
                  onChange={setEnableUsenet}
                />
                <FeatureToggle
                  icon={<Globe size={24} className="text-cyan-400" />}
                  label="Indexarr"
                  desc="Distributed indexer network"
                  checked={enableIndexarr}
                  onChange={setEnableIndexarr}
                />
                <FeatureToggle
                  icon={<MonitorPlay size={24} className="text-yellow-400" />}
                  label="Plex"
                  desc="Media server integration & watchlist sync"
                  checked={enablePlex}
                  onChange={setEnablePlex}
                />
              </div>

              <p className="mt-4 text-xs text-slate-500">
                You can always add external download clients and indexers in Settings.
              </p>
            </div>
          )}

          {/* ── Step: Import ─────────────────────────────────────────── */}
          {currentStep === 'Import' && (
            <div>
              <h2 className="mb-2 text-2xl font-bold text-white">Import Existing Data</h2>
              <p className="mb-6 text-slate-400">
                Migrate your library from Sonarr, Radarr, and Prowlarr. This step is optional.
              </p>

              {!importResult && !importRunning && (
                <div className="space-y-4">
                  <div className="flex items-center gap-2 text-xs font-semibold uppercase tracking-wider text-slate-500">
                    <Database size={14} />
                    Sonarr / Radarr / Prowlarr
                  </div>
                  <FileInput
                    label="sonarr.db"
                    desc="Sonarr database"
                    file={sonarrFile}
                    onFileChange={setSonarrFile}
                    accept=".db"
                  />
                  <FileInput
                    label="radarr.db"
                    desc="Radarr database"
                    file={radarrFile}
                    onFileChange={setRadarrFile}
                    accept=".db"
                  />
                  <FileInput
                    label="prowlarr.db"
                    desc="Prowlarr database (indexers)"
                    file={prowlarrFile}
                    onFileChange={setProwlarrFile}
                    accept=".db"
                  />

                  {enableUsenet && (
                    <>
                      <div className="mt-2 flex items-center gap-2 text-xs font-semibold uppercase tracking-wider text-slate-500">
                        <Server size={14} />
                        SABnzbd
                      </div>
                      <FileInput
                        label="sabnzbd.ini"
                        desc="SABnzbd config (NNTP servers)"
                        file={sabnzbdFile}
                        onFileChange={setSabnzbdFile}
                        accept=".ini,.conf,.cfg"
                      />
                    </>
                  )}

                  {importError && (
                    <div className="rounded-lg border border-red-500/30 bg-red-500/10 p-3 text-sm text-red-400">
                      {importError}
                    </div>
                  )}

                  {(sonarrFile || radarrFile || prowlarrFile || sabnzbdFile) && (
                    <button
                      onClick={handleRunImport}
                      className="flex items-center gap-2 rounded-lg bg-blue-600 px-6 py-2.5 font-medium text-white hover:bg-blue-700 transition-colors"
                    >
                      <Upload size={16} /> Start Import
                    </button>
                  )}
                </div>
              )}

              {importRunning && (
                <div className="flex flex-col items-center gap-3 py-6">
                  <Loader2 size={48} className="animate-spin text-blue-500" />
                  <p className="text-slate-300">Importing data...</p>
                </div>
              )}

              {importResult && (
                <div>
                  <div className="mb-4 flex items-center gap-2">
                    <CheckCircle size={20} className="text-green-500" />
                    <span className="font-semibold text-green-400">Import Complete</span>
                  </div>
                  <div className="space-y-2 text-sm">
                    <div className="flex justify-between rounded-lg bg-slate-700/50 px-4 py-2">
                      <span className="text-slate-400">Series</span>
                      <span className="text-white">{importResult.imported.series}</span>
                    </div>
                    <div className="flex justify-between rounded-lg bg-slate-700/50 px-4 py-2">
                      <span className="text-slate-400">Movies</span>
                      <span className="text-white">{importResult.imported.movies}</span>
                    </div>
                    <div className="flex justify-between rounded-lg bg-slate-700/50 px-4 py-2">
                      <span className="text-slate-400">Indexers</span>
                      <span className="text-white">{importResult.imported.indexers}</span>
                    </div>
                    {sabApplied && (
                      <div className="flex justify-between rounded-lg bg-slate-700/50 px-4 py-2">
                        <span className="text-slate-400">SABnzbd config</span>
                        <span className="text-green-400">Applied</span>
                      </div>
                    )}
                  </div>
                  {importResult.errors.length > 0 && (
                    <div className="mt-3 max-h-24 overflow-y-auto rounded-lg bg-slate-900 p-3">
                      {importResult.errors.map((err, i) => (
                        <div key={i} className="text-xs text-red-300">
                          {err}
                        </div>
                      ))}
                    </div>
                  )}
                </div>
              )}
            </div>
          )}

          {/* ── Step: Indexarr ────────────────────────────────────────── */}
          {currentStep === 'Indexarr' && (
            <div>
              <h2 className="mb-2 text-2xl font-bold text-white">Indexarr Setup</h2>
              <p className="mb-6 text-slate-400">
                Configure the connection to your Indexarr sidecar instance.
              </p>

              <div className="space-y-4">
                <div>
                  <label className="mb-1.5 block text-sm font-medium text-slate-300">
                    Indexarr URL
                  </label>
                  <input
                    type="url"
                    value={indexarrUrl}
                    onChange={(e) => setIndexarrUrl(e.target.value)}
                    className="w-full rounded-lg border border-slate-600 bg-slate-700 px-4 py-2.5 text-white placeholder-slate-400 focus:border-blue-500 focus:outline-none focus:ring-1 focus:ring-blue-500"
                    placeholder="http://indexarr:8080"
                  />
                </div>

                <div>
                  <label className="mb-1.5 block text-sm font-medium text-slate-300">
                    <Key size={14} className="mr-1 inline text-yellow-400" />
                    API Key
                  </label>
                  <div className="flex gap-2">
                    <input
                      type="text"
                      value={indexarrApiKey}
                      readOnly
                      className="w-full rounded-lg border border-slate-600 bg-slate-700 px-4 py-2.5 font-mono text-sm text-white"
                    />
                    <button
                      onClick={handleCopyKey}
                      className="flex shrink-0 items-center gap-1.5 rounded-lg bg-slate-600 px-3 py-2.5 text-sm text-white hover:bg-slate-500 transition-colors"
                    >
                      {keyCopied ? (
                        <><Check size={14} className="text-green-400" /> Copied</>
                      ) : (
                        <><Copy size={14} /> Copy</>
                      )}
                    </button>
                  </div>
                  <p className="mt-2 text-xs text-slate-500">
                    Auto-generated key. Configure your Indexarr instance with this same key.
                  </p>
                </div>
              </div>
            </div>
          )}

          {/* ── Step: Media Libraries ─────────────────────────────────── */}
          {currentStep === 'Media Libraries' && (
            <div>
              <h2 className="mb-2 text-2xl font-bold text-white">Media Library Folders</h2>
              <p className="mb-6 text-slate-400">
                Set the directories for your media libraries.
              </p>
              <div className="space-y-4">
                {enableTv && (
                  <div>
                    <label className="mb-1.5 flex items-center gap-2 text-sm font-medium text-slate-300">
                      <FolderOpen size={16} className="text-blue-400" />
                      TV Library Folder
                    </label>
                    <input
                      type="text"
                      value={tvLibraryFolder}
                      onChange={(e) => setTvLibraryFolder(e.target.value)}
                      className="w-full rounded-lg border border-slate-600 bg-slate-700 px-4 py-2.5 text-white placeholder-slate-400 focus:border-blue-500 focus:outline-none focus:ring-1 focus:ring-blue-500"
                      placeholder="/media/tv"
                    />
                  </div>
                )}
                {enableMovies && (
                  <div>
                    <label className="mb-1.5 flex items-center gap-2 text-sm font-medium text-slate-300">
                      <FolderOpen size={16} className="text-purple-400" />
                      Movie Library Folder
                    </label>
                    <input
                      type="text"
                      value={movieLibraryFolder}
                      onChange={(e) => setMovieLibraryFolder(e.target.value)}
                      className="w-full rounded-lg border border-slate-600 bg-slate-700 px-4 py-2.5 text-white placeholder-slate-400 focus:border-blue-500 focus:outline-none focus:ring-1 focus:ring-blue-500"
                      placeholder="/media/movies"
                    />
                  </div>
                )}
              </div>
            </div>
          )}

          {/* ── Step: Complete ────────────────────────────────────────── */}
          {currentStep === 'Complete' && (
            <div className="text-center">
              {setupMutation.isPending && (
                <div className="flex flex-col items-center gap-3">
                  <Loader2 size={48} className="animate-spin text-blue-500" />
                  <p className="text-slate-300">Setting up StackArr...</p>
                </div>
              )}
              {setupMutation.isError && (
                <div>
                  <div className="mb-4 text-red-400">
                    Setup failed: {setupMutation.error.message}
                  </div>
                  <button
                    onClick={handleFinish}
                    className="rounded-lg bg-blue-600 px-6 py-2.5 font-medium text-white hover:bg-blue-700 transition-colors"
                  >
                    Retry
                  </button>
                </div>
              )}
              {done && (
                <div>
                  <CheckCircle size={48} className="mx-auto mb-4 text-green-500" />
                  <h2 className="mb-2 text-2xl font-bold text-white">All Set!</h2>
                  <p className="mb-6 text-slate-400">
                    StackArr is ready to go. You can configure more in Settings.
                  </p>
                  <button
                    onClick={() => navigate('/series')}
                    className="rounded-lg bg-blue-600 px-6 py-2.5 font-medium text-white hover:bg-blue-700 transition-colors"
                  >
                    Get Started
                  </button>
                </div>
              )}
              {!setupMutation.isPending && !setupMutation.isError && !done && (
                <div>
                  <h2 className="mb-2 text-2xl font-bold text-white">Ready to Go</h2>
                  <p className="mb-6 text-slate-400">
                    Review your choices and finish setup.
                  </p>
                  <div className="mb-6 space-y-2 text-left text-sm">
                    <ReviewRow label="Media" value={[enableTv && 'TV', enableMovies && 'Movies'].filter(Boolean).join(', ')} />
                    {enableTorrent && <ReviewRow label="RustTorrent" value="Enabled" />}
                    {enableUsenet && <ReviewRow label="RustNZB" value="Enabled" />}
                    {enableIndexarr && <ReviewRow label="Indexarr" value={indexarrUrl} />}
                    {enablePlex && <ReviewRow label="Plex" value="Enabled" />}
                    {enableTv && <ReviewRow label="TV Folder" value={tvLibraryFolder} mono />}
                    {enableMovies && <ReviewRow label="Movie Folder" value={movieLibraryFolder} mono />}
                    {importResult && (
                      <ReviewRow
                        label="Imported"
                        value={`${importResult.imported.series} series, ${importResult.imported.movies} movies, ${importResult.imported.indexers} indexers`}
                      />
                    )}
                  </div>
                  <button
                    onClick={handleFinish}
                    className="rounded-lg bg-green-600 px-6 py-2.5 font-medium text-white hover:bg-green-700 transition-colors"
                  >
                    Finish Setup
                  </button>
                </div>
              )}
            </div>
          )}

          {/* ── Navigation buttons ────────────────────────────────────── */}
          {currentStep !== 'Complete' && (
            <div className="mt-8 flex justify-between">
              <button
                onClick={handleBack}
                disabled={step === 0}
                className="flex items-center gap-1 rounded-lg px-4 py-2 text-sm font-medium text-slate-400 hover:text-white disabled:invisible transition-colors"
              >
                <ChevronLeft size={16} /> Back
              </button>

              <div className="flex gap-2">
                {currentStep === 'Import' && !importResult && !importRunning && (
                  <button
                    onClick={handleNext}
                    className="flex items-center gap-1 rounded-lg px-4 py-2 text-sm font-medium text-slate-400 hover:text-white transition-colors"
                  >
                    <SkipForward size={16} /> Skip
                  </button>
                )}
                <button
                  onClick={handleNext}
                  disabled={!canNext()}
                  className="flex items-center gap-1 rounded-lg bg-blue-600 px-4 py-2 text-sm font-medium text-white hover:bg-blue-700 disabled:opacity-50 transition-colors"
                >
                  Next <ChevronRight size={16} />
                </button>
              </div>
            </div>
          )}
        </div>
      </div>
    </div>
  )
}

// ── Shared Components ─────────────────────────────────────────────────────────

function FeatureToggle({
  icon,
  label,
  desc,
  checked,
  onChange,
}: {
  icon: React.ReactNode
  label: string
  desc: string
  checked: boolean
  onChange: (v: boolean) => void
}) {
  return (
    <label
      className={`flex cursor-pointer items-center gap-4 rounded-lg border p-4 transition-colors ${
        checked ? 'border-blue-500 bg-blue-500/10' : 'border-slate-600 hover:border-slate-500'
      }`}
    >
      <input
        type="checkbox"
        checked={checked}
        onChange={(e) => onChange(e.target.checked)}
        className="h-5 w-5 rounded accent-blue-500"
      />
      {icon}
      <div>
        <div className="font-medium text-white">{label}</div>
        <div className="text-sm text-slate-400">{desc}</div>
      </div>
    </label>
  )
}

function FileInput({
  label,
  desc,
  file,
  onFileChange,
  accept,
}: {
  label: string
  desc: string
  file: File | null
  onFileChange: (f: File | null) => void
  accept: string
}) {
  const inputRef = useRef<HTMLInputElement>(null)

  return (
    <div
      onClick={() => inputRef.current?.click()}
      className="flex cursor-pointer items-center gap-4 rounded-lg border border-dashed border-slate-600 p-4 hover:border-blue-500 transition-colors"
    >
      <FileUp size={20} className={file ? 'text-blue-400' : 'text-slate-500'} />
      <div className="min-w-0 flex-1">
        <div className="text-sm font-medium text-white">{label}</div>
        <div className="text-xs text-slate-400">
          {file ? (
            <span className="text-blue-400">
              {file.name} ({(file.size / 1048576).toFixed(1)} MB)
            </span>
          ) : (
            desc
          )}
        </div>
      </div>
      <input
        ref={inputRef}
        type="file"
        accept={accept}
        className="hidden"
        onChange={(e) => onFileChange(e.target.files?.[0] ?? null)}
      />
      {file && (
        <button
          type="button"
          onClick={(e) => {
            e.stopPropagation()
            onFileChange(null)
            if (inputRef.current) inputRef.current.value = ''
          }}
          className="text-slate-400 hover:text-white"
        >
          <XCircle size={16} />
        </button>
      )}
    </div>
  )
}

function ReviewRow({ label, value, mono }: { label: string; value: string; mono?: boolean }) {
  return (
    <div className="flex justify-between rounded-lg bg-slate-700 px-4 py-2">
      <span className="text-slate-400">{label}</span>
      <span className={`text-white ${mono ? 'font-mono text-xs' : ''}`}>{value}</span>
    </div>
  )
}
