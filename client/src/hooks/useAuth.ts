import { useContext } from 'react'
import { AuthContext } from '../context/AuthContext'
import type { AuthContextValue } from '../context/AuthContext'

export type { AuthContextValue }

export function useAuth(): AuthContextValue {
  const ctx = useContext(AuthContext)
  if (!ctx) throw new Error('useAuth must be used within an AuthProvider')
  return ctx
}
