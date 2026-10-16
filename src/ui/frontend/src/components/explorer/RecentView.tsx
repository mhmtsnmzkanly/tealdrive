import React, { useState, useRef } from 'react';
import { useTealDrive } from '../../context/TealDriveContext.tsx';
import type { FileEntry } from '../../sdk/types.ts';
import { FileItemIcon } from './FileItemIcon.tsx';
import { FileContextMenu } from './FileContextMenu.tsx';
import { formatBytes, formatRelativeDate } from '../../utils/formatters.ts';
import { Clock, Download, Star, MoreVertical } from 'lucide-react';
import { t } from '../../i18n/index.ts';

export const RecentView: React.FC = () => {
  const { recentEntries, setPreviewFile, downloadFile, toggleStar } = useTealDrive();

  const [contextMenu, setContextMenu] = useState<{
    file: FileEntry;
    position: { x: number; y: number };
  } | null>(null);

  const longPressTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const touchStartRef = useRef<{ x: number; y: number }>({ x: 0, y: 0 });
  const longPressTriggeredRef = useRef(false);

  const handleItemContextMenu = (e: React.MouseEvent, file: FileEntry) => {
    e.preventDefault();
    e.stopPropagation();
    setContextMenu({
      file,
      position: { x: e.clientX, y: e.clientY },
    });
  };

  const handleTouchStart = (e: React.TouchEvent, file: FileEntry) => {
    if (e.touches.length !== 1) return;
    const touch = e.touches[0];
    touchStartRef.current = { x: touch.clientX, y: touch.clientY };
    longPressTriggeredRef.current = false;

    if (longPressTimerRef.current) clearTimeout(longPressTimerRef.current);
    longPressTimerRef.current = setTimeout(() => {
      longPressTriggeredRef.current = true;
      try {
        if (typeof navigator !== 'undefined' && 'vibrate' in navigator) {
          navigator.vibrate?.(40);
        }
      } catch {}
      setContextMenu({
        file,
        position: { x: touchStartRef.current.x, y: touchStartRef.current.y },
      });
    }, 450);
  };

  const handleTouchMove = (e: React.TouchEvent) => {
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
      clearTimeout(longPressTimerRef.current);
      longPressTimerRef.current = null;
    }
  };

  const handleTouchEnd = (e: React.TouchEvent) => {
    if (longPressTimerRef.current) {
      clearTimeout(longPressTimerRef.current);
      longPressTimerRef.current = null;
    }
    if (longPressTriggeredRef.current) {
      if (e.cancelable) e.preventDefault();
      longPressTriggeredRef.current = false;
    }
  };

  return (
    <div className="p-6 select-none">
      <div className="flex items-center justify-between mb-4">
        <div>
          <h2 className="text-base font-semibold text-slate-100 flex items-center gap-2">
            <Clock className="w-4 h-4 text-sky-400" />
            <span>{t('Recently Modified Files')}</span>
          </h2>
          <p className="text-xs text-slate-400 mt-0.5">
            {t('Files you recently uploaded, edited, renamed, moved or copied')}
          </p>
        </div>
      </div>

      {recentEntries.length === 0 ? (
        <div className="p-12 text-center border border-dashed border-slate-800 rounded-xl bg-slate-900/30">
          <Clock className="w-8 h-8 text-slate-600 mx-auto mb-2" />
          <div className="text-sm font-medium text-slate-300">{t('No recent activity')}</div>
          <div className="text-xs text-slate-500 mt-1">{t('Edited or uploaded files will show up here.')}</div>
        </div>
      ) : (
        <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-3">
          {recentEntries.map((file) => (
            <div
              key={file.id}
              onClick={() => setPreviewFile(file)}
              onContextMenu={(e) => handleItemContextMenu(e, file)}
              onTouchStart={(e) => handleTouchStart(e, file)}
              onTouchMove={handleTouchMove}
              onTouchEnd={handleTouchEnd}
              onTouchCancel={handleTouchEnd}
              className="p-3.5 bg-slate-900/60 hover:bg-slate-800/80 border border-slate-800 hover:border-slate-700 rounded-xl transition-all cursor-pointer flex items-center justify-between group touch-manipulation"
              style={{ WebkitTouchCallout: 'none' }}
            >
              <div className="flex items-center gap-3 min-w-0 pointer-events-none">
                <FileItemIcon
                  name={file.name}
                  isDirectory={file.isDirectory}
                  mimeType={file.mimeType}
                  size="md"
                />
                <div className="min-w-0">
                  <div className="text-xs font-medium text-slate-200 truncate group-hover:text-teal-300 transition-colors">
                    {file.name}
                  </div>
                  <div className="text-[11px] text-slate-500 flex items-center gap-2 mt-0.5 font-mono tabular-nums">
                    <span>{formatBytes(file.size)}</span>
                    <span>·</span>
                    <span>{formatRelativeDate(file.modified)}</span>
                  </div>
                </div>
              </div>

              <div className="flex items-center gap-1 opacity-0 group-hover:opacity-100 transition-opacity">
                <button
                  onClick={(e) => {
                    e.stopPropagation();
                    toggleStar(file.path);
                  }}
                  className="p-1.5 rounded hover:bg-slate-700 text-slate-400 hover:text-amber-400"
                  title={t('Star file')}
                >
                  <Star className={`w-3.5 h-3.5 ${file.starred ? 'text-amber-400 fill-amber-400' : ''}`} />
                </button>
                <button
                  onClick={(e) => {
                    e.stopPropagation();
                    downloadFile(file.path);
                  }}
                  className="p-1.5 rounded hover:bg-slate-700 text-slate-400 hover:text-teal-300"
                  title={t('Download file')}
                >
                  <Download className="w-3.5 h-3.5" />
                </button>
                <button
                  onClick={(e) => {
                    e.stopPropagation();
                    const rect = e.currentTarget.getBoundingClientRect();
                    setContextMenu({
                      file,
                      position: { x: rect.right, y: rect.bottom + 4 },
                    });
                  }}
                  className="p-1.5 rounded hover:bg-slate-700 text-slate-400 hover:text-slate-200"
                  title={t('More actions')}
                >
                  <MoreVertical className="w-3.5 h-3.5" />
                </button>
              </div>
            </div>
          ))}
        </div>
      )}

      {contextMenu && (
        <FileContextMenu
          file={contextMenu.file}
          position={contextMenu.position}
          onClose={() => setContextMenu(null)}
        />
      )}
    </div>
  );
};
