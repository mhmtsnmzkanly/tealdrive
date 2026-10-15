import React, { useState } from 'react';
import { useTealDrive } from '../../context/TealDriveContext.tsx';
import type { FileEntry } from '../../sdk/types.ts';
import { FileItemIcon } from './FileItemIcon.tsx';
import { FileContextMenu } from './FileContextMenu.tsx';
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
  } = useTealDrive();

  const [activeMenuId, setActiveMenuId] = useState<string | null>(null);

  const handleDoubleClick = (file: FileEntry) => {
    if (file.isDirectory) {
      goToPath(file.path);
    } else {
      setPreviewFile(file);
    }
  };

  const allSelected = files.length > 0 && selectedIds.size === files.length;

  return (
    <div className="p-6">
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
            {files.map((file) => {
              const isSelected = selectedIds.has(file.id);

              return (
                <tr
                  key={file.id}
                  onClick={(e) => {
                    e.stopPropagation();
                    toggleSelect(file.id, e.shiftKey || e.metaKey || e.ctrlKey);
                  }}
                  onDoubleClick={() => handleDoubleClick(file)}
                  className={`group transition-colors cursor-pointer select-none ${
                    isSelected ? 'bg-teal-950/30 text-teal-200' : 'hover:bg-slate-800/40 text-slate-300'
                  }`}
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

                  <td className="py-2.5 px-3 text-center relative">
                    <button
                      onClick={(e) => {
                        e.stopPropagation();
                        setActiveMenuId(activeMenuId === file.id ? null : file.id);
                      }}
                      className="p-1 rounded text-slate-400 hover:text-slate-200 hover:bg-slate-700 transition-colors"
                    >
                      <MoreVertical className="w-3.5 h-3.5" />
                    </button>

                    {activeMenuId === file.id && (
                      <FileContextMenu
                        file={file}
                        onClose={() => setActiveMenuId(null)}
                      />
                    )}
                  </td>
                </tr>
              );
            })}
          </tbody>
        </table>
      </div>
    </div>
  );
};
