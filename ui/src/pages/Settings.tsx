import { useState, useEffect, useCallback } from 'react'
import type {
  QualityProfile,
  QualityProfileItem,
  IndexerConfig,
  DownloadClientConfig,
  NamingConfig,
  RootFolder,
  Tag,
} from '../api/types'
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
} from 'lucide-react'

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const API = '/api/v1'

type TabKey =
  | 'general'
  | 'quality'
  | 'indexers'
  | 'downloadclients'
  | 'naming'
  | 'rootfolders'
  | 'tags'

interface TabDef {
  key: TabKey
  label: string
}

const TABS: TabDef[] = [
  { key: 'general', label: 'General' },
  { key: 'quality', label: 'Quality Profiles' },
  { key: 'indexers', label: 'Indexers' },
  { key: 'downloadclients', label: 'Download Clients' },
  { key: 'naming', label: 'Naming' },
  { key: 'rootfolders', label: 'Root Folders' },
  { key: 'tags', label: 'Tags' },
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
  protocol: string
  baseUrl: string
  enabled: boolean
  fields: Record<string, string>
}

const emptyIndexerForm: IndexerFormData = {
  name: '',
  protocol: 'Newznab',
  baseUrl: '',
  enabled: true,
  fields: { apiKey: '' },
}

function IndexersTab({
  showToast,
}: {
  showToast: (msg: string, type: 'success' | 'error') => void
}) {
  const [indexers, setIndexers] = useState<IndexerConfig[]>([])
  const [loading, setLoading] = useState(true)
  const [showForm, setShowForm] = useState(false)
  const [editId, setEditId] = useState<number | null>(null)
  const [form, setForm] = useState<IndexerFormData>(emptyIndexerForm)
  const [testing, setTesting] = useState<number | null>(null)

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

  const openAdd = () => {
    setEditId(null)
    setForm(emptyIndexerForm)
    setShowForm(true)
  }

  const openEdit = (idx: IndexerConfig) => {
    setEditId(idx.id)
    setForm({
      name: idx.name,
      protocol: idx.protocol,
      baseUrl: idx.baseUrl,
      enabled: idx.enabled,
      fields: { ...idx.fields },
    })
    setShowForm(true)
  }

  const saveIndexer = async () => {
    try {
      const method = editId ? 'PUT' : 'POST'
      const url = editId ? `${API}/indexer/${editId}` : `${API}/indexer`
      const res = await fetch(url, {
        method,
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(editId ? { id: editId, ...form } : form),
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
        body: JSON.stringify({ ...idx, enabled: !idx.enabled }),
      })
      if (!res.ok) throw new Error('Toggle failed')
      void load()
    } catch {
      showToast('Failed to toggle indexer', 'error')
    }
  }

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
        <Btn onClick={openAdd}>
          <Plus className="h-4 w-4" /> Add Indexer
        </Btn>
      </div>

      {showForm && (
        <div className="mb-6 rounded-lg border border-slate-600 bg-slate-700/50 p-4">
          <h3 className="mb-4 text-sm font-semibold text-white">
            {editId ? 'Edit Indexer' : 'Add Indexer'}
          </h3>
          <div className="grid grid-cols-1 gap-4 sm:grid-cols-2 max-w-2xl">
            <Input label="Name" value={form.name} onChange={(v) => setForm({ ...form, name: v })} placeholder="My Indexer" />
            <Select
              label="Type"
              value={form.protocol}
              onChange={(v) => setForm({ ...form, protocol: v })}
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
                <td className="py-3 pr-4 text-slate-300">{idx.protocol}</td>
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
// Root Folders Tab
// ---------------------------------------------------------------------------

function RootFoldersTab({
  showToast,
}: {
  showToast: (msg: string, type: 'success' | 'error') => void
}) {
  const [folders, setFolders] = useState<RootFolder[]>([])
  const [loading, setLoading] = useState(true)
  const [showForm, setShowForm] = useState(false)
  const [newPath, setNewPath] = useState('')
  const [newMediaType, setNewMediaType] = useState<'tv' | 'movie'>('tv')

  const load = useCallback(async () => {
    setLoading(true)
    try {
      const res = await fetch(`${API}/rootfolder`)
      const data: RootFolder[] = await res.json()
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
      const res = await fetch(`${API}/rootfolder`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ path: newPath, mediaType: newMediaType }),
      })
      if (!res.ok) throw new Error('Add failed')
      showToast('Root folder added', 'success')
      setShowForm(false)
      setNewPath('')
      void load()
    } catch {
      showToast('Failed to add root folder', 'error')
    }
  }

  const deleteFolder = async (id: number) => {
    try {
      const res = await fetch(`${API}/rootfolder/${id}`, { method: 'DELETE' })
      if (!res.ok) throw new Error('Delete failed')
      showToast('Root folder removed', 'success')
      void load()
    } catch {
      showToast('Failed to remove root folder', 'error')
    }
  }

  if (loading) {
    return (
      <Card>
        <div className="flex items-center gap-2 text-slate-400">
          <Loader2 className="h-5 w-5 animate-spin" /> Loading root folders...
        </div>
      </Card>
    )
  }

  return (
    <Card>
      <div className="mb-6 flex items-center justify-between">
        <h2 className="text-lg font-semibold text-white">Root Folders</h2>
        <Btn onClick={() => setShowForm(true)}>
          <Plus className="h-4 w-4" /> Add Folder
        </Btn>
      </div>

      {showForm && (
        <div className="mb-6 rounded-lg border border-slate-600 bg-slate-700/50 p-4">
          <h3 className="mb-4 text-sm font-semibold text-white">Add Root Folder</h3>
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
        <p className="text-sm text-slate-400">No root folders configured.</p>
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

  return (
    <div className="min-h-screen bg-slate-900 p-6">
      <div className="mx-auto max-w-5xl">
        {/* Header */}
        <div className="mb-6 flex items-center gap-3">
          <SettingsIcon className="h-6 w-6 text-blue-400" />
          <h1 className="text-2xl font-bold text-white">Settings</h1>
        </div>

        {/* Tabs */}
        <div className="mb-6 flex flex-wrap gap-2">
          {TABS.map((tab) => (
            <button
              key={tab.key}
              onClick={() => setActiveTab(tab.key)}
              className={`rounded-full px-4 py-2 text-sm font-medium transition-colors ${
                activeTab === tab.key
                  ? 'bg-blue-600 text-white'
                  : 'bg-slate-800 text-slate-300 hover:bg-slate-700 hover:text-white'
              }`}
            >
              {tab.label}
            </button>
          ))}
        </div>

        {/* Tab Content */}
        {activeTab === 'general' && <GeneralTab showToast={showToast} />}
        {activeTab === 'quality' && <QualityProfilesTab showToast={showToast} />}
        {activeTab === 'indexers' && <IndexersTab showToast={showToast} />}
        {activeTab === 'downloadclients' && <DownloadClientsTab showToast={showToast} />}
        {activeTab === 'naming' && <NamingTab showToast={showToast} />}
        {activeTab === 'rootfolders' && <RootFoldersTab showToast={showToast} />}
        {activeTab === 'tags' && <TagsTab showToast={showToast} />}
      </div>

      {/* Toast Notification */}
      {toast && <Toast toast={toast} onDismiss={dismissToast} />}
    </div>
  )
}
