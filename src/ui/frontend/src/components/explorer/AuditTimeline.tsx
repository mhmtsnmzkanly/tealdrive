import React, { useEffect, useState } from 'react';
import { tealdrive } from '../../lib/tealdrive.ts';
import type { UndoPreferences } from '../../sdk/mapping.ts';
import { useTealDrive } from '../../context/TealDriveContext.tsx';
import { formatRelativeDate } from '../../utils/formatters.ts';
import type { AuditEntry } from '../../sdk/types.ts';
import {
  History,
  Undo2,
  FolderPlus,
  FileEdit,
  Key,
  Trash2,
  Upload,
  ArrowRightLeft,
  CheckCircle2,
} from 'lucide-react';
import { t, locale } from '../../i18n/index.ts';

const UNDO_REASONS: Record<string, string> = {
  operationIncomplete: 'Incomplete',
  unsupportedOperation: t('Not undoable'),
  alreadyClaimed: t('Already undone'),
  restoredBackup: t('Before restore'),
  expired: t('Undo expired'),
  missingExpectedState: t('Not undoable'),
  unsupportedOriginalMode: t('Not undoable'),
  incompleteObservation: t('Not undoable'),
};

function undoLabel(entry: AuditEntry): string {
  if (entry.undoStatus) return entry.undoStatus === 'success' ? 'Undone' : t('Undo {undoStatus}', { undoStatus: entry.undoStatus });
  if (entry.undoUnavailableReason) return UNDO_REASONS[entry.undoUnavailableReason] ?? t('Not undoable');
  return t('Not undoable');
}

export const AuditTimeline: React.FC = () => {
  const { auditEntries, undoOperation, auditHasMore, loadMoreAudit, addToast } = useTealDrive();
  const [loadingMore, setLoadingMore] = useState(false);
  const [prefs, setPrefs] = useState<UndoPreferences | null>(null);
  const [hoursInput, setHoursInput] = useState('');
  const [savingPrefs, setSavingPrefs] = useState(false);

  useEffect(() => {
    let cancelled = false;
    tealdrive.files.undo
      .getPreferences()
      .then((p) => {
        if (cancelled) return;
        setPrefs(p);
        setHoursInput(p.hours === null ? '' : String(p.hours));
      })
      // Undo preferences are optional; the history still works without them.
      .catch(() => {});
    return () => {
      cancelled = true;
    };
  }, []);

  const savePrefs = async () => {
    if (!prefs) return;
    const text = hoursInput.trim();
    const hours = text === '' ? null : Number(text);
    if (hours !== null && (!Number.isSafeInteger(hours) || hours < 1 || hours > prefs.maximumHours)) {
      addToast({ title: t('Invalid undo window'), desc: t('Enter 1-{maximumHours} hours, or leave empty for the default.', { maximumHours: prefs.maximumHours }), type: 'error' });
      return;
    }
    setSavingPrefs(true);
    try {
      const next = await tealdrive.files.undo.setPreferences({ hours, expectedRevision: prefs.revision });
      setPrefs(next);
      setHoursInput(next.hours === null ? '' : String(next.hours));
      addToast({ title: t('Undo window saved'), desc: t('Operations stay undoable for {effectiveHours} hours.', { effectiveHours: next.effectiveHours }), type: 'success' });
    } catch (err: any) {
      addToast({
        title: t('Undo window not saved'),
        desc: err?.code === 'CONFLICT' ? t('It changed elsewhere; reopen this view and try again.') : err?.message,
        type: 'error',
      });
    } finally {
      setSavingPrefs(false);
    }
  };

  const loadMore = async () => {
    setLoadingMore(true);
    try {
      await loadMoreAudit();
    } finally {
      setLoadingMore(false);
    }
  };

  const getActionIcon = (action: string) => {
    switch (action) {
      case 'createFolder':
        return <FolderPlus className="w-3.5 h-3.5 text-teal-400" />;
      case 'upload':
        return <Upload className="w-3.5 h-3.5 text-teal-400" />;
      case 'write':
        return <FileEdit className="w-3.5 h-3.5 text-emerald-400" />;
      case 'rename':
      case 'move':
      case 'copy':
        return <ArrowRightLeft className="w-3.5 h-3.5 text-sky-400" />;
      case 'chmod':
        return <Key className="w-3.5 h-3.5 text-amber-400" />;
      case 'trash':
      case 'delete':
        return <Trash2 className="w-3.5 h-3.5 text-rose-400" />;
      case 'restore':
        return <Undo2 className="w-3.5 h-3.5 text-teal-400" />;
      default:
        return <History className="w-3.5 h-3.5 text-slate-400" />;
    }
  };

  return (
    <div className="p-6 max-w-4xl">
      <div className="mb-6">
        <h2 className="text-base font-semibold text-slate-100 flex items-center gap-2">
          <History className="w-4 h-4 text-amber-400" />
          <span>{t('History & Undo')}</span>
        </h2>
        <p className="text-xs text-slate-400 mt-0.5">
          {t('Your recent file operations. Renames, moves, permission changes and trash moves can be undone while the server still observes the recorded state.')}
        </p>
      </div>

      {prefs && (
        <div className="mb-5 p-3 rounded-xl border border-slate-800 bg-slate-900/50 flex flex-wrap items-center gap-2 text-xs text-slate-300">
          <span>{t('Undo window (hours):')}</span>
          <input
            type="number"
            min={1}
            max={prefs.maximumHours}
            value={hoursInput}
            onChange={(e) => setHoursInput(e.target.value)}
            placeholder={t('default ({effectiveHours})', { effectiveHours: prefs.effectiveHours })}
            className="w-32 px-2 py-1 bg-slate-800 border border-slate-700 rounded-lg text-slate-100 focus:outline-none focus:border-teal-500"
          />
          <span className="text-slate-500">{t('max {hours}', { hours: prefs.maximumHours })}</span>
          <button
            onClick={savePrefs}
            disabled={savingPrefs}
            className="ml-auto px-3 py-1 rounded-lg bg-teal-600 hover:bg-teal-500 disabled:opacity-50 text-slate-950 font-semibold"
          >
            {savingPrefs ? t('Saving…') : 'Save'}
          </button>
        </div>
      )}

      {auditEntries.length === 0 ? (
        <div className="p-16 text-center border border-dashed border-slate-800 rounded-xl bg-slate-900/30">
          <History className="w-8 h-8 text-slate-600 mx-auto mb-2" />
          <div className="text-sm font-medium text-slate-300">{t('No operations recorded yet')}</div>
          <div className="text-xs text-slate-500 mt-1">
            {t('Mutations like file uploads, renames, and permission updates will appear here.')}
          </div>
        </div>
      ) : (
        <div className="relative pl-6 space-y-4 before:absolute before:left-2 before:top-2 before:bottom-2 before:w-0.5 before:bg-slate-800">
          {auditEntries.map((entry) => (
            <div
              key={entry.id}
              className="relative flex items-start justify-between gap-4 p-4 rounded-xl border border-slate-800/80 bg-slate-900/60 hover:bg-slate-900 transition-colors group"
            >
              {/* Dot on timeline line */}
              <div className="absolute -left-[27px] top-4 w-4 h-4 rounded-full bg-slate-950 border-2 border-slate-700 flex items-center justify-center">
                <div className="w-1.5 h-1.5 rounded-full bg-teal-400" />
              </div>

              <div className="flex items-start gap-3 min-w-0 flex-1">
                <div className="p-2 rounded-lg bg-slate-800/80 border border-slate-700/60 shrink-0 mt-0.5">
                  {getActionIcon(entry.action)}
                </div>

                <div className="min-w-0">
                  <div className="text-xs font-semibold text-slate-200">
                    {entry.description}
                  </div>
                  <div className="text-[11px] font-mono text-slate-400 mt-0.5 truncate">
                    Target: {entry.targetPath}
                  </div>
                  <div className="text-[10px] text-slate-500 mt-1 flex items-center gap-2">
                    <span>{formatRelativeDate(entry.timestamp)}</span>
                    <span>·</span>
                    <span className="font-mono">{new Date(entry.timestamp).toLocaleTimeString(locale())}</span>
                    <span>·</span>
                    <span className="font-mono text-slate-600">{entry.id}</span>
                  </div>
                </div>
              </div>

              <div className="shrink-0 flex items-center gap-2">
                {entry.undoable && entry.expectedState ? (
                  <button
                    onClick={() => undoOperation({ operationId: entry.id, expectedState: entry.expectedState! })}
                    className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg bg-teal-950/90 hover:bg-teal-900 text-teal-300 border border-teal-700/60 text-xs font-medium transition-all shadow-sm active:scale-95"
                  >
                    <Undo2 className="w-3.5 h-3.5" />
                    <span>{t('Undo')}</span>
                  </button>
                ) : (
                  <span className="flex items-center gap-1 text-[11px] text-slate-500 px-2 py-1 rounded bg-slate-950 border border-slate-800">
                    <CheckCircle2 className="w-3 h-3 text-slate-600" />
                    <span>{undoLabel(entry)}</span>
                  </span>
                )}
              </div>
            </div>
          ))}
        </div>
      )}

      {auditHasMore && (
        <div className="mt-4 text-center">
          <button
            onClick={loadMore}
            disabled={loadingMore}
            className="px-4 py-1.5 rounded-lg bg-slate-800 hover:bg-slate-700 disabled:opacity-50 text-slate-200 text-xs"
          >
            {loadingMore ? t('Loading…') : t('Load older operations')}
          </button>
        </div>
      )}
    </div>
  );
};
