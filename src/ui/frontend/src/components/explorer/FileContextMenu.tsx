import React, { useEffect, useRef } from 'react';
import { useTealDrive } from '../../context/TealDriveContext.tsx';
import type { FileEntry } from '../../sdk/types.ts';
import {
  Eye,
  FileCode,
  Download,
  FileEdit,
  FolderInput,
  Copy,
  Key,
  Trash2,
  Star,
} from 'lucide-react';
import { getFileCategory, isTextFile } from '../../utils/formatters.ts';

interface FileContextMenuProps {
  file: FileEntry;
  onClose: () => void;
  position?: { x: number; y: number };
}

export const FileContextMenu: React.FC<FileContextMenuProps> = ({ file, onClose, position }) => {
  const {
    goToPath,
    setPreviewFile,
    setEditorFile,
    setChmodFile,
    setRenameModalFile,
    setMoveModalFile,
    promptTrash,
    downloadFile,
    copyItem,
    toggleStar,
    currentPath,
  } = useTealDrive();

  const menuRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const handleOutside = (e: MouseEvent | TouchEvent) => {
      if (menuRef.current && !menuRef.current.contains(e.target as Node)) {
        onClose();
      }
    };
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Escape') onClose();
    };

    document.addEventListener('mousedown', handleOutside);
    document.addEventListener('touchstart', handleOutside, { passive: true });
    window.addEventListener('keydown', handleKeyDown);
    window.addEventListener('resize', onClose);

    return () => {
      document.removeEventListener('mousedown', handleOutside);
      document.removeEventListener('touchstart', handleOutside);
      window.removeEventListener('keydown', handleKeyDown);
      window.removeEventListener('resize', onClose);
    };
  }, [onClose]);

  const category = getFileCategory(file.name, file.isDirectory, file.mimeType);
  const isEditable =
    !file.isDirectory &&
    (category === 'code' || category === 'document' || isTextFile(file.name, file.mimeType));

  const handleAction = (fn: () => void) => {
    fn();
    onClose();
  };

  const menuWidth = 224;
  const menuHeight = 390;
  const padding = 10;

  const boundedX = position
    ? Math.max(padding, Math.min(position.x, (typeof window !== 'undefined' ? window.innerWidth : 1000) - menuWidth - padding))
    : undefined;
  const boundedY = position
    ? Math.max(padding, Math.min(position.y, (typeof window !== 'undefined' ? window.innerHeight : 800) - menuHeight - padding))
    : undefined;

  const style = position
    ? { top: `${boundedY}px`, left: `${boundedX}px` }
    : undefined;

  return (
    <div
      ref={menuRef}
      style={style}
      className={`${
        position ? 'fixed' : 'absolute right-0 top-8'
      } w-52 bg-slate-900 border border-slate-700/80 rounded-xl shadow-2xl p-1.5 z-50 text-xs text-slate-200 backdrop-blur-xl animate-in fade-in zoom-in-95 duration-100`}
      onClick={(e) => e.stopPropagation()}
    >
      <div className="px-2.5 py-1 text-[11px] font-medium text-slate-400 truncate border-b border-slate-800 mb-1">
        {file.name}
      </div>

      {file.isDirectory ? (
        <button
          onClick={() => handleAction(() => goToPath(file.path))}
          className="w-full flex items-center gap-2 px-2.5 py-1.5 rounded-lg hover:bg-slate-800 text-left transition-colors"
        >
          <Eye className="w-3.5 h-3.5 text-teal-400" />
          <span>Open Folder</span>
        </button>
      ) : (
        <button
          onClick={() => handleAction(() => setPreviewFile(file))}
          className="w-full flex items-center gap-2 px-2.5 py-1.5 rounded-lg hover:bg-slate-800 text-left transition-colors"
        >
          <Eye className="w-3.5 h-3.5 text-teal-400" />
          <span>QuickLook (Space)</span>
        </button>
      )}

      {isEditable && (
        <button
          onClick={() => handleAction(() => setEditorFile(file))}
          className="w-full flex items-center gap-2 px-2.5 py-1.5 rounded-lg hover:bg-slate-800 text-left transition-colors"
        >
          <FileCode className="w-3.5 h-3.5 text-emerald-400" />
          <span>Edit Code / Text</span>
        </button>
      )}

      <button
        onClick={() => handleAction(() => toggleStar(file.path))}
        className="w-full flex items-center gap-2 px-2.5 py-1.5 rounded-lg hover:bg-slate-800 text-left transition-colors"
      >
        <Star className={`w-3.5 h-3.5 ${file.starred ? 'text-amber-400 fill-amber-400' : 'text-slate-400'}`} />
        <span>{file.starred ? 'Unstar' : 'Add to Starred'}</span>
      </button>

      {!file.isDirectory && (
        <button
          onClick={() => handleAction(() => downloadFile(file.path))}
          className="w-full flex items-center gap-2 px-2.5 py-1.5 rounded-lg hover:bg-slate-800 text-left transition-colors"
        >
          <Download className="w-3.5 h-3.5 text-sky-400" />
          <span>Download</span>
        </button>
      )}

      <div className="h-px bg-slate-800 my-1" />

      <button
        onClick={() => handleAction(() => setRenameModalFile(file))}
        className="w-full flex items-center gap-2 px-2.5 py-1.5 rounded-lg hover:bg-slate-800 text-left transition-colors"
      >
        <FileEdit className="w-3.5 h-3.5 text-slate-300" />
        <span>Rename (F2)</span>
      </button>

      <button
        onClick={() => handleAction(() => setMoveModalFile(file))}
        className="w-full flex items-center gap-2 px-2.5 py-1.5 rounded-lg hover:bg-slate-800 text-left transition-colors"
      >
        <FolderInput className="w-3.5 h-3.5 text-slate-300" />
        <span>Move to...</span>
      </button>

      {!file.isDirectory && (
        <button
          onClick={() => handleAction(() => copyItem(file.path, currentPath))}
          className="w-full flex items-center gap-2 px-2.5 py-1.5 rounded-lg hover:bg-slate-800 text-left transition-colors"
        >
          <Copy className="w-3.5 h-3.5 text-slate-300" />
          <span>Duplicate</span>
        </button>
      )}

      <button
        onClick={() => handleAction(() => setChmodFile(file))}
        className="w-full flex items-center gap-2 px-2.5 py-1.5 rounded-lg hover:bg-slate-800 text-left transition-colors"
      >
        <Key className="w-3.5 h-3.5 text-amber-400" />
        <span>Permissions (chmod {file.mode})</span>
      </button>

      <div className="h-px bg-slate-800 my-1" />

      <button
        onClick={() => handleAction(() => promptTrash(file.path))}
        className="w-full flex items-center gap-2 px-2.5 py-1.5 rounded-lg hover:bg-rose-950/80 text-rose-300 text-left transition-colors"
      >
        <Trash2 className="w-3.5 h-3.5" />
        <span>Move to Trash</span>
      </button>
    </div>
  );
};
