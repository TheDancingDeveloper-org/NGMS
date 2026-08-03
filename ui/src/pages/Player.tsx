// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 The StackArr Authors

import { useParams, useNavigate } from 'react-router-dom'
import { ArrowLeft } from 'lucide-react'
import VideoPlayer from '../components/VideoPlayer'

export default function Player() {
  const { mediaFileId } = useParams<{ mediaFileId: string }>()
  const navigate = useNavigate()
  const id = Number(mediaFileId)

  if (!id || isNaN(id)) {
    return (
      <div className="rounded-lg bg-red-900/30 p-6 text-red-400">
        Invalid media file ID
      </div>
    )
  }

  return (
    <div className="mx-auto max-w-6xl space-y-4">
      <button
        onClick={() => navigate(-1)}
        className="flex items-center gap-2 text-sm text-slate-400 hover:text-white transition-colors"
      >
        <ArrowLeft size={16} />
        Back
      </button>

      <VideoPlayer mediaFileId={id} />
    </div>
  )
}
