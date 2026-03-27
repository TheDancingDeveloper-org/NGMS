import { useState, useEffect, useCallback, useRef } from 'react'
import type {
  QualityProfile,
  QualityProfileItem,
  IndexerConfig,
  AvailableIndexer,
  DownloadClientConfig,
  NamingConfig,
  MediaLibraryFolder,
  Tag,
  EnabledModules,
  MigrationResult,
} from '../api/types'
import { useSystemStatus, useMigrate } from '../hooks/useApi'
import {
  Settings as SettingsIcon,
  Plus,
  Trash2,
  TestTube,
  Save,
  ChevronDown,
  ChevronRight,
  Loader2,
  Check,
  X,
  AlertCircle,
  Search,
  Globe,
  Lock,
  Shield,
  Download,
  Upload,
  Database,
  FileUp,
  Server,
  CheckCircle,
  XCircle,
} from 'lucide-react'

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const API = '/api/v1'

type TabKey =
  | 'general'
  | 'modules'
  | 'quality'
  | 'indexers'
  | 'downloadclients'
  | 'naming'
  | 'medialibraryfolders'
  | 'tags'
  | 'backup'
  | 'migration'

interface TabDef {
  key: TabKey
  label: string
  group: 'Settings' | 'Data'
}

const TABS: TabDef[] = [
  { key: 'general', label: 'General', group: 'Settings' },
  { key: 'modules', label: 'Modules', group: 'Settings' },
  { key: 'quality', label: 'Quality Profiles', group: 'Settings' },
  { key: 'indexers', label: 'Indexers', group: 'Settings' },
  { key: 'downloadclients', label: 'Download Clients', group: 'Settings' },
  { key: 'naming', label: 'Naming', group: 'Settings' },
  { key: 'medialibraryfolders', label: 'Media Folders', group: 'Settings' },
  { key: 'tags', label: 'Tags', group: 'Settings' },
  { key: 'backup', label: 'Backup / Restore', group: 'Data' },
  { key: 'migration', label: 'Migration', group: 'Data' },
]

// ---------------------------------------------------------------------------
// Small helpers
// ---------------------------------------------------------------------------

function formatBytes(bytes: number): string {
  if (bytes === 0) return '0 B'
  const k = 1024
  const sizes = ['B', 'KB', 'MB', 'GB', 'TB']
  const i = Math.floor(Math.log(bytes) / Math.log(k))
  return `${(bytes / k ** i).toFixed(1)} ${sizes[i]}`
}

interface ToastState {
  message: string
  type: 'success' | 'error'
}

function Toast({ toast, onDismiss }: { toast: ToastState; onDismiss: () => void }) {
  useEffect(() => {
    const t = setTimeout(onDismiss, 3000)
    return () => clearTimeout(t)
  }, [onDismiss])

  return (
    <div
      className={`fixed bottom-6 right-6 z-50 flex items-center gap-2 rounded-lg px-4 py-3 shadow-lg text-sm font-medium ${
        toast.type === 'success'
          ? 'bg-green-600 text-white'
          : 'bg-red-600 text-white'
      }`}
    >
      {toast.type === 'success' ? <Check className="h-4 w-4" /> : <AlertCircle className="h-4 w-4" />}
      {toast.message}
      <button onClick={onDismiss} className="ml-2 hover:opacity-80">
        <X className="h-3 w-3" />
      </button>
    </div>
  )
}

// ---------------------------------------------------------------------------
// Buttons
// ---------------------------------------------------------------------------

function Btn({
  children,
  onClick,
  variant = 'primary',
  disabled = false,
  className = '',
}: {
  children: React.ReactNode
  onClick?: () => void
  variant?: 'primary' | 'danger' | 'ghost'
  disabled?: boolean
  className?: string
}) {
  const base = 'inline-flex items-center gap-1.5 rounded-lg px-4 py-2 text-sm font-medium transition-colors disabled:opacity-50 disabled:cursor-not-allowed'
  const variants = {
    primary: 'bg-blue-600 hover:bg-blue-700 text-white',
    danger: 'bg-red-600 hover:bg-red-700 text-white',
    ghost: 'bg-slate-700 hover:bg-slate-600 text-slate-200',
  }
  return (
    <button
      type="button"
      onClick={onClick}
      disabled={disabled}
      className={`${base} ${variants[variant]} ${className}`}
    >
      {children}
    </button>
  )
}

// ---------------------------------------------------------------------------
// Input / Select
// ---------------------------------------------------------------------------

function Input({
  label,
  value,
  onChange,
  placeholder,
  type = 'text',
}: {
  label?: string
  value: string
  onChange: (v: string) => void
  placeholder?: string
  type?: string
}) {
  return (
    <label className="block">
      {label && <span className="mb-1 block text-sm font-medium text-slate-300">{label}</span>}
      <input
        type={type}
        value={value}
        onChange={(e) => onChange(e.target.value)}
        placeholder={placeholder}
        className="w-full rounded-lg border border-slate-600 bg-slate-700 px-3 py-2 text-sm text-white placeholder-slate-400 focus:border-blue-500 focus:outline-none focus:ring-1 focus:ring-blue-500"
      />
    </label>
  )
}

function Select({
  label,
  value,
  onChange,
  options,
}: {
  label?: string
  value: string
  onChange: (v: string) => void
  options: { value: string; label: string }[]
}) {
  return (
    <label className="block">
      {label && <span className="mb-1 block text-sm font-medium text-slate-300">{label}</span>}
      <select
        value={value}
        onChange={(e) => onChange(e.target.value)}
        className="w-full rounded-lg border border-slate-600 bg-slate-700 px-3 py-2 text-sm text-white focus:border-blue-500 focus:outline-none focus:ring-1 focus:ring-blue-500"
      >
        {options.map((o) => (
          <option key={o.value} value={o.value}>
            {o.label}
          </option>
        ))}
      </select>
    </label>
  )
}

function Toggle({
  checked,
  onChange,
  label,
}: {
  checked: boolean
  onChange: (v: boolean) => void
  label?: string
}) {
  return (
    <label className="inline-flex cursor-pointer items-center gap-2">
      <button
        type="button"
        role="switch"
        aria-checked={checked}
        onClick={() => onChange(!checked)}
        className={`relative h-6 w-11 rounded-full transition-colors ${
          checked ? 'bg-blue-600' : 'bg-slate-600'
        }`}
      >
        <span
          className={`absolute top-0.5 left-0.5 h-5 w-5 rounded-full bg-white transition-transform ${
            checked ? 'translate-x-5' : 'translate-x-0'
          }`}
        />
      </button>
      {label && <span className="text-sm text-slate-300">{label}</span>}
    </label>
  )
}

// ---------------------------------------------------------------------------
// Section card wrapper
// ---------------------------------------------------------------------------

function Card({ children, className = '' }: { children: React.ReactNode; className?: string }) {
  return (
    <div className={`rounded-xl border border-slate-700 bg-slate-800 p-6 ${className}`}>
      {children}
    </div>
  )
}

// ---------------------------------------------------------------------------
// General Tab
// ---------------------------------------------------------------------------

function GeneralTab({ showToast }: { showToast: (msg: string, type: 'success' | 'error') => void }) {
  const [instanceName, setInstanceName] = useState('')
  const [authMethod, setAuthMethod] = useState('none')
  const [saving, setSaving] = useState(false)

  useEffect(() => {
    fetch(`${API}/config/general`)
      .then((r) => r.json())
      .then((d: { instanceName?: string; authMethod?: string }) => {
        setInstanceName(d.instanceName ?? '')
        setAuthMethod(d.authMethod ?? 'none')
      })
      .catch(() => {
        /* endpoint may not exist yet */
      })
  }, [])

  const save = async () => {
    setSaving(true)
    try {
      const res = await fetch(`${API}/config/general`, {
        method: 'PUT',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ instanceName, authMethod }),
      })
      if (!res.ok) throw new Error('Save failed')
      showToast('General settings saved', 'success')
    } catch {
      showToast('Failed to save general settings', 'error')
    } finally {
      setSaving(false)
    }
  }

  return (
    <Card>
      <h2 className="mb-6 text-lg font-semibold text-white">General Settings</h2>
      <div className="space-y-4 max-w-lg">
        <Input label="Instance Name" value={instanceName} onChange={setInstanceName} placeholder="StackArr" />
        <Select
          label="Authentication Method"
          value={authMethod}
          onChange={setAuthMethod}
          options={[
            { value: 'none', label: 'None' },
            { value: 'basic', label: 'Basic (Username / Password)' },
            { value: 'forms', label: 'Forms (Login Page)' },
          ]}
        />
        <Btn onClick={save} disabled={saving}>
          {saving ? <Loader2 className="h-4 w-4 animate-spin" /> : <Save className="h-4 w-4" />}
          Save
        </Btn>
      </div>
    </Card>
  )
}

// ---------------------------------------------------------------------------
// Modules Tab
// ---------------------------------------------------------------------------

interface ModuleDef {
  key: keyof EnabledModules
  label: string
  description: string
  category: string
}

const MODULE_DEFS: ModuleDef[] = [
  { key: 'tvManagement', label: 'TV Series Management', description: 'Add, monitor, and manage TV series', category: 'Media' },
  { key: 'movieManagement', label: 'Movie Management', description: 'Add, monitor, and manage movies', category: 'Media' },
  { key: 'torrentEmbedded', label: 'Embedded Torrent Client', description: 'Built-in torrent download engine (librtbit)', category: 'Download Clients' },
  { key: 'usenetEmbedded', label: 'Embedded Usenet Client', description: 'Built-in NZB download engine', category: 'Download Clients' },
  { key: 'torrentExternal', label: 'External Torrent Clients', description: 'Connect to qBittorrent, Transmission, etc.', category: 'Download Clients' },
  { key: 'usenetExternal', label: 'External Usenet Clients', description: 'Connect to SABnzbd, NZBGet, etc.', category: 'Download Clients' },
  { key: 'indexarrSidecar', label: 'Indexarr Sidecar', description: 'Use Indexarr as a local indexer source', category: 'Indexers' },
  { key: 'externalIndexers', label: 'External Indexers', description: 'Newznab, Torznab, and Cardigann indexers', category: 'Indexers' },
  { key: 'plexIntegration', label: 'Plex Integration', description: 'Library scanning, watchlist sync, and metadata', category: 'Integrations' },
  { key: 'notifications', label: 'Notifications', description: 'Discord, webhooks, and other notification targets', category: 'Integrations' },
  { key: 'streaming', label: 'Streaming Server', description: 'Direct play and HLS transcoding with hardware acceleration', category: 'Integrations' },
]

function ModulesTab({ showToast }: { showToast: (msg: string, type: 'success' | 'error') => void }) {
  const { data: status, refetch } = useSystemStatus()
  const [modules, setModules] = useState<EnabledModules | null>(null)
  const [saving, setSaving] = useState(false)
  const [dirty, setDirty] = useState(false)

  useEffect(() => {
    if (status?.modules) setModules({ ...status.modules })
  }, [status])

  const toggle = (key: keyof EnabledModules) => {
    if (!modules) return
    setModules({ ...modules, [key]: !modules[key] })
    setDirty(true)
  }

  const save = async () => {
    if (!modules) return
    setSaving(true)
    try {
      const res = await fetch(`${API}/modules`, {
        method: 'PUT',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(modules),
      })
      if (!res.ok) throw new Error('Save failed')
      showToast('Modules updated', 'success')
      setDirty(false)
      void refetch()
    } catch {
      showToast('Failed to update modules', 'error')
    } finally {
      setSaving(false)
    }
  }

  if (!modules) {
    return (
      <Card>
        <div className="flex items-center gap-2 text-slate-400">
          <Loader2 className="h-5 w-5 animate-spin" /> Loading modules...
        </div>
      </Card>
    )
  }

  const categories = [...new Set(MODULE_DEFS.map((m) => m.category))]

  return (
    <Card>
      <h2 className="mb-2 text-lg font-semibold text-white">Enabled Modules</h2>
      <p className="mb-6 text-sm text-slate-400">
        Enable or disable features. Disabled modules are hidden from the sidebar and their background tasks are skipped.
      </p>

      <div className="space-y-8">
        {categories.map((cat) => (
          <div key={cat}>
            <h3 className="mb-3 text-sm font-semibold text-slate-300 uppercase tracking-wider">{cat}</h3>
            <div className="space-y-3">
              {MODULE_DEFS.filter((m) => m.category === cat).map((mod) => (
                <div
                  key={mod.key}
                  className="flex items-center justify-between rounded-lg border border-slate-700 bg-slate-800/50 px-4 py-3"
                >
                  <div>
                    <div className="text-sm font-medium text-white">{mod.label}</div>
                    <div className="text-xs text-slate-400">{mod.description}</div>
                  </div>
                  <Toggle checked={modules[mod.key]} onChange={() => toggle(mod.key)} />
                </div>
              ))}
            </div>
          </div>
        ))}
      </div>

      <div className="mt-6">
        <Btn onClick={save} disabled={saving || !dirty}>
          {saving ? <Loader2 className="h-4 w-4 animate-spin" /> : <Save className="h-4 w-4" />}
          Save
        </Btn>
      </div>
    </Card>
  )
}

// ---------------------------------------------------------------------------
// Quality Profiles Tab
// ---------------------------------------------------------------------------

function QualityProfilesTab({
  showToast,
}: {
  showToast: (msg: string, type: 'success' | 'error') => void
}) {
  const [profiles, setProfiles] = useState<QualityProfile[]>([])
  const [loading, setLoading] = useState(true)
  const [expandedId, setExpandedId] = useState<number | null>(null)
  const [editingProfile, setEditingProfile] = useState<QualityProfile | null>(null)

  const load = useCallback(async () => {
    setLoading(true)
    try {
      const res = await fetch(`${API}/qualityprofile`)
      const data: QualityProfile[] = await res.json()
      setProfiles(data)
    } catch {
      /* empty */
    } finally {
      setLoading(false)
    }
  }, [])

  useEffect(() => {
    void load()
  }, [load])

  const toggleExpand = (id: number) => {
    if (expandedId === id) {
      setExpandedId(null)
      setEditingProfile(null)
    } else {
      setExpandedId(id)
      const p = profiles.find((x) => x.id === id)
      if (p) setEditingProfile(structuredClone(p))
    }
  }

  const updateItem = (itemIdx: number, field: 'allowed', value: boolean) => {
    if (!editingProfile) return
    const updated = { ...editingProfile, items: editingProfile.items.map((item, i) => (i === itemIdx ? { ...item, [field]: value } : item)) }
    setEditingProfile(updated)
  }

  const saveProfile = async () => {
    if (!editingProfile) return
    try {
      const res = await fetch(`${API}/qualityprofile/${editingProfile.id}`, {
        method: 'PUT',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(editingProfile),
      })
      if (!res.ok) throw new Error('Save failed')
      showToast('Quality profile saved', 'success')
      void load()
      setExpandedId(null)
      setEditingProfile(null)
    } catch {
      showToast('Failed to save quality profile', 'error')
    }
  }

  const deleteProfile = async (id: number) => {
    try {
      const res = await fetch(`${API}/qualityprofile/${id}`, { method: 'DELETE' })
      if (!res.ok) throw new Error('Delete failed')
      showToast('Quality profile deleted', 'success')
      void load()
    } catch {
      showToast('Failed to delete quality profile', 'error')
    }
  }

  if (loading) {
    return (
      <Card>
        <div className="flex items-center gap-2 text-slate-400">
          <Loader2 className="h-5 w-5 animate-spin" /> Loading quality profiles...
        </div>
      </Card>
    )
  }

  return (
    <Card>
      <h2 className="mb-6 text-lg font-semibold text-white">Quality Profiles</h2>

      {profiles.length === 0 ? (
        <p className="text-sm text-slate-400">No quality profiles configured.</p>
      ) : (
        <table className="w-full text-left text-sm">
          <thead>
            <tr className="border-b border-slate-700 text-slate-400">
              <th className="pb-3 pr-4 font-medium" />
              <th className="pb-3 pr-4 font-medium">Name</th>
              <th className="pb-3 pr-4 font-medium">Cutoff</th>
              <th className="pb-3 pr-4 font-medium">Items</th>
              <th className="pb-3 font-medium" />
            </tr>
          </thead>
          <tbody>
            {profiles.map((p) => (
              <>
                <tr
                  key={p.id}
                  className="border-b border-slate-700/50 hover:bg-slate-700/50 cursor-pointer transition-colors"
                  onClick={() => toggleExpand(p.id)}
                >
                  <td className="py-3 pr-2">
                    {expandedId === p.id ? (
                      <ChevronDown className="h-4 w-4 text-slate-400" />
                    ) : (
                      <ChevronRight className="h-4 w-4 text-slate-400" />
                    )}
                  </td>
                  <td className="py-3 pr-4 text-white">{p.name}</td>
                  <td className="py-3 pr-4 text-slate-300">{p.cutoff}</td>
                  <td className="py-3 pr-4 text-slate-300">{p.items.length}</td>
                  <td className="py-3 text-right">
                    <button
                      onClick={(e) => {
                        e.stopPropagation()
                        void deleteProfile(p.id)
                      }}
                      className="text-slate-400 hover:text-red-400 transition-colors"
                    >
                      <Trash2 className="h-4 w-4" />
                    </button>
                  </td>
                </tr>
                {expandedId === p.id && editingProfile && (
                  <tr key={`${p.id}-edit`}>
                    <td colSpan={5} className="bg-slate-800/50 px-6 py-4">
                      <div className="space-y-4">
                        <div className="grid grid-cols-2 gap-4 max-w-md">
                          <Input
                            label="Name"
                            value={editingProfile.name}
                            onChange={(v) => setEditingProfile({ ...editingProfile, name: v })}
                          />
                          <Input
                            label="Cutoff"
                            value={String(editingProfile.cutoff)}
                            onChange={(v) => setEditingProfile({ ...editingProfile, cutoff: Number(v) || 0 })}
                            type="number"
                          />
                        </div>
                        <div>
                          <span className="mb-2 block text-sm font-medium text-slate-300">Qualities</span>
                          <div className="space-y-1">
                            {editingProfile.items.map((item: QualityProfileItem, idx: number) => (
                              <label
                                key={item.id}
                                className="flex items-center gap-2 rounded px-2 py-1 hover:bg-slate-700/50"
                              >
                                <input
                                  type="checkbox"
                                  checked={item.allowed}
                                  onChange={(e) => updateItem(idx, 'allowed', e.target.checked)}
                                  className="rounded border-slate-600 bg-slate-700 text-blue-500 focus:ring-blue-500"
                                />
                                <span className="text-sm text-slate-200">{item.quality.name}</span>
                              </label>
                            ))}
                          </div>
                        </div>
                        <div className="flex gap-2">
                          <Btn onClick={saveProfile}>
                            <Save className="h-4 w-4" /> Save
                          </Btn>
                          <Btn
                            variant="ghost"
                            onClick={() => {
                              setExpandedId(null)
                              setEditingProfile(null)
                            }}
                          >
                            Cancel
                          </Btn>
                        </div>
                      </div>
                    </td>
                  </tr>
                )}
              </>
            ))}
          </tbody>
        </table>
      )}
    </Card>
  )
}

// ---------------------------------------------------------------------------
// Indexers Tab
// ---------------------------------------------------------------------------

interface IndexerFormData {
  name: string
  indexerType: string
  protocol: string
  baseUrl: string
  enabled: boolean
  fields: Record<string, string>
  definitionFile: string
}

const emptyIndexerForm: IndexerFormData = {
  name: '',
  indexerType: 'Newznab',
  protocol: 'Newznab',
  baseUrl: '',
  enabled: true,
  fields: { apiKey: '' },
  definitionFile: '',
}

const privacyIcon = (p: string) => {
  switch (p) {
    case 'public': return <Globe className="h-3.5 w-3.5 text-green-400" />
    case 'semi-private': return <Shield className="h-3.5 w-3.5 text-yellow-400" />
    case 'private': return <Lock className="h-3.5 w-3.5 text-red-400" />
    default: return null
  }
}

function IndexersTab({
  showToast,
}: {
  showToast: (msg: string, type: 'success' | 'error') => void
}) {
  const [indexers, setIndexers] = useState<IndexerConfig[]>([])
  const [loading, setLoading] = useState(true)
  const [showForm, setShowForm] = useState(false)
  const [showCatalog, setShowCatalog] = useState(false)
  const [editId, setEditId] = useState<number | null>(null)
  const [form, setForm] = useState<IndexerFormData>(emptyIndexerForm)
  const [testing, setTesting] = useState<number | null>(null)

  // Catalog state
  const [catalog, setCatalog] = useState<AvailableIndexer[]>([])
  const [catalogLoading, setCatalogLoading] = useState(false)
  const [catalogSearch, setCatalogSearch] = useState('')
  const [catalogFilter, setCatalogFilter] = useState<string>('all')
  const [selectedDef, setSelectedDef] = useState<AvailableIndexer | null>(null)
  const [defSettings, setDefSettings] = useState<Record<string, string>>({})

  const load = useCallback(async () => {
    setLoading(true)
    try {
      const res = await fetch(`${API}/indexer`)
      const data: IndexerConfig[] = await res.json()
      setIndexers(data)
    } catch {
      /* empty */
    } finally {
      setLoading(false)
    }
  }, [])

  useEffect(() => {
    void load()
  }, [load])

  const loadCatalog = async () => {
    setCatalogLoading(true)
    try {
      const res = await fetch(`${API}/indexer/available`)
      const data: AvailableIndexer[] = await res.json()
      setCatalog(data)
    } catch {
      showToast('Failed to load indexer catalog', 'error')
    } finally {
      setCatalogLoading(false)
    }
  }

  const openCatalog = () => {
    setShowForm(false)
    setShowCatalog(true)
    setSelectedDef(null)
    setCatalogSearch('')
    setCatalogFilter('all')
    if (catalog.length === 0) void loadCatalog()
  }

  const openManual = () => {
    setShowCatalog(false)
    setSelectedDef(null)
    setEditId(null)
    setForm(emptyIndexerForm)
    setShowForm(true)
  }

  const selectDefinition = (def: AvailableIndexer) => {
    setSelectedDef(def)
    // Pre-populate defaults
    const defaults: Record<string, string> = {}
    for (const s of def.settings) {
      if (s.default) defaults[s.name] = s.default
    }
    setDefSettings(defaults)
  }

  const addFromCatalog = async () => {
    if (!selectedDef) return
    try {
      const config: Record<string, unknown> = {
        definitionFile: selectedDef.id,
        ...defSettings,
      }
      const body = {
        name: selectedDef.name,
        indexerType: 'Cardigann',
        baseUrl: selectedDef.urls[0] || '',
        protocol: 'torrent',
        enabled: true,
        config,
      }
      const res = await fetch(`${API}/indexer`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(body),
      })
      if (!res.ok) throw new Error('Failed to add')
      showToast(`Added ${selectedDef.name}`, 'success')
      setSelectedDef(null)
      setShowCatalog(false)
      void load()
    } catch {
      showToast('Failed to add indexer', 'error')
    }
  }

  const openEdit = (idx: IndexerConfig) => {
    setShowCatalog(false)
    setSelectedDef(null)
    setEditId(idx.id)
    setForm({
      name: idx.name,
      indexerType: idx.indexerType || idx.protocol,
      protocol: idx.protocol,
      baseUrl: idx.baseUrl,
      enabled: idx.enabled,
      fields: { ...idx.fields },
      definitionFile: '',
    })
    setShowForm(true)
  }

  const saveIndexer = async () => {
    try {
      const method = editId ? 'PUT' : 'POST'
      const url = editId ? `${API}/indexer/${editId}` : `${API}/indexer`
      const body: Record<string, unknown> = {
        name: form.name,
        indexerType: form.indexerType,
        baseUrl: form.baseUrl,
        protocol: form.protocol === 'Newznab' ? 'usenet' : 'torrent',
        enabled: form.enabled,
        apiKey: form.fields.apiKey || null,
      }
      const res = await fetch(url, {
        method,
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(body),
      })
      if (!res.ok) throw new Error('Save failed')
      showToast(editId ? 'Indexer updated' : 'Indexer added', 'success')
      setShowForm(false)
      void load()
    } catch {
      showToast('Failed to save indexer', 'error')
    }
  }

  const deleteIndexer = async (id: number) => {
    try {
      const res = await fetch(`${API}/indexer/${id}`, { method: 'DELETE' })
      if (!res.ok) throw new Error('Delete failed')
      showToast('Indexer deleted', 'success')
      void load()
    } catch {
      showToast('Failed to delete indexer', 'error')
    }
  }

  const testIndexer = async (id: number) => {
    setTesting(id)
    try {
      const res = await fetch(`${API}/indexer/${id}/test`, { method: 'POST' })
      if (!res.ok) throw new Error('Test failed')
      showToast('Indexer test successful', 'success')
    } catch {
      showToast('Indexer test failed', 'error')
    } finally {
      setTesting(null)
    }
  }

  const toggleEnabled = async (idx: IndexerConfig) => {
    try {
      const res = await fetch(`${API}/indexer/${idx.id}`, {
        method: 'PUT',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ enabled: !idx.enabled }),
      })
      if (!res.ok) throw new Error('Toggle failed')
      void load()
    } catch {
      showToast('Failed to toggle indexer', 'error')
    }
  }

  // Filter catalog
  const filteredCatalog = catalog.filter((def) => {
    const matchesSearch = !catalogSearch ||
      def.name.toLowerCase().includes(catalogSearch.toLowerCase()) ||
      (def.description ?? '').toLowerCase().includes(catalogSearch.toLowerCase())
    const matchesFilter = catalogFilter === 'all' || def.privacy === catalogFilter
    return matchesSearch && matchesFilter
  })

  if (loading) {
    return (
      <Card>
        <div className="flex items-center gap-2 text-slate-400">
          <Loader2 className="h-5 w-5 animate-spin" /> Loading indexers...
        </div>
      </Card>
    )
  }

  return (
    <Card>
      <div className="mb-6 flex items-center justify-between">
        <h2 className="text-lg font-semibold text-white">Indexers</h2>
        <div className="flex gap-2">
          <Btn onClick={openCatalog}>
            <Search className="h-4 w-4" /> Browse Indexers
          </Btn>
          <Btn variant="ghost" onClick={openManual}>
            <Plus className="h-4 w-4" /> Manual
          </Btn>
        </div>
      </div>

      {/* Catalog browser */}
      {showCatalog && (
        <div className="mb-6 rounded-lg border border-slate-600 bg-slate-700/50 p-4">
          <div className="mb-4 flex items-center justify-between">
            <h3 className="text-sm font-semibold text-white">
              Available Indexers ({filteredCatalog.length})
            </h3>
            <button onClick={() => setShowCatalog(false)} className="text-slate-400 hover:text-white">
              <X className="h-4 w-4" />
            </button>
          </div>

          {/* Search + filter */}
          <div className="mb-4 flex gap-3">
            <div className="flex-1">
              <input
                type="text"
                placeholder="Search indexers..."
                value={catalogSearch}
                onChange={(e) => setCatalogSearch(e.target.value)}
                className="w-full rounded-md border border-slate-600 bg-slate-800 px-3 py-2 text-sm text-white placeholder-slate-500 focus:border-blue-500 focus:outline-none"
              />
            </div>
            <div className="flex gap-1">
              {(['all', 'public', 'semi-private', 'private'] as const).map((f) => (
                <button
                  key={f}
                  onClick={() => setCatalogFilter(f)}
                  className={`rounded px-3 py-1.5 text-xs font-medium transition-colors ${
                    catalogFilter === f
                      ? 'bg-blue-600 text-white'
                      : 'bg-slate-700 text-slate-300 hover:bg-slate-600'
                  }`}
                >
                  {f === 'all' ? 'All' : f === 'semi-private' ? 'Semi' : f.charAt(0).toUpperCase() + f.slice(1)}
                </button>
              ))}
            </div>
          </div>

          {catalogLoading ? (
            <div className="flex items-center gap-2 py-8 justify-center text-slate-400">
              <Loader2 className="h-5 w-5 animate-spin" /> Loading catalog...
            </div>
          ) : selectedDef ? (
            /* Selected definition — show settings form */
            <div>
              <div className="mb-4 flex items-center gap-3">
                <button onClick={() => setSelectedDef(null)} className="text-slate-400 hover:text-white">
                  <ChevronRight className="h-4 w-4 rotate-180" />
                </button>
                <div>
                  <h4 className="text-white font-medium">{selectedDef.name}</h4>
                  {selectedDef.description && (
                    <p className="text-xs text-slate-400 mt-0.5">{selectedDef.description}</p>
                  )}
                </div>
              </div>

              {selectedDef.settings.length > 0 ? (
                <div className="grid grid-cols-1 gap-3 sm:grid-cols-2 max-w-2xl mb-4">
                  {selectedDef.settings.map((s) => (
                    <div key={s.name}>
                      {s.fieldType === 'select' && s.options ? (
                        <Select
                          label={s.label || s.name}
                          value={defSettings[s.name] ?? s.default ?? ''}
                          onChange={(v) => setDefSettings({ ...defSettings, [s.name]: v })}
                          options={s.options.map((o) => ({ value: o.value, label: o.label }))}
                        />
                      ) : s.fieldType === 'checkbox' ? (
                        <Toggle
                          checked={(defSettings[s.name] ?? s.default) === 'true'}
                          onChange={(v) => setDefSettings({ ...defSettings, [s.name]: String(v) })}
                          label={s.label || s.name}
                        />
                      ) : (
                        <Input
                          label={s.label || s.name}
                          value={defSettings[s.name] ?? s.default ?? ''}
                          onChange={(v) => setDefSettings({ ...defSettings, [s.name]: v })}
                          type={s.fieldType === 'password' ? 'password' : 'text'}
                        />
                      )}
                    </div>
                  ))}
                </div>
              ) : (
                <p className="mb-4 text-sm text-slate-400">No configuration required — public indexer.</p>
              )}

              <div className="flex gap-2">
                <Btn onClick={addFromCatalog}>
                  <Plus className="h-4 w-4" /> Add {selectedDef.name}
                </Btn>
                <Btn variant="ghost" onClick={() => setSelectedDef(null)}>
                  Back
                </Btn>
              </div>
            </div>
          ) : (
            /* Catalog list */
            <div className="max-h-96 overflow-y-auto space-y-1">
              {filteredCatalog.length === 0 ? (
                <p className="py-4 text-center text-sm text-slate-400">No indexers match your search.</p>
              ) : (
                filteredCatalog.map((def) => (
                  <button
                    key={def.id}
                    onClick={() => selectDefinition(def)}
                    className="flex w-full items-center gap-3 rounded-md px-3 py-2 text-left hover:bg-slate-600/50 transition-colors group"
                  >
                    {privacyIcon(def.privacy)}
                    <div className="flex-1 min-w-0">
                      <span className="text-sm text-white group-hover:text-blue-300">{def.name}</span>
                      {def.description && (
                        <p className="text-xs text-slate-500 truncate">{def.description}</p>
                      )}
                    </div>
                    <span className="text-xs text-slate-500">{def.language}</span>
                    <ChevronRight className="h-4 w-4 text-slate-500 group-hover:text-slate-300" />
                  </button>
                ))
              )}
            </div>
          )}
        </div>
      )}

      {/* Manual add/edit form */}
      {showForm && (
        <div className="mb-6 rounded-lg border border-slate-600 bg-slate-700/50 p-4">
          <h3 className="mb-4 text-sm font-semibold text-white">
            {editId ? 'Edit Indexer' : 'Add Indexer (Manual)'}
          </h3>
          <div className="grid grid-cols-1 gap-4 sm:grid-cols-2 max-w-2xl">
            <Input label="Name" value={form.name} onChange={(v) => setForm({ ...form, name: v })} placeholder="My Indexer" />
            <Select
              label="Type"
              value={form.protocol}
              onChange={(v) => setForm({ ...form, protocol: v, indexerType: v })}
              options={[
                { value: 'Newznab', label: 'Newznab' },
                { value: 'Torznab', label: 'Torznab' },
              ]}
            />
            <Input label="URL" value={form.baseUrl} onChange={(v) => setForm({ ...form, baseUrl: v })} placeholder="https://..." />
            <Input
              label="API Key"
              value={form.fields.apiKey ?? ''}
              onChange={(v) => setForm({ ...form, fields: { ...form.fields, apiKey: v } })}
              placeholder="API key"
            />
          </div>
          <div className="mt-4 flex items-center gap-3">
            <Toggle checked={form.enabled} onChange={(v) => setForm({ ...form, enabled: v })} label="Enabled" />
          </div>
          <div className="mt-4 flex gap-2">
            <Btn onClick={saveIndexer}>
              <Save className="h-4 w-4" /> Save
            </Btn>
            <Btn variant="ghost" onClick={() => setShowForm(false)}>
              Cancel
            </Btn>
          </div>
        </div>
      )}

      {/* Configured indexers table */}
      {indexers.length === 0 ? (
        <p className="text-sm text-slate-400">No indexers configured.</p>
      ) : (
        <table className="w-full text-left text-sm">
          <thead>
            <tr className="border-b border-slate-700 text-slate-400">
              <th className="pb-3 pr-4 font-medium">Name</th>
              <th className="pb-3 pr-4 font-medium">Type</th>
              <th className="pb-3 pr-4 font-medium">URL</th>
              <th className="pb-3 pr-4 font-medium">Enabled</th>
              <th className="pb-3 font-medium" />
            </tr>
          </thead>
          <tbody>
            {indexers.map((idx) => (
              <tr key={idx.id} className="border-b border-slate-700/50 hover:bg-slate-700/50 transition-colors">
                <td className="py-3 pr-4 text-white">{idx.name}</td>
                <td className="py-3 pr-4 text-slate-300">{idx.indexerType || idx.protocol}</td>
                <td className="py-3 pr-4 text-slate-300 max-w-[200px] truncate">{idx.baseUrl}</td>
                <td className="py-3 pr-4">
                  <Toggle checked={idx.enabled} onChange={() => void toggleEnabled(idx)} />
                </td>
                <td className="py-3 text-right">
                  <div className="flex items-center justify-end gap-2">
                    <button
                      onClick={() => void testIndexer(idx.id)}
                      disabled={testing === idx.id}
                      className="text-slate-400 hover:text-blue-400 transition-colors disabled:opacity-50"
                      title="Test"
                    >
                      {testing === idx.id ? (
                        <Loader2 className="h-4 w-4 animate-spin" />
                      ) : (
                        <TestTube className="h-4 w-4" />
                      )}
                    </button>
                    <button
                      onClick={() => openEdit(idx)}
                      className="text-slate-400 hover:text-blue-400 transition-colors"
                      title="Edit"
                    >
                      <SettingsIcon className="h-4 w-4" />
                    </button>
                    <button
                      onClick={() => void deleteIndexer(idx.id)}
                      className="text-slate-400 hover:text-red-400 transition-colors"
                      title="Delete"
                    >
                      <Trash2 className="h-4 w-4" />
                    </button>
                  </div>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
    </Card>
  )
}

// ---------------------------------------------------------------------------
// Download Clients Tab
// ---------------------------------------------------------------------------

interface DlClientFormData {
  name: string
  protocol: string
  implementation: string
  host: string
  port: number
  enabled: boolean
  fields: Record<string, string>
}

const emptyDlClientForm: DlClientFormData = {
  name: '',
  protocol: 'torrent',
  implementation: 'qBittorrent',
  host: 'localhost',
  port: 8080,
  enabled: true,
  fields: {},
}

function DownloadClientsTab({
  showToast,
}: {
  showToast: (msg: string, type: 'success' | 'error') => void
}) {
  const [clients, setClients] = useState<DownloadClientConfig[]>([])
  const [loading, setLoading] = useState(true)
  const [showForm, setShowForm] = useState(false)
  const [editId, setEditId] = useState<number | null>(null)
  const [form, setForm] = useState<DlClientFormData>(emptyDlClientForm)
  const [testing, setTesting] = useState<number | null>(null)

  const load = useCallback(async () => {
    setLoading(true)
    try {
      const res = await fetch(`${API}/downloadclient`)
      const data: DownloadClientConfig[] = await res.json()
      setClients(data)
    } catch {
      /* empty */
    } finally {
      setLoading(false)
    }
  }, [])

  useEffect(() => {
    void load()
  }, [load])

  const openAdd = () => {
    setEditId(null)
    setForm(emptyDlClientForm)
    setShowForm(true)
  }

  const openEdit = (c: DownloadClientConfig) => {
    setEditId(c.id)
    setForm({
      name: c.name,
      protocol: c.protocol,
      implementation: c.implementation,
      host: c.host,
      port: c.port,
      enabled: c.enabled,
      fields: { ...c.fields },
    })
    setShowForm(true)
  }

  const saveClient = async () => {
    try {
      const method = editId ? 'PUT' : 'POST'
      const url = editId ? `${API}/downloadclient/${editId}` : `${API}/downloadclient`
      const res = await fetch(url, {
        method,
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(editId ? { id: editId, ...form } : form),
      })
      if (!res.ok) throw new Error('Save failed')
      showToast(editId ? 'Download client updated' : 'Download client added', 'success')
      setShowForm(false)
      void load()
    } catch {
      showToast('Failed to save download client', 'error')
    }
  }

  const deleteClient = async (id: number) => {
    try {
      const res = await fetch(`${API}/downloadclient/${id}`, { method: 'DELETE' })
      if (!res.ok) throw new Error('Delete failed')
      showToast('Download client deleted', 'success')
      void load()
    } catch {
      showToast('Failed to delete download client', 'error')
    }
  }

  const testClient = async (id: number) => {
    setTesting(id)
    try {
      const res = await fetch(`${API}/downloadclient/${id}/test`, { method: 'POST' })
      if (!res.ok) throw new Error('Test failed')
      showToast('Download client test successful', 'success')
    } catch {
      showToast('Download client test failed', 'error')
    } finally {
      setTesting(null)
    }
  }

  const toggleEnabled = async (c: DownloadClientConfig) => {
    try {
      const res = await fetch(`${API}/downloadclient/${c.id}`, {
        method: 'PUT',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ ...c, enabled: !c.enabled }),
      })
      if (!res.ok) throw new Error('Toggle failed')
      void load()
    } catch {
      showToast('Failed to toggle download client', 'error')
    }
  }

  if (loading) {
    return (
      <Card>
        <div className="flex items-center gap-2 text-slate-400">
          <Loader2 className="h-5 w-5 animate-spin" /> Loading download clients...
        </div>
      </Card>
    )
  }

  return (
    <Card>
      <div className="mb-6 flex items-center justify-between">
        <h2 className="text-lg font-semibold text-white">Download Clients</h2>
        <Btn onClick={openAdd}>
          <Plus className="h-4 w-4" /> Add Client
        </Btn>
      </div>

      {showForm && (
        <div className="mb-6 rounded-lg border border-slate-600 bg-slate-700/50 p-4">
          <h3 className="mb-4 text-sm font-semibold text-white">
            {editId ? 'Edit Download Client' : 'Add Download Client'}
          </h3>
          <div className="grid grid-cols-1 gap-4 sm:grid-cols-2 max-w-2xl">
            <Input label="Name" value={form.name} onChange={(v) => setForm({ ...form, name: v })} placeholder="My Client" />
            <Select
              label="Type"
              value={form.implementation}
              onChange={(v) => {
                const protocol = ['qBittorrent', 'Transmission'].includes(v) ? 'torrent' : 'usenet'
                setForm({ ...form, implementation: v, protocol })
              }}
              options={[
                { value: 'qBittorrent', label: 'qBittorrent' },
                { value: 'Transmission', label: 'Transmission' },
                { value: 'SABnzbd', label: 'SABnzbd' },
                { value: 'NZBGet', label: 'NZBGet' },
              ]}
            />
            <Input label="Host" value={form.host} onChange={(v) => setForm({ ...form, host: v })} placeholder="localhost" />
            <Input
              label="Port"
              value={String(form.port)}
              onChange={(v) => setForm({ ...form, port: Number(v) || 0 })}
              type="number"
            />
          </div>
          <div className="mt-4 flex items-center gap-3">
            <Toggle checked={form.enabled} onChange={(v) => setForm({ ...form, enabled: v })} label="Enabled" />
          </div>
          <div className="mt-4 flex gap-2">
            <Btn onClick={saveClient}>
              <Save className="h-4 w-4" /> Save
            </Btn>
            <Btn variant="ghost" onClick={() => setShowForm(false)}>
              Cancel
            </Btn>
          </div>
        </div>
      )}

      {clients.length === 0 ? (
        <p className="text-sm text-slate-400">No download clients configured.</p>
      ) : (
        <table className="w-full text-left text-sm">
          <thead>
            <tr className="border-b border-slate-700 text-slate-400">
              <th className="pb-3 pr-4 font-medium">Name</th>
              <th className="pb-3 pr-4 font-medium">Type</th>
              <th className="pb-3 pr-4 font-medium">Host</th>
              <th className="pb-3 pr-4 font-medium">Enabled</th>
              <th className="pb-3 font-medium" />
            </tr>
          </thead>
          <tbody>
            {clients.map((c) => (
              <tr key={c.id} className="border-b border-slate-700/50 hover:bg-slate-700/50 transition-colors">
                <td className="py-3 pr-4 text-white">{c.name}</td>
                <td className="py-3 pr-4 text-slate-300">{c.implementation}</td>
                <td className="py-3 pr-4 text-slate-300">
                  {c.host}:{c.port}
                </td>
                <td className="py-3 pr-4">
                  <Toggle checked={c.enabled} onChange={() => void toggleEnabled(c)} />
                </td>
                <td className="py-3 text-right">
                  <div className="flex items-center justify-end gap-2">
                    <button
                      onClick={() => void testClient(c.id)}
                      disabled={testing === c.id}
                      className="text-slate-400 hover:text-blue-400 transition-colors disabled:opacity-50"
                      title="Test"
                    >
                      {testing === c.id ? (
                        <Loader2 className="h-4 w-4 animate-spin" />
                      ) : (
                        <TestTube className="h-4 w-4" />
                      )}
                    </button>
                    <button
                      onClick={() => openEdit(c)}
                      className="text-slate-400 hover:text-blue-400 transition-colors"
                      title="Edit"
                    >
                      <SettingsIcon className="h-4 w-4" />
                    </button>
                    <button
                      onClick={() => void deleteClient(c.id)}
                      className="text-slate-400 hover:text-red-400 transition-colors"
                      title="Delete"
                    >
                      <Trash2 className="h-4 w-4" />
                    </button>
                  </div>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
    </Card>
  )
}

// ---------------------------------------------------------------------------
// Naming Tab
// ---------------------------------------------------------------------------

function NamingTab({ showToast }: { showToast: (msg: string, type: 'success' | 'error') => void }) {
  const [config, setConfig] = useState<NamingConfig>({
    id: 0,
    renameEpisodes: true,
    replaceIllegalCharacters: true,
    standardEpisodeFormat: '{Series Title} - S{season:00}E{episode:00} - {Episode Title}',
    dailyEpisodeFormat: '{Series Title} - {Air-Date} - {Episode Title}',
    animeEpisodeFormat: '{Series Title} - S{season:00}E{episode:00} - {Episode Title}',
    seriesFolderFormat: '{Series Title}',
    seasonFolderFormat: 'Season {season:00}',
    movieFolderFormat: '{Movie Title} ({Release Year})',
    movieFileFormat: '{Movie Title} ({Release Year}) - {Quality Full}',
  })
  const [saving, setSaving] = useState(false)

  useEffect(() => {
    fetch(`${API}/config/naming`)
      .then((r) => r.json())
      .then((d: NamingConfig) => setConfig(d))
      .catch(() => {
        /* use defaults */
      })
  }, [])

  const save = async () => {
    setSaving(true)
    try {
      const res = await fetch(`${API}/config/naming`, {
        method: 'PUT',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(config),
      })
      if (!res.ok) throw new Error('Save failed')
      showToast('Naming config saved', 'success')
    } catch {
      showToast('Failed to save naming config', 'error')
    } finally {
      setSaving(false)
    }
  }

  const update = (field: keyof NamingConfig, value: string) => {
    setConfig((prev) => ({ ...prev, [field]: value }))
  }

  return (
    <Card>
      <h2 className="mb-6 text-lg font-semibold text-white">Naming</h2>
      <div className="space-y-6 max-w-2xl">
        <div>
          <h3 className="mb-3 text-sm font-semibold text-slate-300 uppercase tracking-wider">
            Episode Formats
          </h3>
          <div className="space-y-3">
            <Input
              label="Standard Episode Format"
              value={config.standardEpisodeFormat}
              onChange={(v) => update('standardEpisodeFormat', v)}
            />
            <Input
              label="Daily Episode Format"
              value={config.dailyEpisodeFormat}
              onChange={(v) => update('dailyEpisodeFormat', v)}
            />
            <Input
              label="Anime Episode Format"
              value={config.animeEpisodeFormat}
              onChange={(v) => update('animeEpisodeFormat', v)}
            />
          </div>
        </div>

        <div>
          <h3 className="mb-3 text-sm font-semibold text-slate-300 uppercase tracking-wider">
            Folder Formats
          </h3>
          <div className="space-y-3">
            <Input
              label="Series Folder Format"
              value={config.seriesFolderFormat}
              onChange={(v) => update('seriesFolderFormat', v)}
            />
            <Input
              label="Season Folder Format"
              value={config.seasonFolderFormat}
              onChange={(v) => update('seasonFolderFormat', v)}
            />
          </div>
        </div>

        <div>
          <h3 className="mb-3 text-sm font-semibold text-slate-300 uppercase tracking-wider">
            Movie Formats
          </h3>
          <div className="space-y-3">
            <Input
              label="Movie File Format"
              value={config.movieFileFormat}
              onChange={(v) => update('movieFileFormat', v)}
            />
            <Input
              label="Movie Folder Format"
              value={config.movieFolderFormat}
              onChange={(v) => update('movieFolderFormat', v)}
            />
          </div>
        </div>

        <Btn onClick={save} disabled={saving}>
          {saving ? <Loader2 className="h-4 w-4 animate-spin" /> : <Save className="h-4 w-4" />}
          Save
        </Btn>
      </div>
    </Card>
  )
}

// ---------------------------------------------------------------------------
// Media Library Folders Tab
// ---------------------------------------------------------------------------

function MediaLibraryFoldersTab({
  showToast,
}: {
  showToast: (msg: string, type: 'success' | 'error') => void
}) {
  const [folders, setFolders] = useState<MediaLibraryFolder[]>([])
  const [loading, setLoading] = useState(true)
  const [showForm, setShowForm] = useState(false)
  const [newPath, setNewPath] = useState('')
  const [newMediaType, setNewMediaType] = useState<'tv' | 'movie'>('tv')

  const load = useCallback(async () => {
    setLoading(true)
    try {
      const res = await fetch(`${API}/medialibraryfolder`)
      const data: MediaLibraryFolder[] = await res.json()
      setFolders(data)
    } catch {
      /* empty */
    } finally {
      setLoading(false)
    }
  }, [])

  useEffect(() => {
    void load()
  }, [load])

  const addFolder = async () => {
    if (!newPath.trim()) return
    try {
      const res = await fetch(`${API}/medialibraryfolder`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ path: newPath, mediaType: newMediaType }),
      })
      if (!res.ok) throw new Error('Add failed')
      showToast('Media library folder added', 'success')
      setShowForm(false)
      setNewPath('')
      void load()
    } catch {
      showToast('Failed to add media library folder', 'error')
    }
  }

  const deleteFolder = async (id: number) => {
    try {
      const res = await fetch(`${API}/medialibraryfolder/${id}`, { method: 'DELETE' })
      if (!res.ok) throw new Error('Delete failed')
      showToast('Media library folder removed', 'success')
      void load()
    } catch {
      showToast('Failed to remove media library folder', 'error')
    }
  }

  if (loading) {
    return (
      <Card>
        <div className="flex items-center gap-2 text-slate-400">
          <Loader2 className="h-5 w-5 animate-spin" /> Loading media library folders...
        </div>
      </Card>
    )
  }

  return (
    <Card>
      <div className="mb-6 flex items-center justify-between">
        <h2 className="text-lg font-semibold text-white">Media Library Folders</h2>
        <Btn onClick={() => setShowForm(true)}>
          <Plus className="h-4 w-4" /> Add Folder
        </Btn>
      </div>

      {showForm && (
        <div className="mb-6 rounded-lg border border-slate-600 bg-slate-700/50 p-4">
          <h3 className="mb-4 text-sm font-semibold text-white">Add Media Library Folder</h3>
          <div className="grid grid-cols-1 gap-4 sm:grid-cols-2 max-w-xl">
            <Input label="Path" value={newPath} onChange={setNewPath} placeholder="/media/tv" />
            <Select
              label="Media Type"
              value={newMediaType}
              onChange={(v) => setNewMediaType(v as 'tv' | 'movie')}
              options={[
                { value: 'tv', label: 'TV Series' },
                { value: 'movie', label: 'Movies' },
              ]}
            />
          </div>
          <div className="mt-4 flex gap-2">
            <Btn onClick={addFolder}>
              <Plus className="h-4 w-4" /> Add
            </Btn>
            <Btn variant="ghost" onClick={() => setShowForm(false)}>
              Cancel
            </Btn>
          </div>
        </div>
      )}

      {folders.length === 0 ? (
        <p className="text-sm text-slate-400">No media library folders configured.</p>
      ) : (
        <table className="w-full text-left text-sm">
          <thead>
            <tr className="border-b border-slate-700 text-slate-400">
              <th className="pb-3 pr-4 font-medium">Path</th>
              <th className="pb-3 pr-4 font-medium">Media Type</th>
              <th className="pb-3 pr-4 font-medium">Free Space</th>
              <th className="pb-3 font-medium" />
            </tr>
          </thead>
          <tbody>
            {folders.map((f) => (
              <tr key={f.id} className="border-b border-slate-700/50 hover:bg-slate-700/50 transition-colors">
                <td className="py-3 pr-4 font-mono text-sm text-white">{f.path}</td>
                <td className="py-3 pr-4 text-slate-300 capitalize">{f.mediaType === 'tv' ? 'TV Series' : 'Movies'}</td>
                <td className="py-3 pr-4 text-slate-300">
                  {formatBytes(f.freeSpace)} / {formatBytes(f.totalSpace)}
                </td>
                <td className="py-3 text-right">
                  <button
                    onClick={() => void deleteFolder(f.id)}
                    className="text-slate-400 hover:text-red-400 transition-colors"
                    title="Delete"
                  >
                    <Trash2 className="h-4 w-4" />
                  </button>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
    </Card>
  )
}

// ---------------------------------------------------------------------------
// Tags Tab
// ---------------------------------------------------------------------------

function TagsTab({ showToast }: { showToast: (msg: string, type: 'success' | 'error') => void }) {
  const [tags, setTags] = useState<Tag[]>([])
  const [loading, setLoading] = useState(true)
  const [newLabel, setNewLabel] = useState('')

  const load = useCallback(async () => {
    setLoading(true)
    try {
      const res = await fetch(`${API}/tag`)
      const data: Tag[] = await res.json()
      setTags(data)
    } catch {
      /* empty */
    } finally {
      setLoading(false)
    }
  }, [])

  useEffect(() => {
    void load()
  }, [load])

  const addTag = async () => {
    if (!newLabel.trim()) return
    try {
      const res = await fetch(`${API}/tag`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ label: newLabel.trim() }),
      })
      if (!res.ok) throw new Error('Add failed')
      showToast('Tag added', 'success')
      setNewLabel('')
      void load()
    } catch {
      showToast('Failed to add tag', 'error')
    }
  }

  const deleteTag = async (id: number) => {
    try {
      const res = await fetch(`${API}/tag/${id}`, { method: 'DELETE' })
      if (!res.ok) throw new Error('Delete failed')
      showToast('Tag removed', 'success')
      void load()
    } catch {
      showToast('Failed to remove tag', 'error')
    }
  }

  if (loading) {
    return (
      <Card>
        <div className="flex items-center gap-2 text-slate-400">
          <Loader2 className="h-5 w-5 animate-spin" /> Loading tags...
        </div>
      </Card>
    )
  }

  return (
    <Card>
      <h2 className="mb-6 text-lg font-semibold text-white">Tags</h2>

      <div className="mb-6 flex items-end gap-3 max-w-md">
        <div className="flex-1">
          <Input label="New Tag" value={newLabel} onChange={setNewLabel} placeholder="Tag name" />
        </div>
        <Btn onClick={addTag} disabled={!newLabel.trim()}>
          <Plus className="h-4 w-4" /> Add
        </Btn>
      </div>

      {tags.length === 0 ? (
        <p className="text-sm text-slate-400">No tags configured.</p>
      ) : (
        <div className="flex flex-wrap gap-2">
          {tags.map((t) => (
            <span
              key={t.id}
              className="inline-flex items-center gap-1.5 rounded-full bg-slate-700 px-3 py-1.5 text-sm text-slate-200"
            >
              {t.label}
              <button
                onClick={() => void deleteTag(t.id)}
                className="text-slate-400 hover:text-red-400 transition-colors"
              >
                <X className="h-3 w-3" />
              </button>
            </span>
          ))}
        </div>
      )}
    </Card>
  )
}

// ---------------------------------------------------------------------------
// Settings Page (main export)
// ---------------------------------------------------------------------------

export default function Settings() {
  const [activeTab, setActiveTab] = useState<TabKey>('general')
  const [toast, setToast] = useState<ToastState | null>(null)

  const showToast = useCallback((message: string, type: 'success' | 'error') => {
    setToast({ message, type })
  }, [])

  const dismissToast = useCallback(() => setToast(null), [])

  const groups = [...new Set(TABS.map((t) => t.group))]

  return (
    <div className="flex gap-6">
      {/* Left sidebar nav */}
      <nav className="w-48 shrink-0">
        <div className="sticky top-4 space-y-4">
          {groups.map((group) => (
            <div key={group}>
              <h3 className="mb-1 px-3 text-[10px] font-semibold uppercase tracking-wider text-slate-500">
                {group}
              </h3>
              <div className="space-y-0.5">
                {TABS.filter((t) => t.group === group).map((tab) => (
                  <button
                    key={tab.key}
                    onClick={() => setActiveTab(tab.key)}
                    className={`block w-full rounded-md px-3 py-1.5 text-left text-sm transition-colors ${
                      activeTab === tab.key
                        ? 'bg-blue-600 text-white font-medium'
                        : 'text-slate-400 hover:bg-slate-800 hover:text-white'
                    }`}
                  >
                    {tab.label}
                  </button>
                ))}
              </div>
            </div>
          ))}
        </div>
      </nav>

      {/* Content area */}
      <div className="flex-1 min-w-0">
        {activeTab === 'general' && <GeneralTab showToast={showToast} />}
        {activeTab === 'modules' && <ModulesTab showToast={showToast} />}
        {activeTab === 'quality' && <QualityProfilesTab showToast={showToast} />}
        {activeTab === 'indexers' && <IndexersTab showToast={showToast} />}
        {activeTab === 'downloadclients' && <DownloadClientsTab showToast={showToast} />}
        {activeTab === 'naming' && <NamingTab showToast={showToast} />}
        {activeTab === 'medialibraryfolders' && <MediaLibraryFoldersTab showToast={showToast} />}
        {activeTab === 'tags' && <TagsTab showToast={showToast} />}
        {activeTab === 'backup' && <BackupRestoreTab showToast={showToast} />}
        {activeTab === 'migration' && <MigrationTab showToast={showToast} />}
      </div>

      {/* Toast Notification */}
      {toast && <Toast toast={toast} onDismiss={dismissToast} />}
    </div>
  )
}

// ---------------------------------------------------------------------------
// Backup / Restore Tab
// ---------------------------------------------------------------------------

function BackupRestoreTab({ showToast }: { showToast: (msg: string, type: 'success' | 'error') => void }) {
  const [exporting, setExporting] = useState(false)
  const [importing, setImporting] = useState(false)
  const [restoreFile, setRestoreFile] = useState<File | null>(null)
  const [restoreResult, setRestoreResult] = useState<{ success: boolean; restored: string[]; errors: string[] } | null>(null)
  const inputRef = useRef<HTMLInputElement>(null)

  const handleExport = async () => {
    setExporting(true)
    try {
      const res = await fetch(`${API}/system/backup`)
      if (!res.ok) throw new Error('Export failed')
      const data = await res.json()
      const blob = new Blob([JSON.stringify(data, null, 2)], { type: 'application/json' })
      const url = URL.createObjectURL(blob)
      const a = document.createElement('a')
      a.href = url
      a.download = `stackarr-backup-${new Date().toISOString().split('T')[0]}.json`
      a.click()
      URL.revokeObjectURL(url)
      showToast('Backup exported', 'success')
    } catch {
      showToast('Failed to export backup', 'error')
    } finally {
      setExporting(false)
    }
  }

  const handleImport = async () => {
    if (!restoreFile) return
    setImporting(true)
    setRestoreResult(null)
    try {
      const text = await restoreFile.text()
      const body = JSON.parse(text)
      const res = await fetch(`${API}/system/restore`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(body),
      })
      if (!res.ok && res.status !== 207) throw new Error('Restore failed')
      const result = await res.json()
      setRestoreResult(result)
      showToast(result.success ? 'Restore completed' : 'Restore completed with errors', result.success ? 'success' : 'error')
    } catch {
      showToast('Failed to restore backup', 'error')
    } finally {
      setImporting(false)
    }
  }

  return (
    <div className="space-y-6">
      {/* Export */}
      <Card>
        <h2 className="mb-2 text-lg font-semibold text-white">Export Backup</h2>
        <p className="mb-4 text-sm text-slate-400">
          Download a JSON backup of your configuration (quality profiles, tags, folders, modules, indexers, etc.)
        </p>
        <Btn onClick={handleExport} disabled={exporting}>
          {exporting ? <Loader2 className="h-4 w-4 animate-spin" /> : <Download className="h-4 w-4" />}
          Export Backup
        </Btn>
      </Card>

      {/* Import / Restore */}
      <Card>
        <h2 className="mb-2 text-lg font-semibold text-white">Restore from Backup</h2>
        <p className="mb-4 text-sm text-slate-400">
          Upload a previously exported JSON backup to restore configuration.
          Only config tables are restored (quality profiles, tags, folders, modules). Media must be re-imported separately.
        </p>

        <div
          onClick={() => inputRef.current?.click()}
          className="mb-4 flex cursor-pointer items-center gap-4 rounded-lg border border-dashed border-slate-600 p-4 hover:border-blue-500 transition-colors"
        >
          <Upload className={`h-6 w-6 ${restoreFile ? 'text-blue-400' : 'text-slate-500'}`} />
          <div className="flex-1 min-w-0">
            <div className="font-medium text-white">Backup JSON file</div>
            <div className="text-xs text-slate-400">
              {restoreFile ? (
                <span className="text-blue-400">{restoreFile.name}</span>
              ) : 'Click to select a backup file'}
            </div>
          </div>
          <input
            ref={inputRef}
            type="file"
            accept=".json"
            className="hidden"
            onChange={(e) => { setRestoreFile(e.target.files?.[0] ?? null); setRestoreResult(null) }}
          />
          {restoreFile && (
            <button
              onClick={(e) => { e.stopPropagation(); setRestoreFile(null); setRestoreResult(null); if (inputRef.current) inputRef.current.value = '' }}
              className="text-slate-400 hover:text-white"
            >
              <X className="h-4 w-4" />
            </button>
          )}
        </div>

        <Btn onClick={handleImport} disabled={!restoreFile || importing}>
          {importing ? <Loader2 className="h-4 w-4 animate-spin" /> : <Upload className="h-4 w-4" />}
          Restore
        </Btn>

        {restoreResult && (
          <div className={`mt-4 rounded-lg border p-3 text-sm ${restoreResult.success ? 'border-green-500/30 bg-green-500/10 text-green-400' : 'border-yellow-500/30 bg-yellow-500/10 text-yellow-400'}`}>
            <p className="font-medium mb-1">{restoreResult.success ? 'Restore Complete' : 'Restore completed with errors'}</p>
            {restoreResult.restored.map((r, i) => <div key={i} className="text-xs">{r}</div>)}
            {restoreResult.errors.map((e, i) => <div key={i} className="text-xs text-red-400">{e}</div>)}
          </div>
        )}
      </Card>
    </div>
  )
}

// ---------------------------------------------------------------------------
// Migration Tab (moved from standalone page)
// ---------------------------------------------------------------------------

function MigrationTab({ showToast }: { showToast: (msg: string, type: 'success' | 'error') => void }) {
  return (
    <div className="space-y-6">
      <ArrMigrationSection />
      <SabnzbdImportSection />
      {/* showToast kept for future use */}
      <span className="hidden">{typeof showToast}</span>
    </div>
  )
}

function ArrMigrationSection() {
  const mutation = useMigrate()
  const [sonarrFile, setSonarrFile] = useState<File | null>(null)
  const [radarrFile, setRadarrFile] = useState<File | null>(null)
  const [prowlarrFile, setProwlarrFile] = useState<File | null>(null)
  const formRef = useRef<HTMLFormElement>(null)

  const hasAnyFile = sonarrFile || radarrFile || prowlarrFile

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault()
    if (!hasAnyFile) return
    const formData = new FormData()
    if (sonarrFile) formData.append('sonarr', sonarrFile)
    if (radarrFile) formData.append('radarr', radarrFile)
    if (prowlarrFile) formData.append('prowlarr', prowlarrFile)
    mutation.mutate(formData)
  }

  const handleReset = () => {
    setSonarrFile(null)
    setRadarrFile(null)
    setProwlarrFile(null)
    mutation.reset()
    formRef.current?.reset()
  }

  return (
    <Card>
      <div className="mb-4 flex items-center gap-3">
        <Database className="h-5 w-5 text-blue-400" />
        <div>
          <h3 className="text-base font-semibold">Import from Sonarr / Radarr / Prowlarr</h3>
          <p className="text-xs text-slate-400">Upload database files to migrate existing library data.</p>
        </div>
      </div>

      {!mutation.isSuccess && (
        <form ref={formRef} onSubmit={handleSubmit}>
          <div className="space-y-3">
            <MigrateFileInput label="sonarr.db" description="Sonarr database file" file={sonarrFile} onFileChange={setSonarrFile} accept=".db" />
            <MigrateFileInput label="radarr.db" description="Radarr database file" file={radarrFile} onFileChange={setRadarrFile} accept=".db" />
            <MigrateFileInput label="prowlarr.db" description="Prowlarr database file" file={prowlarrFile} onFileChange={setProwlarrFile} accept=".db" />
          </div>

          {mutation.isError && (
            <div className="mt-3 rounded-lg border border-red-500/30 bg-red-500/10 p-3 text-sm text-red-400">
              Migration failed: {mutation.error.message}
            </div>
          )}

          <div className="mt-4 flex gap-3">
            <Btn onClick={() => {}} disabled={!hasAnyFile || mutation.isPending}>
              {mutation.isPending ? <><Loader2 className="h-4 w-4 animate-spin" /> Migrating...</> : <><Upload className="h-4 w-4" /> Start Migration</>}
            </Btn>
            {hasAnyFile && !mutation.isPending && (
              <Btn variant="ghost" onClick={handleReset}>Clear</Btn>
            )}
          </div>
        </form>
      )}

      {mutation.isSuccess && mutation.data && (
        <MigrationReport result={mutation.data} onReset={handleReset} />
      )}
    </Card>
  )
}

interface SabnzbdPreview {
  servers: Array<{
    name: string; host: string; port: number; ssl: boolean
    username: string; connections: number; priority: number
    enabled: boolean; password_masked: boolean
  }>
  categories: Array<{ name: string; output_dir: string | null; post_processing: number }>
  general: { api_key: string | null; complete_dir: string | null; incomplete_dir: string | null; speed_limit_bps: number }
  rss_feeds: Array<{ name: string; url: string; enabled: boolean }>
  warnings: string[]
  skipped_fields: string[]
}

interface SabnzbdApplyResult {
  success: boolean; serversAdded: number; categoriesAdded: number
  rssFeedsAdded: number; warnings: string[]; skippedFields: string[]
}

function SabnzbdImportSection() {
  const [mode, setMode] = useState<'ini' | 'api'>('ini')
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [preview, setPreview] = useState<SabnzbdPreview | null>(null)
  const [applyResult, setApplyResult] = useState<SabnzbdApplyResult | null>(null)
  const [iniFile, setIniFile] = useState<File | null>(null)
  const [sabUrl, setSabUrl] = useState('')
  const [sabApiKey, setSabApiKey] = useState('')

  const handleImportIni = async () => {
    if (!iniFile) return
    setLoading(true); setError(null)
    try {
      const formData = new FormData()
      formData.append('file', iniFile)
      const res = await fetch('/api/v1/usenet/import-sabnzbd', { method: 'POST', body: formData })
      if (!res.ok) { const err = await res.json().catch(() => ({ error: res.statusText })); throw new Error(err.error || res.statusText) }
      setPreview(await res.json())
    } catch (e) { setError(e instanceof Error ? e.message : String(e)) }
    finally { setLoading(false) }
  }

  const handleImportApi = async () => {
    if (!sabUrl) return
    setLoading(true); setError(null)
    try {
      const res = await fetch('/api/v1/usenet/import-sabnzbd-api', {
        method: 'POST', headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ url: sabUrl, apiKey: sabApiKey }),
      })
      if (!res.ok) { const err = await res.json().catch(() => ({ error: res.statusText })); throw new Error(err.error || res.statusText) }
      setPreview(await res.json())
    } catch (e) { setError(e instanceof Error ? e.message : String(e)) }
    finally { setLoading(false) }
  }

  const handleApply = async () => {
    if (!preview) return
    setLoading(true); setError(null)
    try {
      const res = await fetch('/api/v1/usenet/import-sabnzbd/apply', {
        method: 'POST', headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(preview),
      })
      if (!res.ok) { const err = await res.json().catch(() => ({ error: res.statusText })); throw new Error(err.error || res.statusText) }
      setApplyResult(await res.json())
    } catch (e) { setError(e instanceof Error ? e.message : String(e)) }
    finally { setLoading(false) }
  }

  const handleReset = () => { setPreview(null); setApplyResult(null); setError(null); setIniFile(null); setSabUrl(''); setSabApiKey('') }

  return (
    <Card>
      <div className="mb-4 flex items-center gap-3">
        <Server className="h-5 w-5 text-orange-400" />
        <div>
          <h3 className="text-base font-semibold">Import from SABnzbd</h3>
          <p className="text-xs text-slate-400">Import NNTP servers, categories, and settings.</p>
        </div>
      </div>

      {applyResult && (
        <div>
          <div className="mb-3 flex items-center gap-2">
            <CheckCircle className="h-5 w-5 text-green-500" />
            <span className="font-semibold text-green-400">SABnzbd Import Applied</span>
          </div>
          <div className="space-y-1.5 mb-3">
            <MigrateResultRow label="NNTP servers added" value={applyResult.serversAdded} />
            <MigrateResultRow label="Categories added" value={applyResult.categoriesAdded} />
            <MigrateResultRow label="RSS feeds added" value={applyResult.rssFeedsAdded} />
          </div>
          {applyResult.warnings.length > 0 && (
            <div className="mb-3 rounded-lg border border-yellow-500/30 bg-yellow-500/10 p-3">
              <h4 className="mb-1 text-sm font-medium text-yellow-400">Warnings</h4>
              {applyResult.warnings.map((w, i) => <div key={i} className="text-xs text-yellow-300">{w}</div>)}
            </div>
          )}
          <Btn variant="ghost" onClick={handleReset}>Done</Btn>
        </div>
      )}

      {!applyResult && preview && (
        <div>
          <h4 className="mb-2 text-xs font-semibold text-slate-300 uppercase tracking-wider">Import Preview</h4>
          {preview.servers.length > 0 && (
            <div className="mb-3">
              <h5 className="mb-1 text-xs font-medium text-slate-400">NNTP Servers ({preview.servers.length})</h5>
              <div className="space-y-1">
                {preview.servers.map((s, i) => (
                  <div key={i} className="flex items-center gap-2 rounded bg-slate-700/50 px-3 py-1.5 text-xs">
                    <Server size={12} className="text-orange-400 shrink-0" />
                    <span className="font-medium text-white">{s.name}</span>
                    <span className="text-slate-400">{s.host}:{s.port}</span>
                    {s.ssl && <span className="rounded bg-green-500/20 px-1 py-0.5 text-[10px] text-green-400">SSL</span>}
                  </div>
                ))}
              </div>
            </div>
          )}
          {preview.categories.length > 0 && (
            <div className="mb-3">
              <h5 className="mb-1 text-xs font-medium text-slate-400">Categories ({preview.categories.length})</h5>
              <div className="flex flex-wrap gap-1">
                {preview.categories.map((c, i) => <span key={i} className="rounded bg-slate-700 px-2 py-1 text-xs text-white">{c.name}</span>)}
              </div>
            </div>
          )}
          {error && <div className="mb-3 rounded-lg border border-red-500/30 bg-red-500/10 p-3 text-sm text-red-400">{error}</div>}
          <div className="flex gap-2">
            <Btn onClick={handleApply} disabled={loading}>
              {loading ? <><Loader2 className="h-4 w-4 animate-spin" /> Applying...</> : <><CheckCircle className="h-4 w-4" /> Apply</>}
            </Btn>
            <Btn variant="ghost" onClick={handleReset}>Cancel</Btn>
          </div>
        </div>
      )}

      {!applyResult && !preview && (
        <div>
          <div className="mb-4 flex rounded-lg bg-slate-700/50 p-0.5">
            <button
              onClick={() => setMode('ini')}
              className={`flex-1 rounded-md px-3 py-1.5 text-xs font-medium transition-colors ${mode === 'ini' ? 'bg-slate-600 text-white' : 'text-slate-400 hover:text-white'}`}
            >
              <FileUp size={12} className="mr-1 inline" /> Upload INI
            </button>
            <button
              onClick={() => setMode('api')}
              className={`flex-1 rounded-md px-3 py-1.5 text-xs font-medium transition-colors ${mode === 'api' ? 'bg-slate-600 text-white' : 'text-slate-400 hover:text-white'}`}
            >
              <Globe size={12} className="mr-1 inline" /> Connect
            </button>
          </div>

          {mode === 'ini' && (
            <div>
              <MigrateFileInput label="sabnzbd.ini" description="SABnzbd configuration file" file={iniFile} onFileChange={setIniFile} accept=".ini,.conf,.cfg" />
              {error && <div className="mt-2 rounded-lg border border-red-500/30 bg-red-500/10 p-2 text-xs text-red-400">{error}</div>}
              <div className="mt-3">
                <Btn onClick={handleImportIni} disabled={!iniFile || loading}>
                  {loading ? <><Loader2 className="h-4 w-4 animate-spin" /> Parsing...</> : <><Upload className="h-4 w-4" /> Parse</>}
                </Btn>
              </div>
            </div>
          )}

          {mode === 'api' && (
            <div className="space-y-3">
              <Input label="SABnzbd URL" value={sabUrl} onChange={setSabUrl} placeholder="http://192.168.0.30:8080" />
              <Input label="API Key" value={sabApiKey} onChange={setSabApiKey} placeholder="Your SABnzbd API key" />
              {error && <div className="rounded-lg border border-red-500/30 bg-red-500/10 p-2 text-xs text-red-400">{error}</div>}
              <Btn onClick={handleImportApi} disabled={!sabUrl || loading}>
                {loading ? <><Loader2 className="h-4 w-4 animate-spin" /> Connecting...</> : <><Globe className="h-4 w-4" /> Fetch</>}
              </Btn>
            </div>
          )}
        </div>
      )}
    </Card>
  )
}

function MigrateFileInput({ label, description, file, onFileChange, accept }: {
  label: string; description: string; file: File | null; onFileChange: (f: File | null) => void; accept: string
}) {
  const inputRef = useRef<HTMLInputElement>(null)
  return (
    <div
      onClick={() => inputRef.current?.click()}
      className="flex cursor-pointer items-center gap-3 rounded-lg border border-dashed border-slate-600 p-3 hover:border-blue-500 transition-colors"
    >
      <FileUp className={`h-5 w-5 ${file ? 'text-blue-400' : 'text-slate-500'}`} />
      <div className="flex-1 min-w-0">
        <div className="text-sm font-medium text-white">{label}</div>
        <div className="text-xs text-slate-400">
          {file ? <span className="text-blue-400">{file.name} ({(file.size / 1048576).toFixed(1)} MB)</span> : description}
        </div>
      </div>
      <input ref={inputRef} type="file" accept={accept} className="hidden" onChange={(e) => onFileChange(e.target.files?.[0] ?? null)} />
      {file && (
        <button type="button" onClick={(e) => { e.stopPropagation(); onFileChange(null); if (inputRef.current) inputRef.current.value = '' }} className="text-slate-400 hover:text-white">
          <XCircle size={14} />
        </button>
      )}
    </div>
  )
}

function MigrateResultRow({ label, value }: { label: string; value: number | string }) {
  return (
    <div className="flex justify-between rounded bg-slate-700/50 px-3 py-2 text-sm">
      <span className="text-slate-400">{label}</span>
      <span className="font-medium text-white">{value}</span>
    </div>
  )
}

function MigrationReport({ result, onReset }: { result: MigrationResult; onReset: () => void }) {
  return (
    <div>
      <div className="mb-3 flex items-center gap-2">
        {result.warnings.length === 0 ? (
          <><CheckCircle className="h-5 w-5 text-green-500" /><span className="font-semibold text-green-400">Migration Complete</span></>
        ) : (
          <><XCircle className="h-5 w-5 text-yellow-500" /><span className="font-semibold text-yellow-400">Completed with warnings</span></>
        )}
      </div>
      <div className="mb-3 space-y-1.5">
        <MigrateResultRow label="Series imported" value={result.seriesImported} />
        <MigrateResultRow label="Movies imported" value={result.moviesImported} />
        <MigrateResultRow label="Indexers imported" value={result.indexersImported} />
      </div>
      {result.warnings.length > 0 && (
        <div className="mb-3">
          <h4 className="mb-1 text-xs font-medium text-yellow-400">Warnings</h4>
          <div className="max-h-32 overflow-y-auto rounded bg-slate-900 p-2">
            {result.warnings.map((w, i) => <div key={i} className="text-xs text-yellow-300">{w}</div>)}
          </div>
        </div>
      )}
      <Btn variant="ghost" onClick={onReset}>Start Over</Btn>
    </div>
  )
}
