// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 The StackArr Authors

import { useState, useEffect, useCallback } from 'react'
import { Bell, Plus, Trash2, Loader2, AlertCircle, Check, X, Send, Edit2 } from 'lucide-react'
import { apiFetch } from '../api/client'

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

type ProviderType = 'webhook' | 'discord' | 'telegram' | 'slack' | 'email'

interface NotificationProvider {
  id: number
  name: string
  providerType: ProviderType
  config: Record<string, unknown>
  onGrab: boolean
  onImport: boolean
  onUpgrade: boolean
  onHealthIssue: boolean
  onFailure: boolean
  enabled: boolean
}

interface TestResult {
  success: boolean
  message: string
}

const PROVIDER_TYPES: { value: ProviderType; label: string }[] = [
  { value: 'webhook', label: 'Webhook' },
  { value: 'discord', label: 'Discord' },
  { value: 'slack', label: 'Slack' },
  { value: 'telegram', label: 'Telegram' },
  { value: 'email', label: 'Email' },
]

const EVENT_DEFS: { key: keyof Pick<NotificationProvider, 'onGrab' | 'onImport' | 'onUpgrade' | 'onHealthIssue' | 'onFailure'>; label: string; description: string }[] = [
  { key: 'onGrab', label: 'On Grab', description: 'A release has been grabbed from an indexer' },
  { key: 'onImport', label: 'On Import', description: 'A downloaded file has been imported to the library' },
  { key: 'onUpgrade', label: 'On Upgrade', description: 'An existing item was replaced with a higher-quality version' },
  { key: 'onHealthIssue', label: 'On Health Issue', description: 'A health issue has been raised by the system' },
  { key: 'onFailure', label: 'On Download Failure', description: 'A download has failed and won\'t be retried' },
]

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

interface FormState {
  id: number | null
  name: string
  providerType: ProviderType
  config: Record<string, string>
  onGrab: boolean
  onImport: boolean
  onUpgrade: boolean
  onHealthIssue: boolean
  onFailure: boolean
  enabled: boolean
}

function blankForm(): FormState {
  return {
    id: null,
    name: '',
    providerType: 'discord',
    config: {},
    onGrab: true,
    onImport: true,
    onUpgrade: true,
    onHealthIssue: true,
    onFailure: true,
    enabled: true,
  }
}

function configFields(pt: ProviderType): { key: string; label: string; placeholder?: string; sensitive?: boolean }[] {
  switch (pt) {
    case 'webhook':
      return [{ key: 'url', label: 'URL', placeholder: 'https://example.com/hook' }]
    case 'discord':
      return [{ key: 'webhook_url', label: 'Webhook URL', placeholder: 'https://discord.com/api/webhooks/...', sensitive: true }]
    case 'slack':
      return [{ key: 'webhook_url', label: 'Webhook URL', placeholder: 'https://hooks.slack.com/services/...', sensitive: true }]
    case 'telegram':
      return [
        { key: 'bot_token', label: 'Bot Token', placeholder: '123456:ABC-DEF...', sensitive: true },
        { key: 'chat_id', label: 'Chat ID', placeholder: '-100123456789' },
      ]
    case 'email':
      return [
        { key: 'smtp_url', label: 'SMTP URL', placeholder: 'smtps://user:pass@smtp.example.com:465', sensitive: true },
        { key: 'from', label: 'From', placeholder: 'stackarr@example.com' },
        { key: 'to', label: 'To', placeholder: 'me@example.com' },
      ]
  }
}

export default function Notifications() {
  const [providers, setProviders] = useState<NotificationProvider[]>([])
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<string | null>(null)
  const [editing, setEditing] = useState<FormState | null>(null)
  const [saving, setSaving] = useState(false)
  const [testing, setTesting] = useState(false)
  const [testResult, setTestResult] = useState<TestResult | null>(null)
  const [toast, setToast] = useState<{ msg: string; type: 'success' | 'error' } | null>(null)

  const showToast = (msg: string, type: 'success' | 'error' = 'success') => {
    setToast({ msg, type })
    setTimeout(() => setToast(null), 3500)
  }

  const load = useCallback(async () => {
    setLoading(true)
    setError(null)
    try {
      const data = await apiFetch<NotificationProvider[]>('/notification/provider')
      setProviders(data)
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to load providers')
    } finally {
      setLoading(false)
    }
  }, [])

  useEffect(() => {
    void load()
  }, [load])

  const startCreate = () => {
    setTestResult(null)
    setEditing(blankForm())
  }

  const startEdit = (p: NotificationProvider) => {
    setTestResult(null)
    setEditing({
      id: p.id,
      name: p.name,
      providerType: p.providerType,
      config: Object.fromEntries(
        Object.entries(p.config ?? {}).map(([k, v]) => [k, typeof v === 'string' ? v : '']),
      ),
      onGrab: p.onGrab,
      onImport: p.onImport,
      onUpgrade: p.onUpgrade,
      onHealthIssue: p.onHealthIssue,
      onFailure: p.onFailure,
      enabled: p.enabled,
    })
  }

  const save = async () => {
    if (!editing) return
    if (!editing.name.trim()) {
      showToast('Name is required', 'error')
      return
    }
    setSaving(true)
    try {
      const body = {
        name: editing.name.trim(),
        providerType: editing.providerType,
        config: stripEmpty(editing.config),
        onGrab: editing.onGrab,
        onImport: editing.onImport,
        onUpgrade: editing.onUpgrade,
        onHealthIssue: editing.onHealthIssue,
        onFailure: editing.onFailure,
        enabled: editing.enabled,
      }
      if (editing.id == null) {
        await apiFetch('/notification/provider', {
          method: 'POST',
          body: JSON.stringify(body),
        })
        showToast('Provider created')
      } else {
        await apiFetch(`/notification/provider/${editing.id}`, {
          method: 'PUT',
          body: JSON.stringify(body),
        })
        showToast('Provider updated')
      }
      setEditing(null)
      void load()
    } catch (err) {
      const msg = err instanceof Error ? err.message : 'Save failed'
      showToast(msg, 'error')
    } finally {
      setSaving(false)
    }
  }

  const remove = async (p: NotificationProvider) => {
    if (!confirm(`Delete provider "${p.name}"?`)) return
    try {
      await apiFetch(`/notification/provider/${p.id}`, { method: 'DELETE' })
      showToast('Provider deleted')
      void load()
    } catch {
      showToast('Failed to delete provider', 'error')
    }
  }

  const testConfig = async () => {
    if (!editing) return
    setTesting(true)
    setTestResult(null)
    try {
      const result = await apiFetch<TestResult>('/notification/provider/test', {
        method: 'POST',
        body: JSON.stringify({
          providerType: editing.providerType,
          config: stripEmpty(editing.config),
        }),
      })
      setTestResult(result)
    } catch (err) {
      setTestResult({ success: false, message: err instanceof Error ? err.message : 'Test failed' })
    } finally {
      setTesting(false)
    }
  }

  const testSaved = async (p: NotificationProvider) => {
    try {
      const result = await apiFetch<TestResult>(`/notification/provider/${p.id}/test`, { method: 'POST' })
      showToast(result.success ? `Test sent to ${p.name}` : `Test failed: ${result.message}`, result.success ? 'success' : 'error')
    } catch (err) {
      showToast(err instanceof Error ? err.message : 'Test failed', 'error')
    }
  }

  return (
    <div>
      {toast && (
        <div
          className={`fixed top-4 right-4 z-50 flex items-center gap-2 rounded-lg px-4 py-3 text-sm font-medium text-white shadow-lg ${
            toast.type === 'success' ? 'bg-green-600' : 'bg-red-600'
          }`}
        >
          {toast.type === 'success' ? <Check className="h-4 w-4" /> : <AlertCircle className="h-4 w-4" />}
          {toast.msg}
        </div>
      )}

      <div className="mb-6 flex items-center justify-between">
        <div className="flex items-center gap-3">
          <Bell className="h-6 w-6 text-blue-400" />
          <h1 className="text-2xl font-bold text-white">Notifications</h1>
          {!loading && (
            <span className="ml-2 rounded-full bg-slate-700 px-2.5 py-0.5 text-xs font-medium text-slate-300">
              {providers.length}
            </span>
          )}
        </div>
        <button
          onClick={startCreate}
          className="flex items-center gap-2 rounded-lg bg-blue-600 px-4 py-2 text-sm font-medium text-white hover:bg-blue-700 transition-colors"
        >
          <Plus className="h-4 w-4" />
          Add Provider
        </button>
      </div>

      <div className="rounded-xl border border-slate-700 bg-slate-800">
        {loading ? (
          <div className="flex items-center justify-center gap-2 py-12 text-slate-400">
            <Loader2 className="h-5 w-5 animate-spin" />
            <span>Loading...</span>
          </div>
        ) : error ? (
          <div className="flex flex-col items-center gap-3 py-12 text-slate-400">
            <AlertCircle className="h-8 w-8 text-red-400" />
            <p className="text-sm">{error}</p>
            <button onClick={() => void load()} className="rounded-lg bg-blue-600 px-4 py-2 text-sm font-medium text-white hover:bg-blue-700">
              Retry
            </button>
          </div>
        ) : providers.length === 0 ? (
          <div className="flex flex-col items-center gap-2 py-12 text-slate-400">
            <Bell className="h-8 w-8" />
            <p className="text-sm">No notification providers configured yet.</p>
            <button onClick={startCreate} className="mt-2 rounded-lg bg-blue-600 px-4 py-2 text-sm font-medium text-white hover:bg-blue-700">
              Add your first provider
            </button>
          </div>
        ) : (
          <table className="w-full text-left text-sm">
            <thead>
              <tr className="border-b border-slate-700 text-slate-400">
                <th className="px-4 py-3 font-medium">Name</th>
                <th className="px-4 py-3 font-medium">Type</th>
                <th className="px-4 py-3 font-medium">Events</th>
                <th className="px-4 py-3 font-medium">Status</th>
                <th className="px-4 py-3 font-medium text-right">Actions</th>
              </tr>
            </thead>
            <tbody>
              {providers.map((p) => {
                const events: string[] = []
                if (p.onGrab) events.push('Grab')
                if (p.onImport) events.push('Import')
                if (p.onUpgrade) events.push('Upgrade')
                if (p.onHealthIssue) events.push('Health')
                if (p.onFailure) events.push('Failure')
                return (
                  <tr key={p.id} className="border-b border-slate-700/50 hover:bg-slate-700/40 transition-colors">
                    <td className="px-4 py-3 text-white font-medium">{p.name}</td>
                    <td className="px-4 py-3 text-slate-300 capitalize">{p.providerType}</td>
                    <td className="px-4 py-3">
                      <div className="flex flex-wrap gap-1">
                        {events.length === 0 ? (
                          <span className="text-slate-500">None</span>
                        ) : (
                          events.map((e) => (
                            <span key={e} className="rounded bg-slate-700 px-1.5 py-0.5 text-xs text-slate-300">
                              {e}
                            </span>
                          ))
                        )}
                      </div>
                    </td>
                    <td className="px-4 py-3">
                      <span
                        className={`rounded-full px-2 py-0.5 text-xs font-medium ${
                          p.enabled ? 'bg-green-500/20 text-green-400' : 'bg-slate-600 text-slate-400'
                        }`}
                      >
                        {p.enabled ? 'Enabled' : 'Disabled'}
                      </span>
                    </td>
                    <td className="px-4 py-3">
                      <div className="flex justify-end gap-1.5">
                        <button
                          onClick={() => void testSaved(p)}
                          className="inline-flex items-center gap-1 rounded-lg bg-slate-700 px-3 py-1.5 text-xs font-medium text-slate-300 hover:bg-slate-600 transition-colors"
                          title="Send test notification"
                        >
                          <Send className="h-3.5 w-3.5" />
                          Test
                        </button>
                        <button
                          onClick={() => startEdit(p)}
                          className="inline-flex items-center gap-1 rounded-lg bg-blue-600 px-3 py-1.5 text-xs font-medium text-white hover:bg-blue-700 transition-colors"
                        >
                          <Edit2 className="h-3.5 w-3.5" />
                          Edit
                        </button>
                        <button
                          onClick={() => void remove(p)}
                          className="inline-flex items-center gap-1 rounded-lg bg-red-600/20 px-3 py-1.5 text-xs font-medium text-red-400 hover:bg-red-600/30 transition-colors"
                        >
                          <Trash2 className="h-3.5 w-3.5" />
                          Delete
                        </button>
                      </div>
                    </td>
                  </tr>
                )
              })}
            </tbody>
          </table>
        )}
      </div>

      {editing && (
        <ProviderModal
          form={editing}
          setForm={setEditing}
          onCancel={() => { setEditing(null); setTestResult(null) }}
          onSave={() => void save()}
          onTest={() => void testConfig()}
          saving={saving}
          testing={testing}
          testResult={testResult}
        />
      )}
    </div>
  )
}

// ---------------------------------------------------------------------------
// Modal
// ---------------------------------------------------------------------------

interface ModalProps {
  form: FormState
  setForm: (f: FormState) => void
  onCancel: () => void
  onSave: () => void
  onTest: () => void
  saving: boolean
  testing: boolean
  testResult: TestResult | null
}

function ProviderModal({ form, setForm, onCancel, onSave, onTest, saving, testing, testResult }: ModalProps) {
  const fields = configFields(form.providerType)

  return (
    <div className="fixed inset-0 z-40 flex items-center justify-center bg-black/60 p-4" onClick={onCancel}>
      <div
        onClick={(e) => e.stopPropagation()}
        className="relative w-full max-w-2xl max-h-[90vh] overflow-y-auto rounded-xl border border-slate-700 bg-slate-800 p-6 shadow-2xl"
      >
        <div className="mb-4 flex items-center justify-between">
          <h2 className="text-xl font-semibold text-white">
            {form.id == null ? 'Add Notification Provider' : 'Edit Notification Provider'}
          </h2>
          <button onClick={onCancel} className="text-slate-400 hover:text-white">
            <X className="h-5 w-5" />
          </button>
        </div>

        <div className="space-y-5">
          <div>
            <label className="block text-sm font-medium text-slate-300 mb-1">Name</label>
            <input
              type="text"
              value={form.name}
              onChange={(e) => setForm({ ...form, name: e.target.value })}
              placeholder="My Discord Server"
              className="w-full rounded-lg border border-slate-600 bg-slate-900 px-3 py-2 text-sm text-white focus:border-blue-500 focus:outline-none"
            />
          </div>

          <div>
            <label className="block text-sm font-medium text-slate-300 mb-1">Type</label>
            <select
              value={form.providerType}
              onChange={(e) => setForm({ ...form, providerType: e.target.value as ProviderType, config: {} })}
              className="w-full rounded-lg border border-slate-600 bg-slate-900 px-3 py-2 text-sm text-white focus:border-blue-500 focus:outline-none"
            >
              {PROVIDER_TYPES.map((t) => (
                <option key={t.value} value={t.value}>{t.label}</option>
              ))}
            </select>
          </div>

          <div className="space-y-3 rounded-lg border border-slate-700 p-3">
            <div className="text-xs font-medium uppercase text-slate-400">Configuration</div>
            {fields.map((f) => (
              <div key={f.key}>
                <label className="block text-sm font-medium text-slate-300 mb-1">{f.label}</label>
                <input
                  type={f.sensitive ? 'password' : 'text'}
                  value={form.config[f.key] ?? ''}
                  onChange={(e) => setForm({ ...form, config: { ...form.config, [f.key]: e.target.value } })}
                  placeholder={f.placeholder}
                  className="w-full rounded-lg border border-slate-600 bg-slate-900 px-3 py-2 text-sm text-white focus:border-blue-500 focus:outline-none"
                />
              </div>
            ))}
          </div>

          <div className="space-y-2 rounded-lg border border-slate-700 p-3">
            <div className="text-xs font-medium uppercase text-slate-400 mb-1">Notify On</div>
            {EVENT_DEFS.map((ev) => (
              <label key={ev.key} className="flex items-start gap-3 cursor-pointer">
                <input
                  type="checkbox"
                  checked={form[ev.key]}
                  onChange={(e) => setForm({ ...form, [ev.key]: e.target.checked })}
                  className="mt-1 h-4 w-4 rounded border-slate-600 bg-slate-900 text-blue-600 focus:ring-blue-500"
                />
                <div>
                  <div className="text-sm font-medium text-slate-200">{ev.label}</div>
                  <div className="text-xs text-slate-400">{ev.description}</div>
                </div>
              </label>
            ))}
          </div>

          <label className="flex items-center gap-3 cursor-pointer">
            <input
              type="checkbox"
              checked={form.enabled}
              onChange={(e) => setForm({ ...form, enabled: e.target.checked })}
              className="h-4 w-4 rounded border-slate-600 bg-slate-900 text-blue-600 focus:ring-blue-500"
            />
            <span className="text-sm text-slate-200">Enabled</span>
          </label>

          {testResult && (
            <div
              className={`rounded-lg border px-3 py-2 text-sm ${
                testResult.success
                  ? 'border-green-600/40 bg-green-600/10 text-green-300'
                  : 'border-red-600/40 bg-red-600/10 text-red-300'
              }`}
            >
              {testResult.message}
            </div>
          )}
        </div>

        <div className="mt-6 flex items-center justify-between gap-2">
          <button
            onClick={onTest}
            disabled={testing || saving}
            className="inline-flex items-center gap-2 rounded-lg bg-slate-700 px-4 py-2 text-sm font-medium text-slate-200 hover:bg-slate-600 transition-colors disabled:opacity-50"
          >
            {testing ? <Loader2 className="h-4 w-4 animate-spin" /> : <Send className="h-4 w-4" />}
            Test
          </button>
          <div className="flex gap-2">
            <button
              onClick={onCancel}
              disabled={saving}
              className="rounded-lg bg-slate-700 px-4 py-2 text-sm font-medium text-slate-200 hover:bg-slate-600 transition-colors disabled:opacity-50"
            >
              Cancel
            </button>
            <button
              onClick={onSave}
              disabled={saving}
              className="inline-flex items-center gap-2 rounded-lg bg-blue-600 px-4 py-2 text-sm font-medium text-white hover:bg-blue-700 transition-colors disabled:opacity-50"
            >
              {saving ? <Loader2 className="h-4 w-4 animate-spin" /> : <Check className="h-4 w-4" />}
              Save
            </button>
          </div>
        </div>
      </div>
    </div>
  )
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function stripEmpty(config: Record<string, string>): Record<string, string> {
  const out: Record<string, string> = {}
  for (const [k, v] of Object.entries(config)) {
    if (v != null && v !== '') out[k] = v
  }
  return out
}
