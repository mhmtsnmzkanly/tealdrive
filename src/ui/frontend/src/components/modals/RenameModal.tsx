import React, { useState, useEffect } from 'react';
import { useTealDrive } from '../../context/TealDriveContext.tsx';
import { FileEdit, X } from 'lucide-react';

export const RenameModal: React.FC = () => {
  const { renameModalFile, setRenameModalFile, renameItem } = useTealDrive();
  const [newName, setNewName] = useState(renameModalFile?.name || '');
  const [renaming, setRenaming] = useState(false);

  useEffect(() => {
    if (renameModalFile) {
      setNewName(renameModalFile.name);
    }
  }, [renameModalFile]);

  if (!renameModalFile) return null;

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!newName.trim() || newName.trim() === renameModalFile.name) {
      setRenameModalFile(null);
      return;
    }

    setRenaming(true);
    try {
      await renameItem(renameModalFile.path, newName.trim());
      setRenameModalFile(null);
    } finally {
      setRenaming(false);
    }
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center p-4 bg-slate-950/80 backdrop-blur-sm animate-in fade-in duration-150">
      <div className="w-full max-w-sm bg-slate-900 border border-slate-700/80 rounded-2xl shadow-2xl p-6 text-slate-100">
        <div className="flex items-center justify-between pb-3 border-b border-slate-800 mb-4">
          <div className="flex items-center gap-2">
            <FileEdit className="w-4 h-4 text-teal-400" />
            <h3 className="text-sm font-semibold">Rename Item</h3>
          </div>
          <button
            onClick={() => setRenameModalFile(null)}
            className="p-1 rounded text-slate-400 hover:text-slate-200 hover:bg-slate-800 transition-colors"
          >
            <X className="w-4 h-4" />
          </button>
        </div>

        <form onSubmit={handleSubmit} className="space-y-4">
          <div>
            <label className="block text-xs text-slate-400 mb-1">New Name</label>
            <input
              type="text"
              required
              autoFocus
              value={newName}
              onChange={(e) => setNewName(e.target.value)}
              className="w-full px-3 py-2 bg-slate-800 border border-slate-700 rounded-lg text-xs text-slate-100 focus:outline-none focus:border-teal-500 font-mono"
            />
          </div>

          <div className="flex items-center justify-end gap-2 pt-2">
            <button
              type="button"
              onClick={() => setRenameModalFile(null)}
              className="px-3 py-1.5 bg-slate-800 hover:bg-slate-700 text-slate-300 text-xs rounded-lg transition-colors"
            >
              Cancel
            </button>
            <button
              type="submit"
              disabled={renaming}
              className="px-4 py-1.5 bg-teal-600 hover:bg-teal-500 text-slate-950 font-semibold text-xs rounded-lg shadow transition-all active:scale-95"
            >
              {renaming ? 'Renaming...' : 'Rename'}
            </button>
          </div>
        </form>
      </div>
    </div>
  );
};
