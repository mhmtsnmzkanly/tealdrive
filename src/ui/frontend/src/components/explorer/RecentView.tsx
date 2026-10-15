import React from 'react';
import { useTealDrive } from '../../context/TealDriveContext.tsx';
import { FileItemIcon } from './FileItemIcon.tsx';
import { formatBytes, formatRelativeDate } from '../../utils/formatters.ts';
import { Clock, Eye, Download, Star } from 'lucide-react';

export const RecentView: React.FC = () => {
  const { recentEntries, setPreviewFile, downloadFile, toggleStar } = useTealDrive();

  return (
    <div className="p-6">
      <div className="flex items-center justify-between mb-4">
        <div>
          <h2 className="text-base font-semibold text-slate-100 flex items-center gap-2">
            <Clock className="w-4 h-4 text-sky-400" />
            <span>Recently Modified Files</span>
          </h2>
          <p className="text-xs text-slate-400 mt-0.5">
            Files accessed or written in the last 7 days across the storage pool
          </p>
        </div>
      </div>

      {recentEntries.length === 0 ? (
        <div className="p-12 text-center border border-dashed border-slate-800 rounded-xl bg-slate-900/30">
          <Clock className="w-8 h-8 text-slate-600 mx-auto mb-2" />
          <div className="text-sm font-medium text-slate-300">No recent activity</div>
          <div className="text-xs text-slate-500 mt-1">Edited or uploaded files will show up here.</div>
        </div>
      ) : (
        <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-3">
          {recentEntries.map((file) => (
            <div
              key={file.id}
              onClick={() => setPreviewFile(file)}
              className="p-3.5 bg-slate-900/60 hover:bg-slate-800/80 border border-slate-800 hover:border-slate-700 rounded-xl transition-all cursor-pointer flex items-center justify-between group"
            >
              <div className="flex items-center gap-3 min-w-0">
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
                  title="Star file"
                >
                  <Star className={`w-3.5 h-3.5 ${file.starred ? 'text-amber-400 fill-amber-400' : ''}`} />
                </button>
                <button
                  onClick={(e) => {
                    e.stopPropagation();
                    downloadFile(file.path);
                  }}
                  className="p-1.5 rounded hover:bg-slate-700 text-slate-400 hover:text-teal-300"
                  title="Download file"
                >
                  <Download className="w-3.5 h-3.5" />
                </button>
              </div>
            </div>
          ))}
        </div>
      )}
    </div>
  );
};
