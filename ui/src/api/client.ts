// ── Connection management ───────────────────────────────────────────────────

export interface ServerConnection {
  serverUrl: string
  serverName: string
  serverId: string
  clientToken: string
}

const STORAGE_KEY = 'stackarr_server'

export function getConnection(): ServerConnection | null {
  try {
    const stored = localStorage.getItem(STORAGE_KEY)
    return stored ? JSON.parse(stored) : null
  } catch {
    return null
  }
}

export function saveConnection(conn: ServerConnection) {
  localStorage.setItem(STORAGE_KEY, JSON.stringify(conn))
}

export function clearConnection() {
  localStorage.removeItem(STORAGE_KEY)
}

function getApiBase(): string {
  const conn = getConnection()
  return conn ? `${conn.serverUrl}/api/v1` : '/api/v1'
}

export function authHeaders(): Record<string, string> {
  const conn = getConnection()
  if (conn?.clientToken) {
    return { Authorization: `Bearer ${conn.clientToken}` }
  }
  return {}
}

// ── API client ──────────────────────────────────────────────────────────────

export async function apiFetch<T>(path: string, options?: RequestInit): Promise<T> {
  const base = getApiBase()
  const headers: Record<string, string> = {
    'Content-Type': 'application/json',
    ...authHeaders(),
    ...(options?.headers as Record<string, string>),
  }
  const res = await fetch(`${base}${path}`, { ...options, headers })
  if (!res.ok) throw new Error(`API error: ${res.status} ${res.statusText}`)
  if (res.status === 204 || res.headers.get('content-length') === '0') return undefined as T
  return res.json() as Promise<T>
}

// ── Bootstrap discovery ─────────────────────────────────────────────────────

interface ClaimRedeemResponse {
  serverId: string
  serverName: string
  publicIp: string
  localIps: string[]
  port: number
  clientToken: string
  version: string
}

export async function redeemClaimCode(
  code: string,
  clientName: string,
  bootstrapUrl: string,
): Promise<ServerConnection> {
  const res = await fetch(`${bootstrapUrl}/api/v1/claims/${code.toUpperCase()}/redeem`, {
    method: 'POST',
  })
  if (!res.ok) {
    if (res.status === 404) throw new Error('Invalid or expired claim code')
    throw new Error(`Bootstrap error: ${res.status}`)
  }
  const data: ClaimRedeemResponse = await res.json()

  const urls = [
    ...data.localIps.map((ip: string) => `http://${ip}:${data.port}`),
    `http://${data.publicIp}:${data.port}`,
  ]

  for (const url of urls) {
    try {
      const probe = await fetch(`${url}/api/v1/system/status`, {
        signal: AbortSignal.timeout(3000),
      })
      if (probe.ok) {
        const conn: ServerConnection = {
          serverUrl: url,
          serverName: data.serverName,
          serverId: data.serverId,
          clientToken: data.clientToken,
        }
        saveConnection(conn)

        // Register client name with the server
        try {
          await fetch(`${url}/api/v1/remote/register`, {
            method: 'POST',
            headers: {
              'Content-Type': 'application/json',
              Authorization: `Bearer ${data.clientToken}`,
            },
            body: JSON.stringify({ clientName }),
          })
        } catch {
          // Non-fatal
        }

        return conn
      }
    } catch {
      // Try next
    }
  }

  throw new Error('Server found but unreachable. Check your network/firewall.')
}
