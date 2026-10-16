import React, { useState, useEffect } from 'react';
import { useTealDrive } from '../../context/TealDriveContext.tsx';
import { tealdrive } from '../../lib/tealdrive.ts';
import { FolderInput, Folder, X, Loader2 } from 'lucide-react';
import { t } from '../../i18n/index.ts';

interface FolderOption {
  path: string;
  name: string;
  depth: number;
}

export const MoveModal: React.FC = () => {
  const { moveModalFile, setMoveModalFile, moveItem } = useTealDrive();
  const [selectedTarget, setSelectedTarget] = useState<string>('/');
  const [moving, setMoving] = useState(false);
  const [loadingFolders, setLoadingFolders] = useState(false);
  const [dynamicFolders, setDynamicFolders] = useState<FolderOption[]>([]);

  useEffect(() => {
    if (!moveModalFile) return;

    let isMounted = true;

    const crawlFolders = async () => {
      setLoadingFolders(true);
      const results: FolderOption[] = [{ path: '/', name: t('Root (/)'), depth: 0 }];
      const visited = new Set<string>(['/']);

      async function scanDir(dirPath: string, currentDepth: number) {
        if (currentDepth > 4) return;
        try {
          const res = await tealdrive.files.list({ path: dirPath });
          const dirEntries = res.entries.filter(
            (e) =>
              e.isDirectory &&
              // Cannot move a folder into itself or its children
              e.path !== moveModalFile?.path &&
              !e.path.startsWith(moveModalFile?.path + '/')
          );

          for (const d of dirEntries) {
            if (!visited.has(d.path)) {
              visited.add(d.path);
              results.push({
                path: d.path,
                name: d.name,
                depth: currentDepth + 1,
              });
              await scanDir(d.path, currentDepth + 1);
            }
          }
        } catch {
          // ignore scan error on single path
        }
      }

      await scanDir('/', 0);

      if (isMounted) {
        setDynamicFolders(results);
        setLoadingFolders(false);
      }
    };

    crawlFolders();

    return () => {
      isMounted = false;
    };
  }, [moveModalFile?.path]);

  if (!moveModalFile) return null;

  const handleMove = async () => {
    setMoving(true);
    try {
      await moveItem(moveModalFile.path, selectedTarget);
      setMoveModalFile(null);
    } finally {
      setMoving(false);
    }
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center p-4 bg-slate-950/80 backdrop-blur-sm animate-in fade-in duration-150">
      <div className="w-full max-w-md bg-slate-900 border border-slate-700/80 rounded-2xl shadow-2xl p-6 text-slate-100">
        <div className="flex items-center justify-between pb-3 border-b border-slate-800 mb-4">
          <div className="flex items-center gap-2">
            <FolderInput className="w-4 h-4 text-sky-400" />
            <h3 className="text-sm font-semibold">{t('Move To Destination')}</h3>
          </div>
          <button
            onClick={() => setMoveModalFile(null)}
            className="p-1 rounded text-slate-400 hover:text-slate-200 hover:bg-slate-800 transition-colors"
          >
            <X className="w-4 h-4" />
          </button>
        </div>

        <div className="text-xs text-slate-300 mb-3">
          {t('Moving:')} <span className="font-mono text-teal-300 font-semibold">{moveModalFile.name}</span>
        </div>

        {loadingFolders ? (
          <div className="h-44 flex flex-col items-center justify-center border border-slate-800 rounded-xl bg-slate-950/60 mb-5 text-slate-400 text-xs">
            <Loader2 className="w-5 h-5 text-teal-400 animate-spin mb-2" />
            <span>{t('Scanning real filesystem directories...')}</span>
          </div>
        ) : (
          <div className="space-y-1 max-h-60 overflow-y-auto border border-slate-800 rounded-xl p-2 bg-slate-950/60 mb-5">
            {dynamicFolders.map((folder) => {
              const isSelected = selectedTarget === folder.path;
              const isCurrentParent =
                moveModalFile.path.startsWith(folder.path === '/' ? '/' : folder.path + '/') &&
                moveModalFile.path.replace(folder.path === '/' ? '/' : folder.path + '/', '').indexOf('/') === -1;

              return (
                <button
                  key={folder.path}
                  type="button"
                  onClick={() => setSelectedTarget(folder.path)}
                  style={{ paddingLeft: `${Math.max(8, folder.depth * 16 + 8)}px` }}
                  className={`w-full flex items-center justify-between p-2 rounded-lg text-xs transition-colors ${
                    isSelected
                      ? 'bg-teal-500/20 text-teal-300 border border-teal-500/30 font-medium'
                      : 'text-slate-300 hover:bg-slate-800/80'
                  }`}
                >
                  <div className="flex items-center gap-2 min-w-0">
                    <Folder className="w-3.5 h-3.5 text-teal-400 shrink-0" />
                    <span className="truncate">{folder.name}</span>
                  </div>
                  {isCurrentParent && (
                    <span className="text-[10px] text-slate-500 font-mono shrink-0 ml-2">
                      (current)
                    </span>
                  )}
                </button>
              );
            })}
          </div>
        )}

        <div className="flex items-center justify-end gap-2">
          <button
            type="button"
            onClick={() => setMoveModalFile(null)}
            className="px-3 py-1.5 bg-slate-800 hover:bg-slate-700 text-slate-300 text-xs rounded-lg transition-colors"
          >
            {t('Cancel')}
          </button>
          <button
            type="button"
            onClick={handleMove}
            disabled={moving || loadingFolders}
            className="px-4 py-1.5 bg-teal-600 hover:bg-teal-500 text-slate-950 font-semibold text-xs rounded-lg shadow transition-all active:scale-95"
          >
            {moving ? 'Moving...' : t('Move Here')}
          </button>
        </div>
      </div>
    </div>
  );
};
