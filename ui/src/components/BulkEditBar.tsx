// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 The StackArr Authors

import { useState } from 'react'
import type { QualityProfile } from '../api/types'

export default function BulkEditBar({
  selectedCount,
  totalCount,
  qualityProfiles,
  onApply,
  onSelectAll,
  onSelectNone,
  isPending,
}: {
  selectedCount: number
  totalCount: number
  qualityProfiles: QualityProfile[]
  onApply: (profileId?: number, monitored?: boolean) => void
  onSelectAll: () => void
  onSelectNone: () => void
  isPending: boolean
}) {
  const [profileId, setProfileId] = useState<number | ''>('')
  const [monitored, setMonitored] = useState<'' | 'true' | 'false'>('')

  const canApply = !isPending && (profileId !== '' || monitored !== '')

  return (
    <div className="fixed inset-x-0 bottom-0 z-40 border-t border-slate-600 bg-slate-800 px-6 py-3 shadow-2xl">
      <div className="mx-auto flex max-w-7xl flex-wrap items-center gap-3">
        <span className="text-sm font-medium text-white">
          {selectedCount} of {totalCount} selected
        </span>

        <button onClick={onSelectAll} className="text-xs text-blue-400 hover:text-blue-300">
          Select All
        </button>
        <button onClick={onSelectNone} className="text-xs text-slate-400 hover:text-slate-300">
          Clear
        </button>

        <div className="mx-1 h-6 w-px bg-slate-600" />

        <select
          value={profileId}
          onChange={(e) => setProfileId(e.target.value === '' ? '' : Number(e.target.value))}
          className="rounded-lg border border-slate-600 bg-slate-700 px-3 py-1.5 text-sm text-white focus:border-blue-500 focus:outline-none focus:ring-1 focus:ring-blue-500"
        >
          <option value="">Quality Profile...</option>
          {qualityProfiles.map((p) => (
            <option key={p.id} value={p.id}>{p.name}</option>
          ))}
        </select>

        <select
          value={monitored}
          onChange={(e) => setMonitored(e.target.value as '' | 'true' | 'false')}
          className="rounded-lg border border-slate-600 bg-slate-700 px-3 py-1.5 text-sm text-white focus:border-blue-500 focus:outline-none focus:ring-1 focus:ring-blue-500"
        >
          <option value="">Monitored...</option>
          <option value="true">Monitored</option>
          <option value="false">Unmonitored</option>
        </select>

        <button
          onClick={() => onApply(
            profileId !== '' ? profileId : undefined,
            monitored !== '' ? monitored === 'true' : undefined,
          )}
          disabled={!canApply}
          className="rounded-lg bg-blue-600 px-4 py-1.5 text-sm font-medium text-white hover:bg-blue-700 disabled:opacity-50 transition-colors"
        >
          {isPending ? 'Applying...' : 'Apply'}
        </button>
      </div>
    </div>
  )
}
