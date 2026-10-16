import React, { useState, useRef } from 'react';
import { useTealDrive } from '../../context/TealDriveContext.tsx';
import {
  HardDrive,
  Clock,
  Trash2,
  History,
  ShieldCheck,
  Plus,
  Upload,
  FolderPlus,
  FileCode,
  FolderUp,
  AlertTriangle,
  Lock,
  ChevronDown,
  Gauge,
  X,
} from 'lucide-react';
import { formatBytes } from '../../utils/formatters.ts';
import { t } from '../../i18n/index.ts';

export const Sidebar: React.FC = () => {
  const {
    session,
    quota,
    clientState,
    activeView,
    setActiveView,
    setNewFolderOpen,
    setEditorFile,
    uploadFiles,
    sidebarMobileOpen,
    setSidebarMobileOpen,
    currentPath,
  } = useTealDrive();

  const [newMenuOpen, setNewMenuOpen] = useState(false);
  const fileInputRef = useRef<HTMLInputElement>(null);
  const folderInputRef = useRef<HTMLInputElement>(null);

  const handleFileUpload = (e: React.ChangeEvent<HTMLInputElement>) => {
    if (e.target.files && e.target.files.length > 0) {
      uploadFiles(e.target.files);
      e.target.value = '';
      setNewMenuOpen(false);
    }
  };

  const handleFolderUpload = (e: React.ChangeEvent<HTMLInputElement>) => {
    if (e.target.files && e.target.files.length > 0) {
      uploadFiles(e.target.files);
      e.target.value = '';
      setNewMenuOpen(false);
    }
  };

  const handleNewDocument = () => {
    setNewMenuOpen(false);
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
      content: '',
    });
  };

  const quotaPercent = quota ? quota.percentage : 0;
  const isQuotaCritical = quotaPercent >= 95;
  const isQuotaWarning = quotaPercent >= 80 && !isQuotaCritical;

  return (
    <>
      {/* Mobile backdrop */}
      {sidebarMobileOpen && (
        <div
          onClick={() => setSidebarMobileOpen(false)}
          className="fixed inset-0 bg-slate-950/80 backdrop-blur-sm z-30 md:hidden animate-in fade-in duration-150"
        />
      )}

      <aside
        className={`w-72 sm:w-64 max-w-[85vw] bg-[#090d16] border-r border-slate-800/80 flex flex-col h-[100dvh] max-h-[100dvh] select-none shrink-0 z-40 transition-transform duration-200 ${
          sidebarMobileOpen
            ? 'fixed inset-y-0 left-0 translate-x-0 shadow-2xl'
            : 'fixed inset-y-0 left-0 -translate-x-full md:relative md:translate-x-0'
        }`}
      >
        {/* Brand Header */}
        <div className="h-16 px-5 flex items-center justify-between border-b border-slate-800/60 shrink-0">
          <div className="flex items-center gap-2.5">
            <div className="w-8 h-8 rounded-lg bg-gradient-to-br from-teal-400 to-teal-600 flex items-center justify-center shadow-lg shadow-teal-500/20">
              <HardDrive className="w-4 h-4 text-slate-950 font-bold" />
            </div>
            <div className="flex flex-col">
              <span className="font-bold text-base tracking-tight text-white flex items-center gap-1.5">
                {t('TealDrive')}
                <span className="text-[10px] font-mono px-1.5 py-0.5 rounded bg-teal-950/80 border border-teal-700/50 text-teal-400">
                  v2.4
                </span>
              </span>
            </div>
          </div>

          <button
            onClick={() => setSidebarMobileOpen(false)}
            className="p-1.5 rounded-lg text-slate-400 hover:text-slate-200 hover:bg-slate-800 md:hidden transition-colors"
            title={t('Close menu')}
          >
            <X className="w-4 h-4" />
          </button>
        </div>

        {/* Main Actions Area */}
        <div className="p-4 shrink-0">
        {/* Giant + New button */}
        <div className="relative">
          <button
            onClick={() => setNewMenuOpen(!newMenuOpen)}
            className="w-full flex items-center justify-between px-4 py-2.5 bg-gradient-to-r from-teal-600 to-teal-500 hover:from-teal-500 hover:to-teal-400 text-slate-950 font-semibold text-sm rounded-xl shadow-lg shadow-teal-500/20 transition-all active:scale-[0.98]"
          >
            <div className="flex items-center gap-2">
              <Plus className="w-4 h-4 stroke-[2.5]" />
              <span>{t('New')}</span>
            </div>
            <ChevronDown className={`w-4 h-4 transition-transform duration-200 ${newMenuOpen ? 'rotate-180' : ''}`} />
          </button>

          {/* New dropdown popover */}
          {newMenuOpen && (
            <div className="absolute top-12 left-0 right-0 bg-slate-900 border border-slate-700/80 rounded-xl shadow-2xl p-1.5 z-50 text-slate-200 backdrop-blur-xl animate-in fade-in zoom-in-95 duration-150">
              <button
                onClick={() => {
                  fileInputRef.current?.click();
                }}
                className="w-full flex items-center gap-2.5 px-3 py-2 text-xs font-medium rounded-lg hover:bg-slate-800 hover:text-white transition-colors"
              >
                <Upload className="w-4 h-4 text-teal-400" />
                <span>{t('Upload Files')}</span>
              </button>

              <button
                onClick={() => {
                  folderInputRef.current?.click();
                }}
                className="w-full flex items-center gap-2.5 px-3 py-2 text-xs font-medium rounded-lg hover:bg-slate-800 hover:text-white transition-colors"
              >
                <FolderUp className="w-4 h-4 text-teal-400" />
                <span>{t('Upload Folder')}</span>
              </button>

              <div className="h-px bg-slate-800 my-1" />

              <button
                onClick={() => {
                  setNewMenuOpen(false);
                  setNewFolderOpen(true);
                }}
                className="w-full flex items-center gap-2.5 px-3 py-2 text-xs font-medium rounded-lg hover:bg-slate-800 hover:text-white transition-colors"
              >
                <FolderPlus className="w-4 h-4 text-teal-400" />
                <span>{t('New Folder')}</span>
              </button>

              <button
                onClick={handleNewDocument}
                className="w-full flex items-center gap-2.5 px-3 py-2 text-xs font-medium rounded-lg hover:bg-slate-800 hover:text-white transition-colors"
              >
                <FileCode className="w-4 h-4 text-teal-400" />
                <span>{t('New Text / Code Document')}</span>
              </button>
            </div>
          )}

          {/* Hidden file pickers */}
          <input
            type="file"
            ref={fileInputRef}
            onChange={handleFileUpload}
            multiple
            className="hidden"
          />
          <input
            type="file"
            ref={folderInputRef}
            onChange={handleFolderUpload}
            // @ts-ignore
            webkitdirectory=""
            directory=""
            className="hidden"
          />
        </div>
      </div>

      {/* Navigation Links */}
      <nav className="flex-1 px-3 space-y-1 overflow-y-auto min-h-0 overscroll-contain">
        <button
          onClick={() => setActiveView('drive')}
          className={`w-full flex items-center gap-3 px-3 py-2 rounded-lg text-xs font-medium transition-colors ${
            activeView === 'drive'
              ? 'bg-teal-500/10 text-teal-300 font-semibold border border-teal-500/20'
              : 'text-slate-400 hover:text-slate-100 hover:bg-slate-800/50'
          }`}
        >
          <HardDrive className="w-4 h-4 text-teal-400" />
          <span>{t('My Drive')}</span>
        </button>

        <button
          onClick={() => setActiveView('recent')}
          className={`w-full flex items-center gap-3 px-3 py-2 rounded-lg text-xs font-medium transition-colors ${
            activeView === 'recent'
              ? 'bg-teal-500/10 text-teal-300 font-semibold border border-teal-500/20'
              : 'text-slate-400 hover:text-slate-100 hover:bg-slate-800/50'
          }`}
        >
          <Clock className="w-4 h-4 text-sky-400" />
          <span>{t('Recent')}</span>
        </button>

        <button
          onClick={() => setActiveView('trash')}
          className={`w-full flex items-center gap-3 px-3 py-2 rounded-lg text-xs font-medium transition-colors ${
            activeView === 'trash'
              ? 'bg-teal-500/10 text-teal-300 font-semibold border border-teal-500/20'
              : 'text-slate-400 hover:text-slate-100 hover:bg-slate-800/50'
          }`}
        >
          <Trash2 className="w-4 h-4 text-rose-400" />
          <span>{t('Trash')}</span>
        </button>

        <button
          onClick={() => setActiveView('audit')}
          className={`w-full flex items-center gap-3 px-3 py-2 rounded-lg text-xs font-medium transition-colors ${
            activeView === 'audit'
              ? 'bg-teal-500/10 text-teal-300 font-semibold border border-teal-500/20'
              : 'text-slate-400 hover:text-slate-100 hover:bg-slate-800/50'
          }`}
        >
          <History className="w-4 h-4 text-amber-400" />
          <span>{t('History & Undo')}</span>
        </button>

        {session?.isAdmin && (
          <div className="pt-4 mt-2 border-t border-slate-800/60">
            <div className="px-3 pb-2 text-[10px] font-semibold text-slate-500 uppercase tracking-wider">
              {t('Administration')}
            </div>
            <button
              onClick={() => setActiveView('admin')}
              className={`w-full flex items-center gap-3 px-3 py-2 rounded-lg text-xs font-medium transition-colors ${
                activeView === 'admin'
                  ? 'bg-teal-500/10 text-teal-300 font-semibold border border-teal-500/20'
                  : 'text-slate-400 hover:text-slate-100 hover:bg-slate-800/50'
              }`}
            >
              <ShieldCheck className="w-4 h-4 text-purple-400" />
              <span>{t('Admin Console')}</span>
            </button>
          </div>
        )}
      </nav>

      {/* Storage Quota Widget */}
      <div className="shrink-0 p-3.5 pb-[max(1.25rem,calc(env(safe-area-inset-bottom)+14px))] border-t border-slate-800/80 bg-slate-900/60 backdrop-blur-md">
        <div className="flex items-center justify-between mb-1.5">
          <div className="flex items-center gap-1.5 text-xs font-medium text-slate-300">
            <Gauge className="w-3.5 h-3.5 text-teal-400" />
            <span>{t('Storage Quota')}</span>
          </div>
        </div>

        {/* Progress bar */}
        <div className="w-full bg-slate-800 rounded-full h-2 overflow-hidden mb-1.5">
          <div
            className={`h-full transition-all duration-300 rounded-full ${
              isQuotaCritical
                ? 'bg-rose-500'
                : isQuotaWarning
                ? 'bg-amber-500'
                : 'bg-gradient-to-r from-teal-500 to-teal-400'
            }`}
            style={{ width: `${Math.min(100, quotaPercent)}%` }}
          />
        </div>

        <div className="flex items-center justify-between text-[11px] font-mono tabular-nums text-slate-400">
          <span>{t('{size} used', { size: formatBytes(quota?.usedBytes || 0) })}</span>
          <span>{quotaPercent}%</span>
        </div>

        {/* Warning Badge */}
        {isQuotaCritical ? (
          <div className="mt-2 flex items-center gap-1.5 px-2 py-1 rounded bg-rose-950/70 border border-rose-800/60 text-rose-300 text-[10px]">
            <AlertTriangle className="w-3 h-3 shrink-0" />
            <span>{t('Critical: Storage nearly full')}</span>
          </div>
        ) : isQuotaWarning ? (
          <div className="mt-2 flex items-center gap-1.5 px-2 py-1 rounded bg-amber-950/70 border border-amber-800/60 text-amber-300 text-[10px]">
            <AlertTriangle className="w-3 h-3 shrink-0" />
            <span>{t('Warning: Exceeding 80% threshold')}</span>
          </div>
        ) : null}

        {/* Security / Transport Footer */}
        <div className="mt-3 pt-2 border-t border-slate-800/50 flex items-center justify-between text-[10px] text-slate-400">
          <div className="flex items-center gap-1">
            <Lock className="w-3 h-3 text-teal-400" />
            <span className="font-mono">{clientState.encryption === 'noise' ? 'TLS + Noise NK' : 'TLS'}</span>
          </div>
          <span className="font-mono tabular-nums">{clientState.latency}ms</span>
        </div>
      </div>
    </aside>
    </>
  );
};
