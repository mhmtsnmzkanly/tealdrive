import React, { useState } from 'react';
import type { FileEntry } from '../../sdk/types.ts';
import {
  octalToMatrix,
  matrixToOctal,
  modeToSymbolic,
  checkPermissionSecurity,
  PermissionMatrix,
} from '../../sdk/permissions.ts';
import { useTealDrive } from '../../context/TealDriveContext.tsx';
import { Key, AlertTriangle, ShieldCheck, X } from 'lucide-react';
import { t } from '../../i18n/index.ts';

interface ChmodModalProps {
  file: FileEntry;
  onClose: () => void;
}

export const ChmodModal: React.FC<ChmodModalProps> = ({ file, onClose }) => {
  const { chmodItem } = useTealDrive();
  const [matrix, setMatrix] = useState<PermissionMatrix>(() => octalToMatrix(file.mode));
  const [saving, setSaving] = useState(false);

  const currentOctal = matrixToOctal(matrix);
  const symbolic = modeToSymbolic(currentOctal, file.isDirectory);
  const security = checkPermissionSecurity(matrix);

  const toggleBit = (role: 'owner' | 'group' | 'others', bit: 'read' | 'write' | 'execute') => {
    setMatrix((prev) => ({
      ...prev,
      [role]: {
        ...prev[role],
        [bit]: !prev[role][bit],
      },
    }));
  };

  const handlePreset = (octal: string) => {
    setMatrix(octalToMatrix(octal));
  };

  const handleApply = async () => {
    setSaving(true);
    try {
      await chmodItem(file.path, currentOctal);
      onClose();
    } finally {
      setSaving(false);
    }
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center p-4 bg-slate-950/80 backdrop-blur-sm animate-in fade-in duration-150">
      <div className="w-full max-w-md bg-slate-900 border border-slate-700/80 rounded-2xl shadow-2xl p-6 text-slate-100">
        <div className="flex items-center justify-between pb-4 border-b border-slate-800">
          <div className="flex items-center gap-2.5">
            <div className="p-2 rounded-lg bg-amber-500/10 border border-amber-500/20 text-amber-400">
              <Key className="w-4 h-4" />
            </div>
            <div>
              <h3 className="text-sm font-semibold text-slate-100">{t('UNIX File Permissions (chmod)')}</h3>
              <p className="text-[11px] font-mono text-slate-400 truncate max-w-[280px]">
                {file.name}
              </p>
            </div>
          </div>
          <button
            onClick={onClose}
            className="p-1 rounded text-slate-400 hover:text-slate-200 hover:bg-slate-800 transition-colors"
          >
            <X className="w-4 h-4" />
          </button>
        </div>

        {/* Live Octal & Symbolic Display */}
        <div className="my-5 p-3 bg-slate-950 rounded-xl border border-slate-800 flex items-center justify-around font-mono">
          <div className="text-center">
            <span className="text-[10px] uppercase text-slate-500 block mb-0.5">{t('Octal Mode')}</span>
            <span className="text-xl font-bold text-teal-400">{currentOctal}</span>
          </div>
          <div className="h-8 w-px bg-slate-800" />
          <div className="text-center">
            <span className="text-[10px] uppercase text-slate-500 block mb-0.5">{t('Symbolic')}</span>
            <span className="text-sm font-semibold text-slate-200">{symbolic}</span>
          </div>
        </div>

        {/* Security Warning banner */}
        {!security.isSafe ? (
          <div className="mb-4 p-2.5 rounded-lg bg-rose-950/80 border border-rose-800/80 text-rose-300 text-xs flex items-center gap-2">
            <AlertTriangle className="w-4 h-4 shrink-0" />
            <span>{security.warning}</span>
          </div>
        ) : (
          <div className="mb-4 p-2 rounded-lg bg-slate-950 border border-slate-800 text-teal-400 text-[11px] flex items-center gap-1.5">
            <ShieldCheck className="w-3.5 h-3.5" />
            <span>{t('No risky permission bits selected')}</span>
          </div>
        )}

        {/* 3x3 Matrix Grid */}
        <div className="bg-slate-950/50 border border-slate-800 rounded-xl p-3.5 mb-4">
          <div className="grid grid-cols-4 gap-2 text-center text-xs font-mono font-medium pb-2 border-b border-slate-800 text-slate-400">
            <div className="text-left">{t('Target')}</div>
            <div>{t('Read (r)')}</div>
            <div>{t('Write (w)')}</div>
            <div>{t('Exec (x)')}</div>
          </div>

          {/* Owner row */}
          <div className="grid grid-cols-4 gap-2 text-center items-center py-2.5 border-b border-slate-800/60 text-xs">
            <div className="text-left font-medium text-slate-300">{t('Owner')}</div>
            <div>
              <input
                type="checkbox"
                checked={matrix.owner.read}
                onChange={() => toggleBit('owner', 'read')}
                className="w-4 h-4 rounded border-slate-700 bg-slate-800 text-teal-500 focus:ring-teal-500/20"
              />
            </div>
            <div>
              <input
                type="checkbox"
                checked={matrix.owner.write}
                onChange={() => toggleBit('owner', 'write')}
                className="w-4 h-4 rounded border-slate-700 bg-slate-800 text-teal-500 focus:ring-teal-500/20"
              />
            </div>
            <div>
              <input
                type="checkbox"
                checked={matrix.owner.execute}
                onChange={() => toggleBit('owner', 'execute')}
                className="w-4 h-4 rounded border-slate-700 bg-slate-800 text-teal-500 focus:ring-teal-500/20"
              />
            </div>
          </div>

          {/* Group row */}
          <div className="grid grid-cols-4 gap-2 text-center items-center py-2.5 border-b border-slate-800/60 text-xs">
            <div className="text-left font-medium text-slate-300">{t('Group')}</div>
            <div>
              <input
                type="checkbox"
                checked={matrix.group.read}
                onChange={() => toggleBit('group', 'read')}
                className="w-4 h-4 rounded border-slate-700 bg-slate-800 text-teal-500 focus:ring-teal-500/20"
              />
            </div>
            <div>
              <input
                type="checkbox"
                checked={matrix.group.write}
                onChange={() => toggleBit('group', 'write')}
                className="w-4 h-4 rounded border-slate-700 bg-slate-800 text-teal-500 focus:ring-teal-500/20"
              />
            </div>
            <div>
              <input
                type="checkbox"
                checked={matrix.group.execute}
                onChange={() => toggleBit('group', 'execute')}
                className="w-4 h-4 rounded border-slate-700 bg-slate-800 text-teal-500 focus:ring-teal-500/20"
              />
            </div>
          </div>

          {/* Others row */}
          <div className="grid grid-cols-4 gap-2 text-center items-center py-2.5 text-xs">
            <div className="text-left font-medium text-slate-300">{t('Others')}</div>
            <div>
              <input
                type="checkbox"
                checked={matrix.others.read}
                onChange={() => toggleBit('others', 'read')}
                className="w-4 h-4 rounded border-slate-700 bg-slate-800 text-teal-500 focus:ring-teal-500/20"
              />
            </div>
            <div>
              <input
                type="checkbox"
                checked={matrix.others.write}
                onChange={() => toggleBit('others', 'write')}
                className="w-4 h-4 rounded border-slate-700 bg-slate-800 text-teal-500 focus:ring-teal-500/20"
              />
            </div>
            <div>
              <input
                type="checkbox"
                checked={matrix.others.execute}
                onChange={() => toggleBit('others', 'execute')}
                className="w-4 h-4 rounded border-slate-700 bg-slate-800 text-teal-500 focus:ring-teal-500/20"
              />
            </div>
          </div>
        </div>

        {/* Quick Presets */}
        <div className="flex items-center gap-2 mb-6">
          <span className="text-[11px] text-slate-500 font-medium">{t('Common Presets:')}</span>
          <button
            type="button"
            onClick={() => handlePreset('0644')}
            className="px-2 py-1 rounded bg-slate-800 hover:bg-slate-700 text-slate-300 text-[11px] font-mono transition-colors"
          >
            0644 (File Standard)
          </button>
          <button
            type="button"
            onClick={() => handlePreset('0755')}
            className="px-2 py-1 rounded bg-slate-800 hover:bg-slate-700 text-slate-300 text-[11px] font-mono transition-colors"
          >
            0755 (Directory/Exec)
          </button>
          <button
            type="button"
            onClick={() => handlePreset('0700')}
            className="px-2 py-1 rounded bg-slate-800 hover:bg-slate-700 text-slate-300 text-[11px] font-mono transition-colors"
          >
            0700 (Private)
          </button>
        </div>

        {/* Action Buttons */}
        <div className="flex items-center justify-end gap-2">
          <button
            type="button"
            onClick={onClose}
            className="px-3.5 py-1.5 rounded-lg bg-slate-800 hover:bg-slate-700 text-slate-300 text-xs transition-colors"
          >
            {t('Cancel')}
          </button>
          <button
            type="button"
            onClick={handleApply}
            disabled={saving}
            className="px-4 py-1.5 rounded-lg bg-teal-600 hover:bg-teal-500 text-slate-950 font-semibold text-xs transition-all shadow-md active:scale-95"
          >
            {saving ? 'Applying...' : t('Apply Permissions')}
          </button>
        </div>
      </div>
    </div>
  );
};
