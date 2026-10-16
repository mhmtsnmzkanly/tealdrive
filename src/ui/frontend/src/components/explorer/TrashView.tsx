import React from 'react';
import { useTealDrive } from '../../context/TealDriveContext.tsx';
import { FileItemIcon } from './FileItemIcon.tsx';
import { formatBytes, formatRelativeDate } from '../../utils/formatters.ts';
import { Trash2, RotateCcw, AlertOctagon } from 'lucide-react';
import { t } from '../../i18n/index.ts';

export const TrashView: React.FC = () => {
  const { trashEntries, restoreFromTrash, promptEmptyTrash, promptPermanentDelete } = useTealDrive();

  return (
    <div className="p-6">
      <div className="flex items-center justify-between mb-4">
        <div>
          <h2 className="text-base font-semibold text-slate-100 flex items-center gap-2">
            <Trash2 className="w-4 h-4 text-rose-400" />
            <span>{t('Trash')}</span>
          </h2>
          <p className="text-xs text-slate-400 mt-0.5">
            {t('Deleted files are purged automatically after the retention period set on the server')}
          </p>
        </div>

        {trashEntries.length > 0 && (
          <button
            onClick={() => promptEmptyTrash()}
            className="flex items-center gap-1.5 px-3 py-1.5 bg-rose-950/80 hover:bg-rose-900 border border-rose-800 text-rose-300 text-xs font-medium rounded-lg transition-colors"
          >
            <AlertOctagon className="w-3.5 h-3.5" />
            <span>{t('Empty Trash')}</span>
          </button>
        )}
      </div>

      {trashEntries.length === 0 ? (
        <div className="p-16 text-center border border-dashed border-slate-800 rounded-xl bg-slate-900/30">
          <Trash2 className="w-8 h-8 text-slate-600 mx-auto mb-2" />
          <div className="text-sm font-medium text-slate-300">{t('Trash is empty')}</div>
          <div className="text-xs text-slate-500 mt-1">{t('Deleted items will be staged here.')}</div>
        </div>
      ) : (
        <div className="overflow-x-auto rounded-xl border border-slate-800 bg-slate-900/40">
          <table className="w-full text-left border-collapse text-xs">
            <thead>
              <tr className="border-b border-slate-800 bg-slate-900/80 text-slate-400 font-medium">
                <th className="py-3 px-4">{t('Item Name')}</th>
                <th className="py-3 px-4 w-48">{t('Original Location')}</th>
                <th className="py-3 px-4 w-28 text-right">{t('Size')}</th>
                <th className="py-3 px-4 w-36">{t('Deleted Date')}</th>
                <th className="py-3 px-4 w-44 text-right">{t('Actions')}</th>
              </tr>
            </thead>
            <tbody className="divide-y divide-slate-800/60">
              {trashEntries.map((file) => (
                <tr key={file.id} className="hover:bg-slate-800/40 text-slate-300 transition-colors">
                  <td className="py-2.5 px-4">
                    <div className="flex items-center gap-2.5">
                      <FileItemIcon
                        name={file.name}
                        isDirectory={file.isDirectory}
                        mimeType={file.mimeType}
                        size="sm"
                      />
                      <span className="font-medium text-slate-200">{file.name}</span>
                    </div>
                  </td>
                  <td className="py-2.5 px-4 font-mono text-[11px] text-slate-400 truncate max-w-xs">
                    {file.originalPath || file.path}
                  </td>
                  <td className="py-2.5 px-4 font-mono tabular-nums text-slate-400 text-right">
                    {file.isDirectory ? '—' : formatBytes(file.size)}
                  </td>
                  <td className="py-2.5 px-4 text-slate-400 whitespace-nowrap">
                    {formatRelativeDate(file.trashedAt || file.modified)}
                  </td>
                  <td className="py-2.5 px-4 text-right">
                    <div className="flex items-center justify-end gap-1.5">
                      <button
                        onClick={() => restoreFromTrash(file.id)}
                        className="inline-flex items-center gap-1 px-2.5 py-1 rounded bg-teal-950/80 hover:bg-teal-900 border border-teal-700/50 text-teal-300 transition-colors text-[11px]"
                      >
                        <RotateCcw className="w-3 h-3" />
                        <span>{t('Restore')}</span>
                      </button>
                      <button
                        onClick={() => promptPermanentDelete(file)}
                        className="inline-flex items-center gap-1 px-2.5 py-1 rounded hover:bg-rose-950/80 border border-transparent hover:border-rose-800 text-slate-400 hover:text-rose-300 transition-colors text-[11px]"
                      >
                        <span>{t('Delete')}</span>
                      </button>
                    </div>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
    </div>
  );
};
