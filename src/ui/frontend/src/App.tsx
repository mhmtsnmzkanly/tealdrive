/**
 * @license
 * SPDX-License-Identifier: Apache-2.0
 */

import React from 'react';
import { TealDriveProvider, useTealDrive } from './context/TealDriveContext.tsx';
import { Sidebar } from './components/layout/Sidebar.tsx';
import { TopBar } from './components/layout/TopBar.tsx';
import { FileGrid } from './components/explorer/FileGrid.tsx';
import { FileTable } from './components/explorer/FileTable.tsx';
import { SelectionToolbar } from './components/explorer/SelectionToolbar.tsx';
import { RecentView } from './components/explorer/RecentView.tsx';
import { TrashView } from './components/explorer/TrashView.tsx';
import { AuditTimeline } from './components/explorer/AuditTimeline.tsx';
import { AdminConsole } from './components/admin/AdminConsole.tsx';
import { CodeEditorModal } from './components/modals/CodeEditorModal.tsx';
import { QuickLookModal } from './components/modals/QuickLookModal.tsx';
import { ChmodModal } from './components/modals/ChmodModal.tsx';
import { NewFolderModal } from './components/modals/NewFolderModal.tsx';
import { RenameModal } from './components/modals/RenameModal.tsx';
import { MoveModal } from './components/modals/MoveModal.tsx';
import { OmnisearchModal } from './components/modals/OmnisearchModal.tsx';
import { ConfirmDeleteModal } from './components/modals/ConfirmDeleteModal.tsx';
import { UploadManagerDock } from './components/dock/UploadManagerDock.tsx';
import { ToastContainer } from './components/common/Toast.tsx';
import { LoginModal } from './components/modals/LoginModal.tsx';
import { formatBytes } from './utils/formatters.ts';
import { PasswordChangeModal } from './components/modals/PasswordChangeModal.tsx';
import { AlertTriangle, X } from 'lucide-react';
import { t } from './i18n/index.ts';

const MainExplorer: React.FC = () => {
  const {
    activeView,
    viewMode,
    visibleEntries: sortedEntries,
    filter,
    setFilter,
    uploadFiles,
    quota,
    previewFile,
    setPreviewFile,
    editorFile,
    setEditorFile,
    chmodFile,
    setChmodFile,
    renameModalFile,
  } = useTealDrive();

  const [quotaBannerDismissed, setQuotaBannerDismissed] = React.useState(false);

  const filterTabs = [
    { id: 'all', label: t('All Files') },
    { id: 'folders', label: 'Folders' },
    { id: 'documents', label: 'Documents' },
    { id: 'images', label: 'Images' },
    { id: 'media', label: 'Media' },
    { id: 'code', label: 'Code' },
    { id: 'archives', label: 'Archives' },
  ] as const;

  const isQuotaWarning = quota && quota.percentage >= 80;

  return (
    <div className="flex-1 flex flex-col min-w-0 h-[100dvh] overflow-y-auto bg-[#090d16]">
      <TopBar />

      {/* Proactive Quota Warning Banner */}
      {isQuotaWarning && !quotaBannerDismissed && (
        <div
          className={`mx-6 mt-3 p-3 rounded-xl border flex items-center justify-between text-xs transition-colors ${
            quota.percentage >= 95
              ? 'bg-rose-950/70 border-rose-800 text-rose-200'
              : 'bg-amber-950/70 border-amber-800 text-amber-200'
          }`}
        >
          <div className="flex items-center gap-2.5">
            <AlertTriangle className="w-4 h-4 shrink-0 text-amber-400" />
            <span>
              <strong>{t('Storage Threshold Alert:')}</strong>{' '}
              {t('You are currently using {used} of {limit} ({percent}%).', {
                used: formatBytes(quota.usedBytes),
                limit: formatBytes(quota.limitBytes),
                percent: quota.percentage,
              })}{' '}
              {quota.percentage >= 95
                ? t('Immediate action recommended: Clean up unnecessary files or request a quota increase.')
                : t('Consider archiving large media files.')}
            </span>
          </div>
          <button
            onClick={() => setQuotaBannerDismissed(true)}
            className="p-1 text-slate-400 hover:text-slate-200 transition-colors"
          >
            <X className="w-3.5 h-3.5" />
          </button>
        </div>
      )}

      {/* Selection Action Bar */}
      <SelectionToolbar />

      {/* Primary Content View Switcher */}
      <main className="flex-1 pb-16">
        {activeView === 'drive' && (
          <div>
            {/* Category Filter Pills (Functional button controls) */}
            <div className="px-6 pt-4 pb-2 flex items-center gap-1.5 overflow-x-auto select-none">
              {filterTabs.map((tab) => {
                const isActive = filter === tab.id;
                return (
                  <button
                    key={tab.id}
                    onClick={() => setFilter(tab.id)}
                    className={`px-3 py-1.5 rounded-lg text-xs font-medium transition-colors whitespace-nowrap shrink-0 ${
                      isActive
                        ? 'bg-teal-500/15 text-teal-300 border border-teal-500/30'
                        : 'bg-slate-900/60 hover:bg-slate-800 text-slate-400 hover:text-slate-200 border border-slate-800'
                    }`}
                  >
                    {tab.label}
                  </button>
                );
              })}
            </div>

            {/* Canvas: Grid or Table View */}
            {viewMode === 'grid' ? (
              <FileGrid files={sortedEntries} onUploadDropped={(files) => uploadFiles(files)} />
            ) : (
              <FileTable files={sortedEntries} />
            )}
          </div>
        )}

        {activeView === 'recent' && <RecentView />}
        {activeView === 'trash' && <TrashView />}
        {activeView === 'audit' && <AuditTimeline />}
        {activeView === 'admin' && <AdminConsole />}
      </main>

      {/* Modals & Overlays */}
      {editorFile && (
        <CodeEditorModal file={editorFile} onClose={() => setEditorFile(null)} />
      )}

      {previewFile && (
        <QuickLookModal file={previewFile} onClose={() => setPreviewFile(null)} />
      )}

      {chmodFile && (
        <ChmodModal file={chmodFile} onClose={() => setChmodFile(null)} />
      )}

      <NewFolderModal />
      {renameModalFile && <RenameModal key={renameModalFile.id} />}
      <MoveModal />
      <ConfirmDeleteModal />
      <OmnisearchModal />
      <UploadManagerDock />
      <ToastContainer />
    </div>
  );
};

const AppContent: React.FC = () => {
  const { session, loading } = useTealDrive();

  if (loading) {
    return (
      <div className="min-h-screen w-screen bg-[#090d16] flex items-center justify-center">
        <div className="flex flex-col items-center gap-3">
          <div className="w-8 h-8 border-2 border-teal-500 border-t-transparent rounded-full animate-spin" />
          <span className="text-xs text-slate-400">{t('Loading TealDrive...')}</span>
        </div>
      </div>
    );
  }

  if (!session) {
    return (
      <>
        <LoginModal />
        <ToastContainer />
      </>
    );
  }

  // The server refuses every other operation until the password is replaced.
  if (session.passwordChangeRequired) {
    return (
      <>
        <PasswordChangeModal />
        <ToastContainer />
      </>
    );
  }

  return (
    <div className="flex h-[100dvh] w-screen overflow-hidden bg-[#090d16] text-slate-100 font-sans">
      <Sidebar />
      <MainExplorer />
    </div>
  );
};

export default function App() {
  return (
    <TealDriveProvider>
      <AppContent />
    </TealDriveProvider>
  );
}
