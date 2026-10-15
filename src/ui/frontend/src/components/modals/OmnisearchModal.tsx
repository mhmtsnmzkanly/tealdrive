import React, { useState, useEffect, useRef } from 'react';
import { useTealDrive } from '../../context/TealDriveContext.tsx';
import { tealdrive } from '../../lib/tealdrive.ts';
import type { FileEntry } from '../../sdk/types.ts';
import { FileItemIcon } from '../explorer/FileItemIcon.tsx';
import { formatBytes, formatRelativeDate } from '../../utils/formatters.ts';
import { Search, X, CornerDownLeft, ArrowRight } from 'lucide-react';

export const OmnisearchModal: React.FC = () => {
  const {
    omnisearchOpen,
    setOmnisearchOpen,
    goToPath,
    setPreviewFile,
  } = useTealDrive();

  const [query, setQuery] = useState('');
  const [results, setResults] = useState<FileEntry[]>([]);
  const [selectedIndex, setSelectedIndex] = useState(0);
  const [searching, setSearching] = useState(false);
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    if (omnisearchOpen) {
      setTimeout(() => inputRef.current?.focus(), 50);
      setQuery('');
      setResults([]);
      setSelectedIndex(0);
    }
  }, [omnisearchOpen]);

  useEffect(() => {
    if (!query.trim()) {
      setResults([]);
      return;
    }

    const timer = setTimeout(async () => {
      setSearching(true);
      try {
        const matches = await tealdrive.files.search({ query: query.trim() });
        setResults(matches);
        setSelectedIndex(0);
      } finally {
        setSearching(false);
      }
    }, 60);

    return () => clearTimeout(timer);
  }, [query]);

  if (!omnisearchOpen) return null;

  const handleSelect = (file: FileEntry) => {
    setOmnisearchOpen(false);
    if (file.isDirectory) {
      goToPath(file.path);
    } else {
      setPreviewFile(file);
    }
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'ArrowDown') {
      e.preventDefault();
      setSelectedIndex((prev) => (results.length > 0 ? (prev + 1) % results.length : 0));
    } else if (e.key === 'ArrowUp') {
      e.preventDefault();
      setSelectedIndex((prev) => (results.length > 0 ? (prev - 1 + results.length) % results.length : 0));
    } else if (e.key === 'Enter') {
      e.preventDefault();
      if (results[selectedIndex]) {
        handleSelect(results[selectedIndex]);
      }
    } else if (e.key === 'Escape') {
      setOmnisearchOpen(false);
    }
  };

  return (
    <div
      onClick={() => setOmnisearchOpen(false)}
      className="fixed inset-0 z-50 flex items-start justify-center pt-24 px-4 bg-slate-950/80 backdrop-blur-md animate-in fade-in duration-100"
    >
      <div
        onClick={(e) => e.stopPropagation()}
        className="w-full max-w-xl bg-slate-900 border border-slate-700/80 rounded-2xl shadow-2xl overflow-hidden flex flex-col text-slate-100 animate-in zoom-in-95 duration-100"
      >
        {/* Search Input Bar */}
        <div className="h-14 px-4 border-b border-slate-800 flex items-center gap-3 bg-slate-950/60">
          <Search className="w-5 h-5 text-teal-400 shrink-0" />
          <input
            ref={inputRef}
            type="text"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            onKeyDown={handleKeyDown}
            placeholder="Search files, folders, code, or contents..."
            className="w-full bg-transparent text-sm text-slate-100 focus:outline-none placeholder:text-slate-500"
          />
          {query && (
            <button
              onClick={() => setQuery('')}
              className="p-1 rounded text-slate-400 hover:text-slate-200"
            >
              <X className="w-4 h-4" />
            </button>
          )}
          <kbd className="px-1.5 py-0.5 text-[10px] font-mono text-slate-400 bg-slate-800 border border-slate-700 rounded">
            ESC
          </kbd>
        </div>

        {/* Results Area */}
        <div className="max-h-80 overflow-y-auto p-2">
          {query && results.length === 0 && !searching && (
            <div className="py-12 text-center text-xs text-slate-500">
              No matching files found for &quot;<span className="text-slate-300">{query}</span>&quot;
            </div>
          )}

          {!query && (
            <div className="py-8 px-4 text-center">
              <div className="text-xs text-slate-400 font-medium">Quick suggestions:</div>
              <div className="flex flex-wrap items-center justify-center gap-2 mt-2">
                {['README.md', 'main.rs', 'Reports', 'Configs', 'tealdrive.toml'].map((s) => (
                  <button
                    key={s}
                    onClick={() => setQuery(s)}
                    className="px-2.5 py-1 rounded-lg bg-slate-800 hover:bg-slate-700 text-teal-300 text-xs font-mono transition-colors"
                  >
                    {s}
                  </button>
                ))}
              </div>
            </div>
          )}

          {results.map((file, index) => {
            const isSelected = index === selectedIndex;

            return (
              <div
                key={file.id}
                onClick={() => handleSelect(file)}
                onMouseEnter={() => setSelectedIndex(index)}
                className={`flex items-center justify-between p-2.5 rounded-xl cursor-pointer transition-colors text-xs ${
                  isSelected ? 'bg-teal-500/15 text-teal-200 border border-teal-500/30' : 'hover:bg-slate-800/60 text-slate-300'
                }`}
              >
                <div className="flex items-center gap-3 min-w-0">
                  <FileItemIcon
                    name={file.name}
                    isDirectory={file.isDirectory}
                    mimeType={file.mimeType}
                    size="sm"
                  />
                  <div className="min-w-0">
                    <div className="font-medium text-slate-200 truncate flex items-center gap-2">
                      <span>{file.name}</span>
                      {file.isDirectory && (
                        <span className="text-[10px] font-mono px-1.5 py-0.2 rounded bg-slate-800 text-slate-400">
                          Folder
                        </span>
                      )}
                    </div>
                    <div className="text-[11px] font-mono text-slate-500 truncate">
                      {file.path}
                    </div>
                  </div>
                </div>

                <div className="flex items-center gap-3 shrink-0 text-[11px] text-slate-500">
                  <span className="font-mono tabular-nums">
                    {file.isDirectory ? '—' : formatBytes(file.size)}
                  </span>
                  {isSelected && (
                    <CornerDownLeft className="w-3.5 h-3.5 text-teal-400" />
                  )}
                </div>
              </div>
            );
          })}
        </div>

        {/* Footer */}
        <div className="h-8 px-4 bg-slate-950 border-t border-slate-800 flex items-center justify-between text-[11px] text-slate-500 font-mono">
          <div className="flex items-center gap-3">
            <span>↑↓ Navigate</span>
            <span>↵ Select</span>
            <span>ESC Close</span>
          </div>
          <div>TealDrive Omnisearch v2.4</div>
        </div>
      </div>
    </div>
  );
};
