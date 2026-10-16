import React, { useState, useEffect, useRef } from 'react';
import { useTealDrive } from '../../context/TealDriveContext.tsx';
import { tealdrive } from '../../lib/tealdrive.ts';
import { getMimeType } from '../../sdk/client.ts';
import type { FileEntry } from '../../sdk/types.ts';
import {
  X,
  Save,
  AlertTriangle,
  RotateCcw,
  Check,
  FileCode,
  ShieldCheck,
  Loader2,
} from 'lucide-react';
import { t } from '../../i18n/index.ts';

interface CodeEditorModalProps {
  file: FileEntry;
  onClose: () => void;
}

export const CodeEditorModal: React.FC<CodeEditorModalProps> = ({ file, onClose }) => {
  const { addToast, refreshCurrentDirectory } = useTealDrive();
  const [fileName, setFileName] = useState<string>(file.name);
  const [content, setContent] = useState<string>(file.content || '');
  const [revision, setRevision] = useState<string | number | null>(file.revision === 0 ? 0 : null);
  // The last content known to match the server; edits beyond it are unsaved.
  const [savedContent, setSavedContent] = useState<string>(file.content || '');
  const [currentPath, setCurrentPath] = useState<string>(file.path);
  const [loading, setLoading] = useState(false);
  const [saving, setSaving] = useState(false);
  const [conflictError, setConflictError] = useState<{ message: string } | null>(null);
  const [savedSuccess, setSavedSuccess] = useState(false);

  const gutterRef = useRef<HTMLDivElement>(null);
  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const nameInputRef = useRef<HTMLInputElement>(null);

  const handleScroll = () => {
    if (gutterRef.current && textareaRef.current) {
      gutterRef.current.scrollTop = textareaRef.current.scrollTop;
    }
  };

  // If new file, auto-focus and select file name for easy renaming
  useEffect(() => {
    if (file.revision === 0 && nameInputRef.current) {
      nameInputRef.current.focus();
      nameInputRef.current.select();
    }
  }, [file.revision]);

  // Load current file content & revision
  useEffect(() => {
    if (file.revision === 0) {
      setContent(file.content || '');
      setRevision(0);
      return;
    }

    const load = async () => {
      setLoading(true);
      try {
        const res = await tealdrive.files.readText({ path: currentPath });
        setContent(res.content ?? '');
        setSavedContent(res.content ?? '');
        setRevision(res.revision ?? null);
      } catch (err: any) {
        addToast({ title: t('Error reading file'), desc: err.message, type: 'error' });
      } finally {
        setLoading(false);
      }
    };
    load();
  }, [currentPath, file.revision]);

  const handleSave = async () => {
    const trimmedName = fileName.trim();
    if (!trimmedName) {
      addToast({ title: t('Invalid filename'), desc: t('Filename cannot be empty'), type: 'error' });
      return;
    }
    if (/[\/\\\x00-\x1f\x7f]/.test(trimmedName)) {
      addToast({
        title: t('Invalid filename'),
        desc: t('Filename cannot contain slashes or control characters'),
        type: 'error',
      });
      return;
    }

    setSaving(true);
    setConflictError(null);

    try {
      // If it's a brand new document not yet written. "missing" makes the server
      // refuse rather than overwrite a file that already has this name.
      if (file.revision === 0) {
        const parentDir = file.path.substring(0, file.path.lastIndexOf('/')) || '/';
        const newPath = parentDir === '/' ? `/${trimmedName}` : `${parentDir}/${trimmedName}`;
        await tealdrive.files.writeText({ path: newPath, content, expectedRevision: 'missing' });
        addToast({ title: t('File created'), desc: trimmedName, type: 'success' });
        await refreshCurrentDirectory();
        onClose();
        return;
      }

      // If filename was changed for an existing file
      let targetPath = currentPath;
      if (trimmedName !== file.name) {
        await tealdrive.files.rename({
          path: currentPath,
          newName: trimmedName,
        });
        const parentDir = currentPath.substring(0, currentPath.lastIndexOf('/')) || '';
        targetPath = parentDir === '' ? `/${trimmedName}` : `${parentDir}/${trimmedName}`;
        file.name = trimmedName;
        file.path = targetPath;
        setCurrentPath(targetPath);
      }

      // Optimistic lock check with expectedRevision
      const updated = await tealdrive.files.writeText({
        path: targetPath,
        content,
        expectedRevision: revision,
      });

      setSavedContent(content);
      setRevision(updated.revision);
      if (updated.revision === null) {
        // Someone wrote after our save; the next save must not overwrite them blindly.
        setConflictError({
          message: t('The file changed on the server right after your save. Reload it before saving again.'),
        });
      }
      setSavedSuccess(true);
      setTimeout(() => setSavedSuccess(false), 2000);
      await refreshCurrentDirectory();
      addToast({
        title: t('File saved'),
        desc: t('Saved successfully ({trimmedName})', { trimmedName }),
        type: 'success',
      });
    } catch (err: any) {
      if (err.code === 'REVISION_CONFLICT') {
        setConflictError({
          message: t('Another client changed this file since you opened it. Your changes have not been saved.'),
        });
      } else if (err.code === 'ALREADY_EXISTS' || err.code === 'CONFLICT') {
        addToast({ title: t('Save failed'), desc: t('"{trimmedName}" already exists in this folder.', { trimmedName }), type: 'error' });
      } else {
        addToast({ title: t('Save failed'), desc: err.message, type: 'error' });
      }
    } finally {
      setSaving(false);
    }
  };

  const handleSaveRef = useRef(handleSave);
  handleSaveRef.current = handleSave;

  const dirty = content !== savedContent || (file.revision === 0 && fileName.trim() !== file.name);
  const requestClose = () => {
    if (dirty && !window.confirm(t('Discard unsaved changes?'))) return;
    onClose();
  };
  const requestCloseRef = useRef(requestClose);
  requestCloseRef.current = requestClose;

  // Keyboard shortcut Cmd+S / Ctrl+S
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === 's') {
        e.preventDefault();
        handleSaveRef.current();
      }
      if (e.key === 'Escape') {
        requestCloseRef.current();
      }
    };
    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, []);

  // Closing the tab must not silently drop edits either.
  useEffect(() => {
    if (!dirty) return;
    const warn = (e: BeforeUnloadEvent) => {
      e.preventDefault();
      e.returnValue = '';
    };
    window.addEventListener('beforeunload', warn);
    return () => window.removeEventListener('beforeunload', warn);
  }, [dirty]);

  const lines = content.split('\n');
  const parentDirPath = currentPath.substring(0, currentPath.lastIndexOf('/')) || '/';
  const fileExt = fileName.trim().split('.').pop()?.toLowerCase() || '';
  const currentMimeType = getMimeType(fileExt);

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center p-4 bg-slate-950/85 backdrop-blur-md animate-in fade-in duration-150">
      <div className="w-full max-w-4xl h-[85vh] bg-slate-900 border border-slate-700/80 rounded-2xl shadow-2xl flex flex-col overflow-hidden text-slate-100">
        {/* Editor Top Bar */}
        <div className="h-14 px-5 bg-slate-950/70 border-b border-slate-800 flex items-center justify-between shrink-0">
          <div className="flex items-center gap-3 min-w-0">
            <FileCode className="w-5 h-5 text-teal-400 shrink-0" />
            <div className="min-w-0">
              <div className="flex items-center gap-2">
                <input
                  ref={nameInputRef}
                  type="text"
                  value={fileName}
                  onChange={(e) => setFileName(e.target.value)}
                  onKeyDown={(e) => {
                    if (e.key === 'Enter') {
                      e.preventDefault();
                      textareaRef.current?.focus();
                    }
                  }}
                  placeholder={t('filename.txt')}
                  aria-label={t('Filename')}
                  className="bg-slate-800/80 hover:bg-slate-800 focus:bg-slate-950 border border-slate-700 focus:border-teal-400 rounded px-2 py-0.5 text-xs font-semibold text-slate-100 font-mono focus:outline-none transition-all w-48 sm:w-64 selection:bg-teal-500/40"
                  title={t('Click to rename')}
                />
                <span className="font-mono text-[10px] px-1.5 py-0.5 rounded bg-slate-800 text-teal-300 border border-slate-700 shrink-0">
                  {file.revision === 0
                    ? t('New')
                    : typeof revision === 'string'
                    ? t('Rev #{revision}', { revision: revision.slice(0, 8) })
                    : t('Rev unknown')}
                </span>
              </div>
              <div className="text-[10px] font-mono text-slate-500 truncate mt-0.5">
                {parentDirPath === '/' ? `/${fileName.trim() || 'file'}` : `${parentDirPath}/${fileName.trim() || 'file'}`}
              </div>
            </div>
          </div>

          <div className="flex items-center gap-2">
            {/* Save Button */}
            <button
              onClick={handleSave}
              disabled={saving}
              className={`flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-xs font-semibold transition-all shadow ${
                savedSuccess
                  ? 'bg-emerald-600 text-white'
                  : 'bg-teal-600 hover:bg-teal-500 text-slate-950 active:scale-95'
              }`}
            >
              {savedSuccess ? (
                <>
                  <Check className="w-3.5 h-3.5" />
                  <span>{t('Saved')}</span>
                </>
              ) : (
                <>
                  <Save className="w-3.5 h-3.5" />
                  <span>{saving ? 'Saving...' : t('Save (⌘S)')}</span>
                </>
              )}
            </button>

            <button
              onClick={requestClose}
              className="p-1.5 rounded-lg text-slate-400 hover:text-slate-200 hover:bg-slate-800 transition-colors"
            >
              <X className="w-4 h-4" />
            </button>
          </div>
        </div>

        {/* Revision Conflict Banner */}
        {conflictError && (
          <div className="p-3 bg-rose-950/90 border-b border-rose-800 flex items-center justify-between text-xs text-rose-200 px-5 animate-in slide-in-from-top-2">
            <div className="flex items-center gap-2">
              <AlertTriangle className="w-4 h-4 text-rose-400 shrink-0" />
              <span>
                <strong>{t('Revision Conflict:')}</strong> {conflictError.message}
              </span>
            </div>
            <button
              onClick={async () => {
                if (content !== savedContent && !window.confirm(t('Reloading replaces your unsaved edits. Continue?'))) return;
                try {
                  const res = await tealdrive.files.readText({ path: currentPath });
                  setContent(res.content ?? '');
                  setSavedContent(res.content ?? '');
                  setRevision(res.revision ?? null);
                  setConflictError(null);
                  addToast({ title: t('Reloaded server version'), type: 'info' });
                } catch (err: any) {
                  addToast({ title: t('Reload failed'), desc: err.message, type: 'error' });
                }
              }}
              className="flex items-center gap-1 px-2.5 py-1 bg-rose-900 hover:bg-rose-800 text-rose-100 rounded text-[11px] font-medium transition-colors"
            >
              <RotateCcw className="w-3 h-3" />
              <span>{t('Reload Latest')}</span>
            </button>
          </div>
        )}

        {/* Editor Content Area */}
        {loading ? (
          <div className="flex-1 flex flex-col items-center justify-center text-slate-400 gap-2 font-mono text-xs bg-[#090d16]">
            <Loader2 className="w-5 h-5 animate-spin text-teal-400" />
            <span>{t('Loading file contents...')}</span>
          </div>
        ) : (
          <div className="flex-1 flex overflow-hidden font-mono text-xs bg-[#090d16]">
            {/* Line Numbers */}
            <div
              ref={gutterRef}
              className="w-12 py-4 bg-slate-950/60 border-r border-slate-800/80 text-right pr-3 select-none text-slate-600 font-mono text-[11px] overflow-hidden"
            >
              {lines.map((_, i) => (
                <div key={i} className="leading-5">
                  {i + 1}
                </div>
              ))}
            </div>

            {/* Textarea */}
            <textarea
              ref={textareaRef}
              value={content}
              wrap="off"
              onScroll={handleScroll}
              onChange={(e) => setContent(e.target.value)}
              className="flex-1 p-4 bg-transparent text-slate-200 resize-none focus:outline-none leading-5 font-mono overflow-auto whitespace-pre selection:bg-teal-500/30 selection:text-teal-200"
              spellCheck={false}
              placeholder={t('Start typing your text or code here...')}
            />
          </div>
        )}

        {/* Editor Status Footer */}
        <div className="h-8 px-4 bg-slate-950 border-t border-slate-800 flex items-center justify-between text-[11px] font-mono text-slate-500 shrink-0">
          <div className="flex items-center gap-3">
            <span className="flex items-center gap-1 text-teal-400">
              <ShieldCheck className="w-3 h-3" />
              <span>{typeof revision === 'string' ? t('Overwrite protection on') : t('Revision unknown')}</span>
            </span>
            <span>·</span>
            <span>{t('Lines: {count}', { count: lines.length })}</span>
            <span>·</span>
            <span>{t('Characters: {count}', { count: content.length })}</span>
          </div>
          <div>UTF-8 · LF · {currentMimeType}</div>
        </div>
      </div>
    </div>
  );
};
