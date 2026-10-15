import React, { useState } from 'react';
import { useTealDrive } from '../../context/TealDriveContext.tsx';
import type { FileEntry } from '../../sdk/types.ts';
import { FileItemIcon } from './FileItemIcon.tsx';
import { FileContextMenu } from './FileContextMenu.tsx';
import { formatBytes, formatRelativeDate, getFileCategory } from '../../utils/formatters.ts';
import { svgToSafeDataUrl } from '../../utils/safeSvg.ts';
import { Star, MoreVertical, UploadCloud } from 'lucide-react';

interface FileGridProps {
  files: FileEntry[];
  onUploadDropped: (files: FileList) => void;
}

export const FileGrid: React.FC<FileGridProps> = ({ files, onUploadDropped }) => {
  const {
    selectedIds,
    toggleSelect,
    goToPath,
    setPreviewFile,
    currentPath,
    toggleStar,
    setNewFolderOpen,
  } = useTealDrive();

  const [activeMenuId, setActiveMenuId] = useState<string | null>(null);
  const [dragOver, setDragOver] = useState(false);

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

  return (
    <div
      onDragOver={handleDragOver}
      onDragLeave={handleDragLeave}
      onDrop={handleDrop}
      className="relative min-h-[480px] p-6 focus:outline-none"
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
        <div className="flex flex-col items-center justify-center py-20 text-center">
          <div className="w-16 h-16 rounded-2xl bg-slate-900 border border-slate-800 flex items-center justify-center text-slate-500 mb-4">
            <UploadCloud className="w-8 h-8 text-teal-500/60" />
          </div>
          <h3 className="text-sm font-semibold text-slate-200">This directory is empty</h3>
          <p className="text-xs text-slate-400 max-w-sm mt-1">
            Drag and drop files here, or create a new folder or document to get started.
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
                  toggleSelect(file.id, e.shiftKey || e.metaKey || e.ctrlKey);
                }}
                onDoubleClick={() => handleDoubleClick(file)}
                className={`group relative flex flex-col justify-between p-3 rounded-xl border transition-all cursor-pointer select-none ${
                  isSelected
                    ? 'bg-teal-950/40 border-teal-500 shadow-md shadow-teal-500/10'
                    : 'bg-slate-900/60 hover:bg-slate-800/80 border-slate-800/80 hover:border-slate-700'
                }`}
              >
                {/* Top Action Row */}
                <div className="flex items-center justify-between mb-2">
                  <div
                    onClick={(e) => {
                      e.stopPropagation();
                      toggleSelect(file.id, true);
                    }}
                    className={`w-4 h-4 rounded border flex items-center justify-center transition-all ${
                      isSelected
                        ? 'bg-teal-500 border-teal-400 text-slate-950'
                        : 'border-slate-700 group-hover:border-slate-500 opacity-0 group-hover:opacity-100'
                    }`}
                  >
                    {isSelected && <span className="text-[10px] font-bold">✓</span>}
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

                    <div className="relative">
                      <button
                        onClick={(e) => {
                          e.stopPropagation();
                          setActiveMenuId(activeMenuId === file.id ? null : file.id);
                        }}
                        className="p-1 rounded text-slate-400 hover:text-slate-200 hover:bg-slate-700/60 opacity-0 group-hover:opacity-100 transition-all"
                        title="More actions"
                      >
                        <MoreVertical className="w-3.5 h-3.5" />
                      </button>

                      {activeMenuId === file.id && (
                        <FileContextMenu
                          file={file}
                          onClose={() => setActiveMenuId(null)}
                        />
                      )}
                    </div>
                  </div>
                </div>

                {/* Thumbnail / Icon Center */}
                <div className="h-20 flex items-center justify-center py-2">
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
                <div className="mt-2 min-w-0">
                  <div
                    className="text-xs font-medium text-slate-200 truncate group-hover:text-teal-300 transition-colors"
                    title={file.name}
                  >
                    {file.name}
                  </div>
                  <div className="flex items-center justify-between text-[11px] text-slate-500 mt-1">
                    <span className="font-mono tabular-nums">
                      {file.isDirectory ? 'Folder' : formatBytes(file.size)}
                    </span>
                    <span className="font-mono">{file.mode}</span>
                  </div>
                </div>
              </div>
            );
          })}
        </div>
      )}
    </div>
  );
};
