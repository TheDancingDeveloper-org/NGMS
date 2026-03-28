import { useState } from 'react'
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query'
import { apiFetch } from '../api/client'
import { UserPlus, Trash2, Copy, Shield, User as UserIcon, Key } from 'lucide-react'

interface UserItem {
  id: number
  username: string
  displayName: string
  role: string
  enabled: boolean
  avatarUrl: string | null
  createdAt: string
  updatedAt: string
}

interface InviteItem {
  id: number
  code: string
  createdBy: number
  claimedBy: number | null
  role: string
  expiresAt: string | null
  createdAt: string
}

export default function Users() {
  const queryClient = useQueryClient()
  const [showCreateUser, setShowCreateUser] = useState(false)
  const [showCreateInvite, setShowCreateInvite] = useState(false)
  const [copiedCode, setCopiedCode] = useState<string | null>(null)

  const { data: users = [], isLoading: usersLoading } = useQuery({
    queryKey: ['admin', 'users'],
    queryFn: () => apiFetch<UserItem[]>('/admin/users'),
  })

  const { data: invites = [], isLoading: invitesLoading } = useQuery({
    queryKey: ['admin', 'invites'],
    queryFn: () => apiFetch<InviteItem[]>('/admin/invites'),
  })

  const deleteUser = useMutation({
    mutationFn: (id: number) =>
      apiFetch(`/admin/users/${id}`, { method: 'DELETE' }),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ['admin', 'users'] }),
  })

  const deleteInvite = useMutation({
    mutationFn: (id: number) =>
      apiFetch(`/admin/invites/${id}`, { method: 'DELETE' }),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ['admin', 'invites'] }),
  })

  const createInvite = useMutation({
    mutationFn: (data: { role?: string; expiresInHours?: number }) =>
      apiFetch<InviteItem>('/admin/invites', {
        method: 'POST',
        body: JSON.stringify(data),
      }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['admin', 'invites'] })
      setShowCreateInvite(false)
    },
  })

  function copyCode(code: string) {
    void import('../utils/clipboard').then(({ copyToClipboard }) =>
      copyToClipboard(code).then((ok) => {
        if (ok) {
          setCopiedCode(code)
          setTimeout(() => setCopiedCode(null), 2000)
        }
      }),
    )
  }

  return (
    <div className="space-y-8">
      <div className="flex items-center justify-between">
        <h1 className="text-2xl font-bold">User Management</h1>
        <div className="flex gap-2">
          <button
            onClick={() => setShowCreateInvite(true)}
            className="flex items-center gap-2 rounded-lg bg-emerald-600 px-4 py-2 text-sm font-medium text-white hover:bg-emerald-500"
          >
            <Key size={16} /> Create Invite
          </button>
          <button
            onClick={() => setShowCreateUser(true)}
            className="flex items-center gap-2 rounded-lg bg-blue-600 px-4 py-2 text-sm font-medium text-white hover:bg-blue-500"
          >
            <UserPlus size={16} /> Add User
          </button>
        </div>
      </div>

      {/* Users table */}
      <section>
        <h2 className="mb-3 text-lg font-semibold text-slate-300">Users</h2>
        {usersLoading ? (
          <p className="text-sm text-slate-400">Loading...</p>
        ) : (
          <div className="overflow-hidden rounded-lg border border-slate-700">
            <table className="w-full text-sm">
              <thead>
                <tr className="border-b border-slate-700 bg-slate-800/50">
                  <th className="px-4 py-3 text-left font-medium text-slate-400">User</th>
                  <th className="px-4 py-3 text-left font-medium text-slate-400">Role</th>
                  <th className="px-4 py-3 text-left font-medium text-slate-400">Status</th>
                  <th className="px-4 py-3 text-left font-medium text-slate-400">Created</th>
                  <th className="px-4 py-3 text-right font-medium text-slate-400">Actions</th>
                </tr>
              </thead>
              <tbody>
                {users.map((user) => (
                  <tr key={user.id} className="border-b border-slate-700/50 hover:bg-slate-800/30">
                    <td className="px-4 py-3">
                      <div className="flex items-center gap-3">
                        <div className="flex h-8 w-8 items-center justify-center rounded-full bg-slate-700 text-xs font-bold text-slate-300">
                          {user.displayName.charAt(0).toUpperCase()}
                        </div>
                        <div>
                          <div className="font-medium text-white">{user.displayName}</div>
                          <div className="text-xs text-slate-500">@{user.username}</div>
                        </div>
                      </div>
                    </td>
                    <td className="px-4 py-3">
                      <span className={`inline-flex items-center gap-1 rounded-full px-2 py-0.5 text-xs font-medium ${
                        user.role === 'admin'
                          ? 'bg-amber-500/10 text-amber-400'
                          : 'bg-blue-500/10 text-blue-400'
                      }`}>
                        {user.role === 'admin' ? <Shield size={12} /> : <UserIcon size={12} />}
                        {user.role}
                      </span>
                    </td>
                    <td className="px-4 py-3">
                      <span className={`text-xs ${user.enabled ? 'text-green-400' : 'text-red-400'}`}>
                        {user.enabled ? 'Active' : 'Disabled'}
                      </span>
                    </td>
                    <td className="px-4 py-3 text-slate-400">
                      {new Date(user.createdAt).toLocaleDateString()}
                    </td>
                    <td className="px-4 py-3 text-right">
                      <button
                        onClick={() => {
                          if (confirm(`Delete user ${user.username}?`)) {
                            deleteUser.mutate(user.id)
                          }
                        }}
                        className="text-slate-500 hover:text-red-400"
                        title="Delete user"
                      >
                        <Trash2 size={16} />
                      </button>
                    </td>
                  </tr>
                ))}
                {users.length === 0 && (
                  <tr>
                    <td colSpan={5} className="px-4 py-8 text-center text-slate-500">
                      No users yet
                    </td>
                  </tr>
                )}
              </tbody>
            </table>
          </div>
        )}
      </section>

      {/* Invites table */}
      <section>
        <h2 className="mb-3 text-lg font-semibold text-slate-300">Invite Codes</h2>
        {invitesLoading ? (
          <p className="text-sm text-slate-400">Loading...</p>
        ) : (
          <div className="overflow-hidden rounded-lg border border-slate-700">
            <table className="w-full text-sm">
              <thead>
                <tr className="border-b border-slate-700 bg-slate-800/50">
                  <th className="px-4 py-3 text-left font-medium text-slate-400">Code</th>
                  <th className="px-4 py-3 text-left font-medium text-slate-400">Role</th>
                  <th className="px-4 py-3 text-left font-medium text-slate-400">Status</th>
                  <th className="px-4 py-3 text-left font-medium text-slate-400">Expires</th>
                  <th className="px-4 py-3 text-right font-medium text-slate-400">Actions</th>
                </tr>
              </thead>
              <tbody>
                {invites.map((invite) => (
                  <tr key={invite.id} className="border-b border-slate-700/50 hover:bg-slate-800/30">
                    <td className="px-4 py-3">
                      <div className="flex items-center gap-2">
                        <code className="rounded bg-slate-800 px-2 py-1 font-mono text-sm text-blue-400">
                          {invite.code}
                        </code>
                        <button
                          onClick={() => copyCode(invite.code)}
                          className="text-slate-500 hover:text-blue-400"
                          title="Copy code"
                        >
                          <Copy size={14} />
                        </button>
                        {copiedCode === invite.code && (
                          <span className="text-xs text-green-400">Copied!</span>
                        )}
                      </div>
                    </td>
                    <td className="px-4 py-3 text-slate-300">{invite.role}</td>
                    <td className="px-4 py-3">
                      <span className={`text-xs ${
                        invite.claimedBy ? 'text-slate-500' : 'text-green-400'
                      }`}>
                        {invite.claimedBy ? 'Claimed' : 'Available'}
                      </span>
                    </td>
                    <td className="px-4 py-3 text-slate-400">
                      {invite.expiresAt
                        ? new Date(invite.expiresAt).toLocaleDateString()
                        : 'Never'}
                    </td>
                    <td className="px-4 py-3 text-right">
                      <button
                        onClick={() => deleteInvite.mutate(invite.id)}
                        className="text-slate-500 hover:text-red-400"
                        title="Delete invite"
                      >
                        <Trash2 size={16} />
                      </button>
                    </td>
                  </tr>
                ))}
                {invites.length === 0 && (
                  <tr>
                    <td colSpan={5} className="px-4 py-8 text-center text-slate-500">
                      No invite codes
                    </td>
                  </tr>
                )}
              </tbody>
            </table>
          </div>
        )}
      </section>

      {/* Create User Modal */}
      {showCreateUser && (
        <CreateUserModal onClose={() => setShowCreateUser(false)} />
      )}

      {/* Create Invite Modal */}
      {showCreateInvite && (
        <CreateInviteModal
          onClose={() => setShowCreateInvite(false)}
          onCreate={(data) => createInvite.mutate(data)}
        />
      )}
    </div>
  )
}

function CreateUserModal({ onClose }: { onClose: () => void }) {
  const queryClient = useQueryClient()
  const [username, setUsername] = useState('')
  const [displayName, setDisplayName] = useState('')
  const [password, setPassword] = useState('')
  const [role, setRole] = useState('user')
  const [error, setError] = useState<string | null>(null)

  const createUser = useMutation({
    mutationFn: (data: { username: string; password: string; displayName?: string; role: string }) =>
      apiFetch('/admin/users', {
        method: 'POST',
        body: JSON.stringify(data),
      }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['admin', 'users'] })
      onClose()
    },
    onError: (err: Error) => setError(err.message),
  })

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50" onClick={onClose}>
      <div className="w-full max-w-md rounded-lg border border-slate-700 bg-slate-800 p-6" onClick={(e) => e.stopPropagation()}>
        <h3 className="mb-4 text-lg font-semibold">Create User</h3>
        <div className="space-y-3">
          <div>
            <label className="mb-1 block text-xs font-medium text-slate-400">Username</label>
            <input
              value={username}
              onChange={(e) => setUsername(e.target.value)}
              className="w-full rounded-lg border border-slate-600 bg-slate-900 px-3 py-2 text-sm text-white"
            />
          </div>
          <div>
            <label className="mb-1 block text-xs font-medium text-slate-400">Display Name</label>
            <input
              value={displayName}
              onChange={(e) => setDisplayName(e.target.value)}
              className="w-full rounded-lg border border-slate-600 bg-slate-900 px-3 py-2 text-sm text-white"
            />
          </div>
          <div>
            <label className="mb-1 block text-xs font-medium text-slate-400">Password</label>
            <input
              type="password"
              value={password}
              onChange={(e) => setPassword(e.target.value)}
              className="w-full rounded-lg border border-slate-600 bg-slate-900 px-3 py-2 text-sm text-white"
            />
          </div>
          <div>
            <label className="mb-1 block text-xs font-medium text-slate-400">Role</label>
            <select
              value={role}
              onChange={(e) => setRole(e.target.value)}
              className="w-full rounded-lg border border-slate-600 bg-slate-900 px-3 py-2 text-sm text-white"
            >
              <option value="user">User</option>
              <option value="admin">Admin</option>
            </select>
          </div>
          {error && <p className="text-sm text-red-400">{error}</p>}
          <div className="flex justify-end gap-2 pt-2">
            <button onClick={onClose} className="rounded-lg bg-slate-700 px-4 py-2 text-sm text-slate-300 hover:bg-slate-600">
              Cancel
            </button>
            <button
              onClick={() => createUser.mutate({ username, password, displayName: displayName || undefined, role })}
              className="rounded-lg bg-blue-600 px-4 py-2 text-sm text-white hover:bg-blue-500"
            >
              Create
            </button>
          </div>
        </div>
      </div>
    </div>
  )
}

function CreateInviteModal({
  onClose,
  onCreate,
}: {
  onClose: () => void
  onCreate: (data: { role?: string; expiresInHours?: number }) => void
}) {
  const [role, setRole] = useState('user')
  const [expiresHours, setExpiresHours] = useState('')

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50" onClick={onClose}>
      <div className="w-full max-w-md rounded-lg border border-slate-700 bg-slate-800 p-6" onClick={(e) => e.stopPropagation()}>
        <h3 className="mb-4 text-lg font-semibold">Create Invite Code</h3>
        <div className="space-y-3">
          <div>
            <label className="mb-1 block text-xs font-medium text-slate-400">Role</label>
            <select
              value={role}
              onChange={(e) => setRole(e.target.value)}
              className="w-full rounded-lg border border-slate-600 bg-slate-900 px-3 py-2 text-sm text-white"
            >
              <option value="user">User</option>
              <option value="admin">Admin</option>
            </select>
          </div>
          <div>
            <label className="mb-1 block text-xs font-medium text-slate-400">Expires in (hours, empty = never)</label>
            <input
              type="number"
              value={expiresHours}
              onChange={(e) => setExpiresHours(e.target.value)}
              placeholder="e.g. 24"
              className="w-full rounded-lg border border-slate-600 bg-slate-900 px-3 py-2 text-sm text-white"
            />
          </div>
          <div className="flex justify-end gap-2 pt-2">
            <button onClick={onClose} className="rounded-lg bg-slate-700 px-4 py-2 text-sm text-slate-300 hover:bg-slate-600">
              Cancel
            </button>
            <button
              onClick={() => onCreate({
                role,
                expiresInHours: expiresHours ? parseInt(expiresHours, 10) : undefined,
              })}
              className="rounded-lg bg-emerald-600 px-4 py-2 text-sm text-white hover:bg-emerald-500"
            >
              Create
            </button>
          </div>
        </div>
      </div>
    </div>
  )
}
