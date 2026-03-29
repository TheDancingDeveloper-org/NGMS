import { useState, useEffect, useMemo, useRef } from 'react'
// react-router-dom not needed — we use window.location for full reload after setup
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
  Cast,
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
  Plus,
  Trash2,
  ChevronUp,
  Folder,
  UserPlus,
} from 'lucide-react'
import { useSetupInit, useSystemStatus } from '../hooks/useApi'
import type { SetupInit, MigrationResult } from '../api/types'

// ── Step definitions ─────────────────────────────────────────────────────────

type StepName = 'Account' | 'Features' | 'Import' | 'Indexarr' | 'Media Libraries' | 'Complete'

export default function FirstBoot() {
  const setupMutation = useSetupInit()
  const { data: status } = useSystemStatus()
  const indexarrAvailable = status?.indexarrAvailable ?? false
  const [step, setStep] = useState(0)

  // Step 0: Admin account creation
  const [serverName, setServerName] = useState('')
  const [adminUsername, setAdminUsername] = useState('')
  const [adminDisplayName, setAdminDisplayName] = useState('')
  const [adminPassword, setAdminPassword] = useState('')
  const [adminConfirmPassword, setAdminConfirmPassword] = useState('')
  const [adminError, setAdminError] = useState<string | null>(null)
  const [adminCreating, setAdminCreating] = useState(false)
  const [adminCreated, setAdminCreated] = useState(false)

  // Step 1: Feature selections
  const [enableTv, setEnableTv] = useState(true)
  const [enableMovies, setEnableMovies] = useState(true)
  const [enableTorrent, setEnableTorrent] = useState(false)
  const [enableUsenet, setEnableUsenet] = useState(false)
  const [enableIndexarr, setEnableIndexarr] = useState(false)
  const [enablePlex, setEnablePlex] = useState(false)
  const [enableStreaming, setEnableStreaming] = useState(false)
  const [enableStremio, setEnableStremio] = useState(false)

  // Auto-default Indexarr toggle when the container is available
  const [indexarrDefaultApplied, setIndexarrDefaultApplied] = useState(false)
  useEffect(() => {
    if (indexarrAvailable && !indexarrDefaultApplied) {
      setEnableIndexarr(true)
      setIndexarrDefaultApplied(true)
    }
  }, [indexarrAvailable, indexarrDefaultApplied])

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

  // Step 3: Media library folders (multiple per type)
  // importedFolders tracks original paths from import for path mapping
  const [importedFolders, setImportedFolders] = useState<{ path: string; mediaType: string }[]>([])
  const [tvFolders, setTvFolders] = useState<string[]>(['/media/tv'])
  const [movieFolders, setMovieFolders] = useState<string[]>(['/media/movies'])
  const [browsingFor, setBrowsingFor] = useState<{ type: 'tv' | 'movie'; index: number } | null>(null)
  const [browserPath, setBrowserPath] = useState('/')
  const [browserDirs, setBrowserDirs] = useState<{ name: string; path: string }[]>([])
  const [browserParent, setBrowserParent] = useState<string | null>(null)
  const [browserLoading, setBrowserLoading] = useState(false)

  // Server recovery (first-boot)
  const [showRecovery, setShowRecovery] = useState(false)
  const [recoverServerName, setRecoverServerName] = useState('')
  const [recoverPhraseInput, setRecoverPhraseInput] = useState('')
  const [recoverBootstrapUrl, setRecoverBootstrapUrl] = useState('https://streambootstrap.indexarr.net')
  const [recoverBootstrapToken, setRecoverBootstrapToken] = useState('')
  const [recoverRunning, setRecoverRunning] = useState(false)
  const [recoverError, setRecoverError] = useState<string | null>(null)
  const [recoverSuccess, setRecoverSuccess] = useState(false)
  const [recoverNewPhrase, setRecoverNewPhrase] = useState<string | null>(null)
  const [recoverNewPhraseCopied, setRecoverNewPhraseCopied] = useState(false)

  // Step 4: Complete
  const [done, setDone] = useState(false)
  const [recoveryPhrase, setRecoveryPhrase] = useState<string | null>(null)
  const [recoveryCopied, setRecoveryCopied] = useState(false)

  // Compute active steps based on feature selections
  const steps = useMemo<StepName[]>(() => {
    const s: StepName[] = adminCreated ? ['Account', 'Features', 'Import'] : ['Account']
    if (adminCreated) {
      if (enableIndexarr) s.push('Indexarr')
      s.push('Media Libraries', 'Complete')
    }
    return s
  }, [enableIndexarr, adminCreated])

  const currentStep = steps[step]

  const canNext = () => {
    if (currentStep === 'Account') return adminCreated
    if (currentStep === 'Features') return enableTv || enableMovies
    return true
  }

  const handleNext = () => {
    if (step < steps.length - 1) setStep((s) => s + 1)
  }

  const handleBack = () => {
    if (step > 0) setStep((s) => s - 1)
  }

  // ── Admin account creation ──────────────────────────────────────────────

  const handleCreateAdmin = async () => {
    setAdminError(null)

    const username = adminUsername.trim()
    if (!username) {
      setAdminError('Username is required')
      return
    }
    if (adminPassword.length < 6) {
      setAdminError('Password must be at least 6 characters')
      return
    }
    if (adminPassword !== adminConfirmPassword) {
      setAdminError('Passwords do not match')
      return
    }

    setAdminCreating(true)
    try {
      const res = await fetch('/api/v1/auth/setup', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        credentials: 'include',
        body: JSON.stringify({
          username,
          password: adminPassword,
          displayName: adminDisplayName.trim() || undefined,
          serverName: serverName.trim() || undefined,
        }),
      })

      if (!res.ok) {
        const body = await res.json().catch(() => ({ error: `Setup failed: ${res.statusText}` }))
        throw new Error(body.error || `Setup failed: ${res.statusText}`)
      }

      setAdminCreated(true)
      // Auto-advance to the next step
      setStep((s) => s + 1)
    } catch (e) {
      setAdminError(e instanceof Error ? e.message : String(e))
    } finally {
      setAdminCreating(false)
    }
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

        // Fetch imported folders and populate the folder lists
        try {
          const foldersRes = await fetch('/api/v1/medialibraryfolder')
          if (foldersRes.ok) {
            const folders = (await foldersRes.json()) as { path: string; mediaType: string }[]
            setImportedFolders(folders)
            const importedTv = folders.filter(f => f.mediaType === 'tv' || f.mediaType === 'series').map(f => f.path)
            const importedMovies = folders.filter(f => f.mediaType === 'movie').map(f => f.path)
            if (importedTv.length > 0) setTvFolders(importedTv)
            if (importedMovies.length > 0) setMovieFolders(importedMovies)
          }
        } catch { /* non-critical */ }
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
    // Build path mappings from imported folders that were changed by the user
    const pathMappings: Array<{ from: string; to: string }> = []
    if (importedFolders.length > 0) {
      // importedFolders mirrors the initial tvFolders/movieFolders order from import
      const importedTv = importedFolders.filter(f => f.mediaType === 'tv' || f.mediaType === 'series')
      const importedMovie = importedFolders.filter(f => f.mediaType === 'movie')
      for (let i = 0; i < importedTv.length && i < tvFolders.length; i++) {
        if (tvFolders[i] && importedTv[i].path !== tvFolders[i]) {
          pathMappings.push({ from: importedTv[i].path, to: tvFolders[i] })
        }
      }
      for (let i = 0; i < importedMovie.length && i < movieFolders.length; i++) {
        if (movieFolders[i] && importedMovie[i].path !== movieFolders[i]) {
          pathMappings.push({ from: importedMovie[i].path, to: movieFolders[i] })
        }
      }
    }

    const payload: SetupInit = {
      modules: {
        tvManagement: enableTv,
        movieManagement: enableMovies,
        torrentEmbedded: enableTorrent,
        usenetEmbedded: enableUsenet,
        indexarrSidecar: enableIndexarr,
        plexIntegration: enablePlex,
        streaming: enableStreaming,
        stremioAddon: enableStremio,
      },
      mediaLibraryFolders: [
        ...(enableTv ? tvFolders.filter(f => f).map(f => ({ path: f, mediaType: 'tv' })) : []),
        ...(enableMovies ? movieFolders.filter(f => f).map(f => ({ path: f, mediaType: 'movie' })) : []),
      ],
      ...(pathMappings.length > 0 ? { pathMappings } : {}),
      ...(enableIndexarr ? { indexarr: { url: indexarrUrl, apiKey: indexarrApiKey } } : {}),
    }

    setupMutation.mutate(payload, {
      onSuccess: (data) => {
        if (data.recoveryPhrase) {
          setRecoveryPhrase(data.recoveryPhrase)
        }
        setDone(true)
      },
    })
  }

  // ── Folder browser ───────────────────────────────────────────────────────

  const browseTo = async (path: string) => {
    setBrowserLoading(true)
    try {
      const res = await fetch(`/api/v1/filesystem/browse?path=${encodeURIComponent(path)}`)
      if (res.ok) {
        const data = await res.json()
        setBrowserPath(data.current)
        setBrowserDirs(data.directories)
        setBrowserParent(data.parent)
      }
    } catch { /* ignore */ }
    setBrowserLoading(false)
  }

  const openBrowser = (type: 'tv' | 'movie', index: number) => {
    const folders = type === 'tv' ? tvFolders : movieFolders
    const startPath = folders[index] || '/'
    setBrowsingFor({ type, index })
    browseTo(startPath)
  }

  const selectBrowserPath = () => {
    if (!browsingFor) return
    const { type, index } = browsingFor
    if (type === 'tv') {
      setTvFolders(prev => { const n = [...prev]; n[index] = browserPath; return n })
    } else {
      setMovieFolders(prev => { const n = [...prev]; n[index] = browserPath; return n })
    }
    setBrowsingFor(null)
  }

  const addFolder = (type: 'tv' | 'movie') => {
    if (type === 'tv') setTvFolders(prev => [...prev, ''])
    else setMovieFolders(prev => [...prev, ''])
  }

  const removeFolder = (type: 'tv' | 'movie', index: number) => {
    if (type === 'tv') setTvFolders(prev => prev.filter((_, i) => i !== index))
    else setMovieFolders(prev => prev.filter((_, i) => i !== index))
  }

  const handleCopyKey = async () => {
    try {
      await navigator.clipboard.writeText(indexarrApiKey)
    } catch {
      // Fallback for non-HTTPS contexts
      const ta = document.createElement('textarea')
      ta.value = indexarrApiKey
      ta.style.position = 'fixed'
      ta.style.opacity = '0'
      document.body.appendChild(ta)
      ta.select()
      document.execCommand('copy')
      document.body.removeChild(ta)
    }
    setKeyCopied(true)
    setTimeout(() => setKeyCopied(false), 2000)
  }

  // ── Render ──────────────────────────────────────────────────────────────

  return (
    <div className="flex min-h-screen items-center justify-center bg-slate-900 p-4">
      <div className="w-full max-w-lg">
        {/* Logo */}
        <div className="mb-6 flex justify-center">
          <img src="/images/NGMS_Banner.png" alt="NGMS" className="h-14" />
        </div>

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
          {/* ── Step: Account ────────────────────────────────────────── */}
          {currentStep === 'Account' && (
            <div>
              <h2 className="mb-2 text-2xl font-bold text-white">Create Admin Account</h2>
              <p className="mb-6 text-slate-400">
                Set up your administrator account to get started with NGMS.
              </p>

              {adminCreated ? (
                <div className="flex items-center gap-3 rounded-lg bg-emerald-900/30 p-4 text-emerald-300">
                  <CheckCircle size={20} />
                  <span>Admin account created successfully.</span>
                </div>
              ) : (
                <div className="space-y-4">
                  <div>
                    <label className="mb-1 block text-sm font-medium text-slate-300">Server Name</label>
                    <input
                      type="text"
                      value={serverName}
                      onChange={(e) => setServerName(e.target.value)}
                      className="w-full rounded-lg bg-slate-700 px-4 py-2.5 text-white placeholder-slate-500 outline-none focus:ring-2 focus:ring-blue-500"
                      placeholder="My NGMS"
                      autoFocus
                    />
                    <p className="mt-1 text-xs text-slate-500">A display name for your server. Can be changed later in Settings.</p>
                  </div>
                  <div>
                    <label className="mb-1 block text-sm font-medium text-slate-300">Username</label>
                    <input
                      type="text"
                      value={adminUsername}
                      onChange={(e) => setAdminUsername(e.target.value)}
                      className="w-full rounded-lg bg-slate-700 px-4 py-2.5 text-white placeholder-slate-500 outline-none focus:ring-2 focus:ring-blue-500"
                      placeholder="admin"
                    />
                  </div>
                  <div>
                    <label className="mb-1 block text-sm font-medium text-slate-300">Display Name</label>
                    <input
                      type="text"
                      value={adminDisplayName}
                      onChange={(e) => setAdminDisplayName(e.target.value)}
                      className="w-full rounded-lg bg-slate-700 px-4 py-2.5 text-white placeholder-slate-500 outline-none focus:ring-2 focus:ring-blue-500"
                      placeholder="Admin"
                    />
                  </div>
                  <div>
                    <label className="mb-1 block text-sm font-medium text-slate-300">Password</label>
                    <input
                      type="password"
                      value={adminPassword}
                      onChange={(e) => setAdminPassword(e.target.value)}
                      className="w-full rounded-lg bg-slate-700 px-4 py-2.5 text-white placeholder-slate-500 outline-none focus:ring-2 focus:ring-blue-500"
                      placeholder="Min 6 characters"
                    />
                  </div>
                  <div>
                    <label className="mb-1 block text-sm font-medium text-slate-300">Confirm Password</label>
                    <input
                      type="password"
                      value={adminConfirmPassword}
                      onChange={(e) => setAdminConfirmPassword(e.target.value)}
                      className="w-full rounded-lg bg-slate-700 px-4 py-2.5 text-white placeholder-slate-500 outline-none focus:ring-2 focus:ring-blue-500"
                      placeholder="Repeat password"
                      onKeyDown={(e) => e.key === 'Enter' && handleCreateAdmin()}
                    />
                  </div>

                  {adminError && (
                    <div className="flex items-center gap-2 rounded-lg bg-red-900/30 p-3 text-sm text-red-300">
                      <XCircle size={16} />
                      {adminError}
                    </div>
                  )}

                  <button
                    onClick={handleCreateAdmin}
                    disabled={adminCreating}
                    className="flex w-full items-center justify-center gap-2 rounded-lg bg-blue-600 px-4 py-2.5 font-medium text-white hover:bg-blue-500 disabled:opacity-50"
                  >
                    {adminCreating ? (
                      <Loader2 size={16} className="animate-spin" />
                    ) : (
                      <UserPlus size={16} />
                    )}
                    {adminCreating ? 'Creating...' : 'Create Admin Account'}
                  </button>

                  {/* Server Recovery Section */}
                  <div className="mt-6 border-t border-slate-700 pt-4">
                    {!showRecovery ? (
                      <button
                        onClick={() => setShowRecovery(true)}
                        className="flex items-center gap-2 text-sm text-slate-500 hover:text-slate-300 transition-colors"
                      >
                        <Key size={14} />
                        Recovering an existing server? Enter your recovery phrase
                      </button>
                    ) : (
                      <div className="space-y-3">
                        <h3 className="text-sm font-semibold text-amber-400 flex items-center gap-2">
                          <Key size={16} />
                          Recover Server Name
                        </h3>
                        <p className="text-xs text-slate-500">
                          If you are rebuilding your server and have a recovery phrase from your previous setup,
                          enter it below to reclaim your server name.
                        </p>

                        {recoverSuccess && recoverNewPhrase && (
                          <div className="rounded-lg border border-green-600 bg-green-950/50 p-4">
                            <div className="flex items-center gap-2 text-sm font-semibold text-green-400 mb-2">
                              <CheckCircle size={16} />
                              Server name recovered!
                            </div>
                            <p className="text-xs text-green-200/70 mb-2">
                              Your new recovery phrase (save it now — it replaces the old one):
                            </p>
                            <code className="block rounded bg-slate-900 px-3 py-2 text-sm text-white font-mono select-all">
                              {recoverNewPhrase}
                            </code>
                            <button
                              onClick={() => {
                                void navigator.clipboard.writeText(recoverNewPhrase)
                                setRecoverNewPhraseCopied(true)
                                setTimeout(() => setRecoverNewPhraseCopied(false), 2000)
                              }}
                              className="mt-2 flex items-center gap-1.5 text-xs text-slate-400 hover:text-white transition-colors"
                            >
                              {recoverNewPhraseCopied ? <Check size={14} className="text-green-400" /> : <Copy size={14} />}
                              {recoverNewPhraseCopied ? 'Copied!' : 'Copy to Clipboard'}
                            </button>
                          </div>
                        )}

                        {!recoverSuccess && (
                          <>
                            <div>
                              <label className="mb-1 block text-xs font-medium text-slate-400">Server Name to Recover</label>
                              <input
                                type="text"
                                value={recoverServerName}
                                onChange={(e) => setRecoverServerName(e.target.value)}
                                className="w-full rounded-lg bg-slate-700 px-3 py-2 text-sm text-white placeholder-slate-500 outline-none focus:ring-2 focus:ring-amber-500"
                                placeholder="MyServer"
                              />
                            </div>
                            <div>
                              <label className="mb-1 block text-xs font-medium text-slate-400">Recovery Phrase (12 words)</label>
                              <input
                                type="text"
                                value={recoverPhraseInput}
                                onChange={(e) => setRecoverPhraseInput(e.target.value)}
                                className="w-full rounded-lg bg-slate-700 px-3 py-2 text-sm text-white placeholder-slate-500 outline-none focus:ring-2 focus:ring-amber-500 font-mono"
                                placeholder="word1 word2 word3 word4 ..."
                              />
                            </div>
                            <div>
                              <label className="mb-1 block text-xs font-medium text-slate-400">Bootstrap URL</label>
                              <input
                                type="text"
                                value={recoverBootstrapUrl}
                                onChange={(e) => setRecoverBootstrapUrl(e.target.value)}
                                className="w-full rounded-lg bg-slate-700 px-3 py-2 text-sm text-white placeholder-slate-500 outline-none focus:ring-2 focus:ring-amber-500"
                                placeholder="https://streambootstrap.indexarr.net"
                              />
                            </div>
                            <div>
                              <label className="mb-1 block text-xs font-medium text-slate-400">Bootstrap Token</label>
                              <input
                                type="password"
                                value={recoverBootstrapToken}
                                onChange={(e) => setRecoverBootstrapToken(e.target.value)}
                                className="w-full rounded-lg bg-slate-700 px-3 py-2 text-sm text-white placeholder-slate-500 outline-none focus:ring-2 focus:ring-amber-500"
                                placeholder="Secret token"
                              />
                            </div>

                            {recoverError && (
                              <div className="flex items-center gap-2 rounded-lg bg-red-900/30 p-3 text-xs text-red-300">
                                <XCircle size={14} />
                                {recoverError}
                              </div>
                            )}

                            <div className="flex gap-2">
                              <button
                                onClick={async () => {
                                  if (!recoverServerName.trim() || !recoverPhraseInput.trim() || !recoverBootstrapUrl.trim() || !recoverBootstrapToken.trim()) {
                                    setRecoverError('All fields are required')
                                    return
                                  }
                                  setRecoverRunning(true)
                                  setRecoverError(null)
                                  try {
                                    const res = await fetch('/api/v1/admin/bootstrap/firstboot-recover', {
                                      method: 'POST',
                                      headers: { 'Content-Type': 'application/json' },
                                      body: JSON.stringify({
                                        serverName: recoverServerName.trim(),
                                        recoveryPhrase: recoverPhraseInput.trim(),
                                        bootstrapUrl: recoverBootstrapUrl.trim(),
                                        bootstrapToken: recoverBootstrapToken.trim(),
                                      }),
                                    })
                                    if (!res.ok) {
                                      const err = await res.json().catch(() => ({ error: 'Recovery failed' }))
                                      throw new Error(err.error || 'Recovery failed')
                                    }
                                    const data = await res.json()
                                    setRecoverSuccess(true)
                                    setRecoverNewPhrase(data.recoveryPhrase ?? null)
                                    // Pre-fill server name for the account creation step
                                    if (recoverServerName.trim()) {
                                      setServerName(recoverServerName.trim())
                                    }
                                  } catch (e) {
                                    setRecoverError(e instanceof Error ? e.message : 'Recovery failed')
                                  } finally {
                                    setRecoverRunning(false)
                                  }
                                }}
                                disabled={recoverRunning}
                                className="flex items-center gap-2 rounded-lg bg-amber-600 px-4 py-2 text-sm font-medium text-white hover:bg-amber-500 disabled:opacity-50"
                              >
                                {recoverRunning ? <Loader2 size={14} className="animate-spin" /> : <Key size={14} />}
                                {recoverRunning ? 'Recovering...' : 'Recover'}
                              </button>
                              <button
                                onClick={() => { setShowRecovery(false); setRecoverError(null) }}
                                className="rounded-lg px-4 py-2 text-sm text-slate-400 hover:text-white transition-colors"
                              >
                                Cancel
                              </button>
                            </div>
                          </>
                        )}
                      </div>
                    )}
                  </div>
                </div>
              )}
            </div>
          )}

          {/* ── Step: Features ───────────────────────────────────────── */}
          {currentStep === 'Features' && (
            <div>
              <h2 className="mb-2 text-2xl font-bold text-white">Welcome to NGMS</h2>
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
                  disabled={!indexarrAvailable}
                  warning={!indexarrAvailable ? 'Indexarr container not deployed. Set STACKARR_INDEXARR_ENABLED=true in your compose environment and start the indexarr service.' : undefined}
                />
                <FeatureToggle
                  icon={<MonitorPlay size={24} className="text-yellow-400" />}
                  label="Plex"
                  desc="Media server integration & watchlist sync"
                  checked={enablePlex}
                  onChange={setEnablePlex}
                />
                <FeatureToggle
                  icon={<MonitorPlay size={24} className="text-green-400" />}
                  label="Streaming"
                  desc="Built-in media streaming server with transcoding"
                  checked={enableStreaming}
                  onChange={setEnableStreaming}
                />
                <FeatureToggle
                  icon={<Cast size={24} className="text-rose-400" />}
                  label="Stremio Addon"
                  desc="Expose your library to Stremio clients"
                  checked={enableStremio}
                  onChange={setEnableStremio}
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
                      <span className="text-white">{importResult.seriesImported}</span>
                    </div>
                    <div className="flex justify-between rounded-lg bg-slate-700/50 px-4 py-2">
                      <span className="text-slate-400">Movies</span>
                      <span className="text-white">{importResult.moviesImported}</span>
                    </div>
                    <div className="flex justify-between rounded-lg bg-slate-700/50 px-4 py-2">
                      <span className="text-slate-400">Indexers</span>
                      <span className="text-white">{importResult.indexersImported}</span>
                    </div>
                    {sabApplied && (
                      <div className="flex justify-between rounded-lg bg-slate-700/50 px-4 py-2">
                        <span className="text-slate-400">SABnzbd config</span>
                        <span className="text-green-400">Applied</span>
                      </div>
                    )}
                  </div>
                  {importResult.warnings.length > 0 && (
                    <div className="mt-3 max-h-24 overflow-y-auto rounded-lg bg-slate-900 p-3">
                      {importResult.warnings.map((err, i) => (
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
                      className={`flex shrink-0 items-center gap-1.5 rounded-lg px-3 py-2.5 text-sm text-white transition-all ${
                        keyCopied
                          ? 'bg-green-600 scale-105'
                          : 'bg-slate-600 hover:bg-slate-500'
                      }`}
                    >
                      {keyCopied ? (
                        <><Check size={14} className="animate-bounce" /> Copied!</>
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
                {importedFolders.length > 0
                  ? 'Your imported library paths are shown below. Update them to match the mount points in your NGMS container.'
                  : 'Set the directories for your media libraries. You can add multiple folders per type.'}
              </p>

              {/* Show imported path mapping hint */}
              {importedFolders.length > 0 && (
                <div className="mb-6 rounded-lg border border-amber-700/50 bg-amber-900/20 p-4">
                  <p className="text-sm text-amber-300">
                    Imported paths will be remapped when you finish setup. Update each folder below to its
                    new container path (e.g. <code className="rounded bg-slate-700 px-1">/TV1/</code> {' \u2192 '}
                    <code className="rounded bg-slate-700 px-1">/media/TV1</code>).
                  </p>
                </div>
              )}

              {/* Folder browser modal */}
              {browsingFor && (
                <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60">
                  <div className="w-full max-w-md rounded-xl bg-slate-800 p-6 shadow-2xl">
                    <h3 className="mb-1 text-lg font-semibold text-white">Browse Folders</h3>
                    <p className="mb-3 text-xs text-slate-400 font-mono">{browserPath}</p>

                    <div className="mb-3 max-h-64 overflow-y-auto rounded-lg border border-slate-600 bg-slate-900">
                      {browserParent && (
                        <button
                          onClick={() => browseTo(browserParent)}
                          className="flex w-full items-center gap-2 border-b border-slate-700 px-3 py-2 text-left text-sm text-slate-300 hover:bg-slate-700"
                        >
                          <ChevronUp size={14} className="text-slate-500" /> ..
                        </button>
                      )}
                      {browserLoading ? (
                        <div className="flex items-center justify-center py-6">
                          <Loader2 size={20} className="animate-spin text-blue-500" />
                        </div>
                      ) : browserDirs.length === 0 ? (
                        <div className="py-4 text-center text-sm text-slate-500">No subdirectories</div>
                      ) : (
                        browserDirs.map((d) => (
                          <button
                            key={d.path}
                            onClick={() => browseTo(d.path)}
                            className="flex w-full items-center gap-2 border-b border-slate-700/50 px-3 py-2 text-left text-sm text-slate-200 hover:bg-slate-700 last:border-0"
                          >
                            <Folder size={14} className="shrink-0 text-blue-400" />
                            <span className="truncate">{d.name}</span>
                          </button>
                        ))
                      )}
                    </div>

                    <div className="flex justify-end gap-2">
                      <button
                        onClick={() => setBrowsingFor(null)}
                        className="rounded-lg px-4 py-2 text-sm text-slate-400 hover:text-white"
                      >
                        Cancel
                      </button>
                      <button
                        onClick={selectBrowserPath}
                        className="rounded-lg bg-blue-600 px-4 py-2 text-sm font-medium text-white hover:bg-blue-700"
                      >
                        Select "{browserPath.split('/').pop() || '/'}"
                      </button>
                    </div>
                  </div>
                </div>
              )}

              <div className="space-y-6">
                {enableTv && (
                  <div>
                    <div className="mb-2 flex items-center justify-between">
                      <label className="flex items-center gap-2 text-sm font-medium text-slate-300">
                        <Tv size={16} className="text-blue-400" />
                        TV Library Folders
                      </label>
                      <button
                        onClick={() => addFolder('tv')}
                        className="flex items-center gap-1 text-xs text-blue-400 hover:text-blue-300"
                      >
                        <Plus size={12} /> Add folder
                      </button>
                    </div>
                    <div className="space-y-2">
                      {tvFolders.map((folder, i) => {
                        const importedTv = importedFolders.filter(f => f.mediaType === 'tv' || f.mediaType === 'series')
                        const orig = importedTv[i]
                        const changed = orig && orig.path !== folder
                        return (
                          <div key={i}>
                            {orig && (
                              <div className="mb-1 flex items-center gap-2 text-xs text-slate-500">
                                <span className="font-mono">{orig.path}</span>
                                <ChevronRight size={12} />
                                {changed
                                  ? <span className="font-mono text-green-400">{folder}</span>
                                  : <span className="text-amber-400">needs remapping</span>}
                              </div>
                            )}
                            <div className="flex gap-2">
                              <input
                                type="text"
                                value={folder}
                                onChange={(e) => setTvFolders(prev => { const n = [...prev]; n[i] = e.target.value; return n })}
                                className={`flex-1 rounded-lg border px-4 py-2 text-sm text-white placeholder-slate-400 focus:outline-none ${
                                  orig && !changed ? 'border-amber-600 bg-slate-700 focus:border-amber-500' : 'border-slate-600 bg-slate-700 focus:border-blue-500'
                                }`}
                                placeholder="/media/tv"
                              />
                              <button
                                onClick={() => openBrowser('tv', i)}
                                className="rounded-lg bg-slate-600 px-3 py-2 text-slate-300 hover:bg-slate-500 hover:text-white"
                                title="Browse"
                              >
                                <FolderOpen size={16} />
                              </button>
                              {tvFolders.length > 1 && (
                                <button
                                  onClick={() => removeFolder('tv', i)}
                                  className="rounded-lg px-2 py-2 text-slate-500 hover:text-red-400"
                                  title="Remove"
                                >
                                  <Trash2 size={16} />
                                </button>
                              )}
                            </div>
                          </div>
                        )
                      })}
                    </div>
                  </div>
                )}
                {enableMovies && (
                  <div>
                    <div className="mb-2 flex items-center justify-between">
                      <label className="flex items-center gap-2 text-sm font-medium text-slate-300">
                        <Film size={16} className="text-purple-400" />
                        Movie Library Folders
                      </label>
                      <button
                        onClick={() => addFolder('movie')}
                        className="flex items-center gap-1 text-xs text-purple-400 hover:text-purple-300"
                      >
                        <Plus size={12} /> Add folder
                      </button>
                    </div>
                    <div className="space-y-2">
                      {movieFolders.map((folder, i) => {
                        const importedMovie = importedFolders.filter(f => f.mediaType === 'movie')
                        const orig = importedMovie[i]
                        const changed = orig && orig.path !== folder
                        return (
                          <div key={i}>
                            {orig && (
                              <div className="mb-1 flex items-center gap-2 text-xs text-slate-500">
                                <span className="font-mono">{orig.path}</span>
                                <ChevronRight size={12} />
                                {changed
                                  ? <span className="font-mono text-green-400">{folder}</span>
                                  : <span className="text-amber-400">needs remapping</span>}
                              </div>
                            )}
                            <div className="flex gap-2">
                              <input
                                type="text"
                                value={folder}
                                onChange={(e) => setMovieFolders(prev => { const n = [...prev]; n[i] = e.target.value; return n })}
                                className={`flex-1 rounded-lg border px-4 py-2 text-sm text-white placeholder-slate-400 focus:outline-none ${
                                  orig && !changed ? 'border-amber-600 bg-slate-700 focus:border-amber-500' : 'border-slate-600 bg-slate-700 focus:border-blue-500'
                                }`}
                                placeholder="/media/movies"
                              />
                              <button
                                onClick={() => openBrowser('movie', i)}
                                className="rounded-lg bg-slate-600 px-3 py-2 text-slate-300 hover:bg-slate-500 hover:text-white"
                                title="Browse"
                              >
                                <FolderOpen size={16} />
                              </button>
                              {movieFolders.length > 1 && (
                                <button
                                  onClick={() => removeFolder('movie', i)}
                                  className="rounded-lg px-2 py-2 text-slate-500 hover:text-red-400"
                                  title="Remove"
                                >
                                  <Trash2 size={16} />
                                </button>
                              )}
                            </div>
                          </div>
                        )
                      })}
                    </div>
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
                  <p className="text-slate-300">Setting up NGMS...</p>
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
                  {recoveryPhrase ? (
                    <>
                      <p className="mb-4 text-slate-400">
                        Your server recovery phrase is shown below. Save it somewhere safe — you will need it to recover your server if you ever need to rebuild.
                      </p>
                      <div className="mx-auto mb-4 max-w-md rounded-lg border border-amber-500/30 bg-amber-500/10 p-4">
                        <div className="mb-2 flex items-center justify-center gap-2 text-sm font-medium text-amber-400">
                          <Key size={16} />
                          Recovery Phrase — save this now!
                        </div>
                        <div className="rounded bg-slate-900/80 p-3 font-mono text-sm text-white select-all">
                          {recoveryPhrase}
                        </div>
                        <button
                          onClick={() => {
                            void navigator.clipboard.writeText(recoveryPhrase)
                            setRecoveryCopied(true)
                            setTimeout(() => setRecoveryCopied(false), 2000)
                          }}
                          className="mt-2 inline-flex items-center gap-1.5 rounded px-3 py-1.5 text-xs font-medium text-slate-300 hover:bg-slate-700 transition-colors"
                        >
                          {recoveryCopied ? <Check size={14} className="text-green-400" /> : <Copy size={14} />}
                          {recoveryCopied ? 'Copied!' : 'Copy to Clipboard'}
                        </button>
                      </div>
                      <p className="mb-6 text-xs text-slate-500">
                        This phrase is shown only once and cannot be retrieved later.
                      </p>
                    </>
                  ) : (
                    <p className="mb-6 text-slate-400">
                      NGMS is ready to go. You can configure more in Settings.
                    </p>
                  )}
                  <button
                    onClick={() => { window.location.href = '/series' }}
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
                    {enableStreaming && <ReviewRow label="Streaming" value="Enabled" />}
                    {enableStremio && <ReviewRow label="Stremio Addon" value="Enabled" />}
                    {enableTv && tvFolders.filter(f => f).map((f, i) => (
                      <ReviewRow key={`tv-${i}`} label={tvFolders.length > 1 ? `TV Folder ${i + 1}` : 'TV Folder'} value={f} mono />
                    ))}
                    {enableMovies && movieFolders.filter(f => f).map((f, i) => (
                      <ReviewRow key={`movie-${i}`} label={movieFolders.length > 1 ? `Movie Folder ${i + 1}` : 'Movie Folder'} value={f} mono />
                    ))}
                    {importResult && (
                      <ReviewRow
                        label="Imported"
                        value={`${importResult.seriesImported} series, ${importResult.moviesImported} movies, ${importResult.indexersImported} indexers`}
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
                disabled={step === 0 || importRunning}
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
                {!(currentStep === 'Import' && importRunning) && (
                  <button
                    onClick={handleNext}
                    disabled={!canNext()}
                    className="flex items-center gap-1 rounded-lg bg-blue-600 px-4 py-2 text-sm font-medium text-white hover:bg-blue-700 disabled:opacity-50 transition-colors"
                  >
                    Next <ChevronRight size={16} />
                  </button>
                )}
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
  disabled,
  warning,
}: {
  icon: React.ReactNode
  label: string
  desc: string
  checked: boolean
  onChange: (v: boolean) => void
  disabled?: boolean
  warning?: string
}) {
  return (
    <div>
      <label
        className={`flex items-center gap-4 rounded-lg border p-4 transition-colors ${
          disabled
            ? 'cursor-not-allowed border-slate-700 opacity-50'
            : checked
              ? 'cursor-pointer border-blue-500 bg-blue-500/10'
              : 'cursor-pointer border-slate-600 hover:border-slate-500'
        }`}
      >
        <input
          type="checkbox"
          checked={checked}
          onChange={(e) => onChange(e.target.checked)}
          disabled={disabled}
          className="h-5 w-5 rounded accent-blue-500"
        />
        {icon}
        <div>
          <div className="font-medium text-white">{label}</div>
          <div className="text-sm text-slate-400">{desc}</div>
        </div>
      </label>
      {warning && (
        <p className="mt-1 ml-1 text-xs text-amber-400">{warning}</p>
      )}
    </div>
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
