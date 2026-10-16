import React, { useEffect, useState } from 'react';
import { tealdrive } from '../../lib/tealdrive.ts';
import { useTealDrive } from '../../context/TealDriveContext.tsx';
import { POLICY_ACCESS, type PolicyAccess, type PolicyRule, type PolicyTarget, type PolicyView } from '../../sdk/mapping.ts';
import { ShieldCheck, Plus, Trash2, Loader2, X } from 'lucide-react';
import { t } from '../../i18n/index.ts';

interface PolicyEditorModalProps {
  /** The account to edit as an admin; omit to edit the signed-in account's personal rules. */
  target?: PolicyTarget;
  onClose: () => void;
}

const MAX_RULES = 512;

/** Edits deny rules. Rules only restrict; the server enforces them and signs the account out on save. */
export const PolicyEditorModal: React.FC<PolicyEditorModalProps> = ({ target, onClose }) => {
  const { addToast, logout } = useTealDrive();
  const [view, setView] = useState<PolicyView | null>(null);
  const [rules, setRules] = useState<PolicyRule[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);

  useEffect(() => {
    let cancelled = false;
    tealdrive.policy
      .get(target)
      .then((v) => {
        if (cancelled) return;
        setView(v);
        setRules(v.editable);
      })
      .catch((err: any) => {
        if (!cancelled) setError(err?.code === 'FEATURE_DISABLED' ? t('Policy management is disabled on this server.') : err?.message || t('Policy unavailable'));
      });
    return () => {
      cancelled = true;
    };
  }, [target]);

  const update = (index: number, patch: Partial<PolicyRule>) => {
    setRules((prev) => prev.map((r, i) => (i === index ? { ...r, ...patch } : r)));
  };

  const toggleDeny = (index: number, access: PolicyAccess) => {
    setRules((prev) =>
      prev.map((r, i) =>
        i === index
          ? { ...r, deny: r.deny.includes(access) ? r.deny.filter((d) => d !== access) : [...r.deny, access] }
          : r
      )
    );
  };

  const save = async () => {
    if (!view) return;
    if (rules.some((r) => r.deny.length === 0)) {
      setError(t('Every rule must deny at least one kind of access.'));
      return;
    }
    if (rules.some((r) => /(^|\/)\.\.(\/|$)/.test(r.path))) {
      setError(t('Paths cannot contain "..".'));
      return;
    }
    const whom = target ? `"${target.username}"` : t('your account');
    if (!window.confirm(t('Save these restrictions? This signs out every session of {whom}.', { whom }))) return;
    setSaving(true);
    setError(null);
    try {
      await tealdrive.policy.set(target, { expectedGeneration: view.generation, rules });
      addToast({ title: t('Policy saved'), desc: t('Restrictions updated for {name}.', { name: target?.username ?? t('your account') }), type: 'success' });
      onClose();
      // Saving one's own policy revokes this session too.
      if (!target) await logout();
    } catch (err: any) {
      setError(
        err?.code === 'CONFLICT'
          ? t('The account changed meanwhile. Close and reopen this editor before retrying.')
          : t('Policy was not saved: {error}', { error: err?.message || err?.code || t('Unknown error') })
      );
    } finally {
      setSaving(false);
    }
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center p-4 bg-slate-950/80 backdrop-blur-sm">
      <div className="w-full max-w-2xl max-h-[90vh] flex flex-col bg-slate-900 border border-slate-700 rounded-2xl shadow-2xl text-slate-100 text-xs">
        <div className="flex items-center justify-between p-5 border-b border-slate-800">
          <h3 className="text-sm font-semibold flex items-center gap-2">
            <ShieldCheck className="w-4 h-4 text-teal-400" />
            <span>{t('File access policy · {username}', { username: view?.username || target?.username || '…' })}</span>
          </h3>
          <button onClick={onClose} className="p-1 rounded text-slate-400 hover:text-slate-200" aria-label={t('Close')}>
            <X className="w-4 h-4" />
          </button>
        </div>

        <div className="p-5 overflow-y-auto space-y-4">
          <p className="text-slate-400">
            {t('Rules only restrict access. "discover" hides the path and blocks direct access. Paths are relative to the home folder; "include descendants" applies the rule to everything below the path.')}
          </p>

          {!view && !error && (
            <div className="flex items-center gap-2 text-slate-400">
              <Loader2 className="w-4 h-4 animate-spin" />
              <span>{t('Loading policy…')}</span>
            </div>
          )}

          {view && (view.fixed.length > 0 || view.denySystemComponents.length > 0) && (
            <details className="rounded-lg border border-slate-800 p-3 bg-slate-950/40">
              <summary className="cursor-pointer text-slate-300">{t('Restrictions you cannot change here')}</summary>
              {view.denySystemComponents.length > 0 && (
                <p className="mt-2 text-slate-400">{t('Protected system components: {list}', { list: view.denySystemComponents.join(', ') })}</p>
              )}
              {view.fixed.map(({ layer, rule }, i) => (
                <p key={i} className="mt-1 font-mono text-slate-400">
                  {layer}: {rule.path} · {rule.recursive ? 'subtree' : 'exact'} · {rule.deny.join(', ')}
                </p>
              ))}
            </details>
          )}

          {view &&
            rules.map((rule, index) => (
              <fieldset key={index} className="rounded-lg border border-slate-800 p-3 space-y-2">
                <div className="flex items-center gap-2">
                  <input
                    type="text"
                    value={rule.path}
                    maxLength={4096}
                    onChange={(e) => update(index, { path: e.target.value })}
                    aria-label={t('Relative path')}
                    placeholder={t('/private')}
                    className="flex-1 px-2 py-1.5 bg-slate-800 border border-slate-700 rounded-lg font-mono focus:outline-none focus:border-teal-500"
                  />
                  <button
                    onClick={() => setRules((prev) => prev.filter((_, i) => i !== index))}
                    className="p-1.5 rounded text-slate-500 hover:text-rose-400 hover:bg-rose-950/50"
                    title={t('Remove rule')}
                  >
                    <Trash2 className="w-3.5 h-3.5" />
                  </button>
                </div>
                <div className="flex flex-wrap items-center gap-3 text-slate-300">
                  <label className="flex items-center gap-1.5">
                    <input
                      type="checkbox"
                      checked={rule.recursive}
                      onChange={(e) => update(index, { recursive: e.target.checked })}
                    />
                    <span>{t('Include descendants')}</span>
                  </label>
                  <span className="text-slate-500">{t('Deny:')}</span>
                  {POLICY_ACCESS.map((access) => (
                    <label key={access} className="flex items-center gap-1">
                      <input type="checkbox" checked={rule.deny.includes(access)} onChange={() => toggleDeny(index, access)} />
                      <span>{access}</span>
                    </label>
                  ))}
                </div>
              </fieldset>
            ))}

          {view && rules.length === 0 && <p className="text-slate-500">{t('No restrictions in this layer.')}</p>}

          {error && <p role="alert" className="text-rose-300">{error}</p>}
        </div>

        <div className="flex items-center justify-between gap-2 p-4 border-t border-slate-800">
          <button
            onClick={() => setRules((prev) => [...prev, { path: '/', recursive: true, deny: ['discover'] }])}
            disabled={!view || rules.length >= MAX_RULES}
            className="flex items-center gap-1.5 px-3 py-1.5 bg-slate-800 hover:bg-slate-700 disabled:opacity-50 rounded-lg"
          >
            <Plus className="w-3.5 h-3.5" />
            <span>{t('Add restriction')}</span>
          </button>
          <div className="flex items-center gap-2">
            <button onClick={onClose} className="px-3 py-1.5 bg-slate-800 hover:bg-slate-700 rounded-lg">
              {t('Cancel')}
            </button>
            <button
              onClick={save}
              disabled={!view || saving}
              className="px-4 py-1.5 bg-teal-600 hover:bg-teal-500 disabled:opacity-50 text-slate-950 font-semibold rounded-lg"
            >
              {saving ? t('Saving…') : t('Save policy')}
            </button>
          </div>
        </div>
      </div>
    </div>
  );
};
