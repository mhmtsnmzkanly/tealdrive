import React, { useState, useEffect, useRef } from 'react';
import { useTealDrive } from '../../context/TealDriveContext.tsx';
import { tealdrive } from '../../lib/tealdrive.ts';
import type { FileEntry } from '../../sdk/types.ts';
import { FileItemIcon } from '../explorer/FileItemIcon.tsx';
import { formatBytes, formatRelativeDate } from '../../utils/formatters.ts';
import { Search, X, CornerDownLeft, ArrowRight } from 'lucide-react';
import { t } from '../../i18n/index.ts';

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
  const [searchError, setSearchError] = useState<string | null>(null);
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    if (omnisearchOpen) {
      setTimeout(() => inputRef.current?.focus(), 50);
      setQuery('');
      setResults([]);
      setSelectedIndex(0);
    }
  }, [omnisearchOpen]);

  // Each keystroke cancels the previous search, so a slow reply can never replace a newer one.
  useEffect(() => {
    const term = query.trim();
    setSearchError(null);
    if (!term) {
      setResults([]);
      setSearching(false);
      return;
    }

    const controller = new AbortController();
    const timer = setTimeout(async () => {
      setSearching(true);
      try {
        const matches = await tealdrive.files.search({ query: term, signal: controller.signal });
        if (controller.signal.aborted) return;
        setResults(matches);
        setSelectedIndex(0);
      } catch (err: any) {
        if (controller.signal.aborted) return;
        setResults([]);
        setSearchError(err?.message || err?.code || t('Search failed'));
      } finally {
        if (!controller.signal.aborted) setSearching(false);
      }
    }, 250);

    return () => {
      clearTimeout(timer);
      controller.abort();
    };
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
            placeholder={t('Search file and folder names...')}
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
            {t('ESC')}
          </kbd>
        </div>

        {/* Results Area */}
        <div className="max-h-80 overflow-y-auto p-2">
          {searchError && (
            <div className="py-12 text-center text-xs text-rose-300">{t('Search failed: {error}', { error: searchError })}</div>
          )}

          {query && results.length === 0 && !searching && !searchError && (
            <div className="py-12 text-center text-xs text-slate-500">
              {t('No matching files found for "{query}"', { query })}
            </div>
          )}

          {!query && (
            <div className="py-8 px-4 text-center text-xs text-slate-500">
              {t('Type part of a file or folder name. Results come from your whole drive.')}
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
                          {t('Folder')}
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
            <span>{t('↑↓ Navigate')}</span>
            <span>{t('↵ Select')}</span>
            <span>{t('ESC Close')}</span>
          </div>
          <div>{t('TealDrive Search')}</div>
        </div>
      </div>
    </div>
  );
};
