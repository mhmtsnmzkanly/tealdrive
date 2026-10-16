import React, { useState, useRef } from 'react';
import { useTealDrive } from '../../context/TealDriveContext.tsx';
import type { FileEntry } from '../../sdk/types.ts';
import { FileItemIcon } from './FileItemIcon.tsx';
import { FileContextMenu } from './FileContextMenu.tsx';
import { CanvasContextMenu } from './CanvasContextMenu.tsx';
import { formatBytes, getFileCategory } from '../../utils/formatters.ts';
import { svgToSafeDataUrl } from '../../utils/safeSvg.ts';
import { Star, MoreVertical, UploadCloud, Check } from 'lucide-react';

interface FileGridProps {
  files: FileEntry[];
  onUploadDropped: (files: FileList) => void;
}

export const FileGrid: React.FC<FileGridProps> = ({ files, onUploadDropped }) => {
  const {
    selectedIds,
    toggleSelect,
    selectItem,
    goToPath,
    setPreviewFile,
    currentPath,
    toggleStar,
    setNewFolderOpen,
  } = useTealDrive();

  const [contextMenu, setContextMenu] = useState<{
    file: FileEntry;
    position: { x: number; y: number };
  } | null>(null);

  const [canvasContextMenu, setCanvasContextMenu] = useState<{
    position: { x: number; y: number };
  } | null>(null);

  const [dragOver, setDragOver] = useState(false);

  const fileInputRef = useRef<HTMLInputElement>(null);
  const folderInputRef = useRef<HTMLInputElement>(null);
  const longPressTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const touchStartRef = useRef<{ x: number; y: number }>({ x: 0, y: 0 });
  const touchMovedRef = useRef(false);
  const longPressTriggeredRef = useRef(false);

  const handleDragOver = (e: React.DragEvent) => {
    e.preventDefault();
    setDragOver(true);
  };

  const handleDragLeave = (e: React.DragEvent) => {
    e.preventDefault();
    setDragOver(false);
  };

  const handleDrop = (e: React.DragEvent) => {
    e.preventDefault();
    setDragOver(false);
    if (e.dataTransfer.files && e.dataTransfer.files.length > 0) {
      onUploadDropped(e.dataTransfer.files);
    }
  };

  const handleDoubleClick = (file: FileEntry) => {
    if (file.isDirectory) {
      goToPath(file.path);
    } else {
      setPreviewFile(file);
    }
  };

  // Right-click on file card
  const handleItemContextMenu = (e: React.MouseEvent, file: FileEntry) => {
    e.preventDefault();
    e.stopPropagation();
    if (!selectedIds.has(file.id)) {
      toggleSelect(file.id, false);
    }
    setCanvasContextMenu(null);
    setContextMenu({
      file,
      position: { x: e.clientX, y: e.clientY },
    });
  };

  // Touch Long-Press on file card
  const handleItemTouchStart = (e: React.TouchEvent, file: FileEntry) => {
    if (e.touches.length !== 1) return;
    const touch = e.touches[0];
    touchStartRef.current = { x: touch.clientX, y: touch.clientY };
    touchMovedRef.current = false;
    longPressTriggeredRef.current = false;

    if (longPressTimerRef.current) clearTimeout(longPressTimerRef.current);
    longPressTimerRef.current = setTimeout(() => {
      longPressTriggeredRef.current = true;
      try {
        if (typeof navigator !== 'undefined' && 'vibrate' in navigator) {
          navigator.vibrate?.(40);
        }
      } catch {}
      if (!selectedIds.has(file.id)) {
        toggleSelect(file.id, false);
      }
      setCanvasContextMenu(null);
      setContextMenu({
        file,
        position: { x: touchStartRef.current.x, y: touchStartRef.current.y },
      });
    }, 450);
  };

  const handleItemTouchMove = (e: React.TouchEvent) => {
    if (!longPressTimerRef.current) return;
    if (e.touches.length !== 1) {
      clearTimeout(longPressTimerRef.current);
      longPressTimerRef.current = null;
      return;
    }
    const touch = e.touches[0];
    const dist = Math.hypot(
      touch.clientX - touchStartRef.current.x,
      touch.clientY - touchStartRef.current.y
    );
    if (dist > 10) {
      touchMovedRef.current = true;
      clearTimeout(longPressTimerRef.current);
      longPressTimerRef.current = null;
    }
  };

  const handleItemTouchEnd = (e: React.TouchEvent) => {
    if (longPressTimerRef.current) {
      clearTimeout(longPressTimerRef.current);
      longPressTimerRef.current = null;
    }
    if (longPressTriggeredRef.current) {
      if (e.cancelable) e.preventDefault();
      longPressTriggeredRef.current = false;
    }
  };

  // Right-click on empty canvas / background
  const handleCanvasContextMenu = (e: React.MouseEvent) => {
    e.preventDefault();
    setContextMenu(null);
    setCanvasContextMenu({
      position: { x: e.clientX, y: e.clientY },
    });
  };

  // Touch Long-Press on empty canvas
  const handleCanvasTouchStart = (e: React.TouchEvent) => {
    // Only trigger if directly on the background canvas
    if (e.target !== e.currentTarget && !(e.target as HTMLElement).classList.contains('canvas-empty-area')) {
      return;
    }
    if (e.touches.length !== 1) return;
    const touch = e.touches[0];
    const coords = { x: touch.clientX, y: touch.clientY };

    if (longPressTimerRef.current) clearTimeout(longPressTimerRef.current);
    longPressTimerRef.current = setTimeout(() => {
      try {
        if (typeof navigator !== 'undefined' && 'vibrate' in navigator) {
          navigator.vibrate?.(40);
        }
      } catch {}
      setContextMenu(null);
      setCanvasContextMenu({ position: coords });
    }, 500);
  };

  return (
    <div
      onDragOver={handleDragOver}
      onDragLeave={handleDragLeave}
      onDrop={handleDrop}
      onContextMenu={handleCanvasContextMenu}
      onTouchStart={handleCanvasTouchStart}
      onTouchMove={handleItemTouchMove}
      onTouchEnd={handleItemTouchEnd}
      onTouchCancel={handleItemTouchEnd}
      className="relative min-h-[480px] p-6 focus:outline-none select-none canvas-empty-area"
    >
      {/* Drop files overlay */}
      {dragOver && (
        <div className="absolute inset-4 z-30 rounded-2xl border-2 border-dashed border-teal-400 bg-slate-950/80 backdrop-blur-md flex flex-col items-center justify-center text-teal-300 animate-in fade-in duration-150">
          <UploadCloud className="w-12 h-12 mb-3 text-teal-400 animate-bounce" />
          <div className="text-base font-semibold">Drop files to upload</div>
          <div className="text-xs text-slate-400 font-mono mt-1">Target: {currentPath}</div>
        </div>
      )}

      {files.length === 0 ? (
        <div className="flex flex-col items-center justify-center py-20 text-center canvas-empty-area">
          <div className="w-16 h-16 rounded-2xl bg-slate-900 border border-slate-800 flex items-center justify-center text-slate-500 mb-4 pointer-events-none">
            <UploadCloud className="w-8 h-8 text-teal-500/60" />
          </div>
          <h3 className="text-sm font-semibold text-slate-200 pointer-events-none">This directory is empty</h3>
          <p className="text-xs text-slate-400 max-w-sm mt-1 pointer-events-none">
            Drag and drop files here, right-click, or long-press to open the action menu.
          </p>
          <div className="mt-4 flex gap-2">
            <button
              onClick={() => setNewFolderOpen(true)}
              className="px-3 py-1.5 bg-slate-800 hover:bg-slate-700 text-teal-300 text-xs font-medium rounded-lg border border-slate-700 transition-colors"
            >
              Create Folder
            </button>
          </div>
        </div>
      ) : (
        <div className="grid grid-cols-2 sm:grid-cols-3 md:grid-cols-4 lg:grid-cols-5 xl:grid-cols-6 gap-3.5">
          {files.map((file) => {
            const isSelected = selectedIds.has(file.id);
            const category = getFileCategory(file.name, file.isDirectory, file.mimeType);

            return (
              <div
                key={file.id}
                onClick={(e) => {
                  e.stopPropagation();
                  selectItem(file.id, { toggle: e.metaKey || e.ctrlKey, range: e.shiftKey });
                }}
                onDoubleClick={() => handleDoubleClick(file)}
                onContextMenu={(e) => handleItemContextMenu(e, file)}
                onTouchStart={(e) => handleItemTouchStart(e, file)}
                onTouchMove={handleItemTouchMove}
                onTouchEnd={handleItemTouchEnd}
                onTouchCancel={handleItemTouchEnd}
                className={`group relative flex flex-col justify-between p-3.5 rounded-2xl border transition-all duration-150 cursor-pointer select-none touch-manipulation ${
                  isSelected
                    ? 'bg-gradient-to-b from-teal-950/70 via-slate-900/90 to-slate-900 border-teal-400 ring-2 ring-teal-400/80 shadow-xl shadow-teal-500/20 scale-[1.01]'
                    : 'bg-slate-900/60 hover:bg-slate-800/80 border-slate-800/80 hover:border-slate-700/80 hover:shadow-lg hover:shadow-slate-950/30'
                }`}
                style={{ WebkitTouchCallout: 'none' }}
              >
                {/* Top accent line on selected */}
                {isSelected && (
                  <div className="absolute inset-x-4 top-0 h-0.5 bg-gradient-to-r from-transparent via-teal-400 to-transparent pointer-events-none rounded-t" />
                )}

                {/* Top Action Row */}
                <div className="flex items-center justify-between mb-2">
                  <div
                    onClick={(e) => {
                      e.stopPropagation();
                      toggleSelect(file.id, true);
                    }}
                    className={`w-5 h-5 rounded-lg border flex items-center justify-center transition-all duration-150 ${
                      isSelected
                        ? 'bg-gradient-to-tr from-teal-500 to-teal-400 border-teal-300 text-slate-950 shadow-md shadow-teal-500/40 scale-105'
                        : 'border-slate-600/70 bg-slate-950/50 hover:border-teal-400 opacity-30 sm:opacity-0 group-hover:opacity-100 hover:scale-105'
                    }`}
                    title={isSelected ? 'Deselect item' : 'Select item'}
                  >
                    {isSelected && <Check className="w-3.5 h-3.5 stroke-[3] text-slate-950" />}
                  </div>

                  <div className="flex items-center gap-1">
                    <button
                      onClick={(e) => {
                        e.stopPropagation();
                        toggleStar(file.path);
                      }}
                      className={`p-1 rounded hover:bg-slate-700/60 transition-colors ${
                        file.starred ? 'text-amber-400 opacity-100' : 'text-slate-500 opacity-0 group-hover:opacity-100'
                      }`}
                      title={file.starred ? 'Starred' : 'Star'}
                    >
                      <Star className={`w-3.5 h-3.5 ${file.starred ? 'fill-amber-400' : ''}`} />
                    </button>

                    <button
                      onClick={(e) => {
                        e.stopPropagation();
                        const rect = e.currentTarget.getBoundingClientRect();
                        setCanvasContextMenu(null);
                        if (contextMenu?.file.id === file.id) {
                          setContextMenu(null);
                        } else {
                          setContextMenu({
                            file,
                            position: { x: rect.right, y: rect.bottom + 4 },
                          });
                        }
                      }}
                      className="p-1 rounded text-slate-400 hover:text-slate-200 hover:bg-slate-700/60 opacity-0 group-hover:opacity-100 transition-all"
                      title="More actions"
                    >
                      <MoreVertical className="w-3.5 h-3.5" />
                    </button>
                  </div>
                </div>

                {/* Thumbnail / Icon Center */}
                <div className="h-20 flex items-center justify-center py-2 pointer-events-none">
                  {category === 'image' && file.content?.includes('<svg') ? (
                    <div className="w-16 h-16 rounded overflow-hidden flex items-center justify-center border border-slate-700/50 bg-slate-950 p-1">
                      <img
                        src={svgToSafeDataUrl(file.content)}
                        alt={file.name}
                        className="w-full h-full object-contain pointer-events-none"
                      />
                    </div>
                  ) : (
                    <FileItemIcon
                      name={file.name}
                      isDirectory={file.isDirectory}
                      mimeType={file.mimeType}
                      size="lg"
                    />
                  )}
                </div>

                {/* File Details Footer */}
                <div className="mt-2 min-w-0 pointer-events-none">
                  <div
                    className={`text-xs font-medium truncate transition-colors ${
                      isSelected ? 'text-teal-200 font-semibold' : 'text-slate-200 group-hover:text-teal-300'
                    }`}
                    title={file.name}
                  >
                    {file.name}
                  </div>
                  <div className="flex items-center justify-between text-[11px] text-slate-500 mt-1">
                    <span className={`font-mono tabular-nums ${isSelected ? 'text-teal-300/80 font-medium' : ''}`}>
                      {file.isDirectory ? 'Folder' : formatBytes(file.size)}
                    </span>
                    <span className={`font-mono ${isSelected ? 'text-teal-400/60' : ''}`}>{file.mode}</span>
                  </div>
                </div>
              </div>
            );
          })}
        </div>
      )}

      {/* Item Context Menu (Right Click / Long Press) */}
      {contextMenu && (
        <FileContextMenu
          file={contextMenu.file}
          position={contextMenu.position}
          onClose={() => setContextMenu(null)}
        />
      )}

      {/* Empty Area Context Menu */}
      {canvasContextMenu && (
        <CanvasContextMenu
          position={canvasContextMenu.position}
          onClose={() => setCanvasContextMenu(null)}
          onUploadFileClick={() => fileInputRef.current?.click()}
          onUploadFolderClick={() => folderInputRef.current?.click()}
        />
      )}

      {/* Hidden file pickers for context menu upload actions */}
      <input
        type="file"
        ref={fileInputRef}
        onChange={(e) => {
          if (e.target.files && e.target.files.length > 0) {
            onUploadDropped(e.target.files);
            e.target.value = '';
          }
        }}
        multiple
        className="hidden"
      />
      <input
        type="file"
        ref={folderInputRef}
        onChange={(e) => {
          if (e.target.files && e.target.files.length > 0) {
            onUploadDropped(e.target.files);
            e.target.value = '';
          }
        }}
        // @ts-ignore
        webkitdirectory=""
        directory=""
        className="hidden"
      />
    </div>
  );
};
