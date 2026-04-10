import React, { useState, useEffect, useCallback, useRef } from 'react'
import type {
  QualityProfile,
  QualityProfileItem,
  ProfileFormatItem,
  CustomFormat,
  FormatSpecification,
  FormatField,
  IndexerConfig,
  AvailableIndexer,
  DownloadClientConfig,
  NamingConfig,
  MediaLibraryFolder,
  Tag,
  EnabledModules,
  MigrationResult,
} from '../api/types'
import {
  useSystemStatus,
  useMigrate,
  usePlexServers,
  useAddPlexServer,
  useUpdatePlexServer,
  useDeletePlexServer,
  usePlexLibraries,
  useTogglePlexLibrary,
  usePlexFullScan,
  usePlexRecentScan,
} from '../hooks/useApi'
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
  Film,
  Folder,
  FolderOpen,
  ArrowUp,
  ArrowDown,
  Tv,
  GripVertical,
  RefreshCcw,
} from 'lucide-react'
import { useQuery } from '@tanstack/react-query'
import { apiFetch, authHeaders } from '../api/client'

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const API = '/api/v1'

type TabKey =
  | 'general'
  | 'modules'
  | 'quality'
  | 'customformats'
  | 'indexers'
  | 'downloadclients'
  | 'naming'
  | 'medialibraryfolders'
  | 'tags'
  | 'plex'
  | 'bootstrap'
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
  { key: 'customformats', label: 'Custom Formats', group: 'Settings' },
  { key: 'indexers', label: 'Indexers', group: 'Settings' },
  { key: 'downloadclients', label: 'Download Clients', group: 'Settings' },
  { key: 'naming', label: 'Naming', group: 'Settings' },
  { key: 'medialibraryfolders', label: 'Media Folders', group: 'Settings' },
  { key: 'tags', label: 'Tags', group: 'Settings' },
  { key: 'plex', label: 'Plex', group: 'Settings' },
  { key: 'bootstrap', label: 'Remote Access', group: 'Settings' },
  { key: 'backup', label: 'Backup / Restore', group: 'Data' },
  { key: 'migration', label: 'Migration', group: 'Data' },
]

// ---------------------------------------------------------------------------
// Small helpers
// ---------------------------------------------------------------------------

function formatBytes(bytes: number | null | undefined): string {
  if (bytes == null || !isFinite(bytes) || bytes <= 0) return '-'
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
  const [grabStrategy, setGrabStrategy] = useState('best_quality')
  const [saving, setSaving] = useState(false)

  useEffect(() => {
    fetch(`${API}/config/general`)
      .then((r) => r.json())
      .then((d: { instanceName?: string; authMethod?: string; grabStrategy?: string }) => {
        setInstanceName(d.instanceName ?? '')
        setAuthMethod(d.authMethod ?? 'none')
        setGrabStrategy(d.grabStrategy ?? 'best_quality')
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
        body: JSON.stringify({ instanceName, authMethod, grabStrategy }),
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
        <Input label="Instance Name" value={instanceName} onChange={setInstanceName} placeholder="NGMS" />
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
        <Select
          label="Grab Strategy"
          value={grabStrategy}
          onChange={setGrabStrategy}
          options={[
            { value: 'best_quality', label: 'Best Quality (quality first, indexer priority as tiebreaker)' },
            { value: 'indexer_priority', label: 'Indexer Priority (prefer higher-priority indexers)' },
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
  { key: 'stremioAddon', label: 'Stremio Addon', description: 'Expose your library to Stremio clients for remote playback', category: 'Integrations' },
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

function flattenProfileItems(items: QualityProfileItem[]): QualityProfileItem[] {
  const result: QualityProfileItem[] = []
  for (const item of items) {
    if (item.quality) {
      result.push(item)
    } else if (item.items && item.items.length > 0) {
      result.push(...flattenProfileItems(item.items))
    }
  }
  return result
}

/** Collect all selectable quality/group entries for the cutoff dropdown. */
function cutoffOptions(items: QualityProfileItem[]): { value: number; label: string }[] {
  const opts: { value: number; label: string }[] = []
  for (const item of items) {
    if (item.quality) {
      opts.push({ value: item.quality.id, label: item.quality.name })
    } else if (item.items && item.items.length > 0) {
      // Group — use the group id (1000+) stored as item.id on the raw JSON
      const raw = item as unknown as { id?: number; name?: string }
      if (raw.id != null) {
        opts.push({ value: raw.id, label: raw.name ?? `Group ${raw.id}` })
      }
      // Also include individual qualities within the group
      for (const child of item.items) {
        if (child.quality) {
          opts.push({ value: child.quality.id, label: `  ${child.quality.name}` })
        }
      }
    }
  }
  return opts
}

/** Display name for a cutoff value given profile items. */
function cutoffLabel(cutoff: number, items: QualityProfileItem[]): string {
  for (const opt of cutoffOptions(items)) {
    if (opt.value === cutoff) return opt.label.trim()
  }
  return String(cutoff)
}

const MEDIA_TYPE_LABELS: Record<string, string> = {
  series: 'Series',
  movie: 'Movies',
  any: 'Any',
}

function mediaTypeLabel(mt: string | null): string {
  return mt ? (MEDIA_TYPE_LABELS[mt] ?? mt) : 'Any'
}

function mediaTypeBadgeClass(mt: string | null): string {
  switch (mt) {
    case 'series':
      return 'bg-blue-500/20 text-blue-400'
    case 'movie':
      return 'bg-purple-500/20 text-purple-400'
    default:
      return 'bg-slate-500/20 text-slate-400'
  }
}

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
      showToast('Failed to load quality profiles', 'error')
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

  const toggleItemAllowed = (itemIdx: number, value: boolean) => {
    if (!editingProfile) return
    const updated = editingProfile.items.map((item, i) => {
      if (i === itemIdx) {
        // For groups, toggle the group and all children
        if (item.items && item.items.length > 0) {
          return { ...item, allowed: value, items: item.items.map((c) => ({ ...c, allowed: value })) }
        }
        return { ...item, allowed: value }
      }
      return item
    })
    setEditingProfile({ ...editingProfile, items: updated })
  }

  const toggleChildAllowed = (parentIdx: number, childIdx: number, value: boolean) => {
    if (!editingProfile) return
    const updated = editingProfile.items.map((item, i) => {
      if (i === parentIdx && item.items) {
        const newChildren = item.items.map((c, ci) => ci === childIdx ? { ...c, allowed: value } : c)
        // If all children now enabled, enable group; if all disabled, disable group
        const allEnabled = newChildren.every((c) => c.allowed)
        const allDisabled = newChildren.every((c) => !c.allowed)
        return { ...item, items: newChildren, allowed: allDisabled ? false : allEnabled ? true : item.allowed }
      }
      return item
    })
    setEditingProfile({ ...editingProfile, items: updated })
  }

  const moveItem = (idx: number, dir: -1 | 1) => {
    if (!editingProfile) return
    const items = [...editingProfile.items]
    const target = idx + dir
    if (target < 0 || target >= items.length) return
    ;[items[idx], items[target]] = [items[target], items[idx]]
    setEditingProfile({ ...editingProfile, items })
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

  // Group profiles by media type
  const groups: { key: string; label: string; items: QualityProfile[] }[] = []
  const byType = new Map<string, QualityProfile[]>()
  for (const p of profiles) {
    const key = p.mediaType ?? 'any'
    if (!byType.has(key)) byType.set(key, [])
    byType.get(key)!.push(p)
  }
  // Show in consistent order: series, movie, any
  for (const key of ['series', 'movie', 'any']) {
    const items = byType.get(key)
    if (items && items.length > 0) {
      groups.push({ key, label: MEDIA_TYPE_LABELS[key] ?? key, items })
    }
  }

  return (
    <Card>
      <h2 className="mb-6 text-lg font-semibold text-white">Quality Profiles</h2>

      {profiles.length === 0 ? (
        <p className="text-sm text-slate-400">No quality profiles configured.</p>
      ) : (
        <div className="space-y-6">
          {groups.map((group) => (
            <div key={group.key}>
              <h3 className="mb-3 text-sm font-medium text-slate-400 uppercase tracking-wider">{group.label}</h3>
              <table className="w-full text-left text-sm">
                <thead>
                  <tr className="border-b border-slate-700 text-slate-400">
                    <th className="pb-3 pr-4 font-medium" />
                    <th className="pb-3 pr-4 font-medium">Name</th>
                    <th className="pb-3 pr-4 font-medium">Type</th>
                    <th className="pb-3 pr-4 font-medium">Cutoff</th>
                    <th className="pb-3 pr-4 font-medium">Items</th>
                    <th className="pb-3 font-medium" />
                  </tr>
                </thead>
                <tbody>
                  {group.items.map((p) => (
                    <React.Fragment key={p.id}>
                      <tr
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
                        <td className="py-3 pr-4">
                          <span className={`inline-block rounded px-2 py-0.5 text-xs font-medium ${mediaTypeBadgeClass(p.mediaType)}`}>
                            {mediaTypeLabel(p.mediaType)}
                          </span>
                        </td>
                        <td className="py-3 pr-4 text-slate-300">{cutoffLabel(p.cutoff, p.items)}</td>
                        <td className="py-3 pr-4 text-slate-300">{flattenProfileItems(p.items).length}</td>
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
                          <td colSpan={6} className="bg-slate-800/50 px-6 py-4">
                            <div className="space-y-4">
                              {/* Row 1: Name, Cutoff, Media Type, Language */}
                              <div className="grid grid-cols-4 gap-4 max-w-2xl">
                                <Input
                                  label="Name"
                                  value={editingProfile.name}
                                  onChange={(v) => setEditingProfile({ ...editingProfile, name: v })}
                                />
                                <div>
                                  <label className="mb-1 block text-sm font-medium text-slate-300">Cutoff</label>
                                  <select
                                    value={editingProfile.cutoff}
                                    onChange={(e) => setEditingProfile({ ...editingProfile, cutoff: Number(e.target.value) })}
                                    className="w-full rounded-lg border border-slate-600 bg-slate-700 px-3 py-2 text-sm text-white focus:border-blue-500 focus:outline-none focus:ring-1 focus:ring-blue-500"
                                  >
                                    {cutoffOptions(editingProfile.items).map((opt) => (
                                      <option key={opt.value} value={opt.value}>{opt.label}</option>
                                    ))}
                                  </select>
                                </div>
                                <div>
                                  <label className="mb-1 block text-sm font-medium text-slate-300">Media Type</label>
                                  <select
                                    value={editingProfile.mediaType ?? 'any'}
                                    onChange={(e) => setEditingProfile({ ...editingProfile, mediaType: e.target.value === 'any' ? null : e.target.value })}
                                    className="w-full rounded-lg border border-slate-600 bg-slate-700 px-3 py-2 text-sm text-white focus:border-blue-500 focus:outline-none focus:ring-1 focus:ring-blue-500"
                                  >
                                    <option value="any">Any</option>
                                    <option value="series">Series</option>
                                    <option value="movie">Movies</option>
                                  </select>
                                </div>
                                <div>
                                  <label className="mb-1 block text-sm font-medium text-slate-300">Language</label>
                                  <select
                                    value={String(editingProfile.language ?? -1)}
                                    onChange={(e) => setEditingProfile({ ...editingProfile, language: Number(e.target.value) })}
                                    className="w-full rounded-lg border border-slate-600 bg-slate-700 px-3 py-2 text-sm text-white focus:border-blue-500 focus:outline-none focus:ring-1 focus:ring-blue-500"
                                  >
                                    <option value="-1">Any</option>
                                    <option value="-2">Original</option>
                                    <option value="1">English</option>
                                    <option value="2">French</option>
                                    <option value="3">Spanish</option>
                                    <option value="4">German</option>
                                    <option value="5">Italian</option>
                                    <option value="6">Portuguese</option>
                                    <option value="7">Japanese</option>
                                    <option value="8">Korean</option>
                                    <option value="9">Chinese</option>
                                    <option value="10">Russian</option>
                                  </select>
                                </div>
                              </div>

                              {/* Row 2: Upgrade + Format Score Settings */}
                              <div className="flex items-center gap-6 flex-wrap">
                                <Toggle
                                  label="Upgrade Allowed"
                                  checked={editingProfile.upgradeAllowed ?? true}
                                  onChange={(v) => setEditingProfile({ ...editingProfile, upgradeAllowed: v })}
                                />
                                <Input
                                  label="Min Format Score"
                                  value={String(editingProfile.minFormatScore ?? 0)}
                                  onChange={(v) => setEditingProfile({ ...editingProfile, minFormatScore: Number(v) || 0 })}
                                  type="number"
                                />
                                <Input
                                  label="Cutoff Format Score"
                                  value={String(editingProfile.cutoffFormatScore ?? 0)}
                                  onChange={(v) => setEditingProfile({ ...editingProfile, cutoffFormatScore: Number(v) || 0 })}
                                  type="number"
                                />
                                <Input
                                  label="Min Upgrade Score"
                                  value={String(editingProfile.minUpgradeFormatScore ?? 1)}
                                  onChange={(v) => setEditingProfile({ ...editingProfile, minUpgradeFormatScore: Number(v) || 1 })}
                                  type="number"
                                />
                              </div>

                              {/* Qualities — ordered list (bottom = lowest preference, top = highest) */}
                              <div>
                                <div className="mb-2 flex items-center justify-between">
                                  <span className="text-sm font-medium text-slate-300">Qualities</span>
                                  <span className="text-xs text-slate-500">Drag or use arrows to reorder. Top = highest preference.</span>
                                </div>
                                <div className="rounded-lg border border-slate-700 divide-y divide-slate-700/50">
                                  {[...editingProfile.items].reverse().map((item, reversedIdx) => {
                                    const idx = editingProfile.items.length - 1 - reversedIdx
                                    const isGroup = !item.quality && item.items && item.items.length > 0
                                    const isCutoff = item.quality
                                      ? item.quality.id === editingProfile.cutoff
                                      : (item as unknown as { id?: number }).id === editingProfile.cutoff

                                    return (
                                      <div key={item.quality?.id ?? (item as unknown as { id?: number }).id ?? idx}>
                                        {/* Cutoff indicator */}
                                        {isCutoff && (
                                          <div className="flex items-center gap-2 px-3 py-0.5 bg-amber-500/10">
                                            <div className="flex-1 border-t border-amber-500/50" />
                                            <span className="text-[10px] font-semibold uppercase tracking-wider text-amber-400">Cutoff</span>
                                            <div className="flex-1 border-t border-amber-500/50" />
                                          </div>
                                        )}
                                        <div className={`flex items-center gap-2 px-2 py-1.5 ${isCutoff ? 'bg-amber-500/5' : 'hover:bg-slate-700/30'} transition-colors`}>
                                          {/* Grip / reorder */}
                                          <GripVertical size={14} className="text-slate-600 flex-shrink-0" />
                                          <div className="flex flex-col gap-0.5 flex-shrink-0">
                                            <button
                                              onClick={() => moveItem(idx, 1)}
                                              disabled={idx === editingProfile.items.length - 1}
                                              className="text-slate-500 hover:text-white disabled:opacity-20 transition-colors"
                                              title="Move up (higher preference)"
                                            >
                                              <ArrowUp size={12} />
                                            </button>
                                            <button
                                              onClick={() => moveItem(idx, -1)}
                                              disabled={idx === 0}
                                              className="text-slate-500 hover:text-white disabled:opacity-20 transition-colors"
                                              title="Move down (lower preference)"
                                            >
                                              <ArrowDown size={12} />
                                            </button>
                                          </div>
                                          {/* Checkbox */}
                                          <input
                                            type="checkbox"
                                            checked={item.allowed}
                                            onChange={(e) => toggleItemAllowed(idx, e.target.checked)}
                                            className="rounded border-slate-600 bg-slate-700 text-blue-500 focus:ring-blue-500"
                                          />
                                          {/* Label */}
                                          {isGroup ? (
                                            <span className="text-sm font-medium text-slate-200">
                                              {(item as unknown as { name?: string }).name ?? 'Group'}
                                            </span>
                                          ) : (
                                            <span className={`text-sm ${item.allowed ? 'text-slate-200' : 'text-slate-500'}`}>
                                              {item.quality?.name ?? 'Unknown'}
                                            </span>
                                          )}
                                          {/* Rank badge */}
                                          <span className="ml-auto text-[10px] text-slate-600 tabular-nums">#{idx + 1}</span>
                                        </div>
                                        {/* Group children */}
                                        {isGroup && item.items && (
                                          <div className="ml-10 border-l border-slate-700 divide-y divide-slate-700/30">
                                            {item.items.map((child, ci) => (
                                              <div key={child.quality?.id ?? ci} className="flex items-center gap-2 px-3 py-1 hover:bg-slate-700/20 transition-colors">
                                                <input
                                                  type="checkbox"
                                                  checked={child.allowed}
                                                  onChange={(e) => toggleChildAllowed(idx, ci, e.target.checked)}
                                                  className="rounded border-slate-600 bg-slate-700 text-blue-500 focus:ring-blue-500"
                                                  disabled={!item.allowed}
                                                />
                                                <span className={`text-sm ${child.allowed && item.allowed ? 'text-slate-300' : 'text-slate-500'}`}>
                                                  {child.quality?.name ?? 'Unknown'}
                                                </span>
                                              </div>
                                            ))}
                                          </div>
                                        )}
                                      </div>
                                    )
                                  })}
                                </div>
                              </div>

                              {/* Custom Format Scores */}
                              {editingProfile.formatItems && editingProfile.formatItems.length > 0 && (
                                <div>
                                  <span className="mb-2 block text-sm font-medium text-slate-300">Custom Format Scores</span>
                                  <div className="max-h-64 overflow-y-auto rounded-lg border border-slate-700">
                                    <table className="w-full text-left text-sm">
                                      <thead>
                                        <tr className="border-b border-slate-700 text-slate-400 sticky top-0 bg-slate-800">
                                          <th className="px-3 py-2 font-medium">Format</th>
                                          <th className="px-3 py-2 font-medium w-28">Score</th>
                                        </tr>
                                      </thead>
                                      <tbody>
                                        {editingProfile.formatItems.map((fi: ProfileFormatItem) => (
                                          <tr key={fi.format} className="border-b border-slate-700/50">
                                            <td className="px-3 py-1.5 text-slate-200">{fi.name}</td>
                                            <td className="px-3 py-1.5">
                                              <input
                                                type="number"
                                                value={fi.score}
                                                onChange={(e) => {
                                                  const score = Number(e.target.value) || 0
                                                  const updated = editingProfile.formatItems.map((item: ProfileFormatItem) =>
                                                    item.format === fi.format ? { ...item, score } : item,
                                                  )
                                                  setEditingProfile({ ...editingProfile, formatItems: updated })
                                                }}
                                                className="w-24 rounded border border-slate-600 bg-slate-700 px-2 py-1 text-xs text-white focus:border-blue-500 focus:outline-none"
                                              />
                                            </td>
                                          </tr>
                                        ))}
                                      </tbody>
                                    </table>
                                  </div>
                                </div>
                              )}

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
                    </React.Fragment>
                  ))}
                </tbody>
              </table>
            </div>
          ))}
        </div>
      )}
    </Card>
  )
}

// ---------------------------------------------------------------------------
// Custom Formats Tab
// ---------------------------------------------------------------------------

const FORMAT_FIELD_OPTIONS: { value: FormatField; label: string }[] = [
  { value: 'releaseName', label: 'Release Title' },
  { value: 'quality', label: 'Quality' },
  { value: 'language', label: 'Language' },
  { value: 'releaseGroup', label: 'Release Group' },
  { value: 'indexerFlag', label: 'Indexer Flag' },
  { value: 'size', label: 'Size' },
]

const emptySpec: FormatSpecification = {
  field: 'releaseName',
  pattern: '',
  negate: false,
  required: false,
}

function CustomFormatsTab({
  showToast,
}: {
  showToast: (msg: string, type: 'success' | 'error') => void
}) {
  const [formats, setFormats] = useState<CustomFormat[]>([])
  const [loading, setLoading] = useState(true)
  const [editingFormat, setEditingFormat] = useState<CustomFormat | null>(null)
  const [isCreating, setIsCreating] = useState(false)
  const [testTitle, setTestTitle] = useState('')
  const [testResult, setTestResult] = useState<boolean | null>(null)
  const [testing, setTesting] = useState(false)

  const load = useCallback(async () => {
    setLoading(true)
    try {
      const res = await fetch(`${API}/customformat`)
      const data: CustomFormat[] = await res.json()
      setFormats(data)
    } catch {
      showToast('Failed to load custom formats', 'error')
    } finally {
      setLoading(false)
    }
  }, [])

  useEffect(() => {
    void load()
  }, [load])

  const startCreate = () => {
    setEditingFormat({
      id: 0,
      name: '',
      specifications: [],
      includeCustomFormatWhenRenaming: false,
    })
    setIsCreating(true)
    setTestResult(null)
  }

  const startEdit = (cf: CustomFormat) => {
    setEditingFormat(structuredClone(cf))
    setIsCreating(false)
    setTestResult(null)
  }

  const cancelEdit = () => {
    setEditingFormat(null)
    setIsCreating(false)
    setTestResult(null)
  }

  const addSpec = () => {
    if (!editingFormat) return
    setEditingFormat({
      ...editingFormat,
      specifications: [...editingFormat.specifications, { ...emptySpec }],
    })
  }

  const updateSpec = (idx: number, patch: Partial<FormatSpecification>) => {
    if (!editingFormat) return
    const specs = editingFormat.specifications.map((s, i) =>
      i === idx ? { ...s, ...patch } : s,
    )
    setEditingFormat({ ...editingFormat, specifications: specs })
  }

  const removeSpec = (idx: number) => {
    if (!editingFormat) return
    setEditingFormat({
      ...editingFormat,
      specifications: editingFormat.specifications.filter((_, i) => i !== idx),
    })
  }

  const saveFormat = async () => {
    if (!editingFormat || !editingFormat.name.trim()) {
      showToast('Name is required', 'error')
      return
    }
    if (editingFormat.specifications.length === 0) {
      showToast('At least one condition is required', 'error')
      return
    }
    try {
      const url = isCreating
        ? `${API}/customformat`
        : `${API}/customformat/${editingFormat.id}`
      const method = isCreating ? 'POST' : 'PUT'
      const res = await fetch(url, {
        method,
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(editingFormat),
      })
      if (!res.ok) {
        const text = await res.text()
        throw new Error(text || 'Save failed')
      }
      showToast(isCreating ? 'Custom format created' : 'Custom format saved', 'success')
      cancelEdit()
      void load()
    } catch (e) {
      showToast(`Failed to save: ${e instanceof Error ? e.message : 'unknown error'}`, 'error')
    }
  }

  const deleteFormat = async (id: number) => {
    try {
      const res = await fetch(`${API}/customformat/${id}`, { method: 'DELETE' })
      if (!res.ok) throw new Error('Delete failed')
      showToast('Custom format deleted', 'success')
      void load()
    } catch {
      showToast('Failed to delete custom format', 'error')
    }
  }

  const runTest = async () => {
    if (!editingFormat || !testTitle.trim()) return
    setTesting(true)
    try {
      const res = await fetch(`${API}/customformat/test`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          releaseTitle: testTitle,
          specifications: editingFormat.specifications,
        }),
      })
      const data = await res.json()
      setTestResult(data.matched)
    } catch {
      showToast('Test failed', 'error')
    } finally {
      setTesting(false)
    }
  }

  if (loading) {
    return (
      <Card>
        <div className="flex items-center gap-2 text-slate-400">
          <Loader2 className="h-5 w-5 animate-spin" /> Loading custom formats...
        </div>
      </Card>
    )
  }

  // Edit/Create form
  if (editingFormat) {
    return (
      <Card>
        <h2 className="mb-6 text-lg font-semibold text-white">
          {isCreating ? 'New Custom Format' : `Edit: ${editingFormat.name}`}
        </h2>
        <div className="space-y-6">
          <div className="grid grid-cols-2 gap-4 max-w-lg">
            <Input
              label="Name"
              value={editingFormat.name}
              onChange={(v) => setEditingFormat({ ...editingFormat, name: v })}
            />
            <div className="flex items-end pb-1">
              <Toggle
                label="Include in renaming"
                checked={editingFormat.includeCustomFormatWhenRenaming}
                onChange={(v) =>
                  setEditingFormat({ ...editingFormat, includeCustomFormatWhenRenaming: v })
                }
              />
            </div>
          </div>

          {/* Conditions */}
          <div>
            <div className="flex items-center justify-between mb-3">
              <span className="text-sm font-medium text-slate-300">Conditions</span>
              <Btn variant="ghost" onClick={addSpec} className="!px-3 !py-1.5 !text-xs">
                <Plus className="h-3.5 w-3.5" /> Add Condition
              </Btn>
            </div>
            {editingFormat.specifications.length === 0 ? (
              <p className="text-sm text-slate-500 italic">
                No conditions yet. Add at least one condition.
              </p>
            ) : (
              <div className="space-y-2">
                {editingFormat.specifications.map((spec, idx) => (
                  <div
                    key={idx}
                    className="flex items-center gap-2 rounded-lg border border-slate-700 bg-slate-900/50 px-3 py-2"
                  >
                    <select
                      value={spec.field}
                      onChange={(e) => updateSpec(idx, { field: e.target.value as FormatField })}
                      className="rounded border border-slate-600 bg-slate-700 px-2 py-1.5 text-xs text-white focus:border-blue-500 focus:outline-none w-32"
                    >
                      {FORMAT_FIELD_OPTIONS.map((o) => (
                        <option key={o.value} value={o.value}>
                          {o.label}
                        </option>
                      ))}
                    </select>

                    {spec.field === 'size' ? (
                      <div className="flex items-center gap-1 flex-1">
                        <input
                          type="number"
                          placeholder="Min GB"
                          value={
                            spec.pattern.includes('-')
                              ? String(
                                  Number(spec.pattern.split('-')[0]) / 1073741824,
                                )
                              : ''
                          }
                          onChange={(e) => {
                            const min = Number(e.target.value) * 1073741824
                            const parts = spec.pattern.split('-')
                            const max = parts.length > 1 ? parts[1] : '0'
                            updateSpec(idx, { pattern: `${Math.round(min)}-${max}` })
                          }}
                          className="w-20 rounded border border-slate-600 bg-slate-700 px-2 py-1.5 text-xs text-white focus:border-blue-500 focus:outline-none"
                        />
                        <span className="text-slate-500 text-xs">to</span>
                        <input
                          type="number"
                          placeholder="Max GB"
                          value={
                            spec.pattern.includes('-')
                              ? String(
                                  Number(spec.pattern.split('-')[1]) / 1073741824,
                                )
                              : ''
                          }
                          onChange={(e) => {
                            const max = Number(e.target.value) * 1073741824
                            const parts = spec.pattern.split('-')
                            const min = parts.length > 0 ? parts[0] : '0'
                            updateSpec(idx, { pattern: `${min}-${Math.round(max)}` })
                          }}
                          className="w-20 rounded border border-slate-600 bg-slate-700 px-2 py-1.5 text-xs text-white focus:border-blue-500 focus:outline-none"
                        />
                        <span className="text-slate-500 text-xs">GB</span>
                      </div>
                    ) : (
                      <input
                        type="text"
                        value={spec.pattern}
                        onChange={(e) => updateSpec(idx, { pattern: e.target.value })}
                        placeholder={
                          spec.field === 'releaseName' || spec.field === 'releaseGroup'
                            ? 'Regex pattern'
                            : 'Value'
                        }
                        className="flex-1 rounded border border-slate-600 bg-slate-700 px-2 py-1.5 text-xs text-white placeholder-slate-400 focus:border-blue-500 focus:outline-none font-mono"
                      />
                    )}

                    <label className="inline-flex items-center gap-1 text-xs text-slate-400 cursor-pointer whitespace-nowrap">
                      <input
                        type="checkbox"
                        checked={spec.negate}
                        onChange={(e) => updateSpec(idx, { negate: e.target.checked })}
                        className="rounded border-slate-600 bg-slate-700 text-blue-500 focus:ring-blue-500"
                      />
                      Negate
                    </label>

                    <label className="inline-flex items-center gap-1 text-xs text-slate-400 cursor-pointer whitespace-nowrap">
                      <input
                        type="checkbox"
                        checked={spec.required}
                        onChange={(e) => updateSpec(idx, { required: e.target.checked })}
                        className="rounded border-slate-600 bg-slate-700 text-blue-500 focus:ring-blue-500"
                      />
                      Required
                    </label>

                    <button
                      onClick={() => removeSpec(idx)}
                      className="text-slate-500 hover:text-red-400 transition-colors p-1"
                    >
                      <X className="h-3.5 w-3.5" />
                    </button>
                  </div>
                ))}
              </div>
            )}
          </div>

          {/* Test */}
          <div className="border-t border-slate-700 pt-4">
            <span className="mb-2 block text-sm font-medium text-slate-300">Test</span>
            <div className="flex items-center gap-2 max-w-2xl">
              <input
                type="text"
                value={testTitle}
                onChange={(e) => {
                  setTestTitle(e.target.value)
                  setTestResult(null)
                }}
                placeholder="Enter a release title to test against..."
                className="flex-1 rounded-lg border border-slate-600 bg-slate-700 px-3 py-2 text-sm text-white placeholder-slate-400 focus:border-blue-500 focus:outline-none font-mono"
              />
              <Btn
                variant="ghost"
                onClick={runTest}
                disabled={testing || !testTitle.trim() || editingFormat.specifications.length === 0}
              >
                {testing ? <Loader2 className="h-4 w-4 animate-spin" /> : <TestTube className="h-4 w-4" />}
                Test
              </Btn>
              {testResult !== null && (
                <span className={`flex items-center gap-1 text-sm font-medium ${testResult ? 'text-green-400' : 'text-red-400'}`}>
                  {testResult ? <Check className="h-4 w-4" /> : <X className="h-4 w-4" />}
                  {testResult ? 'Matched' : 'No match'}
                </span>
              )}
            </div>
          </div>

          {/* Actions */}
          <div className="flex gap-2">
            <Btn onClick={saveFormat}>
              <Save className="h-4 w-4" /> Save
            </Btn>
            <Btn variant="ghost" onClick={cancelEdit}>
              Cancel
            </Btn>
          </div>
        </div>
      </Card>
    )
  }

  // List view
  return (
    <Card>
      <div className="flex items-center justify-between mb-6">
        <h2 className="text-lg font-semibold text-white">Custom Formats</h2>
        <Btn onClick={startCreate}>
          <Plus className="h-4 w-4" /> Add Custom Format
        </Btn>
      </div>

      {formats.length === 0 ? (
        <p className="text-sm text-slate-400">
          No custom formats configured. Custom formats allow you to score releases based on
          regex patterns matching release titles, groups, quality, and more.
        </p>
      ) : (
        <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-3">
          {formats.map((cf) => (
            <div
              key={cf.id}
              onClick={() => startEdit(cf)}
              className="flex items-center justify-between rounded-lg border border-slate-700 bg-slate-900/50 px-4 py-3 cursor-pointer hover:border-slate-600 hover:bg-slate-800/50 transition-colors"
            >
              <div>
                <div className="text-sm font-medium text-white">{cf.name}</div>
                <div className="text-xs text-slate-500">
                  {cf.specifications.length} condition{cf.specifications.length !== 1 ? 's' : ''}
                </div>
              </div>
              <button
                onClick={(e) => {
                  e.stopPropagation()
                  void deleteFormat(cf.id)
                }}
                className="text-slate-500 hover:text-red-400 transition-colors p-1"
              >
                <Trash2 className="h-4 w-4" />
              </button>
            </div>
          ))}
        </div>
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
  priority: number
  fields: Record<string, string>
  definitionFile: string
}

const emptyIndexerForm: IndexerFormData = {
  name: '',
  indexerType: 'Newznab',
  protocol: 'Newznab',
  baseUrl: '',
  enabled: true,
  priority: 25,
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
      showToast('Failed to load indexers', 'error')
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
    // Map DB indexerType to form values that match Select options
    const iType = idx.indexerType || idx.protocol
    const formProtocol = iType === 'Cardigann' ? 'Cardigann'
      : iType === 'Torznab' ? 'Torznab' : 'Newznab'
    setForm({
      name: idx.name,
      indexerType: iType,
      protocol: formProtocol,
      baseUrl: idx.baseUrl,
      enabled: idx.enabled,
      priority: idx.priority ?? 25,
      fields: { ...idx.fields, apiKey: idx.apiKey ?? '' },
      definitionFile: '',
    })
    setShowForm(true)
  }

  const testUnsaved = async () => {
    setTesting(-1)
    try {
      const apiKeyValue = form.fields.apiKey || ''
      const isRedacted = apiKeyValue.includes('…')
      const res = await fetch(`${API}/indexer/test`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          name: form.name || 'test',
          indexerType: form.indexerType,
          baseUrl: form.baseUrl,
          protocol: form.indexerType === 'Newznab' ? 'usenet' : 'torrent',
          apiKey: isRedacted ? null : (apiKeyValue || null),
        }),
      })
      if (!res.ok) throw new Error('Test failed')
      const data: { success: boolean; message: string } = await res.json()
      if (data.success) {
        showToast(data.message || 'Test successful', 'success')
      } else {
        showToast(data.message || 'Test failed', 'error')
      }
    } catch {
      showToast('Test failed', 'error')
    } finally {
      setTesting(null)
    }
  }

  const saveIndexer = async () => {
    try {
      const method = editId ? 'PUT' : 'POST'
      const url = editId ? `${API}/indexer/${editId}` : `${API}/indexer`
      const apiKeyValue = form.fields.apiKey || ''
      // Don't send masked/redacted values back — they contain '…' from the backend redaction.
      // Sending null lets the backend COALESCE preserve the existing key.
      const isRedacted = apiKeyValue.includes('…')
      const isCardigann = form.indexerType === 'Cardigann'
      const body: Record<string, unknown> = {
        name: form.name,
        // For Cardigann edits, don't send indexerType — let COALESCE preserve it
        indexerType: isCardigann && editId ? null : form.indexerType,
        baseUrl: form.baseUrl,
        protocol: form.indexerType === 'Newznab' ? 'usenet' : 'torrent',
        enabled: form.enabled,
        priority: Math.max(1, Math.min(100, form.priority || 25)),
        apiKey: isRedacted ? null : (apiKeyValue || null),
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
      const data: { success: boolean; message: string; correctedUrl?: string } = await res.json()
      if (data.success) {
        showToast(data.message || 'Indexer test successful', 'success')
        // If the URL was auto-corrected, refresh the list to show updated URL
        if (data.correctedUrl) void load()
      } else {
        showToast(data.message || 'Indexer test failed', 'error')
      }
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
                <Btn variant="ghost" onClick={async () => {
                  setTesting(-1)
                  try {
                    const res = await fetch(`${API}/indexer/test`, {
                      method: 'POST',
                      headers: { 'Content-Type': 'application/json' },
                      body: JSON.stringify({
                        name: selectedDef.name,
                        indexerType: 'Cardigann',
                        baseUrl: selectedDef.urls[0] || '',
                        protocol: 'torrent',
                      }),
                    })
                    if (!res.ok) throw new Error('Test failed')
                    const data: { success: boolean; message: string } = await res.json()
                    showToast(data.success ? (data.message || 'Test successful') : (data.message || 'Test failed'), data.success ? 'success' : 'error')
                  } catch { showToast('Test failed', 'error') } finally { setTesting(null) }
                }} disabled={testing === -1}>
                  {testing === -1 ? <Loader2 className="h-4 w-4 animate-spin" /> : <TestTube className="h-4 w-4" />} Test
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
            {form.indexerType === 'Cardigann' ? (
              <div>
                <span className="mb-1 block text-sm font-medium text-slate-300">Type</span>
                <span className="inline-flex items-center rounded-lg border border-slate-600 bg-slate-700 px-3 py-2 text-sm text-slate-300">Cardigann</span>
              </div>
            ) : (
              <Select
                label="Type"
                value={form.indexerType}
                onChange={(v) => setForm({ ...form, protocol: v, indexerType: v })}
                options={[
                  { value: 'Newznab', label: 'Newznab' },
                  { value: 'Torznab', label: 'Torznab' },
                ]}
              />
            )}
            <Input label="URL" value={form.baseUrl} onChange={(v) => setForm({ ...form, baseUrl: v })} placeholder="https://..." />
            {form.indexerType !== 'Cardigann' && (
              <Input
                label="API Key"
                value={form.fields.apiKey ?? ''}
                onChange={(v) => setForm({ ...form, fields: { ...form.fields, apiKey: v } })}
                placeholder="API key"
              />
            )}
          </div>
          <div className="mt-4 flex items-center gap-4">
            <Toggle checked={form.enabled} onChange={(v) => setForm({ ...form, enabled: v })} label="Enabled" />
            <div className="w-32">
              <Input
                label="Priority (1-100)"
                value={String(form.priority)}
                onChange={(v) => setForm({ ...form, priority: Number(v) || 0 })}
                type="number"
              />
            </div>
          </div>
          <div className="mt-4 flex gap-2">
            <Btn onClick={saveIndexer}>
              <Save className="h-4 w-4" /> Save
            </Btn>
            {editId ? (
              <Btn variant="ghost" onClick={() => void testIndexer(editId)} disabled={testing === editId}>
                {testing === editId ? <Loader2 className="h-4 w-4 animate-spin" /> : <TestTube className="h-4 w-4" />} Test
              </Btn>
            ) : (
              <Btn variant="ghost" onClick={() => void testUnsaved()} disabled={testing === -1}>
                {testing === -1 ? <Loader2 className="h-4 w-4 animate-spin" /> : <TestTube className="h-4 w-4" />} Test
              </Btn>
            )}
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
              <th className="pb-3 pr-4 font-medium">Priority</th>
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
                <td className="py-3 pr-4 text-slate-300">{idx.priority}</td>
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
  priority: number
  fields: Record<string, string>
}

const emptyDlClientForm: DlClientFormData = {
  name: '',
  protocol: 'torrent',
  implementation: 'qBittorrent',
  host: 'localhost',
  port: 8080,
  enabled: true,
  priority: 5,
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
      showToast('Failed to load download clients', 'error')
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
    const cfg = (c.config ?? {}) as Record<string, unknown>
    setForm({
      name: c.name,
      protocol: c.protocol,
      implementation: c.clientType ?? '',
      host: String(cfg.host ?? 'localhost'),
      port: Number(cfg.port) || 8080,
      enabled: c.enabled,
      priority: c.priority ?? 5,
      fields: {},
    })
    setShowForm(true)
  }

  const saveClient = async () => {
    try {
      const method = editId ? 'PUT' : 'POST'
      const url = editId ? `${API}/downloadclient/${editId}` : `${API}/downloadclient`
      const body = {
        name: form.name,
        clientType: form.implementation.toLowerCase(),
        protocol: form.protocol,
        config: { host: form.host, port: form.port, ...form.fields },
        enabled: form.enabled,
        priority: Math.max(1, Math.min(10, form.priority || 5)),
      }
      const res = await fetch(url, {
        method,
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(body),
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
          <div className="mt-4 flex items-center gap-4">
            <Toggle checked={form.enabled} onChange={(v) => setForm({ ...form, enabled: v })} label="Enabled" />
            <div className="w-32">
              <Input
                label="Priority (1-10)"
                value={String(form.priority)}
                onChange={(v) => setForm({ ...form, priority: Number(v) || 0 })}
                type="number"
              />
            </div>
          </div>
          <div className="mt-4 flex gap-2">
            <Btn onClick={saveClient}>
              <Save className="h-4 w-4" /> Save
            </Btn>
            {editId && (
              <Btn variant="ghost" onClick={() => void testClient(editId)} disabled={testing === editId}>
                {testing === editId ? <Loader2 className="h-4 w-4 animate-spin" /> : <TestTube className="h-4 w-4" />} Test
              </Btn>
            )}
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
              <th className="pb-3 pr-4 font-medium">Protocol</th>
              <th className="pb-3 pr-4 font-medium">Priority</th>
              <th className="pb-3 pr-4 font-medium">Enabled</th>
              <th className="pb-3 font-medium" />
            </tr>
          </thead>
          <tbody>
            {clients.map((c) => {
              const isEmbedded = c.id < 0
              return (
              <tr key={c.id} className="border-b border-slate-700/50 hover:bg-slate-700/50 transition-colors">
                <td className="py-3 pr-4 text-white">
                  {c.name}
                  {isEmbedded && <span className="ml-2 rounded bg-slate-600 px-1.5 py-0.5 text-[10px] font-medium text-slate-300">BUILT-IN</span>}
                </td>
                <td className="py-3 pr-4 text-slate-300">{isEmbedded ? (c.protocol === 'torrent' ? 'librtbit' : 'rustnzb') : c.clientType}</td>
                <td className="py-3 pr-4">
                  <span className={`rounded-full px-2 py-0.5 text-xs font-medium ${
                    c.protocol === 'torrent' ? 'bg-orange-500/20 text-orange-400' : 'bg-blue-500/20 text-blue-400'
                  }`}>
                    {c.protocol === 'torrent' ? 'Torrent' : 'Usenet'}
                  </span>
                </td>
                <td className="py-3 pr-4 text-slate-300">
                  {isEmbedded ? (
                    <input
                      type="number"
                      min={0}
                      max={10}
                      value={c.priority}
                      onChange={async (e) => {
                        const priority = Math.max(0, Math.min(10, Number(e.target.value) || 0))
                        try {
                          const res = await fetch(`${API}/downloadclient/${c.id}`, {
                            method: 'PUT',
                            headers: { 'Content-Type': 'application/json' },
                            body: JSON.stringify({ priority }),
                          })
                          if (!res.ok) throw new Error('Update failed')
                          void load()
                        } catch {
                          showToast('Failed to update priority', 'error')
                        }
                      }}
                      className="w-16 rounded bg-slate-700 border border-slate-600 px-2 py-0.5 text-sm text-slate-200 focus:border-blue-500 focus:outline-none"
                    />
                  ) : (
                    c.priority
                  )}
                </td>
                <td className="py-3 pr-4">
                  {isEmbedded
                    ? <span className={`text-xs font-medium ${c.enabled ? 'text-green-400' : 'text-slate-500'}`}>{c.enabled ? 'Running' : 'Stopped'}</span>
                    : <Toggle checked={c.enabled} onChange={() => void toggleEnabled(c)} />
                  }
                </td>
                <td className="py-3 text-right">
                  {!isEmbedded && (
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
                  )}
                </td>
              </tr>
              )
            })}
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

// ── Folder Picker (uses filebrowser API) ──────────────────────────────────

function FolderPicker({ value, onChange, onClose }: {
  value: string
  onChange: (path: string) => void
  onClose: () => void
}) {
  const [entries, setEntries] = useState<Array<{ name: string; path: string; isDir: boolean }>>([])
  const [currentPath, setCurrentPath] = useState(value || '/')
  const [parentPath, setParentPath] = useState<string | null>(null)
  const [loading, setLoading] = useState(true)

  const browse = useCallback(async (path: string) => {
    setLoading(true)
    try {
      const url = path === '/'
        ? `${API}/filebrowser/browse?mode=media`
        : `${API}/filebrowser/browse?mode=media&path=${encodeURIComponent(path)}`
      const res = await fetch(url)
      if (!res.ok) throw new Error('Browse failed')
      const data = await res.json()
      setEntries((data.entries ?? []).filter((e: { isDir: boolean }) => e.isDir))
      setCurrentPath(data.path ?? path)
      setParentPath(data.parent ?? null)
    } catch {
      setEntries([])
    } finally {
      setLoading(false)
    }
  }, [])

  useEffect(() => {
    void browse(value || '/')
  }, [browse, value])

  return (
    <div className="rounded-lg border border-slate-500 bg-slate-800 p-3">
      <div className="mb-2 flex items-center justify-between">
        <span className="text-xs font-medium text-slate-300 truncate flex-1">{currentPath}</span>
        <button onClick={onClose} className="ml-2 text-slate-400 hover:text-white"><X className="h-4 w-4" /></button>
      </div>
      <div className="mb-2 flex gap-1">
        {parentPath && (
          <button
            onClick={() => void browse(parentPath)}
            className="flex items-center gap-1 rounded px-2 py-1 text-xs text-slate-300 hover:bg-slate-700"
          >
            <ArrowUp className="h-3 w-3" /> Up
          </button>
        )}
        {!parentPath && currentPath !== '/' && (
          <button
            onClick={() => void browse('/')}
            className="flex items-center gap-1 rounded px-2 py-1 text-xs text-slate-300 hover:bg-slate-700"
          >
            <ArrowUp className="h-3 w-3" /> Roots
          </button>
        )}
      </div>
      {loading ? (
        <div className="flex items-center gap-2 py-4 justify-center text-slate-400 text-xs">
          <Loader2 className="h-4 w-4 animate-spin" /> Loading...
        </div>
      ) : entries.length === 0 ? (
        <p className="py-3 text-center text-xs text-slate-500">No subdirectories</p>
      ) : (
        <div className="max-h-48 overflow-y-auto space-y-0.5">
          {entries.map((e) => (
            <button
              key={e.path}
              onClick={() => void browse(e.path)}
              className="flex w-full items-center gap-2 rounded px-2 py-1.5 text-left text-xs hover:bg-slate-700 transition-colors group"
            >
              <Folder className="h-3.5 w-3.5 text-yellow-500 shrink-0" />
              <span className="text-slate-300 group-hover:text-white truncate">{e.name}</span>
            </button>
          ))}
        </div>
      )}
      <div className="mt-2 flex gap-2">
        <button
          onClick={() => { onChange(currentPath); onClose() }}
          className="flex-1 rounded bg-blue-600 px-3 py-1.5 text-xs font-medium text-white hover:bg-blue-500"
        >
          Select "{currentPath}"
        </button>
      </div>
    </div>
  )
}

function MediaLibraryFoldersTab({
  showToast,
}: {
  showToast: (msg: string, type: 'success' | 'error') => void
}) {
  const [folders, setFolders] = useState<MediaLibraryFolder[]>([])
  const [loading, setLoading] = useState(true)
  const [showForm, setShowForm] = useState(false)
  const [showBrowser, setShowBrowser] = useState(false)
  const [newPath, setNewPath] = useState('')
  const [newMediaType, setNewMediaType] = useState<'tv' | 'movie'>('tv')
  const [scanning, setScanning] = useState(false)

  const scanLibrary = async () => {
    setScanning(true)
    try {
      const res = await fetch(`${API}/scheduler/tasks/disk_scan/trigger`, {
        method: 'POST',
      })
      if (!res.ok) throw new Error(`HTTP ${res.status}`)
      showToast('Library scan started — watch Activity for progress', 'success')
    } catch (e) {
      showToast(`Failed to start library scan: ${e instanceof Error ? e.message : String(e)}`, 'error')
    } finally {
      setScanning(false)
    }
  }

  const load = useCallback(async () => {
    setLoading(true)
    try {
      const res = await fetch(`${API}/medialibraryfolder`)
      const data: MediaLibraryFolder[] = await res.json()
      setFolders(data)
    } catch {
      showToast('Failed to load media library folders', 'error')
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
        <div className="flex gap-2">
          <Btn onClick={scanLibrary} disabled={scanning}>
            {scanning ? (
              <Loader2 className="h-4 w-4 animate-spin" />
            ) : (
              <RefreshCcw className="h-4 w-4" />
            )}
            {scanning ? 'Starting…' : 'Scan Library'}
          </Btn>
          <Btn onClick={() => setShowForm(true)}>
            <Plus className="h-4 w-4" /> Add Folder
          </Btn>
        </div>
      </div>

      {showForm && (
        <div className="mb-6 rounded-lg border border-slate-600 bg-slate-700/50 p-4">
          <h3 className="mb-4 text-sm font-semibold text-white">Add Media Library Folder</h3>
          <div className="grid grid-cols-1 gap-4 sm:grid-cols-2 max-w-xl">
            <div>
              <span className="mb-1 block text-sm font-medium text-slate-300">Path</span>
              <div className="flex gap-2">
                <input
                  type="text"
                  value={newPath}
                  onChange={(e) => setNewPath(e.target.value)}
                  placeholder="/media/tv"
                  className="flex-1 rounded-lg border border-slate-600 bg-slate-700 px-3 py-2 text-sm text-white placeholder-slate-500 focus:border-blue-500 focus:outline-none focus:ring-1 focus:ring-blue-500"
                />
                <button
                  onClick={() => setShowBrowser(!showBrowser)}
                  className="rounded-lg border border-slate-600 bg-slate-700 px-3 py-2 text-slate-300 hover:bg-slate-600 hover:text-white transition-colors"
                  title="Browse folders"
                >
                  <FolderOpen className="h-4 w-4" />
                </button>
              </div>
            </div>
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
          {showBrowser && (
            <div className="mt-3 max-w-xl">
              <FolderPicker
                value={newPath}
                onChange={(p) => setNewPath(p)}
                onClose={() => setShowBrowser(false)}
              />
            </div>
          )}
          <div className="mt-4 flex gap-2">
            <Btn onClick={addFolder}>
              <Plus className="h-4 w-4" /> Add
            </Btn>
            <Btn variant="ghost" onClick={() => { setShowForm(false); setShowBrowser(false) }}>
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
                  {formatBytes(f.freeSpace)}{f.totalSpace ? ` / ${formatBytes(f.totalSpace)}` : ''} free
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
      <h2 className="mb-2 text-lg font-semibold text-white">Tags</h2>
      <p className="mb-6 text-sm text-slate-400">
        Tags let you organize and group your series and movies. Assign tags to media to filter your library,
        apply bulk actions, or link specific indexers and download clients to tagged content.
      </p>

      <div className="mb-6 flex items-end gap-3 max-w-md">
        <div className="flex-1">
          <Input label="New Tag" value={newLabel} onChange={setNewLabel} placeholder="Tag name" />
        </div>
        <Btn onClick={addTag} disabled={!newLabel.trim()}>
          <Plus className="h-4 w-4" /> Add
        </Btn>
      </div>

      {tags.length === 0 ? (
        <p className="text-sm text-slate-400">No tags yet. Create one above to start organizing your library.</p>
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
        {activeTab === 'customformats' && <CustomFormatsTab showToast={showToast} />}
        {activeTab === 'indexers' && <IndexersTab showToast={showToast} />}
        {activeTab === 'downloadclients' && <DownloadClientsTab showToast={showToast} />}
        {activeTab === 'naming' && <NamingTab showToast={showToast} />}
        {activeTab === 'medialibraryfolders' && <MediaLibraryFoldersTab showToast={showToast} />}
        {activeTab === 'tags' && <TagsTab showToast={showToast} />}
        {activeTab === 'plex' && <PlexTab showToast={showToast} />}
        {activeTab === 'bootstrap' && <BootstrapTab showToast={showToast} />}
        {activeTab === 'backup' && <BackupRestoreTab showToast={showToast} />}
        {activeTab === 'migration' && <MigrationTab showToast={showToast} />}
      </div>

      {/* Toast Notification */}
      {toast && <Toast toast={toast} onDismiss={dismissToast} />}
    </div>
  )
}

// ---------------------------------------------------------------------------
// Bootstrap / Remote Access Tab
// ---------------------------------------------------------------------------

function BootstrapTab({ showToast }: { showToast: (msg: string, type: 'success' | 'error') => void }) {
  const [enabled, setEnabled] = useState(false)
  const [url, setUrl] = useState('')
  const [token, setToken] = useState('')
  const [advertisePort, setAdvertisePort] = useState('')
  const [upnpEnabled, setUpnpEnabled] = useState(false)
  const [discoveryName, setDiscoveryName] = useState('')
  const [nameAvailable, setNameAvailable] = useState<boolean | null>(null)
  const [checkingName, setCheckingName] = useState(false)
  const [saving, setSaving] = useState(false)

  // Port forward test state
  const [portTestResult, setPortTestResult] = useState<{
    reachable: boolean; publicIp?: string; port?: number;
    latencyMs?: number; error?: string
  } | null>(null)
  const [portTesting, setPortTesting] = useState(false)

  // Bootstrap name registration state
  const [nameStatus, setNameStatus] = useState<{ enabled: boolean; nameRegistered: boolean; serverName: string } | null>(null)
  const [recoveryPhrase, setRecoveryPhrase] = useState<string | null>(null)
  const [registering, setRegistering] = useState(false)
  const [recoverMode, setRecoverMode] = useState(false)
  const [recoverName, setRecoverName] = useState('')
  const [recoverPhrase, setRecoverPhrase] = useState('')
  const [recovering, setRecovering] = useState(false)

  useEffect(() => {
    // Fetch bootstrap config
    fetch(`${API}/config/bootstrap`)
      .then((r) => r.json())
      .then((d: { enabled?: boolean; url?: string; token?: string; advertisePort?: number; upnpEnabled?: boolean; discoveryName?: string }) => {
        setEnabled(d.enabled ?? false)
        setUrl(d.url ?? '')
        setToken(d.token ?? '')
        setAdvertisePort(d.advertisePort ? String(d.advertisePort) : '')
        setUpnpEnabled(d.upnpEnabled ?? false)
        setDiscoveryName(d.discoveryName ?? '')
      })
      .catch(() => {})

    // Fetch bootstrap name status
    fetch(`${API}/admin/bootstrap/status`)
      .then((r) => (r.ok ? r.json() : null))
      .then((d: { enabled: boolean; nameRegistered: boolean; serverName: string } | null) => d && setNameStatus(d))
      .catch(() => {})
  }, [])

  const save = async () => {
    setSaving(true)
    try {
      const res = await fetch(`${API}/config/bootstrap`, {
        method: 'PUT',
        headers: { 'Content-Type': 'application/json' },
        credentials: 'include',
        body: JSON.stringify({
          enabled,
          url: url || undefined,
          token: token || undefined,
          advertisePort: advertisePort ? Number(advertisePort) : null,
          upnpEnabled,
          discoveryName: discoveryName || undefined,
        }),
      })
      if (!res.ok) throw new Error('Save failed')
      showToast('Bootstrap settings saved', 'success')
    } catch {
      showToast('Failed to save bootstrap settings', 'error')
    } finally {
      setSaving(false)
    }
  }

  const registerName = async () => {
    setRegistering(true)
    setRecoveryPhrase(null)
    try {
      const res = await fetch(`${API}/admin/bootstrap/register-name`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        credentials: 'include',
        body: JSON.stringify({}),
      })
      if (!res.ok) throw new Error('Registration failed')
      const data = await res.json()
      setRecoveryPhrase(data.recoveryPhrase)
      setNameStatus((prev) => (prev ? { ...prev, nameRegistered: true } : prev))
      showToast('Server name registered', 'success')
    } catch (e) {
      showToast(e instanceof Error ? e.message : 'Failed to register name', 'error')
    } finally {
      setRegistering(false)
    }
  }

  const doRecover = async () => {
    setRecovering(true)
    try {
      const res = await fetch(`${API}/admin/bootstrap/recover-name`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        credentials: 'include',
        body: JSON.stringify({ serverName: recoverName, recoveryPhrase: recoverPhrase }),
      })
      if (!res.ok) {
        const body = await res.json().catch(() => ({ error: 'Recovery failed' }))
        throw new Error(body.error || 'Recovery failed')
      }
      const data = await res.json()
      setRecoveryPhrase(data.recoveryPhrase)
      setRecoverMode(false)
      setNameStatus((prev) => (prev ? { ...prev, nameRegistered: true } : prev))
      showToast('Server name recovered! Save your new recovery phrase.', 'success')
    } catch (e) {
      showToast(e instanceof Error ? e.message : 'Recovery failed', 'error')
    } finally {
      setRecovering(false)
    }
  }

  const checkNameAvailability = async () => {
    if (!discoveryName.trim()) return
    setCheckingName(true)
    setNameAvailable(null)
    try {
      const res = await fetch(`${API}/admin/bootstrap/check-name/${encodeURIComponent(discoveryName.trim())}`, { credentials: 'include' })
      if (!res.ok) throw new Error('Check failed')
      const data = await res.json()
      setNameAvailable(data.available ?? false)
    } catch {
      showToast('Failed to check name availability', 'error')
    } finally {
      setCheckingName(false)
    }
  }

  const testPort = async () => {
    setPortTesting(true)
    setPortTestResult(null)
    try {
      const res = await fetch(`${API}/admin/bootstrap/check-port`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        credentials: 'include',
      })
      if (!res.ok) throw new Error('Port check failed')
      const data = await res.json()
      setPortTestResult(data)
    } catch {
      showToast('Failed to test port forward', 'error')
    } finally {
      setPortTesting(false)
    }
  }

  return (
    <div className="space-y-6">
      <Card>
        <h2 className="mb-6 text-lg font-semibold text-white">Remote Access / Bootstrap</h2>
        <p className="mb-4 text-sm text-slate-400">
          Bootstrap enables remote clients to discover your server by name. Configure the connection to the bootstrap
          discovery node.
        </p>
        <div className="space-y-4 max-w-lg">
          <Toggle checked={enabled} onChange={setEnabled} label="Enable Bootstrap" />
          <div>
            <Input
              label="Discovery Name"
              value={discoveryName}
              onChange={(v) => { setDiscoveryName(v); setNameAvailable(null) }}
              placeholder="Unique name for clients to find your server"
            />
            <p className="mt-1 text-xs text-slate-500">
              Unique name for clients to find your server. This is what users type when connecting.
            </p>
            <div className="mt-2 flex items-center gap-3">
              <Btn onClick={checkNameAvailability} disabled={checkingName || !discoveryName.trim()} variant="ghost">
                {checkingName ? <Loader2 className="h-4 w-4 animate-spin" /> : <Search className="h-4 w-4" />}
                Check Availability
              </Btn>
              {nameAvailable === true && (
                <span className="text-sm text-green-400 flex items-center gap-1">
                  <CheckCircle className="h-4 w-4" /> Available
                </span>
              )}
              {nameAvailable === false && (
                <span className="text-sm text-red-400 flex items-center gap-1">
                  <XCircle className="h-4 w-4" /> Name taken
                </span>
              )}
            </div>
          </div>
          <Input
            label="Bootstrap URL"
            value={url}
            onChange={setUrl}
            placeholder="https://streambootstrap.indexarr.net"
          />
          <Input label="Bootstrap Token" value={token} onChange={setToken} placeholder="Secret token" type="password" />
          <Input
            label="Advertise Port"
            value={advertisePort}
            onChange={setAdvertisePort}
            placeholder="Auto (same as server port)"
            type="number"
          />
          <p className="text-xs text-slate-500">
            The external port clients will use to connect. Set this to your router's forwarded port if different from the
            server's listen port.
          </p>
          <Toggle checked={upnpEnabled} onChange={setUpnpEnabled} label="UPnP Auto Port Forward" />
          <p className="text-xs text-slate-500">
            Automatically forward the advertise port via UPnP on your router. Requires a UPnP-capable router.
          </p>
          <div className="flex items-center gap-3">
            <Btn onClick={testPort} disabled={portTesting} variant="ghost">
              {portTesting ? <Loader2 className="h-4 w-4 animate-spin" /> : <Globe className="h-4 w-4" />}
              Test Port Forward
            </Btn>
            {portTestResult && (
              portTestResult.reachable ? (
                <span className="text-sm text-green-400">
                  Port {portTestResult.port} reachable from {portTestResult.publicIp} ({portTestResult.latencyMs}ms)
                </span>
              ) : (
                <span className="text-sm text-red-400">
                  Not reachable from {portTestResult.publicIp}:{portTestResult.port} — {portTestResult.error}
                </span>
              )
            )}
          </div>
          <Btn onClick={save} disabled={saving}>
            {saving ? <Loader2 className="h-4 w-4 animate-spin" /> : <Save className="h-4 w-4" />}
            Save
          </Btn>
        </div>
      </Card>

      {/* Server Name Registration */}
      {nameStatus?.enabled && (
        <Card>
          <h2 className="mb-4 text-lg font-semibold text-white">Server Name</h2>
          <p className="mb-2 text-sm text-slate-400">
            Your server name: <span className="text-white font-medium">{nameStatus.serverName}</span>
          </p>

          {nameStatus.nameRegistered ? (
            <p className="text-sm text-green-400">Name registered with bootstrap node.</p>
          ) : (
            <div className="space-y-3">
              <p className="text-sm text-amber-400">
                Your server name is not yet registered. Register it to enable name-based discovery and recovery.
              </p>
              <Btn onClick={registerName} disabled={registering}>
                {registering ? <Loader2 className="h-4 w-4 animate-spin" /> : null}
                Register Server Name
              </Btn>
            </div>
          )}

          {/* Recovery phrase display (shown once after registration) */}
          {recoveryPhrase && (
            <div className="mt-4 rounded-lg border border-amber-600 bg-amber-950/50 p-4">
              <h3 className="mb-2 text-sm font-semibold text-amber-400">Recovery Phrase -- Save This!</h3>
              <p className="text-xs text-amber-200/70 mb-2">
                This phrase is shown only once. You will need it to reclaim your server name if you rebuild your server.
              </p>
              <code className="block rounded bg-slate-900 px-3 py-2 text-sm text-white font-mono select-all">
                {recoveryPhrase}
              </code>
            </div>
          )}

          {/* Recovery mode */}
          <div className="mt-4">
            {!recoverMode ? (
              <button onClick={() => setRecoverMode(true)} className="text-xs text-slate-500 hover:text-slate-300">
                Recover a server name...
              </button>
            ) : (
              <div className="space-y-3 max-w-lg">
                <Input
                  label="Server Name to Recover"
                  value={recoverName}
                  onChange={setRecoverName}
                  placeholder="MyServer"
                />
                <Input
                  label="Recovery Phrase"
                  value={recoverPhrase}
                  onChange={setRecoverPhrase}
                  placeholder="word1 word2 word3 ..."
                />
                <div className="flex gap-2">
                  <Btn onClick={doRecover} disabled={recovering}>
                    {recovering ? <Loader2 className="h-4 w-4 animate-spin" /> : null}
                    Recover
                  </Btn>
                  <Btn onClick={() => setRecoverMode(false)} variant="ghost">
                    Cancel
                  </Btn>
                </div>
              </div>
            )}
          </div>
        </Card>
      )}
    </div>
  )
}

// ---------------------------------------------------------------------------
// Plex Tab
// ---------------------------------------------------------------------------

function WebhookUrlDisplay({ serverId }: { serverId: number }) {
  const { data } = useQuery({
    queryKey: ['plex', 'webhook-url', serverId],
    queryFn: () => apiFetch<{ webhookUrl: string }>(`/plex/servers/${serverId}/webhook-url`),
    staleTime: 60 * 60 * 1000,
  })
  if (!data?.webhookUrl) return null
  const fullUrl = `${window.location.origin}${data.webhookUrl}`
  return (
    <p
      className="text-[10px] text-slate-500 truncate max-w-xs cursor-pointer hover:text-slate-400"
      title={`Click to copy. Add to Plex Settings → Webhooks:\n${fullUrl}`}
      onClick={() => { void navigator.clipboard.writeText(fullUrl) }}
    >
      Webhook: {fullUrl}
    </p>
  )
}

function PlexTab({ showToast }: { showToast: (msg: string, type: 'success' | 'error') => void }) {
  const { data: status } = useSystemStatus()
  const plexEnabled = status?.modules.plexIntegration ?? false

  // ── Token validation / server discovery state ──
  const [token, setToken] = useState(() => sessionStorage.getItem('plex_token') ?? '')
  const [validatedUser, setValidatedUser] = useState<{ username: string; thumb: string | null } | null>(() => {
    try { const s = sessionStorage.getItem('plex_user'); return s ? JSON.parse(s) : null } catch { return null }
  })
  const [discoveredServers, setDiscoveredServers] = useState<Array<{ name: string; clientIdentifier: string; connections: Array<{ uri: string; local: boolean; protocol: string }> }>>(() => {
    try { const s = sessionStorage.getItem('plex_servers'); return s ? JSON.parse(s) : [] } catch { return [] }
  })
  const [validating, setValidating] = useState(false)
  const [showManualToken, setShowManualToken] = useState(false)
  const [oauthStatus, setOauthStatus] = useState<string | null>(null)

  // Persist Plex session to sessionStorage
  useEffect(() => {
    if (token) sessionStorage.setItem('plex_token', token); else sessionStorage.removeItem('plex_token')
  }, [token])
  useEffect(() => {
    if (validatedUser) sessionStorage.setItem('plex_user', JSON.stringify(validatedUser)); else sessionStorage.removeItem('plex_user')
  }, [validatedUser])
  useEffect(() => {
    if (discoveredServers.length) sessionStorage.setItem('plex_servers', JSON.stringify(discoveredServers)); else sessionStorage.removeItem('plex_servers')
  }, [discoveredServers])

  // ── Add server form ──
  const [showAddForm, setShowAddForm] = useState(false)
  const [addName, setAddName] = useState('')
  const [addIp, setAddIp] = useState('')
  const [addPort, setAddPort] = useState('32400')
  const [addSsl, setAddSsl] = useState(false)
  const [addToken, setAddToken] = useState('')

  // ── Edit server ──
  const [editId, setEditId] = useState<number | null>(null)
  const [editName, setEditName] = useState('')
  const [editIp, setEditIp] = useState('')
  const [editPort, setEditPort] = useState('32400')
  const [editSsl, setEditSsl] = useState(false)
  const [editToken, setEditToken] = useState('')

  // ── Library expansion per server ──
  const [expandedServer, setExpandedServer] = useState<number | null>(null)

  // ── Scanning state ──
  const [scanning, setScanning] = useState(false)

  // ── Data hooks ──
  const { data: servers, isLoading: serversLoading } = usePlexServers()
  const { data: libraries, isLoading: libsLoading } = usePlexLibraries(expandedServer ?? 0)
  const addServer = useAddPlexServer()
  const updateServer = useUpdatePlexServer()
  const deleteServer = useDeletePlexServer()
  const toggleLibrary = useTogglePlexLibrary()
  const fullScan = usePlexFullScan()
  const recentScan = usePlexRecentScan()

  if (!plexEnabled) {
    return (
      <Card>
        <h2 className="mb-4 text-lg font-semibold text-white">Plex Integration</h2>
        <p className="text-sm text-slate-400">
          Plex integration is disabled. Enable it in the <strong>Modules</strong> tab to configure Plex servers.
        </p>
      </Card>
    )
  }

  const handleValidateToken = async () => {
    if (!token.trim()) return
    setValidating(true)
    setValidatedUser(null)
    setDiscoveredServers([])
    try {
      const res = await fetch(`${API}/plex/auth/validate`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json', ...authHeaders() },
        body: JSON.stringify({ authToken: token }),
      })
      if (!res.ok) {
        showToast('Invalid Plex token', 'error')
        setValidating(false)
        return
      }
      const data = await res.json()
      setValidatedUser({ username: data.user.username, thumb: data.user.thumb })

      // Discover servers
      const srvRes = await fetch(`${API}/plex/auth/servers`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json', ...authHeaders() },
        body: JSON.stringify({ authToken: token }),
      })
      if (srvRes.ok) {
        const srvData = await srvRes.json()
        const serverResources = (srvData as Array<{ provides: string; name: string; clientIdentifier: string; connections: Array<{ uri: string; local: boolean; protocol: string }> }>).filter((r) => r.provides.includes('server'))
        setDiscoveredServers(serverResources)
      }
      showToast(`Authenticated as ${data.user.username}`, 'success')
    } catch {
      showToast('Failed to validate token', 'error')
    } finally {
      setValidating(false)
    }
  }

  const handlePlexOAuth = async () => {
    setValidating(true)
    setOauthStatus('Creating PIN...')
    setValidatedUser(null)
    setDiscoveredServers([])

    const clientId = typeof crypto.randomUUID === 'function'
      ? crypto.randomUUID()
      : 'xxxxxxxx-xxxx-4xxx-yxxx-xxxxxxxxxxxx'.replace(/[xy]/g, (c) => {
          const r = (Math.random() * 16) | 0
          return (c === 'x' ? r : (r & 0x3) | 0x8).toString(16)
        })

    try {
      // Create PIN
      const pinRes = await fetch(`${API}/plex/auth/pin`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json', ...authHeaders() },
        body: JSON.stringify({ clientId }),
      })
      if (!pinRes.ok) throw new Error('Failed to create PIN')
      const pin: { id: number; code: string } = await pinRes.json()

      // Open popup
      const authUrl = `https://app.plex.tv/auth#?clientID=${encodeURIComponent(clientId)}&code=${encodeURIComponent(pin.code)}&context%5Bdevice%5D%5Bproduct%5D=StackArr&forwardUrl=${encodeURIComponent('https://app.plex.tv')}`
      const popup = window.open(authUrl, 'PlexAuth', 'width=800,height=600')

      setOauthStatus('Waiting for authorization...')

      // Poll for token
      let authToken: string | null = null
      for (let i = 0; i < 120; i++) {
        await new Promise((r) => setTimeout(r, 1000))
        const checkRes = await fetch(`${API}/plex/auth/pin/${pin.id}?clientId=${encodeURIComponent(clientId)}`, { headers: authHeaders() })
        if (checkRes.ok) {
          const checkData: { authToken: string | null } = await checkRes.json()
          if (checkData.authToken) {
            authToken = checkData.authToken
            break
          }
        }
        if (popup?.closed) break
      }

      popup?.close()

      if (!authToken) {
        showToast('Authorization timed out', 'error')
        setOauthStatus(null)
        setValidating(false)
        return
      }

      // Set the token and trigger validation + discovery
      setToken(authToken)
      setOauthStatus('Discovering servers...')

      // Validate
      const valRes = await fetch(`${API}/plex/auth/validate`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json', ...authHeaders() },
        body: JSON.stringify({ authToken }),
      })
      if (valRes.ok) {
        const data = await valRes.json()
        setValidatedUser({ username: data.user.username, thumb: data.user.thumb })
        showToast(`Authenticated as ${data.user.username}`, 'success')
      }

      // Discover servers
      const srvRes = await fetch(`${API}/plex/auth/servers`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json', ...authHeaders() },
        body: JSON.stringify({ authToken }),
      })
      if (srvRes.ok) {
        const srvData = await srvRes.json()
        const serverResources = (srvData as Array<{ provides: string; name: string; clientIdentifier: string; connections: Array<{ uri: string; local: boolean; protocol: string }> }>).filter((r) => r.provides.includes('server'))
        setDiscoveredServers(serverResources)
      }

      setOauthStatus(null)
    } catch {
      showToast('Plex sign-in failed', 'error')
      setOauthStatus(null)
    } finally {
      setValidating(false)
    }
  }

  const handleQuickAdd = async (srv: typeof discoveredServers[0], conn: typeof discoveredServers[0]['connections'][0]) => {
    try {
      const url = new URL(conn.uri)
      await addServer.mutateAsync({
        name: srv.name,
        ip: url.hostname,
        port: parseInt(url.port) || 32400,
        useSsl: url.protocol === 'https:',
        authToken: token,
      })
      showToast(`Added ${srv.name}`, 'success')
    } catch (e) {
      showToast(`Failed to add ${srv.name}: ${e instanceof Error ? e.message : 'unknown error'}`, 'error')
    }
  }

  const handleAddManual = async () => {
    if (!addIp.trim() || !addToken.trim()) {
      showToast('IP and token are required', 'error')
      return
    }
    try {
      await addServer.mutateAsync({
        name: addName || undefined,
        ip: addIp,
        port: parseInt(addPort) || 32400,
        useSsl: addSsl,
        authToken: addToken,
      })
      showToast('Plex server added', 'success')
      setShowAddForm(false)
      setAddName('')
      setAddIp('')
      setAddPort('32400')
      setAddSsl(false)
      setAddToken('')
    } catch {
      showToast('Failed to add server — check connection details', 'error')
    }
  }

  const handleUpdate = async () => {
    if (editId == null) return
    try {
      await updateServer.mutateAsync({
        id: editId,
        name: editName || undefined,
        ip: editIp || undefined,
        port: parseInt(editPort) || undefined,
        useSsl: editSsl,
        authToken: editToken || undefined,
      })
      showToast('Server updated', 'success')
      setEditId(null)
    } catch {
      showToast('Failed to update server', 'error')
    }
  }

  const handleDelete = async (id: number, name: string) => {
    if (!confirm(`Delete Plex server "${name}"?`)) return
    try {
      await deleteServer.mutateAsync(id)
      showToast('Server deleted', 'success')
      if (expandedServer === id) setExpandedServer(null)
    } catch {
      showToast('Failed to delete server', 'error')
    }
  }

  const handleToggleLibrary = async (libId: number, enabled: boolean) => {
    try {
      await toggleLibrary.mutateAsync({ id: libId, enabled })
    } catch {
      showToast('Failed to toggle library', 'error')
    }
  }

  const handleFullScan = async () => {
    setScanning(true)
    try {
      await fullScan.mutateAsync()
      showToast('Full scan started', 'success')
    } catch {
      showToast('Failed to start scan', 'error')
    } finally {
      setScanning(false)
    }
  }

  const handleRecentScan = async () => {
    setScanning(true)
    try {
      await recentScan.mutateAsync()
      showToast('Recent scan started', 'success')
    } catch {
      showToast('Failed to start scan', 'error')
    } finally {
      setScanning(false)
    }
  }

  return (
    <div className="space-y-6">
      {/* Token validation & discovery */}
      <Card>
        <h2 className="mb-4 text-lg font-semibold text-white">Plex Account</h2>
        <p className="mb-4 text-sm text-slate-400">
          Sign in with your Plex account to automatically discover and add servers.
        </p>

        <div className="flex flex-wrap items-center gap-3">
          <Btn onClick={handlePlexOAuth} disabled={validating}>
            {validating ? <Loader2 className="h-4 w-4 animate-spin" /> : <Shield className="h-4 w-4" />}
            {oauthStatus ?? 'Sign in with Plex'}
          </Btn>
          <button
            onClick={() => setShowManualToken(!showManualToken)}
            className="text-xs text-slate-500 hover:text-slate-400 transition-colors"
          >
            {showManualToken ? 'Hide manual token' : 'Enter token manually'}
          </button>
        </div>

        {showManualToken && (
          <div className="mt-3 flex gap-3 max-w-xl">
            <input
              type="password"
              value={token}
              onChange={(e) => setToken(e.target.value)}
              placeholder="Plex auth token"
              className="flex-1 rounded-lg border border-slate-600 bg-slate-700 px-3 py-2 text-sm text-white placeholder-slate-400 focus:border-blue-500 focus:outline-none focus:ring-1 focus:ring-blue-500"
            />
            <Btn onClick={handleValidateToken} disabled={validating || !token.trim()}>
              Validate
            </Btn>
          </div>
        )}

        {validatedUser && (
          <div className="mt-4 flex items-center gap-3 rounded-lg border border-slate-600 bg-slate-700/50 p-3">
            {validatedUser.thumb && (
              <img src={validatedUser.thumb} alt="" className="h-8 w-8 rounded-full" />
            )}
            <div>
              <p className="text-sm font-medium text-white">{validatedUser.username}</p>
              <p className="text-xs text-slate-400">Authenticated</p>
            </div>
            <CheckCircle className="ml-auto h-5 w-5 text-green-400" />
          </div>
        )}

        {discoveredServers.length > 0 && (
          <div className="mt-4">
            <h3 className="mb-2 text-sm font-medium text-slate-300">Discovered Servers</h3>
            <div className="space-y-2">
              {discoveredServers.map((srv) => {
                const alreadyAdded = servers?.some(
                  (s) => s.machineId === srv.clientIdentifier,
                )
                return srv.connections.map((conn, ci) => {
                  let host = conn.uri
                  try { host = new URL(conn.uri).host } catch { /* use raw uri */ }
                  return (
                    <div
                      key={`${srv.clientIdentifier}-${ci}`}
                      className="flex items-center justify-between rounded-lg border border-slate-600 bg-slate-700/50 p-3"
                    >
                      <div>
                        <p className="text-sm font-medium text-white">{srv.name}</p>
                        <p className="text-xs text-slate-400">
                          {host}
                          {conn.local && <span className="ml-1.5 rounded bg-green-500/20 px-1 py-px text-[10px] text-green-400">local</span>}
                          {!conn.local && <span className="ml-1.5 rounded bg-slate-600 px-1 py-px text-[10px] text-slate-400">remote</span>}
                        </p>
                      </div>
                      {alreadyAdded ? (
                        <span className="text-xs text-slate-500">Already added</span>
                      ) : (
                        <Btn variant="ghost" onClick={() => handleQuickAdd(srv, conn)}>
                          <Plus className="h-4 w-4" /> Add
                        </Btn>
                      )}
                    </div>
                  )
                })
              })}
            </div>
          </div>
        )}
      </Card>

      {/* Server list */}
      <Card>
        <div className="mb-4 flex items-center justify-between">
          <h2 className="text-lg font-semibold text-white">Plex Servers</h2>
          <Btn variant="ghost" onClick={() => setShowAddForm(!showAddForm)}>
            <Plus className="h-4 w-4" /> Add Manually
          </Btn>
        </div>

        {/* Add server form */}
        {showAddForm && (
          <div className="mb-4 rounded-lg border border-slate-600 bg-slate-700/50 p-4 space-y-3">
            <div className="grid grid-cols-2 gap-3">
              <Input label="Name" value={addName} onChange={setAddName} placeholder="My Plex Server" />
              <Input label="IP / Hostname" value={addIp} onChange={setAddIp} placeholder="192.168.1.100" />
              <Input label="Port" value={addPort} onChange={setAddPort} placeholder="32400" />
              <Input label="Auth Token" value={addToken} onChange={setAddToken} placeholder="Plex token" />
            </div>
            <Toggle checked={addSsl} onChange={setAddSsl} label="Use SSL" />
            <div className="flex gap-2 pt-2">
              <Btn onClick={handleAddManual} disabled={addServer.isPending}>
                {addServer.isPending ? <Loader2 className="h-4 w-4 animate-spin" /> : <Save className="h-4 w-4" />}
                Save
              </Btn>
              <Btn variant="ghost" onClick={() => setShowAddForm(false)}>Cancel</Btn>
            </div>
          </div>
        )}

        {/* Server list */}
        {serversLoading ? (
          <div className="flex items-center gap-2 text-sm text-slate-400">
            <Loader2 className="h-4 w-4 animate-spin" /> Loading servers...
          </div>
        ) : !servers?.length ? (
          <p className="text-sm text-slate-400">No Plex servers configured. Add one above.</p>
        ) : (
          <div className="space-y-3">
            {servers.map((srv) => (
              <div key={srv.id} className="rounded-lg border border-slate-600 bg-slate-700/50">
                {/* Server header */}
                <div className="flex items-center gap-3 p-4">
                  <button
                    onClick={() => setExpandedServer(expandedServer === srv.id ? null : srv.id)}
                    className="text-slate-400 hover:text-white"
                  >
                    {expandedServer === srv.id ? (
                      <ChevronDown className="h-4 w-4" />
                    ) : (
                      <ChevronRight className="h-4 w-4" />
                    )}
                  </button>
                  <Server className="h-5 w-5 text-blue-400" />
                  <div className="flex-1 min-w-0">
                    <p className="text-sm font-medium text-white">{srv.name}</p>
                    <p className="text-xs text-slate-400">
                      {srv.ip}:{srv.port}{srv.useSsl ? ' (SSL)' : ''}
                    </p>
                    <WebhookUrlDisplay serverId={srv.id} />
                  </div>
                  {srv.authToken ? (
                    <span title="Token configured"><CheckCircle className="h-4 w-4 text-green-400" /></span>
                  ) : (
                    <span title="No token"><XCircle className="h-4 w-4 text-red-400" /></span>
                  )}
                  <Btn
                    variant="ghost"
                    onClick={() => {
                      if (editId === srv.id) {
                        setEditId(null)
                      } else {
                        setEditId(srv.id)
                        setEditName(srv.name)
                        setEditIp(srv.ip)
                        setEditPort(String(srv.port))
                        setEditSsl(srv.useSsl)
                        setEditToken('')
                      }
                    }}
                  >
                    <SettingsIcon className="h-4 w-4" />
                  </Btn>
                  <Btn variant="danger" onClick={() => handleDelete(srv.id, srv.name)}>
                    <Trash2 className="h-4 w-4" />
                  </Btn>
                </div>

                {/* Edit form */}
                {editId === srv.id && (
                  <div className="border-t border-slate-600 p-4 space-y-3">
                    <div className="grid grid-cols-2 gap-3">
                      <Input label="Name" value={editName} onChange={setEditName} />
                      <Input label="IP / Hostname" value={editIp} onChange={setEditIp} />
                      <Input label="Port" value={editPort} onChange={setEditPort} />
                      <Input label="Auth Token" value={editToken} onChange={setEditToken} placeholder="Leave empty to keep current" />
                    </div>
                    <Toggle checked={editSsl} onChange={setEditSsl} label="Use SSL" />
                    <div className="flex gap-2 pt-2">
                      <Btn onClick={handleUpdate} disabled={updateServer.isPending}>
                        {updateServer.isPending ? <Loader2 className="h-4 w-4 animate-spin" /> : <Save className="h-4 w-4" />}
                        Save
                      </Btn>
                      <Btn variant="ghost" onClick={() => setEditId(null)}>Cancel</Btn>
                    </div>
                  </div>
                )}

                {/* Libraries */}
                {expandedServer === srv.id && (
                  <div className="border-t border-slate-600 p-4">
                    <h3 className="mb-3 text-sm font-medium text-slate-300">Libraries</h3>
                    {libsLoading ? (
                      <div className="flex items-center gap-2 text-sm text-slate-400">
                        <Loader2 className="h-4 w-4 animate-spin" /> Fetching libraries...
                      </div>
                    ) : !libraries?.length ? (
                      <p className="text-sm text-slate-400">No libraries found on this server.</p>
                    ) : (
                      <div className="space-y-2">
                        {libraries.map((lib) => (
                          <div key={lib.id} className="flex items-center justify-between rounded-md border border-slate-600 px-3 py-2">
                            <div className="flex items-center gap-2">
                              {lib.libraryType === 'movie' ? (
                                <Film className="h-4 w-4 text-amber-400" />
                              ) : (
                                <Tv className="h-4 w-4 text-blue-400" />
                              )}
                              <span className="text-sm text-white">{lib.name}</span>
                              <span className="rounded bg-slate-600 px-1.5 py-0.5 text-[10px] text-slate-300 uppercase">
                                {lib.libraryType}
                              </span>
                            </div>
                            <div className="flex items-center gap-3">
                              {lib.lastScan && (
                                <span className="text-xs text-slate-500" title={lib.lastScan}>
                                  Scanned {new Date(lib.lastScan).toLocaleDateString()}
                                </span>
                              )}
                              <Toggle
                                checked={lib.enabled}
                                onChange={(v) => handleToggleLibrary(lib.id, v)}
                              />
                            </div>
                          </div>
                        ))}
                      </div>
                    )}
                  </div>
                )}
              </div>
            ))}
          </div>
        )}
      </Card>

      {/* Scan controls */}
      <Card>
        <h2 className="mb-4 text-lg font-semibold text-white">Library Scanning</h2>
        <p className="mb-4 text-sm text-slate-400">
          Scan your Plex libraries to match media with your NGMS library. Recent scan checks only newly added items.
        </p>
        <div className="flex gap-3">
          <Btn onClick={handleFullScan} disabled={scanning || !servers?.length}>
            {scanning ? <Loader2 className="h-4 w-4 animate-spin" /> : <Search className="h-4 w-4" />}
            Full Scan
          </Btn>
          <Btn variant="ghost" onClick={handleRecentScan} disabled={scanning || !servers?.length}>
            {scanning ? <Loader2 className="h-4 w-4 animate-spin" /> : <Download className="h-4 w-4" />}
            Recent Scan
          </Btn>
        </div>
      </Card>

      {/* Watchlist Auto-Request */}
      <WatchlistAutoRequestPanel showToast={showToast} />
    </div>
  )
}

function WatchlistAutoRequestPanel({ showToast }: { showToast: (msg: string, type: 'success' | 'error') => void }) {
  const { data: config, isLoading } = useQuery({
    queryKey: ['plex', 'watchlist', 'config'],
    queryFn: () => apiFetch<{ mode: string }>('/plex/watchlist/config'),
  })
  const [mode, setMode] = useState<string>('disabled')
  const [saving, setSaving] = useState(false)

  // Sync state from server
  useEffect(() => {
    if (config?.mode) setMode(config.mode)
  }, [config])

  const handleSave = async () => {
    setSaving(true)
    try {
      await apiFetch('/plex/watchlist/config', {
        method: 'PUT',
        body: JSON.stringify({ mode }),
      })
      showToast('Watchlist config saved', 'success')
    } catch {
      showToast('Failed to save config', 'error')
    } finally {
      setSaving(false)
    }
  }

  if (isLoading) return null

  return (
    <Card>
      <h2 className="mb-4 text-lg font-semibold text-white">Watchlist Auto-Request</h2>
      <p className="mb-4 text-sm text-slate-400">
        Automatically create media requests when new items appear in your Plex watchlist.
      </p>

      <div className="space-y-4 max-w-md">
        <div>
          <label className="mb-1 block text-xs font-medium text-slate-400">Mode</label>
          <select
            value={mode}
            onChange={(e) => setMode(e.target.value)}
            className="w-full rounded-lg border border-slate-600 bg-slate-700 px-3 py-2 text-sm text-white focus:border-blue-500 focus:outline-none"
          >
            <option value="disabled">Disabled</option>
            <option value="request">Create request (auto-approved)</option>
          </select>
          <p className="mt-1 text-xs text-slate-500">
            {mode === 'disabled'
              ? 'Watchlist items are synced but no action is taken.'
              : 'New watchlist items automatically become approved media requests.'}
          </p>
        </div>

        <Btn onClick={handleSave} disabled={saving}>
          {saving ? <Loader2 className="h-4 w-4 animate-spin" /> : <Save className="h-4 w-4" />}
          Save
        </Btn>
      </div>
    </Card>
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
              <Input label="SABnzbd URL" value={sabUrl} onChange={setSabUrl} placeholder="http://192.168.1.75:8080" />
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
        <MigrateResultRow label="Custom formats imported" value={result.customFormatsImported} />
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
