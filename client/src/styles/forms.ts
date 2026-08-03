// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 The StackArr Authors

import type { CSSProperties } from 'react'

export const labelStyle: CSSProperties = {
  display: 'block', color: '#94a3b8', fontSize: 12,
  fontWeight: 500, marginBottom: 4,
}

export const inputStyle: CSSProperties = {
  width: '100%', padding: '10px 12px', borderRadius: 6,
  border: '1px solid #475569', background: '#0f172a',
  color: '#e2e8f0', fontSize: 14, marginBottom: 12,
  boxSizing: 'border-box',
}

export const buttonStyle: CSSProperties = {
  width: '100%', padding: '12px 16px', borderRadius: 8,
  border: 'none', background: '#3b82f6', color: '#fff',
  fontSize: 15, fontWeight: 600, cursor: 'pointer',
}

export const buttonStyleDisabled = (disabled: boolean): CSSProperties => ({
  ...buttonStyle,
  ...(disabled ? { opacity: 0.6, cursor: 'not-allowed' } : {}),
})
