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
import { PolicyEditorModal } from '../modals/PolicyEditorModal.tsx';
import type { PolicyTarget } from '../../sdk/mapping.ts';
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
import { t, locale } from '../../i18n/index.ts';

const USERNAME_PATTERN = /^[a-z][a-z0-9_.-]{0,63}$/;

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
  const [newPassword, setNewPassword] = useState('');
  const [newQuotaGB, setNewQuotaGB] = useState('5');
  const [creating, setCreating] = useState(false);
  const [quotaUser, setQuotaUser] = useState<VirtualUser | null>(null);
  const [quotaInput, setQuotaInput] = useState('');
  const [policyTarget, setPolicyTarget] = useState<PolicyTarget | null>(null);
  const [systemAccounts, setSystemAccounts] = useState<
    Map<string, { id: string; generation: number; status: string; loginEnabled: boolean }>
  >(new Map());
  const [loadErrors, setLoadErrors] = useState<Record<string, string>>({});

  const loadData = async () => {
    setLoading(true);
    try {
      const [vuRes, suRes, bRes, sRes, saRes] = await Promise.allSettled([
        tealdrive.admin.virtualUsers.list(),
        tealdrive.admin.systemUsers.list(),
        tealdrive.admin.quota.getBudget(),
        tealdrive.admin.settings.inspect(),
        tealdrive.admin.systemUsers.accounts(),
      ]);
      // Registered Linux accounts carry the id that access and policy changes need.
      setSystemAccounts(saRes.status === 'fulfilled' ? saRes.value : new Map());
      // Keep "access denied" distinguishable from "nothing to show".
      const errors: Record<string, string> = {};
      const reason = (r: PromiseRejectedResult) => r.reason?.code || r.reason?.message || 'Unavailable';
      if (vuRes.status === 'fulfilled') setVirtualUsers(vuRes.value);
      else errors.virtual = reason(vuRes);
      if (suRes.status === 'fulfilled') setSystemUsers(suRes.value);
      else errors.system = reason(suRes);
      if (bRes.status === 'fulfilled') setBudget(bRes.value);
      else errors.budget = reason(bRes);
      if (sRes.status === 'fulfilled') setSettings(sRes.value);
      else errors.settings = reason(sRes);
      setLoadErrors(errors);
    } catch (err: any) {
      addToast({ title: t('Admin API error'), desc: err.message, type: 'error' });
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    loadData();
  }, []);

  const handleCreateUser = async (e: React.FormEvent) => {
    e.preventDefault();
    const username = newUsername.trim();
    if (!USERNAME_PATTERN.test(username)) {
      addToast({
        title: t('Invalid username'),
        desc: t('Use 1-64 lowercase letters, digits, "_", "." or "-", starting with a letter.'),
        type: 'error',
      });
      return;
    }
    if ([...newPassword].length < 12) {
      addToast({ title: t('Password too short'), desc: t('Use at least 12 characters.'), type: 'error' });
      return;
    }
    const quotaBytes = Math.round(parseFloat(newQuotaGB) * 1024 * 1024 * 1024);
    if (!Number.isSafeInteger(quotaBytes) || quotaBytes < 1) {
      addToast({ title: t('Invalid quota'), desc: t('Enter a positive size in GB.'), type: 'error' });
      return;
    }

    setCreating(true);
    try {
      await tealdrive.admin.virtualUsers.create({ username, quotaBytes, password: newPassword });
      addToast({
        title: t('Virtual user created'),
        desc: t('"{username}" must choose a new password at first sign-in.', { username }),
        type: 'success',
      });
      setCreateModalOpen(false);
      setNewUsername('');
      setNewPassword('');
      await loadData();
    } catch (err: any) {
      addToast({ title: t('Failed to create user'), desc: err.message, type: 'error' });
    } finally {
      setCreating(false);
    }
  };

  const generatePassword = () => {
    const alphabet = 'ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz23456789-_.';
    const bytes = crypto.getRandomValues(new Uint8Array(20));
    // 60 symbols: the modulo bias over 256 values is negligible for a one-time password.
    setNewPassword(Array.from(bytes, (b) => alphabet[b % alphabet.length]).join(''));
  };

  const handleToggleStatus = async (user: VirtualUser) => {
    try {
      await tealdrive.admin.virtualUsers.toggleStatus({ user, enabled: !user.enabled });
      addToast({
        title: user.enabled ? t('User suspended') : t('User activated'),
        desc: t('{username} account status updated', { username: user.username }),
        type: 'info',
      });
      await loadData();
    } catch (err: any) {
      addToast({ title: t('Status update failed'), desc: err.message, type: 'error' });
    }
  };

  const handleDeleteUser = (user: VirtualUser) => {
    const username = user.username;
    promptCustomDelete({
      title: t('Delete Virtual User "{username}"?', { username }),
      description: t('Are you sure you want to delete virtual account "{username}"? All assigned quota and account credentials will be permanently erased.', { username }),
      isPermanent: true,
      confirmLabel: t('Delete Account'),
      items: [{ name: username, isDirectory: false }],
      onConfirm: async () => {
        try {
          await tealdrive.admin.virtualUsers.delete({ user });
          addToast({ title: t('User deleted'), desc: t('Removed {username}', { username }), type: 'info' });
          await loadData();
        } catch (err: any) {
          addToast({ title: t('Delete failed'), desc: err.message, type: 'error' });
        }
      },
    });
  };

  const handleSaveQuota = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!quotaUser) return;
    const quotaBytes = Math.round(parseFloat(quotaInput) * 1024 * 1024 * 1024);
    if (!Number.isSafeInteger(quotaBytes) || quotaBytes < 1) {
      addToast({ title: t('Invalid quota'), desc: t('Enter a positive size in GB.'), type: 'error' });
      return;
    }
    try {
      await tealdrive.admin.virtualUsers.setQuota({ user: quotaUser, quotaBytes });
      addToast({ title: t('Quota updated'), desc: `${quotaUser.username}: ${formatBytes(quotaBytes)}`, type: 'success' });
      setQuotaUser(null);
      await loadData();
    } catch (err: any) {
      addToast({
        title: t('Quota update failed'),
        desc: err?.code === 'CONFLICT' ? t('The account changed meanwhile; reload and try again.') : err.message,
        type: 'error',
      });
    }
  };

  const handleSystemAccess = async (username: string, enabled: boolean) => {
    const account = systemAccounts.get(username);
    if (!account) return;
    try {
      await tealdrive.admin.systemUsers.setAccess({ accountId: account.id, enabled });
      addToast({
        title: enabled ? t('Web access enabled') : t('Web access suspended'),
        desc: username,
        type: 'info',
      });
      await loadData();
    } catch (err: any) {
      addToast({ title: t('Access change failed'), desc: err.message, type: 'error' });
    }
  };

  const handleApproveSystemUser = async (username: string) => {
    try {
      await tealdrive.admin.systemUsers.approve({ username });
      addToast({
        title: t('PAM Web Access Approved'),
        desc: t('System user \'{username}\' authorized to sign in to TealDrive', { username }),
        type: 'success',
      });
      await loadData();
    } catch (err: any) {
      addToast({ title: t('Approval failed'), desc: err.message, type: 'error' });
    }
  };

  return (
    <div className="p-6 max-w-6xl">
      <div className="flex items-center justify-between mb-6">
        <div>
          <h2 className="text-base font-semibold text-slate-100 flex items-center gap-2">
            <Server className="w-5 h-5 text-teal-400" />
            <span>{t('Admin Console')}</span>
          </h2>
          <p className="text-xs text-slate-400 mt-0.5">
            {t('Manage virtual tenant users, Linux PAM host accounts, physical storage quotas, and daemon settings.')}
          </p>
        </div>

        {activeTab === 'virtual' && (
          <button
            onClick={() => setCreateModalOpen(true)}
            className="flex items-center gap-1.5 px-3.5 py-1.5 bg-teal-600 hover:bg-teal-500 text-slate-950 font-semibold text-xs rounded-xl shadow-md transition-colors"
          >
            <UserPlus className="w-4 h-4" />
            <span>{t('Create Virtual User')}</span>
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
          <span>{t('Virtual Users ({count})', { count: virtualUsers.length })}</span>
        </button>

        <button
          onClick={() => setActiveTab('system')}
          className={`flex items-center gap-2 px-3.5 py-1.5 rounded-lg text-xs font-medium transition-colors ${
            activeTab === 'system' ? 'bg-slate-800 text-teal-300 shadow-sm' : 'text-slate-400 hover:text-slate-200'
          }`}
        >
          <Terminal className="w-3.5 h-3.5" />
          <span>{t('System PAM Users ({count})', { count: systemUsers.length })}</span>
        </button>

        <button
          onClick={() => setActiveTab('budget')}
          className={`flex items-center gap-2 px-3.5 py-1.5 rounded-lg text-xs font-medium transition-colors ${
            activeTab === 'budget' ? 'bg-slate-800 text-teal-300 shadow-sm' : 'text-slate-400 hover:text-slate-200'
          }`}
        >
          <PieChart className="w-3.5 h-3.5" />
          <span>{t('Storage Budget')}</span>
        </button>

        <button
          onClick={() => setActiveTab('settings')}
          className={`flex items-center gap-2 px-3.5 py-1.5 rounded-lg text-xs font-medium transition-colors ${
            activeTab === 'settings' ? 'bg-slate-800 text-teal-300 shadow-sm' : 'text-slate-400 hover:text-slate-200'
          }`}
        >
          <Settings className="w-3.5 h-3.5" />
          <span>{t('Server Settings')}</span>
        </button>
      </div>

      {/* Tab 1: Virtual Users */}
      {activeTab === 'virtual' && (
        <div className="overflow-x-auto rounded-xl border border-slate-800 bg-slate-900/40">
          <table className="w-full text-left border-collapse text-xs">
            <thead>
              <tr className="border-b border-slate-800 bg-slate-900/80 text-slate-400 font-medium">
                <th className="py-3 px-4">{t('Username')}</th>
                <th className="py-3 px-4 w-28 font-mono">{t('UID')}</th>
                <th className="py-3 px-4 w-44">{t('Storage Quota')}</th>
                <th className="py-3 px-4 w-28">{t('Status')}</th>
                <th className="py-3 px-4 w-32">{t('Created')}</th>
                <th className="py-3 px-4 w-32 text-right">{t('Actions')}</th>
              </tr>
            </thead>
            <tbody className="divide-y divide-slate-800/60">
              {virtualUsers.length === 0 ? (
                <tr>
                  <td colSpan={6} className="py-8 text-center text-slate-500">
                    <div className="flex flex-col items-center justify-center gap-1.5">
                      <UserX className="w-5 h-5 text-slate-600" />
                      <span>
                        {loadErrors.virtual
                          ? t('Virtual accounts unavailable ({virtual}).', { virtual: loadErrors.virtual })
                          : t('No virtual accounts found. Create one using the button above.')}
                      </span>
                    </div>
                  </td>
                </tr>
              ) : (
                virtualUsers.map((user) => {
                  const percent =
                    user.usedBytes !== undefined && user.quotaBytes > 0
                      ? Math.min(100, Math.round((user.usedBytes / user.quotaBytes) * 100))
                      : null;
                  return (
                    <tr key={user.id} className="hover:bg-slate-800/40 text-slate-300 transition-colors">
                      <td className="py-3 px-4">
                        <div className="font-semibold text-slate-200">{user.username}</div>
                        {user.passwordChangeRequired && (
                          <div className="text-[11px] text-amber-400">{t('Password change pending')}</div>
                        )}
                      </td>
                      <td className="py-3 px-4 font-mono text-slate-400">{user.uid}</td>
                      <td className="py-3 px-4">
                        {percent !== null && (
                          <div className="w-full bg-slate-800 rounded-full h-1.5 mb-1 overflow-hidden">
                            <div
                              className="bg-teal-500 h-full rounded-full"
                              style={{ width: `${percent}%` }}
                            />
                          </div>
                        )}
                        <div className="text-[10px] font-mono tabular-nums text-slate-400 flex justify-between">
                          {percent !== null && <span>{formatBytes(user.usedBytes ?? 0)}</span>}
                          <span>{formatBytes(user.quotaBytes)}{percent !== null ? ` (${percent}%)` : ''}</span>
                        </div>
                      </td>
                      <td className="py-3 px-4">
                        {user.enabled ? (
                          <span className="inline-flex items-center gap-1 text-[11px] text-emerald-400">
                            <CheckCircle className="w-3 h-3" />
                            <span>{t('Active')}</span>
                          </span>
                        ) : (
                          <span className="inline-flex items-center gap-1 text-[11px] text-rose-400">
                            <XCircle className="w-3 h-3" />
                            <span>{t(user.status)}</span>
                          </span>
                        )}
                      </td>
                      <td className="py-3 px-4 text-slate-400">{user.createdAt ? formatRelativeDate(user.createdAt) : '—'}</td>
                      <td className="py-3 px-4 text-right">
                        <div className="flex items-center justify-end gap-1.5">
                          <button
                            onClick={() => {
                              setQuotaUser(user);
                              setQuotaInput((user.quotaBytes / 1024 ** 3).toFixed(2).replace(/\.?0+$/, ''));
                            }}
                            className="px-2 py-1 rounded bg-slate-800 hover:bg-slate-700 text-slate-300 text-[11px] transition-colors"
                          >
                            {t('Quota')}
                          </button>
                          <button
                            onClick={() => setPolicyTarget({ kind: 'virtual', accountId: user.id, username: user.username })}
                            className="px-2 py-1 rounded bg-slate-800 hover:bg-slate-700 text-slate-300 text-[11px] transition-colors"
                          >
                            {t('Policy')}
                          </button>
                          <button
                            onClick={() => handleToggleStatus(user)}
                            className="px-2 py-1 rounded bg-slate-800 hover:bg-slate-700 text-slate-300 text-[11px] transition-colors"
                          >
                            {user.enabled ? 'Suspend' : 'Activate'}
                          </button>
                          <button
                            onClick={() => handleDeleteUser(user)}
                            className="p-1 rounded text-slate-500 hover:text-rose-400 hover:bg-rose-950/50 transition-colors"
                            title={t('Delete user')}
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
                <th className="py-3 px-4">{t('System User')}</th>
                <th className="py-3 px-4 w-28 font-mono">{t('UID')}</th>
                <th className="py-3 px-4 font-mono">{t('Shell & Home')}</th>
                <th className="py-3 px-4 w-36">{t('PAM Web Login')}</th>
                <th className="py-3 px-4 w-36">{t('Last Host Login')}</th>
                <th className="py-3 px-4 w-32 text-right">{t('Action')}</th>
              </tr>
            </thead>
            <tbody className="divide-y divide-slate-800/60">
              {systemUsers.length === 0 ? (
                <tr>
                  <td colSpan={6} className="py-8 text-center text-slate-500">
                    <div className="flex flex-col items-center justify-center gap-1.5">
                      <ShieldAlert className="w-5 h-5 text-slate-600" />
                      <span>
                        {loadErrors.system
                          ? t('System accounts unavailable ({system}).', { system: loadErrors.system })
                          : t('No eligible regular Linux PAM accounts discovered on this host.')}
                      </span>
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
                      <div>{su.homeDir || '—'}</div>
                      {su.shell && <div className="text-slate-500">{su.shell}</div>}
                    </td>
                    <td className="py-3 px-4">
                      {su.approvedForWeb ? (
                        systemAccounts.get(su.username) && systemAccounts.get(su.username)!.status !== 'active' ? (
                          <span className="inline-flex items-center gap-1 text-[11px] text-rose-400 font-medium">
                            <XCircle className="w-3 h-3" />
                            <span>{t(systemAccounts.get(su.username)!.status)}</span>
                          </span>
                        ) : (
                          <span className="inline-flex items-center gap-1 text-[11px] text-emerald-400 font-medium">
                            <CheckCircle className="w-3 h-3" />
                            <span>{t('Authorized')}</span>
                          </span>
                        )
                      ) : (
                        <span className="inline-flex items-center gap-1 text-[11px] text-amber-400 font-medium">
                          <ShieldAlert className="w-3 h-3" />
                          <span>{t('Pending Approval')}</span>
                        </span>
                      )}
                    </td>
                    <td className="py-3 px-4 text-slate-400">
                      {su.lastLogin ? formatRelativeDate(su.lastLogin) : '—'}
                    </td>
                    <td className="py-3 px-4 text-right">
                      {su.approvedForWeb && systemAccounts.get(su.username) && (
                        <div className="flex items-center justify-end gap-1.5">
                          <button
                            onClick={() =>
                              setPolicyTarget({
                                kind: 'linux',
                                accountId: systemAccounts.get(su.username)!.id,
                                username: su.username,
                              })
                            }
                            className="px-2 py-1 rounded bg-slate-800 hover:bg-slate-700 text-slate-300 text-[11px] transition-colors"
                          >
                            {t('Policy')}
                          </button>
                          <button
                            onClick={() =>
                              handleSystemAccess(su.username, systemAccounts.get(su.username)!.status !== 'active')
                            }
                            className="px-2 py-1 rounded bg-slate-800 hover:bg-slate-700 text-slate-300 text-[11px] transition-colors"
                          >
                            {systemAccounts.get(su.username)!.status === 'active' ? 'Suspend' : 'Allow'}
                          </button>
                        </div>
                      )}
                      {!su.approvedForWeb && (
                        <button
                          onClick={() => handleApproveSystemUser(su.username)}
                          className="px-2.5 py-1 rounded bg-teal-950 hover:bg-teal-900 border border-teal-700/60 text-teal-300 text-[11px] transition-colors"
                        >
                          {t('Approve Access')}
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
      {(['budget', 'settings'] as const).map((tab) =>
        activeTab === tab && !(tab === 'budget' ? budget : settings) ? (
          <div key={tab} className="p-8 text-center text-xs text-slate-500 border border-dashed border-slate-800 rounded-xl">
            {loading
              ? t('Loading…')
              : `${tab === 'budget' ? 'Storage budget' : 'Server settings'} unavailable${loadErrors[tab] ? ` (${loadErrors[tab]})` : ''}.`}
          </div>
        ) : null
      )}

      {activeTab === 'budget' && budget && (
        <div className="space-y-6">
          <div className="p-5 bg-slate-900/60 border border-slate-800 rounded-xl">
            <div className="flex items-center justify-between mb-3">
              <h3 className="text-sm font-semibold text-slate-200">
                Admin Storage Budget (Total: {formatBytes(budget.totalDiskBytes)})
              </h3>
              <span className="px-2 py-0.5 rounded text-[10px] font-mono bg-emerald-500/10 text-emerald-400 border border-emerald-500/20 flex items-center gap-1">
                <span className="w-1.5 h-1.5 rounded-full bg-emerald-400 animate-pulse" />
                {t('Live Daemon Quota')}
              </span>
            </div>

            {/* Split Progress bar */}
            <div className="w-full bg-slate-800 rounded-xl h-4 overflow-hidden flex mb-3">
              <div
                style={{
                  width: `${(budget.virtualUsersAllocatedBytes / budget.totalDiskBytes) * 100}%`,
                }}
                className="bg-teal-500 h-full transition-all duration-300"
                title={t('Virtual Users Allocation')}
              />
              <div
                style={{
                  width: `${(budget.systemReserveBytes / budget.totalDiskBytes) * 100}%`,
                }}
                className="bg-purple-500 h-full transition-all duration-300"
                title={t('System OS Reserve')}
              />
              <div
                style={{
                  width: `${(budget.freePhysicalBytes / budget.totalDiskBytes) * 100}%`,
                }}
                className="bg-slate-700 h-full transition-all duration-300"
                title={t('Free Physical Unallocated')}
              />
            </div>

            <div className="grid grid-cols-1 md:grid-cols-3 gap-4 text-xs mt-4">
              <div className="p-3 bg-slate-900 border border-slate-800 rounded-lg">
                <div className="flex items-center gap-2 text-teal-400 font-semibold mb-1">
                  <div className="w-2.5 h-2.5 rounded-full bg-teal-500" />
                  <span>{t('Virtual Users Allocated')}</span>
                </div>
                <div className="text-lg font-mono tabular-nums font-bold text-slate-100">
                  {formatBytes(budget.virtualUsersAllocatedBytes)}
                </div>
                <div className="text-[11px] text-slate-500 mt-0.5">
                  {budget.totalDiskBytes > 0
                    ? t('{value}% of total pool', { value: ((budget.virtualUsersAllocatedBytes / budget.totalDiskBytes) * 100).toFixed(1) })
                    : t('0% of pool')}
                </div>
              </div>

              <div className="p-3 bg-slate-900 border border-slate-800 rounded-lg">
                <div className="flex items-center gap-2 text-purple-400 font-semibold mb-1">
                  <div className="w-2.5 h-2.5 rounded-full bg-purple-500" />
                  <span>{t('Admin Own Quota')}</span>
                </div>
                <div className="text-lg font-mono tabular-nums font-bold text-slate-100">
                  {formatBytes(budget.systemReserveBytes)}
                </div>
                <div className="text-[11px] text-slate-500 mt-0.5">
                  {budget.totalDiskBytes > 0
                    ? t('{value}% currently utilized', { value: ((budget.systemReserveBytes / budget.totalDiskBytes) * 100).toFixed(1) })
                    : '0% utilized'}
                </div>
              </div>

              <div className="p-3 bg-slate-900 border border-slate-800 rounded-lg">
                <div className="flex items-center gap-2 text-slate-400 font-semibold mb-1">
                  <div className="w-2.5 h-2.5 rounded-full bg-slate-500" />
                  <span>{t('Unallocated Budget')}</span>
                </div>
                <div className="text-lg font-mono tabular-nums font-bold text-slate-100">
                  {formatBytes(budget.freePhysicalBytes)}
                </div>
                <div className="text-[11px] text-slate-500 mt-0.5">
                  {t('Available for new user provisioning')}
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
              {t('TealDrive Core Daemon Configuration')}
            </h3>
            <span className="px-2 py-0.5 rounded text-[10px] font-mono bg-emerald-500/10 text-emerald-400 border border-emerald-500/20 flex items-center gap-1">
              <span className="w-1.5 h-1.5 rounded-full bg-emerald-400 animate-pulse" />
              {t('Live Server Config')}
            </span>
          </div>

          <div className="grid grid-cols-2 gap-4 text-xs">
            <div className="p-3 bg-slate-900 border border-slate-800 rounded-lg">
              <span className="text-slate-500 block mb-1">{t('Daemon Version')}</span>
              <span className="font-mono text-slate-200 font-semibold">{settings.version}</span>
            </div>

            <div className="p-3 bg-slate-900 border border-slate-800 rounded-lg">
              <span className="text-slate-500 block mb-1">{t('Listen Address')}</span>
              <span className="font-mono text-slate-200 font-semibold">{settings.listenAddr}</span>
            </div>

            <div className="p-3 bg-slate-900 border border-slate-800 rounded-lg">
              <span className="text-slate-500 block mb-1">{t('Transport Crypto')}</span>
              <span className="font-mono text-teal-300 font-semibold flex items-center gap-1.5">
                <Lock className="w-3.5 h-3.5" />
                {settings.wsEncryption}
              </span>
            </div>

            <div className="p-3 bg-slate-900 border border-slate-800 rounded-lg">
              <span className="text-slate-500 block mb-1">{t('TLS Certificate Expiry')}</span>
              <span className="font-mono text-slate-200">{settings.tlsCertExpiry ? new Date(settings.tlsCertExpiry).toLocaleDateString(locale()) : '—'}</span>
            </div>

            <div className="p-3 bg-slate-900 border border-slate-800 rounded-lg">
              <span className="text-slate-500 block mb-1">{t('Login Failure Limit')}</span>
              <span className="font-mono text-slate-200">{settings.loginMaxFailures || '—'}</span>
            </div>

            <div className="p-3 bg-slate-900 border border-slate-800 rounded-lg">
              <span className="text-slate-500 block mb-1">{t('Max Upload Stream Size')}</span>
              <span className="font-mono text-slate-200">{settings.maxUploadSizeMb} MB</span>
            </div>

            <div className="p-3 bg-slate-900 border border-slate-800 rounded-lg">
              <span className="text-slate-500 block mb-1">{t('Auto Trash Retention')}</span>
              <span className="font-mono text-slate-200">{t('{days} days', { days: settings.autoTrashPurgeDays })}</span>
            </div>

            <div className="p-3 bg-slate-900 border border-slate-800 rounded-lg">
              <span className="text-slate-500 block mb-1">{t('Handshake Pattern')}</span>
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
              <span>{t('Create Virtual User')}</span>
            </h3>

            <form onSubmit={handleCreateUser} className="space-y-4 text-xs">
              <div>
                <label className="block text-slate-400 font-medium mb-1">{t('Username')}</label>
                <input
                  type="text"
                  required
                  value={newUsername}
                  onChange={(e) => setNewUsername(e.target.value)}
                  placeholder={t('e.g. alex_dev')}
                  pattern="[a-z][a-z0-9_.\-]{0,63}"
                  autoComplete="off"
                  className="w-full px-3 py-2 bg-slate-800 border border-slate-700 rounded-lg text-slate-100 focus:outline-none focus:border-teal-500"
                />
              </div>

              <div>
                <label className="block text-slate-400 font-medium mb-1">{t('Initial Password')}</label>
                <div className="flex gap-2">
                  <input
                    type="text"
                    required
                    minLength={12}
                    autoComplete="new-password"
                    spellCheck={false}
                    value={newPassword}
                    onChange={(e) => setNewPassword(e.target.value)}
                    placeholder={t('At least 12 characters')}
                    className="w-full px-3 py-2 bg-slate-800 border border-slate-700 rounded-lg text-slate-100 font-mono focus:outline-none focus:border-teal-500"
                  />
                  <button
                    type="button"
                    onClick={generatePassword}
                    className="px-3 py-2 bg-slate-800 hover:bg-slate-700 border border-slate-700 text-slate-300 rounded-lg whitespace-nowrap"
                  >
                    {t('Generate')}
                  </button>
                </div>
                <p className="mt-1 text-[11px] text-slate-500">
                  {t('Share it privately. The user must replace it at first sign-in.')}
                </p>
              </div>

              <div>
                <label className="block text-slate-400 font-medium mb-1">{t('Storage Quota (GB)')}</label>
                <input
                  type="number"
                  min="0.01"
                  step="any"
                  required
                  value={newQuotaGB}
                  onChange={(e) => setNewQuotaGB(e.target.value)}
                  className="w-full px-3 py-2 bg-slate-800 border border-slate-700 rounded-lg text-slate-100 focus:outline-none focus:border-teal-500"
                />
              </div>

              <div className="flex items-center justify-end gap-2 pt-3">
                <button
                  type="button"
                  onClick={() => {
                    setCreateModalOpen(false);
                    setNewPassword('');
                  }}
                  className="px-3 py-1.5 bg-slate-800 hover:bg-slate-700 text-slate-300 rounded-lg"
                >
                  {t('Cancel')}
                </button>
                <button
                  type="submit"
                  disabled={creating}
                  className="px-4 py-1.5 bg-teal-600 hover:bg-teal-500 disabled:opacity-50 text-slate-950 font-semibold rounded-lg shadow"
                >
                  {creating ? t('Creating…') : t('Create User')}
                </button>
              </div>
            </form>
          </div>
        </div>
      )}

      {quotaUser && (
        <div className="fixed inset-0 z-50 flex items-center justify-center p-4 bg-slate-950/80 backdrop-blur-sm">
          <form
            onSubmit={handleSaveQuota}
            className="w-full max-w-sm bg-slate-900 border border-slate-700 rounded-2xl shadow-2xl p-6 text-slate-100 space-y-4 text-xs"
          >
            <h3 className="text-sm font-semibold">{t('Storage quota · {username}', { username: quotaUser.username })}</h3>
            <div>
              <label className="block text-slate-400 font-medium mb-1">{t('Quota (GB)')}</label>
              <input
                type="number"
                min="0.01"
                step="any"
                required
                autoFocus
                value={quotaInput}
                onChange={(e) => setQuotaInput(e.target.value)}
                className="w-full px-3 py-2 bg-slate-800 border border-slate-700 rounded-lg text-slate-100 focus:outline-none focus:border-teal-500"
              />
            </div>
            <div className="flex items-center justify-end gap-2">
              <button type="button" onClick={() => setQuotaUser(null)} className="px-3 py-1.5 bg-slate-800 hover:bg-slate-700 rounded-lg">
                {t('Cancel')}
              </button>
              <button type="submit" className="px-4 py-1.5 bg-teal-600 hover:bg-teal-500 text-slate-950 font-semibold rounded-lg">
                {t('Save')}
              </button>
            </div>
          </form>
        </div>
      )}

      {policyTarget && <PolicyEditorModal target={policyTarget} onClose={() => setPolicyTarget(null)} />}
    </div>
  );
};
