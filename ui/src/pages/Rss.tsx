import { useState } from 'react'
import {
  Rss as RssIcon,
  Plus,
  Trash2,
  Pencil,
  RefreshCw,
  Download,
  Check,
  X,
  Loader2,
  Magnet,
  HardDrive,
} from 'lucide-react'
import {
  useRssFeeds,
  useRssItems,
  useRssRules,
  useCreateRssFeed,
  useUpdateRssFeed,
  useDeleteRssFeed,
  useCheckRssFeed,
  useDownloadRssItem,
  useCreateRssRule,
  useUpdateRssRule,
  useDeleteRssRule,
} from '../hooks/useApi'
import type { RssFeed, RssRule } from '../api/types'

type Tab = 'feeds' | 'rules' | 'items'

export default function Rss() {
  const [tab, setTab] = useState<Tab>('feeds')

  return (
    <div>
      <div className="mb-6 flex items-center justify-between">
        <div className="flex items-center gap-3">
          <RssIcon size={24} className="text-orange-400" />
          <h2 className="text-2xl font-bold">RSS Feeds</h2>
        </div>
      </div>

      {/* Tab bar */}
      <div className="mb-4 flex gap-1 rounded-lg bg-slate-800 p-1 w-fit">
        {(['feeds', 'rules', 'items'] as Tab[]).map((t) => (
          <button
            key={t}
            onClick={() => setTab(t)}
            className={`rounded-md px-4 py-1.5 text-sm font-medium capitalize transition-colors ${
              tab === t
                ? 'bg-blue-600 text-white'
                : 'text-slate-400 hover:text-white hover:bg-slate-700'
            }`}
          >
            {t}
          </button>
        ))}
      </div>

      {tab === 'feeds' && <FeedsTab />}
      {tab === 'rules' && <RulesTab />}
      {tab === 'items' && <ItemsTab />}
    </div>
  )
}

// ── Feeds Tab ──────────────────────────────────────────────────────────────

function FeedsTab() {
  const { data: feeds, isLoading, error } = useRssFeeds()
  const deleteFeed = useDeleteRssFeed()
  const checkFeed = useCheckRssFeed()
  const [editFeed, setEditFeed] = useState<RssFeed | null>(null)
  const [showAdd, setShowAdd] = useState(false)

  if (isLoading) return <Spinner />
  if (error) return <ErrorBox message={error.message} />

  return (
    <div>
      <div className="mb-4 flex justify-end">
        <button
          onClick={() => setShowAdd(true)}
          className="flex items-center gap-1.5 rounded-lg bg-blue-600 px-4 py-2 text-sm font-medium hover:bg-blue-700"
        >
          <Plus size={16} /> Add Feed
        </button>
      </div>

      {feeds?.length === 0 && (
        <div className="flex flex-col items-center justify-center py-20 text-slate-400">
          <RssIcon size={48} className="mb-4 text-slate-600" />
          <p>No RSS feeds configured</p>
        </div>
      )}

      {feeds && feeds.length > 0 && (
        <div className="overflow-x-auto rounded-lg bg-slate-800">
          <table className="w-full text-sm">
            <thead>
              <tr className="border-b border-slate-700 text-left text-xs uppercase text-slate-400">
                <th className="px-4 py-3 font-medium">Status</th>
                <th className="px-4 py-3 font-medium">Name</th>
                <th className="px-4 py-3 font-medium">Protocol</th>
                <th className="px-4 py-3 font-medium">URL</th>
                <th className="px-4 py-3 font-medium">Interval</th>
                <th className="px-4 py-3 font-medium">Category</th>
                <th className="px-4 py-3 font-medium">Auto-DL</th>
                <th className="px-4 py-3 font-medium text-right">Actions</th>
              </tr>
            </thead>
            <tbody>
              {feeds.map((feed) => (
                <tr key={feed.id} className="border-b border-slate-700/50 hover:bg-slate-700/30">
                  <td className="px-4 py-3">
                    <span
                      className={`inline-block h-2.5 w-2.5 rounded-full ${
                        feed.enabled ? 'bg-green-500' : 'bg-slate-500'
                      }`}
                    />
                  </td>
                  <td className="px-4 py-3 font-medium text-white">{feed.name}</td>
                  <td className="px-4 py-3">
                    <ProtocolBadge protocol={feed.protocol} />
                  </td>
                  <td className="px-4 py-3 max-w-xs truncate text-slate-400" title={feed.url}>
                    {feed.url}
                  </td>
                  <td className="px-4 py-3 text-slate-400">{Math.round(feed.pollIntervalSecs / 60)}m</td>
                  <td className="px-4 py-3 text-slate-400">{feed.category || '-'}</td>
                  <td className="px-4 py-3">
                    {feed.autoDownload ? (
                      <Check size={16} className="text-green-400" />
                    ) : (
                      <X size={16} className="text-slate-500" />
                    )}
                  </td>
                  <td className="px-4 py-3">
                    <div className="flex justify-end gap-1">
                      <button
                        onClick={() => checkFeed.mutate(feed.id)}
                        disabled={checkFeed.isPending}
                        className="rounded p-1.5 text-slate-400 hover:bg-slate-600 hover:text-white"
                        title="Check now"
                      >
                        <RefreshCw size={14} className={checkFeed.isPending ? 'animate-spin' : ''} />
                      </button>
                      <button
                        onClick={() => setEditFeed(feed)}
                        className="rounded p-1.5 text-slate-400 hover:bg-slate-600 hover:text-white"
                        title="Edit"
                      >
                        <Pencil size={14} />
                      </button>
                      <button
                        onClick={() => deleteFeed.mutate(feed.id)}
                        className="rounded p-1.5 text-slate-400 hover:bg-red-600/80 hover:text-white"
                        title="Delete"
                      >
                        <Trash2 size={14} />
                      </button>
                    </div>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}

      {(showAdd || editFeed) && (
        <FeedModal
          feed={editFeed}
          onClose={() => {
            setShowAdd(false)
            setEditFeed(null)
          }}
        />
      )}
    </div>
  )
}

// ── Rules Tab ──────────────────────────────────────────────────────────────

function RulesTab() {
  const { data: rules, isLoading, error } = useRssRules()
  const { data: feeds } = useRssFeeds()
  const deleteRule = useDeleteRssRule()
  const [editRule, setEditRule] = useState<RssRule | null>(null)
  const [showAdd, setShowAdd] = useState(false)

  const feedNameMap = new Map(feeds?.map((f) => [f.id, f.name]) ?? [])

  if (isLoading) return <Spinner />
  if (error) return <ErrorBox message={error.message} />

  return (
    <div>
      <div className="mb-4 flex justify-end">
        <button
          onClick={() => setShowAdd(true)}
          className="flex items-center gap-1.5 rounded-lg bg-blue-600 px-4 py-2 text-sm font-medium hover:bg-blue-700"
        >
          <Plus size={16} /> Add Rule
        </button>
      </div>

      {rules?.length === 0 && (
        <div className="flex flex-col items-center justify-center py-20 text-slate-400">
          <RssIcon size={48} className="mb-4 text-slate-600" />
          <p>No download rules configured</p>
        </div>
      )}

      {rules && rules.length > 0 && (
        <div className="overflow-x-auto rounded-lg bg-slate-800">
          <table className="w-full text-sm">
            <thead>
              <tr className="border-b border-slate-700 text-left text-xs uppercase text-slate-400">
                <th className="px-4 py-3 font-medium">Status</th>
                <th className="px-4 py-3 font-medium">Name</th>
                <th className="px-4 py-3 font-medium">Feeds</th>
                <th className="px-4 py-3 font-medium">Regex</th>
                <th className="px-4 py-3 font-medium">Category</th>
                <th className="px-4 py-3 font-medium">Priority</th>
                <th className="px-4 py-3 font-medium text-right">Actions</th>
              </tr>
            </thead>
            <tbody>
              {rules.map((rule) => (
                <tr key={rule.id} className="border-b border-slate-700/50 hover:bg-slate-700/30">
                  <td className="px-4 py-3">
                    <span
                      className={`inline-block h-2.5 w-2.5 rounded-full ${
                        rule.enabled ? 'bg-green-500' : 'bg-slate-500'
                      }`}
                    />
                  </td>
                  <td className="px-4 py-3 font-medium text-white">{rule.name}</td>
                  <td className="px-4 py-3 text-slate-400">
                    {rule.feedIds.map((fid) => feedNameMap.get(fid) || `#${fid}`).join(', ')}
                  </td>
                  <td className="px-4 py-3">
                    <code className="rounded bg-slate-700 px-1.5 py-0.5 text-xs text-orange-300">
                      {rule.matchRegex}
                    </code>
                  </td>
                  <td className="px-4 py-3 text-slate-400">{rule.category || '-'}</td>
                  <td className="px-4 py-3 text-slate-400">{priorityLabel(rule.priority)}</td>
                  <td className="px-4 py-3">
                    <div className="flex justify-end gap-1">
                      <button
                        onClick={() => setEditRule(rule)}
                        className="rounded p-1.5 text-slate-400 hover:bg-slate-600 hover:text-white"
                        title="Edit"
                      >
                        <Pencil size={14} />
                      </button>
                      <button
                        onClick={() => deleteRule.mutate(rule.id)}
                        className="rounded p-1.5 text-slate-400 hover:bg-red-600/80 hover:text-white"
                        title="Delete"
                      >
                        <Trash2 size={14} />
                      </button>
                    </div>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}

      {(showAdd || editRule) && (
        <RuleModal
          rule={editRule}
          onClose={() => {
            setShowAdd(false)
            setEditRule(null)
          }}
        />
      )}
    </div>
  )
}

// ── Items Tab ──────────────────────────────────────────────────────────────

function ItemsTab() {
  const { data: feeds } = useRssFeeds()
  const [feedFilter, setFeedFilter] = useState<number | undefined>(undefined)
  const { data: items, isLoading, error } = useRssItems(feedFilter)
  const downloadItem = useDownloadRssItem()

  if (isLoading) return <Spinner />
  if (error) return <ErrorBox message={error.message} />

  return (
    <div>
      <div className="mb-4 flex items-center gap-3">
        <select
          value={feedFilter ?? ''}
          onChange={(e) => setFeedFilter(e.target.value ? Number(e.target.value) : undefined)}
          className="rounded-lg border border-slate-600 bg-slate-700 px-3 py-2 text-sm text-white"
        >
          <option value="">All Feeds</option>
          {feeds?.map((f) => (
            <option key={f.id} value={f.id}>
              {f.name}
            </option>
          ))}
        </select>
        <span className="text-xs text-slate-500">
          {items?.length ?? 0} items
        </span>
      </div>

      {items?.length === 0 && (
        <div className="flex flex-col items-center justify-center py-20 text-slate-400">
          <RssIcon size={48} className="mb-4 text-slate-600" />
          <p>No items found</p>
        </div>
      )}

      {items && items.length > 0 && (
        <div className="overflow-x-auto rounded-lg bg-slate-800">
          <table className="w-full text-sm">
            <thead>
              <tr className="border-b border-slate-700 text-left text-xs uppercase text-slate-400">
                <th className="px-4 py-3 font-medium">Title</th>
                <th className="px-4 py-3 font-medium">Feed</th>
                <th className="px-4 py-3 font-medium">Size</th>
                <th className="px-4 py-3 font-medium">Date</th>
                <th className="px-4 py-3 font-medium">Status</th>
                <th className="px-4 py-3 font-medium text-right">Action</th>
              </tr>
            </thead>
            <tbody>
              {items.map((item) => {
                const feedName = feeds?.find((f) => f.id === item.feedId)?.name ?? `#${item.feedId}`
                return (
                  <tr key={item.id} className="border-b border-slate-700/50 hover:bg-slate-700/30">
                    <td className="px-4 py-3 text-white max-w-md truncate" title={item.title}>
                      {item.title}
                    </td>
                    <td className="px-4 py-3 text-slate-400 whitespace-nowrap">{feedName}</td>
                    <td className="px-4 py-3 text-slate-400 whitespace-nowrap">
                      {item.sizeBytes ? formatSize(item.sizeBytes) : '-'}
                    </td>
                    <td className="px-4 py-3 text-slate-400 whitespace-nowrap">
                      {item.publishedAt ? formatDate(item.publishedAt) : formatDate(item.firstSeenAt)}
                    </td>
                    <td className="px-4 py-3">
                      {item.downloaded ? (
                        <span className="inline-flex items-center gap-1 rounded-full bg-green-500/20 px-2 py-0.5 text-xs font-medium text-green-400">
                          <Check size={12} /> Downloaded
                        </span>
                      ) : (
                        <span className="inline-flex items-center gap-1 rounded-full bg-slate-600/40 px-2 py-0.5 text-xs font-medium text-slate-400">
                          Pending
                        </span>
                      )}
                    </td>
                    <td className="px-4 py-3 text-right">
                      {!item.downloaded && item.url && (
                        <button
                          onClick={() => downloadItem.mutate(item.id)}
                          disabled={downloadItem.isPending}
                          className="rounded bg-blue-600 px-2.5 py-1 text-xs font-medium hover:bg-blue-700 disabled:opacity-50"
                        >
                          <Download size={12} className="inline mr-1" />
                          Grab
                        </button>
                      )}
                    </td>
                  </tr>
                )
              })}
            </tbody>
          </table>
        </div>
      )}
    </div>
  )
}

// ── Feed Modal ─────────────────────────────────────────────────────────────

function FeedModal({ feed, onClose }: { feed: RssFeed | null; onClose: () => void }) {
  const isEdit = feed !== null
  const createFeed = useCreateRssFeed()
  const updateFeed = useUpdateRssFeed()
  const [error, setError] = useState<string | null>(null)

  const [name, setName] = useState(feed?.name ?? '')
  const [url, setUrl] = useState(feed?.url ?? '')
  const [protocol, setProtocol] = useState<'usenet' | 'torrent'>(feed?.protocol ?? 'torrent')
  const [pollInterval, setPollInterval] = useState(Math.round((feed?.pollIntervalSecs ?? 900) / 60))
  const [category, setCategory] = useState(feed?.category ?? '')
  const [filterRegex, setFilterRegex] = useState(feed?.filterRegex ?? '')
  const [enabled, setEnabled] = useState(feed?.enabled ?? true)
  const [autoDownload, setAutoDownload] = useState(feed?.autoDownload ?? false)

  const isPending = createFeed.isPending || updateFeed.isPending

  const handleSave = () => {
    setError(null)
    const data = {
      name,
      url,
      protocol,
      pollIntervalSecs: pollInterval * 60,
      category: category || null,
      filterRegex: filterRegex || null,
      enabled,
      autoDownload,
    }

    if (isEdit) {
      updateFeed.mutate(
        { id: feed.id, ...data },
        { onSuccess: onClose, onError: (e) => setError(e.message) },
      )
    } else {
      createFeed.mutate(data, {
        onSuccess: onClose,
        onError: (e) => setError(e.message),
      })
    }
  }

  return (
    <ModalOverlay onClose={onClose}>
      <h3 className="text-lg font-semibold text-white mb-4">{isEdit ? 'Edit Feed' : 'Add Feed'}</h3>

      <div className="flex flex-col gap-3">
        <Field label="Name">
          <input value={name} onChange={(e) => setName(e.target.value)} className={inputCls} />
        </Field>
        <Field label="URL">
          <input value={url} onChange={(e) => setUrl(e.target.value)} className={inputCls} placeholder="https://..." />
        </Field>
        <Field label="Protocol">
          <select value={protocol} onChange={(e) => setProtocol(e.target.value as 'usenet' | 'torrent')} className={inputCls}>
            <option value="torrent">Torrent</option>
            <option value="usenet">Usenet</option>
          </select>
        </Field>
        <Field label="Poll Interval (minutes)">
          <input type="number" min={1} value={pollInterval} onChange={(e) => setPollInterval(Number(e.target.value))} className={inputCls} />
        </Field>
        <Field label="Category">
          <input value={category} onChange={(e) => setCategory(e.target.value)} className={inputCls} placeholder="Optional" />
        </Field>
        <Field label="Filter Regex">
          <input value={filterRegex} onChange={(e) => setFilterRegex(e.target.value)} className={inputCls} placeholder="Optional — e.g. 1080p|2160p" />
        </Field>
        <div className="flex gap-6">
          <label className="flex items-center gap-2 text-sm text-slate-300">
            <input type="checkbox" checked={enabled} onChange={(e) => setEnabled(e.target.checked)} className="rounded" />
            Enabled
          </label>
          <label className="flex items-center gap-2 text-sm text-slate-300">
            <input type="checkbox" checked={autoDownload} onChange={(e) => setAutoDownload(e.target.checked)} className="rounded" />
            Auto-download
          </label>
        </div>
      </div>

      {error && <p className="mt-3 text-xs text-red-400">{error}</p>}

      <div className="mt-5 flex justify-end gap-2">
        <button onClick={onClose} className="rounded-lg bg-slate-700 px-4 py-2 text-sm hover:bg-slate-600">
          Cancel
        </button>
        <button onClick={handleSave} disabled={isPending} className="rounded-lg bg-blue-600 px-4 py-2 text-sm font-medium hover:bg-blue-700 disabled:opacity-50">
          {isPending ? <Loader2 size={14} className="animate-spin" /> : isEdit ? 'Save' : 'Add'}
        </button>
      </div>
    </ModalOverlay>
  )
}

// ── Rule Modal ─────────────────────────────────────────────────────────────

function RuleModal({ rule, onClose }: { rule: RssRule | null; onClose: () => void }) {
  const isEdit = rule !== null
  const { data: feeds } = useRssFeeds()
  const createRule = useCreateRssRule()
  const updateRule = useUpdateRssRule()
  const [error, setError] = useState<string | null>(null)

  const [name, setName] = useState(rule?.name ?? '')
  const [feedIds, setFeedIds] = useState<number[]>(rule?.feedIds ?? [])
  const [matchRegex, setMatchRegex] = useState(rule?.matchRegex ?? '')
  const [category, setCategory] = useState(rule?.category ?? '')
  const [priority, setPriority] = useState(rule?.priority ?? 1)
  const [enabled, setEnabled] = useState(rule?.enabled ?? true)

  const isPending = createRule.isPending || updateRule.isPending

  const toggleFeed = (id: number) => {
    setFeedIds((prev) => (prev.includes(id) ? prev.filter((x) => x !== id) : [...prev, id]))
  }

  const handleSave = () => {
    setError(null)

    // Validate regex
    try {
      new RegExp(matchRegex)
    } catch {
      setError('Invalid regex pattern')
      return
    }

    const data = {
      name,
      feedIds,
      matchRegex,
      category: category || null,
      priority,
      enabled,
    }

    if (isEdit) {
      updateRule.mutate(
        { id: rule.id, ...data },
        { onSuccess: onClose, onError: (e) => setError(e.message) },
      )
    } else {
      createRule.mutate(data, {
        onSuccess: onClose,
        onError: (e) => setError(e.message),
      })
    }
  }

  return (
    <ModalOverlay onClose={onClose}>
      <h3 className="text-lg font-semibold text-white mb-4">{isEdit ? 'Edit Rule' : 'Add Rule'}</h3>

      <div className="flex flex-col gap-3">
        <Field label="Name">
          <input value={name} onChange={(e) => setName(e.target.value)} className={inputCls} />
        </Field>
        <Field label="Match Regex">
          <input value={matchRegex} onChange={(e) => setMatchRegex(e.target.value)} className={inputCls} placeholder="e.g. S01E\\d+.*1080p" />
        </Field>
        <Field label="Linked Feeds">
          <div className="flex flex-wrap gap-2">
            {feeds?.map((f) => (
              <label key={f.id} className="flex items-center gap-1.5 text-sm text-slate-300">
                <input
                  type="checkbox"
                  checked={feedIds.includes(f.id)}
                  onChange={() => toggleFeed(f.id)}
                  className="rounded"
                />
                {f.name}
              </label>
            ))}
            {(!feeds || feeds.length === 0) && <span className="text-xs text-slate-500">No feeds yet</span>}
          </div>
        </Field>
        <Field label="Category">
          <input value={category} onChange={(e) => setCategory(e.target.value)} className={inputCls} placeholder="Optional override" />
        </Field>
        <Field label="Priority">
          <select value={priority} onChange={(e) => setPriority(Number(e.target.value))} className={inputCls}>
            <option value={0}>Low</option>
            <option value={1}>Normal</option>
            <option value={2}>High</option>
            <option value={3}>Force</option>
          </select>
        </Field>
        <label className="flex items-center gap-2 text-sm text-slate-300">
          <input type="checkbox" checked={enabled} onChange={(e) => setEnabled(e.target.checked)} className="rounded" />
          Enabled
        </label>
      </div>

      {error && <p className="mt-3 text-xs text-red-400">{error}</p>}

      <div className="mt-5 flex justify-end gap-2">
        <button onClick={onClose} className="rounded-lg bg-slate-700 px-4 py-2 text-sm hover:bg-slate-600">
          Cancel
        </button>
        <button onClick={handleSave} disabled={isPending} className="rounded-lg bg-blue-600 px-4 py-2 text-sm font-medium hover:bg-blue-700 disabled:opacity-50">
          {isPending ? <Loader2 size={14} className="animate-spin" /> : isEdit ? 'Save' : 'Add'}
        </button>
      </div>
    </ModalOverlay>
  )
}

// ── Shared Components ──────────────────────────────────────────────────────

function ModalOverlay({ onClose, children }: { onClose: () => void; children: React.ReactNode }) {
  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60" onClick={onClose}>
      <div className="w-full max-w-lg rounded-xl bg-slate-800 p-6 shadow-2xl" onClick={(e) => e.stopPropagation()}>
        {children}
      </div>
    </div>
  )
}

function Field({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <label className="flex flex-col gap-1">
      <span className="text-xs font-medium text-slate-400">{label}</span>
      {children}
    </label>
  )
}

function ProtocolBadge({ protocol }: { protocol: 'usenet' | 'torrent' }) {
  const Icon = protocol === 'torrent' ? Magnet : HardDrive
  const color = protocol === 'torrent' ? 'bg-orange-500/20 text-orange-400' : 'bg-blue-500/20 text-blue-400'
  return (
    <span className={`inline-flex items-center gap-1 rounded-full px-2 py-0.5 text-xs font-medium ${color}`}>
      <Icon size={12} />
      {protocol === 'torrent' ? 'Torrent' : 'Usenet'}
    </span>
  )
}

function Spinner() {
  return (
    <div className="flex items-center justify-center py-20">
      <Loader2 size={32} className="animate-spin text-blue-500" />
    </div>
  )
}

function ErrorBox({ message }: { message: string }) {
  return (
    <div className="rounded-lg border border-red-500/30 bg-red-500/10 p-4 text-red-400">
      Failed to load: {message}
    </div>
  )
}

const inputCls =
  'w-full rounded-lg border border-slate-600 bg-slate-700 px-3 py-2 text-sm text-white placeholder-slate-500 focus:border-blue-500 focus:outline-none'

function priorityLabel(p: number): string {
  switch (p) {
    case 0: return 'Low'
    case 1: return 'Normal'
    case 2: return 'High'
    case 3: return 'Force'
    default: return String(p)
  }
}

function formatSize(bytes: number): string {
  if (bytes === 0) return '0 B'
  const units = ['B', 'KB', 'MB', 'GB', 'TB']
  const i = Math.floor(Math.log(bytes) / Math.log(1024))
  return `${(bytes / Math.pow(1024, i)).toFixed(1)} ${units[i]}`
}

function formatDate(iso: string): string {
  const d = new Date(iso)
  const now = new Date()
  const diff = now.getTime() - d.getTime()
  const mins = Math.floor(diff / 60000)
  if (mins < 1) return 'just now'
  if (mins < 60) return `${mins}m ago`
  const hours = Math.floor(mins / 60)
  if (hours < 24) return `${hours}h ago`
  const days = Math.floor(hours / 24)
  if (days < 7) return `${days}d ago`
  return d.toLocaleDateString()
}
