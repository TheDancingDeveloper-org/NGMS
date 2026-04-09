import { useState } from 'react'
import { Monitor, Smartphone, Globe, Trash2, LogOut } from 'lucide-react'
import { useAuth } from '../hooks/useAuth'
import { useMobile } from '../hooks/useMobile'
import {
  useDevices, useDeleteDevice,
  useSessions, useUpdateProfile,
} from '../hooks/useApi'

function relativeTime(dateStr: string): string {
  const diff = Date.now() - new Date(dateStr).getTime()
  const mins = Math.floor(diff / 60_000)
  if (mins < 1) return 'just now'
  if (mins < 60) return `${mins}m ago`
  const hours = Math.floor(mins / 60)
  if (hours < 24) return `${hours}h ago`
  const days = Math.floor(hours / 24)
  return `${days}d ago`
}

function deviceIcon(name: string) {
  const lower = name.toLowerCase()
  if (lower.includes('mobile') || lower.includes('phone') || lower.includes('android') || lower.includes('ios')) {
    return <Smartphone size={18} />
  }
  if (lower.includes('web') || lower.includes('browser')) {
    return <Globe size={18} />
  }
  return <Monitor size={18} />
}

const sectionStyle: React.CSSProperties = {
  background: '#1e293b', borderRadius: 12, border: '1px solid #334155',
  padding: 20, marginBottom: 20,
}

const labelStyle: React.CSSProperties = {
  fontSize: 12, color: '#64748b', marginBottom: 4,
}

export default function AccountPage() {
  const { user } = useAuth()
  const isMobile = useMobile()
  const { data: devices = [], isLoading: devicesLoading } = useDevices()
  const { data: sessions = [], isLoading: sessionsLoading } = useSessions()
  const deleteDevice = useDeleteDevice()
  const updateProfile = useUpdateProfile()

  const [displayName, setDisplayName] = useState(user?.displayName ?? '')
  const [saving, setSaving] = useState(false)
  const [saveMessage, setSaveMessage] = useState<string | null>(null)

  if (!user) return null

  const handleSaveProfile = async () => {
    setSaving(true)
    setSaveMessage(null)
    try {
      await updateProfile.mutateAsync({ displayName })
      setSaveMessage('Profile updated')
    } catch (e) {
      setSaveMessage(e instanceof Error ? e.message : 'Failed to save')
    } finally {
      setSaving(false)
    }
  }

  return (
    <div style={{ maxWidth: 700, margin: '0 auto' }}>
      <h2 style={{ color: '#f1f5f9', fontSize: 20, fontWeight: 700, marginBottom: 20 }}>
        Account
      </h2>

      {/* Profile */}
      <div style={sectionStyle}>
        <h3 style={{ color: '#e2e8f0', fontSize: 16, fontWeight: 600, marginBottom: 16 }}>
          Profile
        </h3>
        <div style={{ marginBottom: 12 }}>
          <div style={labelStyle}>Username</div>
          <div style={{ color: '#94a3b8', fontSize: 14 }}>{user.username}</div>
        </div>
        <div style={{ marginBottom: 12 }}>
          <div style={labelStyle}>Role</div>
          <span style={{
            padding: '2px 8px', borderRadius: 4, fontSize: 12, fontWeight: 600,
            background: user.role === 'admin' ? '#1e40af' : '#334155',
            color: user.role === 'admin' ? '#93c5fd' : '#94a3b8',
          }}>
            {user.role}
          </span>
        </div>
        <div style={{ marginBottom: 16 }}>
          <div style={labelStyle}>Display Name</div>
          <div style={{ display: 'flex', gap: 8 }}>
            <input
              type="text"
              value={displayName}
              onChange={(e) => setDisplayName(e.target.value)}
              style={{
                flex: 1, padding: '8px 12px', borderRadius: 8,
                border: '1px solid #334155', background: '#0f172a',
                color: '#f1f5f9', fontSize: 14, outline: 'none',
              }}
            />
            <button
              onClick={handleSaveProfile}
              disabled={saving || displayName === user.displayName}
              style={{
                padding: '8px 16px', borderRadius: 8, border: 'none',
                background: displayName !== user.displayName ? '#1e40af' : '#334155',
                color: '#fff', fontSize: 13, fontWeight: 500,
                cursor: displayName !== user.displayName ? 'pointer' : 'default',
                opacity: displayName !== user.displayName ? 1 : 0.5,
              }}
            >
              {saving ? 'Saving...' : 'Save'}
            </button>
          </div>
          {saveMessage && (
            <div style={{
              fontSize: 12, marginTop: 6,
              color: saveMessage === 'Profile updated' ? '#4ade80' : '#f87171',
            }}>
              {saveMessage}
            </div>
          )}
        </div>
      </div>

      {/* Devices */}
      <div style={sectionStyle}>
        <h3 style={{ color: '#e2e8f0', fontSize: 16, fontWeight: 600, marginBottom: 16 }}>
          Devices
        </h3>
        {devicesLoading ? (
          <div style={{ color: '#64748b', fontSize: 13 }}>Loading...</div>
        ) : devices.length === 0 ? (
          <div style={{ color: '#64748b', fontSize: 13 }}>No linked devices</div>
        ) : (
          <div style={{ display: 'flex', flexDirection: 'column', gap: 8 }}>
            {devices.map((device) => (
              <div
                key={device.id}
                style={{
                  display: 'flex', alignItems: 'center', justifyContent: 'space-between',
                  padding: '10px 14px', borderRadius: 8, background: '#0f172a',
                }}
              >
                <div style={{ display: 'flex', alignItems: 'center', gap: 12 }}>
                  <span style={{ color: '#64748b' }}>{deviceIcon(device.deviceName)}</span>
                  <div>
                    <div style={{ fontSize: 14, color: '#f1f5f9' }}>{device.deviceName}</div>
                    <div style={{ fontSize: 11, color: '#64748b' }}>
                      Last active {relativeTime(device.lastActive)}
                    </div>
                  </div>
                </div>
                <button
                  onClick={() => deleteDevice.mutate(device.id)}
                  style={{
                    display: 'flex', alignItems: 'center', gap: 4,
                    padding: '4px 10px', borderRadius: 6,
                    background: 'transparent', border: '1px solid #334155',
                    color: '#f87171', cursor: 'pointer', fontSize: 12,
                  }}
                  title="Remove device"
                >
                  <Trash2 size={12} /> Remove
                </button>
              </div>
            ))}
          </div>
        )}
      </div>

      {/* Sessions */}
      <div style={sectionStyle}>
        <h3 style={{ color: '#e2e8f0', fontSize: 16, fontWeight: 600, marginBottom: 16 }}>
          Active Sessions
        </h3>
        {sessionsLoading ? (
          <div style={{ color: '#64748b', fontSize: 13 }}>Loading...</div>
        ) : sessions.length === 0 ? (
          <div style={{ color: '#64748b', fontSize: 13 }}>No active sessions</div>
        ) : (
          <div style={{ display: 'flex', flexDirection: 'column', gap: 8 }}>
            {sessions.map((session) => (
              <div
                key={session.id}
                style={{
                  display: 'flex', alignItems: isMobile ? 'flex-start' : 'center',
                  justifyContent: 'space-between',
                  padding: '10px 14px', borderRadius: 8, background: '#0f172a',
                  flexDirection: isMobile ? 'column' : 'row',
                  gap: isMobile ? 8 : 0,
                }}
              >
                <div style={{ display: 'flex', alignItems: 'center', gap: 12 }}>
                  <LogOut size={16} style={{ color: '#64748b' }} />
                  <div>
                    <div style={{
                      fontSize: 13, color: '#94a3b8',
                      maxWidth: isMobile ? 240 : 400,
                      overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap',
                    }}>
                      {session.userAgent || 'Unknown client'}
                    </div>
                    <div style={{ fontSize: 11, color: '#64748b' }}>
                      Active {relativeTime(session.lastActive)} · Created {relativeTime(session.createdAt)}
                    </div>
                  </div>
                </div>
              </div>
            ))}
          </div>
        )}
      </div>
    </div>
  )
}
