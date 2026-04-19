/* eslint-disable react-refresh/only-export-components -- context files co-export provider + context value */
import { createContext, useState, useEffect, useCallback, type ReactNode } from 'react'

export interface AuthUser {
  id: number
  username: string
  displayName: string
  role: string
  avatarUrl: string | null
  authMethod?: string
}

export interface AuthContextValue {
  user: AuthUser | null
  loading: boolean
  login: (username: string, password: string) => Promise<void>
  register: (username: string, password: string, displayName: string, inviteCode: string) => Promise<void>
  logout: () => Promise<void>
}

export const AuthContext = createContext<AuthContextValue | null>(null)

function getApiBase(): string {
  const stored = localStorage.getItem('stackarr_server')
  if (stored) {
    try {
      const conn = JSON.parse(stored)
      return `${conn.serverUrl}/api/v1`
    } catch { /* fall through */ }
  }
  return '/api/v1'
}

export function AuthProvider({ children }: { children: ReactNode }) {
  const [user, setUser] = useState<AuthUser | null>(null)
  const [loading, setLoading] = useState(true)

  const fetchMe = useCallback(async () => {
    try {
      const res = await fetch(`${getApiBase()}/auth/me`, {
        credentials: 'include',
      })
      if (res.ok) {
        const data = await res.json()
        setUser(data)
      } else {
        setUser(null)
      }
    } catch {
      setUser(null)
    } finally {
      setLoading(false)
    }
  }, [])

  useEffect(() => {
    fetchMe()
  }, [fetchMe])

  const login = useCallback(async (username: string, password: string) => {
    const res = await fetch(`${getApiBase()}/auth/login`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      credentials: 'include',
      body: JSON.stringify({ username, password }),
    })
    if (!res.ok) {
      const body = await res.json().catch(() => ({ error: 'Login failed' }))
      throw new Error(body.error || 'Login failed')
    }
    const data = await res.json()
    setUser(data.user)
  }, [])

  const register = useCallback(async (
    username: string,
    password: string,
    displayName: string,
    inviteCode: string,
  ) => {
    const res = await fetch(`${getApiBase()}/auth/register`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      credentials: 'include',
      body: JSON.stringify({ username, password, displayName, inviteCode }),
    })
    if (!res.ok) {
      const body = await res.json().catch(() => ({ error: 'Registration failed' }))
      throw new Error(body.error || 'Registration failed')
    }
    const data = await res.json()
    setUser(data.user)
  }, [])

  const logout = useCallback(async () => {
    try {
      await fetch(`${getApiBase()}/auth/logout`, {
        method: 'POST',
        credentials: 'include',
      })
    } catch {
      // best effort
    }
    setUser(null)
  }, [])

  return (
    <AuthContext.Provider value={{ user, loading, login, register, logout }}>
      {children}
    </AuthContext.Provider>
  )
}

