import React from 'react';
import { useTealDrive } from '../../context/TealDriveContext.tsx';
import { CheckCircle2, AlertTriangle, Info, XCircle, Undo2, X } from 'lucide-react';

export const ToastContainer: React.FC = () => {
  const { toasts, dismissToast, undoOperation } = useTealDrive();

  if (toasts.length === 0) return null;

  return (
    <div className="fixed top-20 right-6 z-50 flex flex-col gap-2 max-w-md w-full pointer-events-none">
      {toasts.map((toast) => {
        const isSuccess = toast.type === 'success';
        const isError = toast.type === 'error';
        const isWarning = toast.type === 'warning';

        return (
          <div
            key={toast.id}
            className="pointer-events-auto flex items-start gap-3 p-3.5 bg-slate-900/95 border border-slate-700/80 rounded-xl shadow-2xl backdrop-blur-md text-slate-100 transition-all duration-200 animate-in fade-in slide-in-from-bottom-2"
          >
            <div className="shrink-0 mt-0.5">
              {isSuccess && <CheckCircle2 className="w-4 h-4 text-emerald-400" />}
              {isError && <XCircle className="w-4 h-4 text-rose-400" />}
              {isWarning && <AlertTriangle className="w-4 h-4 text-amber-400" />}
              {!isSuccess && !isError && !isWarning && <Info className="w-4 h-4 text-teal-400" />}
            </div>

            <div className="flex-1 min-w-0">
              <div className="text-xs font-semibold text-slate-100">{toast.title}</div>
              {toast.desc && <div className="text-xs text-slate-400 mt-0.5 break-words">{toast.desc}</div>}

              {toast.undoOpId && (
                <button
                  onClick={() => {
                    undoOperation(toast.undoOpId!);
                    dismissToast(toast.id);
                  }}
                  className="mt-2 inline-flex items-center gap-1.5 px-2.5 py-1 text-xs font-medium text-teal-300 bg-teal-950/80 hover:bg-teal-900/90 border border-teal-700/50 rounded-md transition-colors"
                >
                  <Undo2 className="w-3.5 h-3.5" />
                  <span>Undo</span>
                </button>
              )}
            </div>

            <button
              onClick={() => dismissToast(toast.id)}
              className="shrink-0 text-slate-500 hover:text-slate-300 p-0.5 rounded transition-colors"
              aria-label="Dismiss notification"
            >
              <X className="w-3.5 h-3.5" />
            </button>
          </div>
        );
      })}
    </div>
  );
};
