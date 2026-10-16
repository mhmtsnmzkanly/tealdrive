import React, { useState, useEffect } from 'react';
import { tealdrive } from '../../lib/tealdrive.ts';
import type {
  VirtualUser,
  SystemUser,
  StorageBudget,
  ServerSettings,
} from '../../sdk/types.ts';
import { formatBytes, formatRelativeDate } from '../../utils/formatters.ts';
import { useTealDrive } from '../../context/TealDriveContext.tsx';
import {
  Users,
  Terminal,
  PieChart,
  Settings,
  UserPlus,
  UserX,
  ShieldAlert,
  CheckCircle,
  XCircle,
  Trash2,
  Sliders,
  Server,
  Lock,
} from 'lucide-react';

export const AdminConsole: React.FC = () => {
  const { addToast, promptCustomDelete } = useTealDrive();
  const [activeTab, setActiveTab] = useState<'virtual' | 'system' | 'budget' | 'settings'>('virtual');

  const [virtualUsers, setVirtualUsers] = useState<VirtualUser[]>([]);
  const [systemUsers, setSystemUsers] = useState<SystemUser[]>([]);
  const [budget, setBudget] = useState<StorageBudget | null>(null);
  const [settings, setSettings] = useState<ServerSettings | null>(null);
  const [loading, setLoading] = useState(false);

  // Create Virtual User modal
  const [createModalOpen, setCreateModalOpen] = useState(false);
  const [newUsername, setNewUsername] = useState('');
  const [newEmail, setNewEmail] = useState('');
  const [newQuotaGB, setNewQuotaGB] = useState('5');

  const loadData = async () => {
    setLoading(true);
    try {
      const [vuRes, suRes, bRes, sRes] = await Promise.allSettled([
        tealdrive.admin.virtualUsers.list(),
        tealdrive.admin.systemUsers.list(),
        tealdrive.admin.quota.getBudget(),
        tealdrive.admin.settings.inspect(),
      ]);
      if (vuRes.status === 'fulfilled') setVirtualUsers(vuRes.value);
      if (suRes.status === 'fulfilled') setSystemUsers(suRes.value);
      if (bRes.status === 'fulfilled') setBudget(bRes.value);
      if (sRes.status === 'fulfilled') setSettings(sRes.value);
    } catch (err: any) {
      addToast({ title: 'Admin API error', desc: err.message, type: 'error' });
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    loadData();
  }, []);

  const handleCreateUser = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!newUsername.trim()) return;

    try {
      const quotaBytes = parseFloat(newQuotaGB) * 1024 * 1024 * 1024;
      await tealdrive.admin.virtualUsers.create({
        username: newUsername.trim(),
        quotaBytes,
        email: newEmail.trim() || undefined,
      });
      addToast({
        title: 'Virtual user created',
        desc: `Account "${newUsername}" provisioned with ${newQuotaGB} GB quota`,
        type: 'success',
      });
      setCreateModalOpen(false);
      setNewUsername('');
      setNewEmail('');
      await loadData();
    } catch (err: any) {
      addToast({ title: 'Failed to create user', desc: err.message, type: 'error' });
    }
  };

  const handleToggleStatus = async (user: VirtualUser) => {
    try {
      await tealdrive.admin.virtualUsers.toggleStatus({
        username: user.username,
        enabled: !user.enabled,
      });
      addToast({
        title: user.enabled ? 'User suspended' : 'User activated',
        desc: `${user.username} account status updated`,
        type: 'info',
      });
      await loadData();
    } catch (err: any) {
      addToast({ title: 'Status update failed', desc: err.message, type: 'error' });
    }
  };

  const handleDeleteUser = (username: string) => {
    promptCustomDelete({
      title: `Delete Virtual User "${username}"?`,
      description: `Are you sure you want to delete virtual account "${username}"? All assigned quota and account credentials will be permanently erased.`,
      isPermanent: true,
      confirmLabel: 'Delete Account',
      items: [{ name: username, isDirectory: false }],
      onConfirm: async () => {
        try {
          await tealdrive.admin.virtualUsers.delete({ username });
          addToast({ title: 'User deleted', desc: `Removed ${username}`, type: 'info' });
          await loadData();
        } catch (err: any) {
          addToast({ title: 'Delete failed', desc: err.message, type: 'error' });
        }
      },
    });
  };

  const handleApproveSystemUser = async (username: string) => {
    try {
      await tealdrive.admin.systemUsers.approve({ username });
      addToast({
        title: 'PAM Web Access Approved',
        desc: `System user '${username}' authorized to sign in to TealDrive`,
        type: 'success',
      });
      await loadData();
    } catch (err: any) {
      addToast({ title: 'Approval failed', desc: err.message, type: 'error' });
    }
  };

  return (
    <div className="p-6 max-w-6xl">
      <div className="flex items-center justify-between mb-6">
        <div>
          <h2 className="text-base font-semibold text-slate-100 flex items-center gap-2">
            <Server className="w-5 h-5 text-teal-400" />
            <span>TealDrive Security &amp; Admin Console</span>
          </h2>
          <p className="text-xs text-slate-400 mt-0.5">
            Manage virtual tenant users, Linux PAM host accounts, physical storage quotas, and daemon settings.
          </p>
        </div>

        {activeTab === 'virtual' && (
          <button
            onClick={() => setCreateModalOpen(true)}
            className="flex items-center gap-1.5 px-3.5 py-1.5 bg-teal-600 hover:bg-teal-500 text-slate-950 font-semibold text-xs rounded-xl shadow-md transition-colors"
          >
            <UserPlus className="w-4 h-4" />
            <span>Create Virtual User</span>
          </button>
        )}
      </div>

      {/* Tabs */}
      <div className="flex items-center gap-1 p-1 bg-slate-900 border border-slate-800 rounded-xl mb-6 w-fit">
        <button
          onClick={() => setActiveTab('virtual')}
          className={`flex items-center gap-2 px-3.5 py-1.5 rounded-lg text-xs font-medium transition-colors ${
            activeTab === 'virtual' ? 'bg-slate-800 text-teal-300 shadow-sm' : 'text-slate-400 hover:text-slate-200'
          }`}
        >
          <Users className="w-3.5 h-3.5" />
          <span>Virtual Users ({virtualUsers.length})</span>
        </button>

        <button
          onClick={() => setActiveTab('system')}
          className={`flex items-center gap-2 px-3.5 py-1.5 rounded-lg text-xs font-medium transition-colors ${
            activeTab === 'system' ? 'bg-slate-800 text-teal-300 shadow-sm' : 'text-slate-400 hover:text-slate-200'
          }`}
        >
          <Terminal className="w-3.5 h-3.5" />
          <span>System PAM Users ({systemUsers.length})</span>
        </button>

        <button
          onClick={() => setActiveTab('budget')}
          className={`flex items-center gap-2 px-3.5 py-1.5 rounded-lg text-xs font-medium transition-colors ${
            activeTab === 'budget' ? 'bg-slate-800 text-teal-300 shadow-sm' : 'text-slate-400 hover:text-slate-200'
          }`}
        >
          <PieChart className="w-3.5 h-3.5" />
          <span>Storage Budget</span>
        </button>

        <button
          onClick={() => setActiveTab('settings')}
          className={`flex items-center gap-2 px-3.5 py-1.5 rounded-lg text-xs font-medium transition-colors ${
            activeTab === 'settings' ? 'bg-slate-800 text-teal-300 shadow-sm' : 'text-slate-400 hover:text-slate-200'
          }`}
        >
          <Settings className="w-3.5 h-3.5" />
          <span>Server Settings</span>
        </button>
      </div>

      {/* Tab 1: Virtual Users */}
      {activeTab === 'virtual' && (
        <div className="overflow-x-auto rounded-xl border border-slate-800 bg-slate-900/40">
          <table className="w-full text-left border-collapse text-xs">
            <thead>
              <tr className="border-b border-slate-800 bg-slate-900/80 text-slate-400 font-medium">
                <th className="py-3 px-4">Username &amp; Email</th>
                <th className="py-3 px-4 w-28 font-mono">UID</th>
                <th className="py-3 px-4 w-44">Storage Quota</th>
                <th className="py-3 px-4 w-28">Status</th>
                <th className="py-3 px-4 w-32">Created</th>
                <th className="py-3 px-4 w-32 text-right">Actions</th>
              </tr>
            </thead>
            <tbody className="divide-y divide-slate-800/60">
              {virtualUsers.length === 0 ? (
                <tr>
                  <td colSpan={6} className="py-8 text-center text-slate-500">
                    <div className="flex flex-col items-center justify-center gap-1.5">
                      <UserX className="w-5 h-5 text-slate-600" />
                      <span>No virtual accounts found. Create one using the form above.</span>
                    </div>
                  </td>
                </tr>
              ) : (
                virtualUsers.map((user) => {
                  const percent = Math.min(100, Math.round((user.usedBytes / user.quotaBytes) * 100));
                  return (
                    <tr key={user.id} className="hover:bg-slate-800/40 text-slate-300 transition-colors">
                      <td className="py-3 px-4">
                        <div className="font-semibold text-slate-200">{user.username}</div>
                        <div className="text-[11px] text-slate-500">{user.email}</div>
                      </td>
                      <td className="py-3 px-4 font-mono text-slate-400">{user.uid}</td>
                      <td className="py-3 px-4">
                        <div className="w-full bg-slate-800 rounded-full h-1.5 mb-1 overflow-hidden">
                          <div
                            className="bg-teal-500 h-full rounded-full"
                            style={{ width: `${percent}%` }}
                          />
                        </div>
                        <div className="text-[10px] font-mono tabular-nums text-slate-400 flex justify-between">
                          <span>{formatBytes(user.usedBytes)}</span>
                          <span>{formatBytes(user.quotaBytes)} ({percent}%)</span>
                        </div>
                      </td>
                      <td className="py-3 px-4">
                        {user.enabled ? (
                          <span className="inline-flex items-center gap-1 text-[11px] text-emerald-400">
                            <CheckCircle className="w-3 h-3" />
                            <span>Active</span>
                          </span>
                        ) : (
                          <span className="inline-flex items-center gap-1 text-[11px] text-rose-400">
                            <XCircle className="w-3 h-3" />
                            <span>Suspended</span>
                          </span>
                        )}
                      </td>
                      <td className="py-3 px-4 text-slate-400">{formatRelativeDate(user.createdAt)}</td>
                      <td className="py-3 px-4 text-right">
                        <div className="flex items-center justify-end gap-1.5">
                          <button
                            onClick={() => handleToggleStatus(user)}
                            className="px-2 py-1 rounded bg-slate-800 hover:bg-slate-700 text-slate-300 text-[11px] transition-colors"
                          >
                            {user.enabled ? 'Suspend' : 'Activate'}
                          </button>
                          <button
                            onClick={() => handleDeleteUser(user.username)}
                            className="p-1 rounded text-slate-500 hover:text-rose-400 hover:bg-rose-950/50 transition-colors"
                            title="Delete user"
                          >
                            <Trash2 className="w-3.5 h-3.5" />
                          </button>
                        </div>
                      </td>
                    </tr>
                  );
                })
              )}
            </tbody>
          </table>
        </div>
      )}

      {/* Tab 2: System Users (Linux PAM) */}
      {activeTab === 'system' && (
        <div className="overflow-x-auto rounded-xl border border-slate-800 bg-slate-900/40">
          <table className="w-full text-left border-collapse text-xs">
            <thead>
              <tr className="border-b border-slate-800 bg-slate-900/80 text-slate-400 font-medium">
                <th className="py-3 px-4">System User</th>
                <th className="py-3 px-4 w-28 font-mono">UID</th>
                <th className="py-3 px-4 font-mono">Shell &amp; Home</th>
                <th className="py-3 px-4 w-36">PAM Web Login</th>
                <th className="py-3 px-4 w-36">Last Host Login</th>
                <th className="py-3 px-4 w-32 text-right">Action</th>
              </tr>
            </thead>
            <tbody className="divide-y divide-slate-800/60">
              {systemUsers.length === 0 ? (
                <tr>
                  <td colSpan={6} className="py-8 text-center text-slate-500">
                    <div className="flex flex-col items-center justify-center gap-1.5">
                      <ShieldAlert className="w-5 h-5 text-slate-600" />
                      <span>No eligible regular Linux PAM accounts discovered on this host.</span>
                    </div>
                  </td>
                </tr>
              ) : (
                systemUsers.map((su) => (
                  <tr key={su.username} className="hover:bg-slate-800/40 text-slate-300 transition-colors">
                    <td className="py-3 px-4 font-semibold text-slate-200 font-mono">
                      {su.username}
                    </td>
                    <td className="py-3 px-4 font-mono text-slate-400">{su.uid}</td>
                    <td className="py-3 px-4 font-mono text-[11px] text-slate-400">
                      <div>{su.homeDir}</div>
                      <div className="text-slate-500">{su.shell}</div>
                    </td>
                    <td className="py-3 px-4">
                      {su.approvedForWeb ? (
                        <span className="inline-flex items-center gap-1 text-[11px] text-emerald-400 font-medium">
                          <CheckCircle className="w-3 h-3" />
                          <span>Authorized</span>
                        </span>
                      ) : (
                        <span className="inline-flex items-center gap-1 text-[11px] text-amber-400 font-medium">
                          <ShieldAlert className="w-3 h-3" />
                          <span>Pending Approval</span>
                        </span>
                      )}
                    </td>
                    <td className="py-3 px-4 text-slate-400">
                      {su.lastLogin ? formatRelativeDate(su.lastLogin) : 'Never'}
                    </td>
                    <td className="py-3 px-4 text-right">
                      {!su.approvedForWeb && (
                        <button
                          onClick={() => handleApproveSystemUser(su.username)}
                          className="px-2.5 py-1 rounded bg-teal-950 hover:bg-teal-900 border border-teal-700/60 text-teal-300 text-[11px] transition-colors"
                        >
                          Approve Access
                        </button>
                      )}
                    </td>
                  </tr>
                ))
              )}
            </tbody>
          </table>
        </div>
      )}

      {/* Tab 3: Storage Budget */}
      {activeTab === 'budget' && budget && (
        <div className="space-y-6">
          <div className="p-5 bg-slate-900/60 border border-slate-800 rounded-xl">
            <div className="flex items-center justify-between mb-3">
              <h3 className="text-sm font-semibold text-slate-200">
                NVMe Physical Storage Pool Allocation (Total: {formatBytes(budget.totalDiskBytes)})
              </h3>
              <span className="px-2 py-0.5 rounded text-[10px] font-mono bg-emerald-500/10 text-emerald-400 border border-emerald-500/20 flex items-center gap-1">
                <span className="w-1.5 h-1.5 rounded-full bg-emerald-400 animate-pulse" />
                Live Daemon Quota
              </span>
            </div>

            {/* Split Progress bar */}
            <div className="w-full bg-slate-800 rounded-xl h-4 overflow-hidden flex mb-3">
              <div
                style={{
                  width: `${(budget.virtualUsersAllocatedBytes / budget.totalDiskBytes) * 100}%`,
                }}
                className="bg-teal-500 h-full transition-all duration-300"
                title="Virtual Users Allocation"
              />
              <div
                style={{
                  width: `${(budget.systemReserveBytes / budget.totalDiskBytes) * 100}%`,
                }}
                className="bg-purple-500 h-full transition-all duration-300"
                title="System OS Reserve"
              />
              <div
                style={{
                  width: `${(budget.freePhysicalBytes / budget.totalDiskBytes) * 100}%`,
                }}
                className="bg-slate-700 h-full transition-all duration-300"
                title="Free Physical Unallocated"
              />
            </div>

            <div className="grid grid-cols-1 md:grid-cols-3 gap-4 text-xs mt-4">
              <div className="p-3 bg-slate-900 border border-slate-800 rounded-lg">
                <div className="flex items-center gap-2 text-teal-400 font-semibold mb-1">
                  <div className="w-2.5 h-2.5 rounded-full bg-teal-500" />
                  <span>Virtual Users Allocated</span>
                </div>
                <div className="text-lg font-mono tabular-nums font-bold text-slate-100">
                  {formatBytes(budget.virtualUsersAllocatedBytes)}
                </div>
                <div className="text-[11px] text-slate-500 mt-0.5">
                  {budget.totalDiskBytes > 0
                    ? `${((budget.virtualUsersAllocatedBytes / budget.totalDiskBytes) * 100).toFixed(1)}% of total pool`
                    : '0% of pool'}
                </div>
              </div>

              <div className="p-3 bg-slate-900 border border-slate-800 rounded-lg">
                <div className="flex items-center gap-2 text-purple-400 font-semibold mb-1">
                  <div className="w-2.5 h-2.5 rounded-full bg-purple-500" />
                  <span>Used Storage / System Reserve</span>
                </div>
                <div className="text-lg font-mono tabular-nums font-bold text-slate-100">
                  {formatBytes(budget.systemReserveBytes)}
                </div>
                <div className="text-[11px] text-slate-500 mt-0.5">
                  {budget.totalDiskBytes > 0
                    ? `${((budget.systemReserveBytes / budget.totalDiskBytes) * 100).toFixed(1)}% currently utilized`
                    : '0% utilized'}
                </div>
              </div>

              <div className="p-3 bg-slate-900 border border-slate-800 rounded-lg">
                <div className="flex items-center gap-2 text-slate-400 font-semibold mb-1">
                  <div className="w-2.5 h-2.5 rounded-full bg-slate-500" />
                  <span>Unallocated Free Disk</span>
                </div>
                <div className="text-lg font-mono tabular-nums font-bold text-slate-100">
                  {formatBytes(budget.freePhysicalBytes)}
                </div>
                <div className="text-[11px] text-slate-500 mt-0.5">
                  Available for new user provisioning
                </div>
              </div>
            </div>
          </div>
        </div>
      )}

      {/* Tab 4: Server Settings */}
      {activeTab === 'settings' && settings && (
        <div className="p-5 bg-slate-900/60 border border-slate-800 rounded-xl space-y-4 max-w-3xl">
          <div className="flex items-center justify-between">
            <h3 className="text-sm font-semibold text-slate-200">
              TealDrive Core Daemon Configuration
            </h3>
            <span className="px-2 py-0.5 rounded text-[10px] font-mono bg-emerald-500/10 text-emerald-400 border border-emerald-500/20 flex items-center gap-1">
              <span className="w-1.5 h-1.5 rounded-full bg-emerald-400 animate-pulse" />
              Live Server Config
            </span>
          </div>

          <div className="grid grid-cols-2 gap-4 text-xs">
            <div className="p-3 bg-slate-900 border border-slate-800 rounded-lg">
              <span className="text-slate-500 block mb-1">Daemon Version</span>
              <span className="font-mono text-slate-200 font-semibold">{settings.version}</span>
            </div>

            <div className="p-3 bg-slate-900 border border-slate-800 rounded-lg">
              <span className="text-slate-500 block mb-1">Listen Address</span>
              <span className="font-mono text-slate-200 font-semibold">{settings.listenAddr}</span>
            </div>

            <div className="p-3 bg-slate-900 border border-slate-800 rounded-lg">
              <span className="text-slate-500 block mb-1">Transport Crypto</span>
              <span className="font-mono text-teal-300 font-semibold flex items-center gap-1.5">
                <Lock className="w-3.5 h-3.5" />
                {settings.wsEncryption}
              </span>
            </div>

            <div className="p-3 bg-slate-900 border border-slate-800 rounded-lg">
              <span className="text-slate-500 block mb-1">TLS Certificate Expiry</span>
              <span className="font-mono text-slate-200">{settings.tlsCertExpiry ? new Date(settings.tlsCertExpiry).toLocaleDateString() : '—'}</span>
            </div>

            <div className="p-3 bg-slate-900 border border-slate-800 rounded-lg">
              <span className="text-slate-500 block mb-1">Login Failure Limit</span>
              <span className="font-mono text-slate-200">{settings.loginMaxFailures || '—'}</span>
            </div>

            <div className="p-3 bg-slate-900 border border-slate-800 rounded-lg">
              <span className="text-slate-500 block mb-1">Max Upload Stream Size</span>
              <span className="font-mono text-slate-200">{settings.maxUploadSizeMb} MB</span>
            </div>

            <div className="p-3 bg-slate-900 border border-slate-800 rounded-lg">
              <span className="text-slate-500 block mb-1">Auto Trash Retention</span>
              <span className="font-mono text-slate-200">{settings.autoTrashPurgeDays} days</span>
            </div>

            <div className="p-3 bg-slate-900 border border-slate-800 rounded-lg">
              <span className="text-slate-500 block mb-1">Handshake Pattern</span>
              <span className="font-mono text-teal-300 truncate block" title={settings.noiseHandshakePattern}>
                {settings.noiseHandshakePattern}
              </span>
            </div>
          </div>
        </div>
      )}

      {/* Create Virtual User Modal */}
      {createModalOpen && (
        <div className="fixed inset-0 z-50 flex items-center justify-center p-4 bg-slate-950/80 backdrop-blur-sm animate-in fade-in duration-150">
          <div className="w-full max-w-md bg-slate-900 border border-slate-700 rounded-2xl shadow-2xl p-6 text-slate-100">
            <h3 className="text-sm font-semibold mb-4 flex items-center gap-2">
              <UserPlus className="w-4 h-4 text-teal-400" />
              <span>Create Virtual User</span>
            </h3>

            <form onSubmit={handleCreateUser} className="space-y-4 text-xs">
              <div>
                <label className="block text-slate-400 font-medium mb-1">Username</label>
                <input
                  type="text"
                  required
                  value={newUsername}
                  onChange={(e) => setNewUsername(e.target.value)}
                  placeholder="e.g. alex_developer"
                  className="w-full px-3 py-2 bg-slate-800 border border-slate-700 rounded-lg text-slate-100 focus:outline-none focus:border-teal-500"
                />
              </div>

              <div>
                <label className="block text-slate-400 font-medium mb-1">Email (Optional)</label>
                <input
                  type="email"
                  value={newEmail}
                  onChange={(e) => setNewEmail(e.target.value)}
                  placeholder="alex@company.com"
                  className="w-full px-3 py-2 bg-slate-800 border border-slate-700 rounded-lg text-slate-100 focus:outline-none focus:border-teal-500"
                />
              </div>

              <div>
                <label className="block text-slate-400 font-medium mb-1">Storage Quota (GB)</label>
                <input
                  type="number"
                  min="1"
                  max="100"
                  required
                  value={newQuotaGB}
                  onChange={(e) => setNewQuotaGB(e.target.value)}
                  className="w-full px-3 py-2 bg-slate-800 border border-slate-700 rounded-lg text-slate-100 focus:outline-none focus:border-teal-500"
                />
              </div>

              <div className="flex items-center justify-end gap-2 pt-3">
                <button
                  type="button"
                  onClick={() => setCreateModalOpen(false)}
                  className="px-3 py-1.5 bg-slate-800 hover:bg-slate-700 text-slate-300 rounded-lg"
                >
                  Cancel
                </button>
                <button
                  type="submit"
                  className="px-4 py-1.5 bg-teal-600 hover:bg-teal-500 text-slate-950 font-semibold rounded-lg shadow"
                >
                  Create User
                </button>
              </div>
            </form>
          </div>
        </div>
      )}
    </div>
  );
};
