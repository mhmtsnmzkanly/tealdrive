import React, { useState } from 'react';
import { useTealDrive } from '../../context/TealDriveContext.tsx';
import { formatBytes } from '../../utils/formatters.ts';
import {
  UploadCloud,
  ChevronDown,
  ChevronUp,
  X,
  CheckCircle2,
  AlertCircle,
  XCircle,
} from 'lucide-react';
import { t } from '../../i18n/index.ts';

export const UploadManagerDock: React.FC = () => {
  const { uploads, cancelUpload, clearCompletedUploads } = useTealDrive();
  const [collapsed, setCollapsed] = useState(false);

  if (uploads.length === 0) return null;

  const activeUploads = uploads.filter((u) => u.status === 'uploading' || u.status === 'queued');
  const completedUploads = uploads.filter((u) => u.status === 'completed');
  const overallProgress =
    uploads.reduce((acc, curr) => acc + curr.progress, 0) / uploads.length;

  return (
    <div className="fixed bottom-6 right-6 z-40 w-80 bg-slate-900 border border-slate-700/80 rounded-2xl shadow-2xl overflow-hidden text-slate-100 backdrop-blur-xl animate-in slide-in-from-bottom-3 duration-200">
      {/* Dock Header */}
      <div
        onClick={() => setCollapsed(!collapsed)}
        className="h-12 px-4 bg-slate-950/80 border-b border-slate-800 flex items-center justify-between cursor-pointer select-none hover:bg-slate-950 transition-colors"
      >
        <div className="flex items-center gap-2.5">
          <UploadCloud className="w-4 h-4 text-teal-400" />
          <div className="text-xs font-semibold text-slate-200">
            {activeUploads.length > 0
              ? t('Uploading {count} files ({percent}%)', {
                  count: activeUploads.length,
                  percent: Math.round(overallProgress),
                })
              : t('{count} uploads complete', { count: completedUploads.length })}
          </div>
        </div>

        <div className="flex items-center gap-1.5">
          {completedUploads.length > 0 && activeUploads.length === 0 && (
            <button
              onClick={(e) => {
                e.stopPropagation();
                clearCompletedUploads();
              }}
              className="text-[10px] text-teal-400 hover:text-teal-300 font-medium px-1.5 py-0.5 rounded hover:bg-slate-800 transition-colors"
            >
              {t('Clear')}
            </button>
          )}

          <button className="text-slate-400 hover:text-slate-200 p-0.5">
            {collapsed ? <ChevronUp className="w-4 h-4" /> : <ChevronDown className="w-4 h-4" />}
          </button>
        </div>
      </div>

      {/* Expanded list of uploads */}
      {!collapsed && (
        <div className="max-h-60 overflow-y-auto divide-y divide-slate-800/60 p-1">
          {uploads.map((item) => (
            <div key={item.fileId} className="p-2.5 hover:bg-slate-800/40 rounded-lg transition-colors text-xs">
              <div className="flex items-center justify-between mb-1.5">
                <span className="font-medium text-slate-200 truncate max-w-[190px]" title={item.fileName}>
                  {item.fileName}
                </span>

                <div className="flex items-center gap-2">
                  {(item.status === 'uploading' || item.status === 'queued') && (
                    <button
                      onClick={() => cancelUpload(item.fileId)}
                      className="text-slate-400 hover:text-rose-400 p-0.5"
                      title={t('Cancel upload')}
                    >
                      <X className="w-3.5 h-3.5" />
                    </button>
                  )}
                  {item.status === 'completed' && (
                    <CheckCircle2 className="w-4 h-4 text-emerald-400 shrink-0" />
                  )}
                  {item.status === 'error' && (
                    <span title={item.error}>
                      <AlertCircle className="w-4 h-4 text-rose-400 shrink-0" />
                    </span>
                  )}
                  {item.status === 'aborted' && (
                    <XCircle className="w-4 h-4 text-slate-500 shrink-0" />
                  )}
                </div>
              </div>

              {/* Progress bar */}
              <div className="w-full bg-slate-800 rounded-full h-1.5 overflow-hidden mb-1">
                <div
                  className={`h-full rounded-full transition-all duration-150 ${
                    item.status === 'completed'
                      ? 'bg-emerald-500'
                      : item.status === 'error' || item.status === 'aborted'
                      ? 'bg-rose-500'
                      : 'bg-teal-500'
                  }`}
                  style={{ width: `${item.progress}%` }}
                />
              </div>

              <div className="flex items-center justify-between text-[10px] font-mono tabular-nums text-slate-500">
                <span className="truncate" title={item.error}>
                  {item.status === 'error' && item.error
                    ? item.error
                    : `${formatBytes(item.uploadedBytes)} of ${formatBytes(item.size)}`}
                </span>
                <span>
                  {item.status === 'uploading'
                    ? item.speedBytesPerSec > 0
                      ? `${formatBytes(item.speedBytesPerSec)}/s`
                      : t('starting')
                    : t(item.status)}
                </span>
              </div>
            </div>
          ))}
        </div>
      )}
    </div>
  );
};
