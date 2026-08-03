// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 The StackArr Authors

import { useContext } from 'react'
import { AuthContext } from '../context/AuthContext'
import type { AuthContextValue } from '../context/AuthContext'

export type { AuthContextValue }

export function useAuth(): AuthContextValue {
  const ctx = useContext(AuthContext)
  if (!ctx) throw new Error('useAuth must be used within an AuthProvider')
  return ctx
}
