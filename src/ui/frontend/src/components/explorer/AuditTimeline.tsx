import React from 'react';
import { useTealDrive } from '../../context/TealDriveContext.tsx';
import { formatRelativeDate } from '../../utils/formatters.ts';
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

export const AuditTimeline: React.FC = () => {
  const { auditEntries, undoOperation } = useTealDrive();

  const getActionIcon = (action: string) => {
    switch (action) {
      case 'create':
        return <FolderPlus className="w-3.5 h-3.5 text-teal-400" />;
      case 'upload':
        return <Upload className="w-3.5 h-3.5 text-teal-400" />;
      case 'edit':
        return <FileEdit className="w-3.5 h-3.5 text-emerald-400" />;
      case 'rename':
      case 'move':
        return <ArrowRightLeft className="w-3.5 h-3.5 text-sky-400" />;
      case 'chmod':
        return <Key className="w-3.5 h-3.5 text-amber-400" />;
      case 'delete':
        return <Trash2 className="w-3.5 h-3.5 text-rose-400" />;
      default:
        return <History className="w-3.5 h-3.5 text-slate-400" />;
    }
  };

  return (
    <div className="p-6 max-w-4xl">
      <div className="mb-6">
        <h2 className="text-base font-semibold text-slate-100 flex items-center gap-2">
          <History className="w-4 h-4 text-amber-400" />
          <span>Audit History &amp; Instant Undo Engine</span>
        </h2>
        <p className="text-xs text-slate-400 mt-0.5">
          Immutable server-side transaction log. Any filesystem mutation can be rolled back with single-click precision.
        </p>
      </div>

      {auditEntries.length === 0 ? (
        <div className="p-16 text-center border border-dashed border-slate-800 rounded-xl bg-slate-900/30">
          <History className="w-8 h-8 text-slate-600 mx-auto mb-2" />
          <div className="text-sm font-medium text-slate-300">No operations recorded yet</div>
          <div className="text-xs text-slate-500 mt-1">
            Mutations like file uploads, renames, and permission updates will appear here.
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
                    <span className="font-mono">{new Date(entry.timestamp).toLocaleTimeString()}</span>
                    <span>·</span>
                    <span className="font-mono text-slate-600">{entry.id}</span>
                  </div>
                </div>
              </div>

              <div className="shrink-0 flex items-center gap-2">
                {entry.undoable ? (
                  <button
                    onClick={() => undoOperation(entry.id)}
                    className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg bg-teal-950/90 hover:bg-teal-900 text-teal-300 border border-teal-700/60 text-xs font-medium transition-all shadow-sm active:scale-95"
                  >
                    <Undo2 className="w-3.5 h-3.5" />
                    <span>Undo</span>
                  </button>
                ) : (
                  <span className="flex items-center gap-1 text-[11px] text-slate-500 px-2 py-1 rounded bg-slate-950 border border-slate-800">
                    <CheckCircle2 className="w-3 h-3 text-slate-600" />
                    <span>Reverted / Static</span>
                  </span>
                )}
              </div>
            </div>
          ))}
        </div>
      )}
    </div>
  );
};
