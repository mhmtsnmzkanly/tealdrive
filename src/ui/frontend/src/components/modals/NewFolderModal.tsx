import React, { useState } from 'react';
import { useTealDrive } from '../../context/TealDriveContext.tsx';
import { FolderPlus, X } from 'lucide-react';
import { t } from '../../i18n/index.ts';

export const NewFolderModal: React.FC = () => {
  const { newFolderOpen, setNewFolderOpen, createFolder, currentPath } = useTealDrive();
  const [folderName, setFolderName] = useState('');
  const [creating, setCreating] = useState(false);

  if (!newFolderOpen) return null;

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!folderName.trim()) return;

    setCreating(true);
    try {
      await createFolder(folderName.trim());
      setFolderName('');
      setNewFolderOpen(false);
    } finally {
      setCreating(false);
    }
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center p-4 bg-slate-950/80 backdrop-blur-sm animate-in fade-in duration-150">
      <div className="w-full max-w-sm bg-slate-900 border border-slate-700/80 rounded-2xl shadow-2xl p-6 text-slate-100">
        <div className="flex items-center justify-between pb-3 border-b border-slate-800 mb-4">
          <div className="flex items-center gap-2">
            <FolderPlus className="w-4 h-4 text-teal-400" />
            <h3 className="text-sm font-semibold">{t('New Folder')}</h3>
          </div>
          <button
            onClick={() => setNewFolderOpen(false)}
            className="p-1 rounded text-slate-400 hover:text-slate-200 hover:bg-slate-800 transition-colors"
          >
            <X className="w-4 h-4" />
          </button>
        </div>

        <form onSubmit={handleSubmit} className="space-y-4">
          <div>
            <label className="block text-xs text-slate-400 mb-1">{t('Folder Name')}</label>
            <input
              type="text"
              required
              autoFocus
              value={folderName}
              onChange={(e) => setFolderName(e.target.value)}
              placeholder={t('e.g. Invoices_2026')}
              className="w-full px-3 py-2 bg-slate-800 border border-slate-700 rounded-lg text-xs text-slate-100 focus:outline-none focus:border-teal-500"
            />
            <div className="text-[11px] font-mono text-slate-500 mt-1">{t('Target: {path}', { path: currentPath })}</div>
          </div>

          <div className="flex items-center justify-end gap-2 pt-2">
            <button
              type="button"
              onClick={() => setNewFolderOpen(false)}
              className="px-3 py-1.5 bg-slate-800 hover:bg-slate-700 text-slate-300 text-xs rounded-lg transition-colors"
            >
              {t('Cancel')}
            </button>
            <button
              type="submit"
              disabled={creating}
              className="px-4 py-1.5 bg-teal-600 hover:bg-teal-500 text-slate-950 font-semibold text-xs rounded-lg shadow transition-all active:scale-95"
            >
              {creating ? 'Creating...' : t('Create Folder')}
            </button>
          </div>
        </form>
      </div>
    </div>
  );
};
