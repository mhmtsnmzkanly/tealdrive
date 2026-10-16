import React, { useState } from 'react';
import { useTealDrive } from '../../context/TealDriveContext.tsx';
import { KeyRound, AlertCircle, Loader2 } from 'lucide-react';
import { t } from '../../i18n/index.ts';

interface PasswordChangeModalProps {
  /** Given for a voluntary change: offers Cancel instead of Sign out. */
  onCancel?: () => void;
}

/**
 * Replaces the password. Without onCancel it is the mandatory first-sign-in screen.
 * Either way the server ends the session afterwards, so the user signs in again.
 */
export const PasswordChangeModal: React.FC<PasswordChangeModalProps> = ({ onCancel }) => {
  const { session, changePassword, logout } = useTealDrive();
  const [currentPassword, setCurrentPassword] = useState('');
  const [newPassword, setNewPassword] = useState('');
  const [confirmPassword, setConfirmPassword] = useState('');
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if ([...newPassword].length < 12) {
      setError(t('The new password must have at least 12 characters.'));
      return;
    }
    if (newPassword !== confirmPassword) {
      setError(t('The new passwords do not match.'));
      return;
    }
    if (newPassword === currentPassword) {
      setError(t('Choose a password different from the current one.'));
      return;
    }
    setError(null);
    setLoading(true);
    try {
      await changePassword(currentPassword, newPassword);
    } catch (err: any) {
      setError(
        err?.code === 'INVALID_CREDENTIALS' || err?.status === 401
          ? t('The current password is incorrect.')
          : err?.message || t('Password change failed. Please try again.')
      );
      setLoading(false);
    }
  };

  const inputClass =
    'w-full bg-slate-950 border border-slate-800 rounded-xl px-4 py-2.5 text-sm text-white placeholder-slate-600 focus:outline-none focus:border-teal-500/50 focus:ring-1 focus:ring-teal-500/30 transition-all';

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-[#060a12]/80 backdrop-blur-md px-4">
      <div className="w-full max-w-md bg-slate-900/90 border border-slate-800 rounded-2xl shadow-2xl p-8">
        <div className="flex flex-col items-center text-center mb-6">
          <div className="w-14 h-14 rounded-2xl bg-teal-500/10 border border-teal-500/20 flex items-center justify-center mb-4">
            <KeyRound className="w-7 h-7 text-teal-400" />
          </div>
          <h1 className="text-xl font-bold tracking-tight text-white mb-1">{t('Choose a new password')}</h1>
          <p className="text-sm text-slate-400">
            {onCancel
              ? t('You will be signed out and can sign in with the new password.')
              : `${session?.username ? `${session.username}, your` : 'Your'} account uses a password set by an administrator. Replace it to continue.`}
          </p>
        </div>

        {error && (
          <div className="mb-5 p-3.5 bg-rose-500/10 border border-rose-500/20 rounded-xl flex items-start gap-3 text-rose-300 text-xs">
            <AlertCircle className="w-4 h-4 shrink-0 text-rose-400 mt-0.5" />
            <span>{error}</span>
          </div>
        )}

        <form onSubmit={handleSubmit} className="space-y-4">
          <div>
            <label className="block text-xs font-semibold text-slate-300 uppercase tracking-wider mb-2">
              {t('Current password')}
            </label>
            <input
              type="password"
              value={currentPassword}
              onChange={(e) => setCurrentPassword(e.target.value)}
              autoComplete="current-password"
              required
              autoFocus
              className={inputClass}
            />
          </div>
          <div>
            <label className="block text-xs font-semibold text-slate-300 uppercase tracking-wider mb-2">
              {t('New password')}
            </label>
            <input
              type="password"
              value={newPassword}
              onChange={(e) => setNewPassword(e.target.value)}
              autoComplete="new-password"
              minLength={12}
              required
              className={inputClass}
            />
          </div>
          <div>
            <label className="block text-xs font-semibold text-slate-300 uppercase tracking-wider mb-2">
              {t('Confirm new password')}
            </label>
            <input
              type="password"
              value={confirmPassword}
              onChange={(e) => setConfirmPassword(e.target.value)}
              autoComplete="new-password"
              minLength={12}
              required
              className={inputClass}
            />
          </div>

          <button
            type="submit"
            disabled={loading}
            className="w-full py-3 px-4 bg-teal-500 hover:bg-teal-400 disabled:opacity-50 disabled:cursor-not-allowed text-slate-950 font-semibold rounded-xl transition-all flex items-center justify-center gap-2 mt-2 cursor-pointer"
          >
            {loading ? (
              <>
                <Loader2 className="w-4 h-4 animate-spin" />
                <span>{t('Saving…')}</span>
              </>
            ) : (
              <span>{t('Change password')}</span>
            )}
          </button>
          <button
            type="button"
            onClick={() => (onCancel ? onCancel() : logout())}
            disabled={loading}
            className="w-full py-2 text-xs text-slate-400 hover:text-slate-200"
          >
            {onCancel ? 'Cancel' : t('Sign out')}
          </button>
        </form>
      </div>
    </div>
  );
};
