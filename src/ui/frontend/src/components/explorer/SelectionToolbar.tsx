import React from 'react';
import { useTealDrive } from '../../context/TealDriveContext.tsx';
import { formatBytes } from '../../utils/formatters.ts';
import {
  Download,
  Trash2,
  FolderInput,
  Copy,
  Key,
  X,
  Star,
  FileEdit,
  Eye,
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
    setPreviewFile,
    toggleStar,
    copyItem,
    currentPath,
  } = useTealDrive();

  if (selectedIds.size === 0) return null;

  const selectedEntries = entries.filter((e) => selectedIds.has(e.id));
  const isSingle = selectedEntries.length === 1;
  const singleItem = isSingle ? selectedEntries[0] : null;

  const totalBytes = selectedEntries.reduce(
    (acc, curr) => acc + (curr.isDirectory ? 0 : curr.size || 0),
    0
  );
  const allStarred = selectedEntries.length > 0 && selectedEntries.every((e) => e.starred);

  return (
    <div className="sticky top-16 z-30 mx-4 md:mx-auto max-w-4xl my-2 p-2 px-3.5 bg-slate-900/95 border border-teal-500/40 rounded-2xl shadow-2xl shadow-slate-950/80 backdrop-blur-xl flex items-center justify-between gap-3 text-xs text-slate-200 animate-in fade-in slide-in-from-top-2 duration-200">
      {/* Left: Counter badge & Deselect */}
      <div className="flex items-center gap-2.5 shrink-0">
        <div className="flex items-center gap-2 px-2.5 py-1 rounded-xl bg-teal-500/15 border border-teal-500/30 text-teal-300 font-semibold font-mono">
          <span className="w-2 h-2 rounded-full bg-teal-400 animate-pulse shrink-0" />
          <span>
            {selectedIds.size} {selectedIds.size === 1 ? 'selected' : 'selected'}
          </span>
          {totalBytes > 0 && (
            <span className="text-teal-400/80 font-normal hidden sm:inline">
              ({formatBytes(totalBytes)})
            </span>
          )}
        </div>

        <button
          onClick={clearSelection}
          title="Deselect all (Esc)"
          className="text-slate-400 hover:text-slate-200 flex items-center gap-1 px-2 py-1 rounded-lg hover:bg-slate-800 transition-colors"
        >
          <X className="w-3.5 h-3.5" />
          <span className="hidden sm:inline">Deselect</span>
          <kbd className="hidden md:inline-block ml-0.5 text-[10px] text-slate-500 font-mono bg-slate-800 px-1 rounded border border-slate-700/60">
            Esc
          </kbd>
        </button>
      </div>

      {/* Right: Actions */}
      <div className="flex items-center gap-1.5 flex-wrap justify-end">
        {isSingle && singleItem && (
          <>
            <button
              onClick={() => setPreviewFile(singleItem)}
              className="flex items-center gap-1.5 px-2.5 py-1.5 bg-slate-800/90 hover:bg-slate-700 border border-slate-700/50 rounded-lg text-slate-200 transition-colors"
              title="QuickLook (Space)"
            >
              <Eye className="w-3.5 h-3.5 text-teal-400" />
              <span className="hidden sm:inline">Preview</span>
            </button>

            <button
              onClick={() => setRenameModalFile(singleItem)}
              className="flex items-center gap-1.5 px-2.5 py-1.5 bg-slate-800/90 hover:bg-slate-700 border border-slate-700/50 rounded-lg text-slate-200 transition-colors"
              title="Rename (F2)"
            >
              <FileEdit className="w-3.5 h-3.5 text-teal-400" />
              <span className="hidden sm:inline">Rename</span>
            </button>

            <button
              onClick={() => setChmodFile(singleItem)}
              className="flex items-center gap-1.5 px-2.5 py-1.5 bg-slate-800/90 hover:bg-slate-700 border border-slate-700/50 rounded-lg text-slate-200 transition-colors"
              title="Edit Permissions"
            >
              <Key className="w-3.5 h-3.5 text-amber-400" />
              <span className="font-mono text-[11px]">{singleItem.mode}</span>
            </button>
          </>
        )}

        <button
          onClick={() => {
            selectedEntries.forEach((e) => toggleStar(e.path));
          }}
          className="flex items-center gap-1.5 px-2.5 py-1.5 bg-slate-800/90 hover:bg-slate-700 border border-slate-700/50 rounded-lg text-slate-200 transition-colors"
          title={allStarred ? 'Remove from starred' : 'Add to starred'}
        >
          <Star
            className={`w-3.5 h-3.5 ${
              allStarred ? 'text-amber-400 fill-amber-400' : 'text-slate-400'
            }`}
          />
          <span className="hidden md:inline">{allStarred ? 'Unstar' : 'Star'}</span>
        </button>

        <button
          onClick={() => {
            if (isSingle && singleItem) {
              setMoveModalFile(singleItem);
            }
          }}
          className="flex items-center gap-1.5 px-2.5 py-1.5 bg-slate-800/90 hover:bg-slate-700 border border-slate-700/50 rounded-lg text-slate-200 transition-colors"
          title="Move to folder"
        >
          <FolderInput className="w-3.5 h-3.5 text-sky-400" />
          <span className="hidden sm:inline">Move</span>
        </button>

        {isSingle && singleItem && !singleItem.isDirectory && (
          <button
            onClick={() => copyItem(singleItem.path, currentPath)}
            className="flex items-center gap-1.5 px-2.5 py-1.5 bg-slate-800/90 hover:bg-slate-700 border border-slate-700/50 rounded-lg text-slate-200 transition-colors"
            title="Duplicate file"
          >
            <Copy className="w-3.5 h-3.5 text-purple-400" />
            <span className="hidden md:inline">Duplicate</span>
          </button>
        )}

        <button
          onClick={() => {
            if (isSingle && singleItem) {
              downloadFile(singleItem.path);
            } else {
              selectedEntries.forEach((e) => downloadFile(e.path));
            }
          }}
          className="flex items-center gap-1.5 px-2.5 py-1.5 bg-slate-800/90 hover:bg-slate-700 border border-slate-700/50 rounded-lg text-slate-200 transition-colors"
          title="Download"
        >
          <Download className="w-3.5 h-3.5 text-teal-400" />
          <span className="hidden sm:inline">Download</span>
        </button>

        <button
          onClick={() => promptTrash()}
          className="flex items-center gap-1.5 px-2.5 py-1.5 bg-rose-950/80 hover:bg-rose-900 border border-rose-800/60 rounded-lg text-rose-300 transition-colors shadow-sm shadow-rose-950/50 ml-0.5"
          title="Move to Trash (Del)"
        >
          <Trash2 className="w-3.5 h-3.5" />
          <span>Delete</span>
        </button>
      </div>
    </div>
  );
};
