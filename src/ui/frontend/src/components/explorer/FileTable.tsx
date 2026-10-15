import React, { useState, useRef } from 'react';
import { useTealDrive } from '../../context/TealDriveContext.tsx';
import type { FileEntry } from '../../sdk/types.ts';
import { FileItemIcon } from './FileItemIcon.tsx';
import { FileContextMenu } from './FileContextMenu.tsx';
import { CanvasContextMenu } from './CanvasContextMenu.tsx';
import { formatBytes, formatRelativeDate } from '../../utils/formatters.ts';
import { modeToSymbolic } from '../../sdk/permissions.ts';
import { Star, MoreVertical, ArrowUpDown, Key } from 'lucide-react';

interface FileTableProps {
  files: FileEntry[];
}

export const FileTable: React.FC<FileTableProps> = ({ files }) => {
  const {
    selectedIds,
    toggleSelect,
    selectAll,
    clearSelection,
    goToPath,
    setPreviewFile,
    setChmodFile,
    toggleStar,
    sortField,
    setSortField,
    toggleSortOrder,
    uploadFiles,
  } = useTealDrive();

  const [contextMenu, setContextMenu] = useState<{
    file: FileEntry;
    position: { x: number; y: number };
  } | null>(null);

  const [canvasContextMenu, setCanvasContextMenu] = useState<{
    position: { x: number; y: number };
  } | null>(null);

  const fileInputRef = useRef<HTMLInputElement>(null);
  const folderInputRef = useRef<HTMLInputElement>(null);
  const longPressTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const touchStartRef = useRef<{ x: number; y: number }>({ x: 0, y: 0 });
  const touchMovedRef = useRef(false);
  const longPressTriggeredRef = useRef(false);

  const handleDoubleClick = (file: FileEntry) => {
    if (file.isDirectory) {
      goToPath(file.path);
    } else {
      setPreviewFile(file);
    }
  };

  // Right-click on file row
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

  // Touch Long-Press on file row
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

  const allSelected = files.length > 0 && selectedIds.size === files.length;

  return (
    <div
      onContextMenu={handleCanvasContextMenu}
      onTouchStart={handleCanvasTouchStart}
      onTouchMove={handleItemTouchMove}
      onTouchEnd={handleItemTouchEnd}
      onTouchCancel={handleItemTouchEnd}
      className="p-6 select-none canvas-empty-area min-h-[480px]"
    >
      <div className="overflow-x-auto rounded-xl border border-slate-800 bg-slate-900/40">
        <table className="w-full text-left border-collapse text-xs">
          <thead>
            <tr className="border-b border-slate-800 bg-slate-900/80 text-slate-400 font-medium select-none">
              <th className="py-3 px-4 w-10">
                <input
                  type="checkbox"
                  checked={allSelected}
                  onChange={(e) => (e.target.checked ? selectAll() : clearSelection())}
                  className="rounded border-slate-700 bg-slate-800 text-teal-500 focus:ring-teal-500/20"
                />
              </th>
              <th
                onClick={() => {
                  if (sortField === 'name') toggleSortOrder();
                  else setSortField('name');
                }}
                className="py-3 px-3 cursor-pointer hover:text-slate-200 transition-colors"
              >
                <div className="flex items-center gap-1.5">
                  <span>Name</span>
                  {sortField === 'name' && <ArrowUpDown className="w-3 h-3 text-teal-400" />}
                </div>
              </th>
              <th
                onClick={() => {
                  if (sortField === 'size') toggleSortOrder();
                  else setSortField('size');
                }}
                className="py-3 px-4 w-28 cursor-pointer hover:text-slate-200 transition-colors text-right"
              >
                <div className="flex items-center justify-end gap-1.5">
                  <span>Size</span>
                  {sortField === 'size' && <ArrowUpDown className="w-3 h-3 text-teal-400" />}
                </div>
              </th>
              <th
                onClick={() => {
                  if (sortField === 'modified') toggleSortOrder();
                  else setSortField('modified');
                }}
                className="py-3 px-4 w-36 cursor-pointer hover:text-slate-200 transition-colors"
              >
                <div className="flex items-center gap-1.5">
                  <span>Modified</span>
                  {sortField === 'modified' && <ArrowUpDown className="w-3 h-3 text-teal-400" />}
                </div>
              </th>
              <th className="py-3 px-4 w-36 font-mono">Permissions</th>
              <th className="py-3 px-4 w-28">Owner/Group</th>
              <th className="py-3 px-3 w-12 text-center"></th>
            </tr>
          </thead>
          <tbody className="divide-y divide-slate-800/60">
            {files.length === 0 ? (
              <tr>
                <td colSpan={7} className="py-12 text-center text-slate-500">
                  This directory is empty. Right-click or long-press to open options.
                </td>
              </tr>
            ) : (
              files.map((file) => {
                const isSelected = selectedIds.has(file.id);

                return (
                  <tr
                    key={file.id}
                    onClick={(e) => {
                      e.stopPropagation();
                      toggleSelect(file.id, e.shiftKey || e.metaKey || e.ctrlKey);
                    }}
                    onDoubleClick={() => handleDoubleClick(file)}
                    onContextMenu={(e) => handleItemContextMenu(e, file)}
                    onTouchStart={(e) => handleItemTouchStart(e, file)}
                    onTouchMove={handleItemTouchMove}
                    onTouchEnd={handleItemTouchEnd}
                    onTouchCancel={handleItemTouchEnd}
                    className={`group transition-colors cursor-pointer select-none touch-manipulation ${
                      isSelected ? 'bg-teal-950/30 text-teal-200' : 'hover:bg-slate-800/40 text-slate-300'
                    }`}
                    style={{ WebkitTouchCallout: 'none' }}
                  >
                    <td className="py-2.5 px-4">
                      <input
                        type="checkbox"
                        checked={isSelected}
                        onChange={() => toggleSelect(file.id, true)}
                        onClick={(e) => e.stopPropagation()}
                        className="rounded border-slate-700 bg-slate-800 text-teal-500 focus:ring-teal-500/20"
                      />
                    </td>

                    <td className="py-2.5 px-3">
                      <div className="flex items-center gap-2.5 min-w-0">
                        <FileItemIcon
                          name={file.name}
                          isDirectory={file.isDirectory}
                          mimeType={file.mimeType}
                          size="sm"
                        />
                        <span className="font-medium text-slate-200 truncate group-hover:text-teal-300 transition-colors">
                          {file.name}
                        </span>
                        {file.starred && (
                          <Star className="w-3 h-3 text-amber-400 fill-amber-400 shrink-0" />
                        )}
                      </div>
                    </td>

                    <td className="py-2.5 px-4 text-right font-mono tabular-nums text-slate-400">
                      {file.isDirectory ? '—' : formatBytes(file.size)}
                    </td>

                    <td className="py-2.5 px-4 text-slate-400 whitespace-nowrap">
                      {formatRelativeDate(file.modified)}
                    </td>

                    <td className="py-2.5 px-4 font-mono text-slate-400">
                      <button
                        onClick={(e) => {
                          e.stopPropagation();
                          setChmodFile(file);
                        }}
                        title="Edit UNIX Permissions"
                        className="inline-flex items-center gap-1.5 px-2 py-0.5 rounded bg-slate-800/80 hover:bg-slate-700 border border-slate-700/60 text-[11px] text-teal-300 transition-colors"
                      >
                        <Key className="w-3 h-3 text-amber-400" />
                        <span>{file.mode}</span>
                        <span className="text-slate-500">({modeToSymbolic(file.mode, file.isDirectory)})</span>
                      </button>
                    </td>

                    <td className="py-2.5 px-4 font-mono text-[11px] text-slate-400">
                      {file.owner}:{file.group}
                    </td>

                    <td className="py-2.5 px-3 text-center">
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
                        className="p-1 rounded text-slate-400 hover:text-slate-200 hover:bg-slate-700 transition-colors"
                        title="More actions"
                      >
                        <MoreVertical className="w-3.5 h-3.5" />
                      </button>
                    </td>
                  </tr>
                );
              })
            )}
          </tbody>
        </table>
      </div>

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
            uploadFiles(e.target.files);
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
            uploadFiles(e.target.files);
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
