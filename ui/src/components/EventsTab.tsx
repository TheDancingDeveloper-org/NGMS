import {
  Download,
  Upload,
  Trash2,
  ArrowUpCircle,
  XCircle,
  FileText,
  Eye,
} from 'lucide-react'
import type { HistoryEvent } from '../api/types'
import { qualityName } from '../api/types'

function relativeTime(dateStr: string): string {
  const diff = (Date.now() - new Date(dateStr).getTime()) / 1000
  if (diff < 60) return 'just now'
  if (diff < 3600) return `${Math.floor(diff / 60)}m ago`
  if (diff < 86400) return `${Math.floor(diff / 3600)}h ago`
  if (diff < 604800) return `${Math.floor(diff / 86400)}d ago`
  return new Date(dateStr).toLocaleDateString()
}

function eventIcon(type: string) {
  switch (type) {
    case 'grabbed':
      return <Download size={14} />
    case 'imported':
      return <Upload size={14} />
    case 'fileDeleted':
      return <Trash2 size={14} />
    case 'upgraded':
      return <ArrowUpCircle size={14} />
    case 'downloadFailed':
      return <XCircle size={14} />
    case 'fileRenamed':
      return <FileText size={14} />
    case 'downloadIgnored':
      return <Eye size={14} />
    default:
      return <FileText size={14} />
  }
}

function eventStyle(type: string) {
  switch (type) {
    case 'grabbed':
      return { icon: 'bg-blue-900/60 text-blue-400', label: 'Grabbed', labelColor: 'text-blue-400' }
    case 'imported':
      return { icon: 'bg-green-900/60 text-green-400', label: 'Imported', labelColor: 'text-green-400' }
    case 'fileDeleted':
      return { icon: 'bg-orange-900/60 text-orange-400', label: 'Upgraded', labelColor: 'text-orange-400' }
    case 'upgraded':
      return { icon: 'bg-cyan-900/60 text-cyan-400', label: 'Upgraded', labelColor: 'text-cyan-400' }
    case 'downloadFailed':
      return { icon: 'bg-red-900/60 text-red-400', label: 'Failed', labelColor: 'text-red-400' }
    case 'fileRenamed':
      return { icon: 'bg-purple-900/60 text-purple-400', label: 'Renamed', labelColor: 'text-purple-400' }
    case 'downloadIgnored':
      return { icon: 'bg-yellow-900/60 text-yellow-400', label: 'Ignored', labelColor: 'text-yellow-400' }
    default:
      return { icon: 'bg-slate-700 text-slate-400', label: type, labelColor: 'text-slate-400' }
  }
}

/** Build a context line from the event's data field for upgrade/delete events. */
function upgradeContext(event: HistoryEvent): string | null {
  if (!event.data) return null
  const d = event.data
  if (d.reason === 'upgrade' && d.replaced_by_quality) {
    const recycled = d.recycled ? 'moved to recycle bin' : 'permanently deleted'
    return `Replaced by ${d.replaced_by_quality} (${recycled})`
  }
  return null
}

export default function EventsTab({ events }: { events: HistoryEvent[] }) {
  if (events.length === 0) {
    return (
      <div className="flex items-center justify-center py-12 text-sm text-slate-500">
        No recent events
      </div>
    )
  }

  return (
    <div className="max-h-[380px] overflow-y-auto">
      {events.map((event) => {
        const style = eventStyle(event.eventType)
        const quality = qualityName(event.quality)
        const context = upgradeContext(event)

        return (
          <div
            key={event.id}
            className="flex gap-3 border-b border-slate-700 px-4 py-2.5 transition-colors hover:bg-slate-700/30"
          >
            {/* Icon */}
            <div
              className={`flex h-7 w-7 flex-shrink-0 items-center justify-center rounded-full ${style.icon}`}
            >
              {eventIcon(event.eventType)}
            </div>

            {/* Content */}
            <div className="min-w-0 flex-1">
              <div className="flex items-center gap-2">
                <span className={`text-[11px] font-semibold uppercase ${style.labelColor}`}>
                  {style.label}
                </span>
                {quality && quality !== 'Unknown' && (
                  <span className="rounded bg-blue-500/20 px-1.5 py-0.5 text-[10px] font-medium text-blue-400">
                    {quality}
                  </span>
                )}
              </div>
              <div className="truncate text-[12px] text-slate-300" title={event.sourceTitle}>
                {event.sourceTitle}
              </div>
              {context && (
                <div className="truncate text-[11px] text-slate-500">{context}</div>
              )}
              <div className="mt-0.5 text-[10px] text-slate-600">
                {relativeTime(event.date)}
              </div>
            </div>
          </div>
        )
      })}
    </div>
  )
}
