import React, { useState } from 'react';
import { useTealDrive } from '../../context/TealDriveContext.tsx';
import { Trash2, AlertTriangle, X, Loader2 } from 'lucide-react';
import { formatBytes } from '../../utils/formatters.ts';
import { FileItemIcon } from '../explorer/FileItemIcon.tsx';
import { t } from '../../i18n/index.ts';

export const ConfirmDeleteModal: React.FC = () => {
  const { deleteConfirm, setDeleteConfirm } = useTealDrive();
  const [submitting, setSubmitting] = useState(false);

  if (!deleteConfirm || !deleteConfirm.isOpen) return null;

  const {
    title,
    description,
    items = [],
    isPermanent = false,
    confirmLabel,
    onConfirm,
  } = deleteConfirm;

  const handleClose = () => {
    if (submitting) return;
    setDeleteConfirm(null);
  };

  const handleConfirm = async () => {
    setSubmitting(true);
    try {
      await onConfirm();
      setDeleteConfirm(null);
    } catch (err) {
      console.error('Delete action failed:', err);
    } finally {
      setSubmitting(false);
    }
  };

  const defaultButtonLabel = isPermanent ? t('Delete Permanently') : t('Move to Trash');
  const actionLabel = confirmLabel || defaultButtonLabel;

  // Show up to 4 items in detail preview, then collapse remaining
  const previewItems = items.slice(0, 4);
  const remainingCount = items.length - previewItems.length;

  return (
    <div
      role="dialog"
      aria-modal="true"
      className="fixed inset-0 z-50 flex items-center justify-center p-4 bg-slate-950/80 backdrop-blur-sm animate-in fade-in duration-150"
      onKeyDown={(e) => {
        if (e.key === 'Escape') handleClose();
      }}
    >
      <div className="w-full max-w-md bg-slate-900 border border-slate-700/80 rounded-2xl shadow-2xl overflow-hidden text-slate-100 animate-in zoom-in-95 duration-150">
        {/* Header with Danger Accent */}
        <div className="flex items-center justify-between p-5 border-b border-slate-800 bg-slate-900/60">
          <div className="flex items-center gap-3">
            <div
              className={`w-10 h-10 rounded-xl flex items-center justify-center shrink-0 shadow-inner ${
                isPermanent
                  ? 'bg-rose-500/15 border border-rose-500/30 text-rose-400'
                  : 'bg-amber-500/15 border border-amber-500/30 text-amber-400'
              }`}
            >
              {isPermanent ? (
                <AlertTriangle className="w-5 h-5 text-rose-400" />
              ) : (
                <Trash2 className="w-5 h-5 text-amber-400" />
              )}
            </div>
            <div>
              <h3 className="text-sm font-semibold text-slate-100 leading-snug">
                {title}
              </h3>
              <p className="text-[11px] text-slate-400 mt-0.5">
                {isPermanent ? t('Permanent deletion warning') : t('Staged trash confirmation')}
              </p>
            </div>
          </div>
          <button
            onClick={handleClose}
            disabled={submitting}
            className="p-1 rounded-lg text-slate-400 hover:text-slate-200 hover:bg-slate-800 transition-colors disabled:opacity-50"
            title={t('Cancel')}
          >
            <X className="w-4 h-4" />
          </button>
        </div>

        {/* Content Body */}
        <div className="p-5 space-y-4 text-xs">
          {description && (
            <p className="text-slate-300 leading-relaxed">{description}</p>
          )}

          {/* Staged Items Preview */}
          {items.length > 0 && (
            <div className="space-y-1.5">
              <div className="text-[11px] font-medium text-slate-400 flex items-center justify-between">
                <span>{items.length === 1 ? t('Affected Item') : t('Affected Items')}</span>
                <span className="font-mono text-slate-500">{t('{count} total', { count: items.length })}</span>
              </div>
              <div className="bg-slate-950/70 border border-slate-800 rounded-xl p-2.5 max-h-48 overflow-y-auto divide-y divide-slate-800/60">
                {previewItems.map((item, idx) => (
                  <div
                    key={item.path || item.name || idx}
                    className="flex items-center justify-between py-1.5 first:pt-0 last:pb-0 text-slate-200"
                  >
                    <div className="flex items-center gap-2.5 min-w-0 pr-2">
                      <FileItemIcon
                        name={item.name}
                        isDirectory={Boolean(item.isDirectory)}
                        mimeType={item.mimeType}
                        size="sm"
                      />
                      <span className="truncate font-medium text-slate-200" title={item.name}>
                        {item.name}
                      </span>
                    </div>
                    {item.size !== undefined && !item.isDirectory && (
                      <span className="text-[11px] font-mono text-slate-400 shrink-0">
                        {formatBytes(item.size)}
                      </span>
                    )}
                  </div>
                ))}
                {remainingCount > 0 && (
                  <div className="pt-2 text-[11px] text-slate-500 font-medium text-center">
                    {t('+ {count} more items', { count: remainingCount })}
                  </div>
                )}
              </div>
            </div>
          )}

          {/* Warning Callout for Permanent Deletions */}
          {isPermanent && (
            <div className="p-3 bg-rose-950/40 border border-rose-900/60 rounded-xl flex items-start gap-2.5 text-rose-300">
              <AlertTriangle className="w-4 h-4 text-rose-400 shrink-0 mt-0.5" />
              <div className="text-[11px] leading-relaxed">
                <span className="font-semibold text-rose-200">{t('Irreversible Action:')} </span>
                {t('This will be permanently erased from disk and cannot be restored from the Trash bin.')}
              </div>
            </div>
          )}
        </div>

        {/* Action Buttons Footer */}
        <div className="flex items-center justify-end gap-2.5 p-4 border-t border-slate-800/80 bg-slate-900/40">
          <button
            type="button"
            onClick={handleClose}
            disabled={submitting}
            className="px-4 py-2 bg-slate-800 hover:bg-slate-700 active:scale-95 text-slate-300 font-medium text-xs rounded-xl transition-all disabled:opacity-50"
          >
            {t('Cancel')}
          </button>
          <button
            type="button"
            autoFocus
            onClick={handleConfirm}
            disabled={submitting}
            className={`px-4 py-2 font-semibold text-xs rounded-xl shadow-lg flex items-center gap-1.5 transition-all active:scale-95 disabled:opacity-50 ${
              isPermanent
                ? 'bg-rose-600 hover:bg-rose-500 text-white shadow-rose-950/50'
                : 'bg-rose-600 hover:bg-rose-500 text-white shadow-rose-950/50'
            }`}
          >
            {submitting ? (
              <>
                <Loader2 className="w-3.5 h-3.5 animate-spin" />
                <span>{t('Processing...')}</span>
              </>
            ) : (
              <>
                <Trash2 className="w-3.5 h-3.5" />
                <span>{actionLabel}</span>
              </>
            )}
          </button>
        </div>
      </div>
    </div>
  );
};
