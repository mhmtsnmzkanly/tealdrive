import React from 'react';
import { useTealDrive } from '../../context/TealDriveContext.tsx';
import {
  Download,
  Trash2,
  FolderInput,
  Copy,
  Key,
  X,
  Star,
  FileEdit,
} from 'lucide-react';

export const SelectionToolbar: React.FC = () => {
  const {
    selectedIds,
    clearSelection,
    entries,
    promptTrash,
    downloadFile,
    setChmodFile,
    setMoveModalFile,
    setRenameModalFile,
    copyItem,
    currentPath,
  } = useTealDrive();

  if (selectedIds.size === 0) return null;

  const selectedEntries = entries.filter((e) => selectedIds.has(e.id));
  const isSingle = selectedEntries.length === 1;
  const singleItem = isSingle ? selectedEntries[0] : null;

  return (
    <div className="sticky top-16 z-10 mx-6 my-2 p-2 px-4 bg-slate-900/95 border border-teal-500/30 rounded-xl shadow-xl flex items-center justify-between text-xs text-slate-200 backdrop-blur-md animate-in fade-in slide-in-from-top-1 duration-150">
      <div className="flex items-center gap-3">
        <span className="font-semibold text-teal-300 font-mono tabular-nums">
          {selectedIds.size} {selectedIds.size === 1 ? 'item' : 'items'} selected
        </span>
        <div className="h-4 w-px bg-slate-800" />
        <button
          onClick={clearSelection}
          className="text-slate-400 hover:text-slate-200 flex items-center gap-1 transition-colors"
        >
          <X className="w-3.5 h-3.5" />
          <span>Deselect</span>
        </button>
      </div>

      <div className="flex items-center gap-1.5 flex-wrap">
        {isSingle && singleItem && (
          <>
            <button
              onClick={() => setRenameModalFile(singleItem)}
              className="flex items-center gap-1.5 px-2.5 py-1.5 bg-slate-800 hover:bg-slate-700 rounded-lg text-slate-200 transition-colors"
            >
              <FileEdit className="w-3.5 h-3.5 text-teal-400" />
              <span>Rename (F2)</span>
            </button>

            <button
              onClick={() => setChmodFile(singleItem)}
              className="flex items-center gap-1.5 px-2.5 py-1.5 bg-slate-800 hover:bg-slate-700 rounded-lg text-slate-200 transition-colors"
            >
              <Key className="w-3.5 h-3.5 text-amber-400" />
              <span>Chmod ({singleItem.mode})</span>
            </button>
          </>
        )}

        <button
          onClick={() => {
            if (isSingle && singleItem) {
              setMoveModalFile(singleItem);
            }
          }}
          className="flex items-center gap-1.5 px-2.5 py-1.5 bg-slate-800 hover:bg-slate-700 rounded-lg text-slate-200 transition-colors"
        >
          <FolderInput className="w-3.5 h-3.5 text-sky-400" />
          <span>Move to...</span>
        </button>

        {isSingle && singleItem && !singleItem.isDirectory && (
          <button
            onClick={() => copyItem(singleItem.path, currentPath)}
            className="flex items-center gap-1.5 px-2.5 py-1.5 bg-slate-800 hover:bg-slate-700 rounded-lg text-slate-200 transition-colors"
          >
            <Copy className="w-3.5 h-3.5 text-purple-400" />
            <span>Duplicate</span>
          </button>
        )}

        <button
          onClick={() => {
            if (isSingle && singleItem) {
              downloadFile(singleItem.path);
            } else {
              // Batch download
              selectedEntries.forEach((e) => downloadFile(e.path));
            }
          }}
          className="flex items-center gap-1.5 px-2.5 py-1.5 bg-slate-800 hover:bg-slate-700 rounded-lg text-slate-200 transition-colors"
        >
          <Download className="w-3.5 h-3.5 text-teal-400" />
          <span>Download</span>
        </button>

        <button
          onClick={() => promptTrash()}
          className="flex items-center gap-1.5 px-2.5 py-1.5 bg-rose-950/80 hover:bg-rose-900 border border-rose-800/60 rounded-lg text-rose-300 transition-colors ml-1"
        >
          <Trash2 className="w-3.5 h-3.5" />
          <span>Delete</span>
        </button>
      </div>
    </div>
  );
};
