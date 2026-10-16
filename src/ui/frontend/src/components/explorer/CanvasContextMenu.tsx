import React, { useEffect, useRef } from 'react';
import { useTealDrive } from '../../context/TealDriveContext.tsx';
import {
  FolderPlus,
  FileCode,
  UploadCloud,
  FolderUp,
  RefreshCw,
  CheckSquare,
  LayoutGrid,
  List,
} from 'lucide-react';
import { t } from '../../i18n/index.ts';

interface CanvasContextMenuProps {
  position?: { x: number; y: number };
  onClose: () => void;
  onUploadFileClick?: () => void;
  onUploadFolderClick?: () => void;
}

export const CanvasContextMenu: React.FC<CanvasContextMenuProps> = ({
  position,
  onClose,
  onUploadFileClick,
  onUploadFolderClick,
}) => {
  const {
    setNewFolderOpen,
    refreshCurrentDirectory,
    selectAll,
    viewMode,
    setViewMode,
    currentPath,
    setEditorFile,
    session,
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

  const handleAction = (fn: () => void) => {
    fn();
    onClose();
  };

  const handleNewDocument = () => {
    const docPath = `${currentPath === '/' ? '' : currentPath}/Untitled.txt`;
    setEditorFile({
      id: `new_doc_${Date.now()}`,
      name: 'Untitled.txt',
      path: docPath,
      isDirectory: false,
      size: 0,
      modified: new Date().toISOString(),
      mode: '0644',
      owner: session?.username || 'user',
      group: 'tealdrive',
      mimeType: 'text/plain',
      revision: 0,
    });
  };

  const menuWidth = 210;
  const menuHeight = 280;
  const padding = 10;

  const boundedX = position
    ? Math.max(
        padding,
        Math.min(
          position.x,
          (typeof window !== 'undefined' ? window.innerWidth : 1000) - menuWidth - padding
        )
      )
    : undefined;
  const boundedY = position
    ? Math.max(
        padding,
        Math.min(
          position.y,
          (typeof window !== 'undefined' ? window.innerHeight : 800) - menuHeight - padding
        )
      )
    : undefined;

  const style = position
    ? { top: `${boundedY}px`, left: `${boundedX}px` }
    : undefined;

  return (
    <div
      ref={menuRef}
      style={style}
      className={`${
        position ? 'fixed' : 'absolute'
      } w-52 bg-slate-900 border border-slate-700/80 rounded-xl shadow-2xl p-1.5 z-50 text-xs text-slate-200 backdrop-blur-xl animate-in fade-in zoom-in-95 duration-100`}
      onClick={(e) => e.stopPropagation()}
    >
      <div className="px-2.5 py-1 text-[11px] font-medium text-slate-400 truncate border-b border-slate-800 mb-1">
        {t('Folder Actions ({path})', { path: currentPath })}
      </div>

      <button
        onClick={() => handleAction(() => setNewFolderOpen(true))}
        className="w-full flex items-center gap-2 px-2.5 py-1.5 rounded-lg hover:bg-slate-800 text-left transition-colors text-teal-300 hover:text-teal-200"
      >
        <FolderPlus className="w-3.5 h-3.5 text-teal-400" />
        <span>{t('New Folder')}</span>
      </button>

      <button
        onClick={() => handleAction(handleNewDocument)}
        className="w-full flex items-center gap-2 px-2.5 py-1.5 rounded-lg hover:bg-slate-800 text-left transition-colors"
      >
        <FileCode className="w-3.5 h-3.5 text-emerald-400" />
        <span>{t('New Text Document')}</span>
      </button>

      <div className="h-px bg-slate-800 my-1" />

      {onUploadFileClick && (
        <button
          onClick={() => handleAction(onUploadFileClick)}
          className="w-full flex items-center gap-2 px-2.5 py-1.5 rounded-lg hover:bg-slate-800 text-left transition-colors"
        >
          <UploadCloud className="w-3.5 h-3.5 text-sky-400" />
          <span>{t('Upload Files')}</span>
        </button>
      )}

      {onUploadFolderClick && (
        <button
          onClick={() => handleAction(onUploadFolderClick)}
          className="w-full flex items-center gap-2 px-2.5 py-1.5 rounded-lg hover:bg-slate-800 text-left transition-colors"
        >
          <FolderUp className="w-3.5 h-3.5 text-teal-400" />
          <span>{t('Upload Folder')}</span>
        </button>
      )}

      <button
        onClick={() => handleAction(() => refreshCurrentDirectory())}
        className="w-full flex items-center gap-2 px-2.5 py-1.5 rounded-lg hover:bg-slate-800 text-left transition-colors"
      >
        <RefreshCw className="w-3.5 h-3.5 text-slate-400" />
        <span>{t('Refresh Folder')}</span>
      </button>

      <div className="h-px bg-slate-800 my-1" />

      <button
        onClick={() => handleAction(() => selectAll())}
        className="w-full flex items-center gap-2 px-2.5 py-1.5 rounded-lg hover:bg-slate-800 text-left transition-colors"
      >
        <CheckSquare className="w-3.5 h-3.5 text-slate-400" />
        <span>{t('Select All')}</span>
      </button>

      <button
        onClick={() => handleAction(() => setViewMode(viewMode === 'grid' ? 'table' : 'grid'))}
        className="w-full flex items-center gap-2 px-2.5 py-1.5 rounded-lg hover:bg-slate-800 text-left transition-colors"
      >
        {viewMode === 'grid' ? (
          <>
            <List className="w-3.5 h-3.5 text-slate-400" />
            <span>{t('Switch to Table View')}</span>
          </>
        ) : (
          <>
            <LayoutGrid className="w-3.5 h-3.5 text-slate-400" />
            <span>{t('Switch to Grid View')}</span>
          </>
        )}
      </button>
    </div>
  );
};
