import React, { createContext, useContext, useState, useEffect, useCallback, useMemo } from 'react';
import { tealdrive } from '../lib/tealdrive.ts';
import type {
  FileEntry,
  Session,
  ClientState,
  StorageQuota,
  UploadProgress,
  AuditEntry,
} from '../sdk/types.ts';

export type ViewType = 'drive' | 'recent' | 'trash' | 'audit' | 'admin';
export type FilterType = 'all' | 'folders' | 'documents' | 'images' | 'media' | 'code' | 'archives';
export type SortField = 'name' | 'size' | 'modified';

interface ToastData {
  id: string;
  title: string;
  desc?: string;
  type: 'info' | 'success' | 'warning' | 'error';
  undoOpId?: string;
}

interface TealDriveContextType {
  // Connection & session
  session: Session | null;
  clientState: ClientState;
  quota: StorageQuota | null;
  loading: boolean;
  activeView: ViewType;
  setActiveView: (view: ViewType) => void;
  login: (username: string, password: string) => Promise<void>;
  logout: () => Promise<void>;

  // Navigation & directory files
  currentPath: string;
  entries: FileEntry[];
  recentEntries: FileEntry[];
  trashEntries: FileEntry[];
  auditEntries: AuditEntry[];
  goToPath: (path: string) => Promise<void>;
  refreshCurrentDirectory: () => Promise<void>;

  // Selection & views
  selectedIds: Set<string>;
  toggleSelect: (id: string, isShift?: boolean) => void;
  selectAll: () => void;
  clearSelection: () => void;
  viewMode: 'grid' | 'table';
  setViewMode: (mode: 'grid' | 'table') => void;
  filter: FilterType;
  setFilter: (filter: FilterType) => void;
  sortField: SortField;
  setSortField: (field: SortField) => void;
  sortOrder: 'asc' | 'desc';
  toggleSortOrder: () => void;

  // File operations
  createFolder: (name: string) => Promise<void>;
  renameItem: (path: string, newName: string) => Promise<void>;
  moveItem: (sourcePath: string, targetDirPath: string) => Promise<void>;
  copyItem: (sourcePath: string, targetDirPath: string) => Promise<void>;
  chmodItem: (path: string, mode: string) => Promise<void>;
  trashSelectedOrPath: (path?: string) => Promise<void>;
  restoreFromTrash: (id: string) => Promise<void>;
  emptyTrash: () => Promise<void>;
  deletePermanent: (id: string) => Promise<void>;
  toggleStar: (path: string) => Promise<void>;
  downloadFile: (path: string) => Promise<void>;

  // Undo system
  undoOperation: (opId: string) => Promise<void>;

  // Uploads
  uploads: UploadProgress[];
  uploadFiles: (files: FileList | File[], targetDir?: string) => void;
  cancelUpload: (fileId: string) => void;
  clearCompletedUploads: () => void;

  // Modals state
  previewFile: FileEntry | null;
  setPreviewFile: (file: FileEntry | null) => void;
  editorFile: FileEntry | null;
  setEditorFile: (file: FileEntry | null) => void;
  chmodFile: FileEntry | null;
  setChmodFile: (file: FileEntry | null) => void;
  renameModalFile: FileEntry | null;
  setRenameModalFile: (file: FileEntry | null) => void;
  moveModalFile: FileEntry | null;
  setMoveModalFile: (file: FileEntry | null) => void;
  newFolderOpen: boolean;
  setNewFolderOpen: (open: boolean) => void;
  omnisearchOpen: boolean;
  setOmnisearchOpen: (open: boolean) => void;
  sidebarMobileOpen: boolean;
  setSidebarMobileOpen: (open: boolean) => void;

  // Delete confirmation modal
  deleteConfirm: DeleteConfirmModalState | null;
  setDeleteConfirm: (state: DeleteConfirmModalState | null) => void;
  promptTrash: (singlePath?: string) => void;
  promptPermanentDelete: (file: FileEntry) => void;
  promptEmptyTrash: () => void;
  promptCustomDelete: (options: Omit<DeleteConfirmModalState, 'isOpen'>) => void;

  // Toast notifications
  toasts: ToastData[];
  addToast: (toast: Omit<ToastData, 'id'>) => void;
  dismissToast: (id: string) => void;
}

export interface DeleteConfirmModalState {
  isOpen: boolean;
  title: string;
  description?: string;
  isPermanent?: boolean;
  confirmLabel?: string;
  items?: Array<{ name: string; isDirectory?: boolean; size?: number; path?: string; mimeType?: string }>;
  onConfirm: () => Promise<void> | void;
}

const TealDriveContext = createContext<TealDriveContextType | null>(null);

export const TealDriveProvider: React.FC<{ children: React.ReactNode }> = ({ children }) => {
  const [session, setSession] = useState<Session | null>(null);
  const [clientState, setClientState] = useState<ClientState>(tealdrive.getState());
  const [quota, setQuota] = useState<StorageQuota | null>(null);
  const [loading, setLoading] = useState(true);
  const [activeView, setActiveView] = useState<ViewType>('drive');

  const [currentPath, setCurrentPath] = useState<string>('/');
  const [entries, setEntries] = useState<FileEntry[]>([]);
  const [recentEntries, setRecentEntries] = useState<FileEntry[]>([]);
  const [trashEntries, setTrashEntries] = useState<FileEntry[]>([]);
  const [auditEntries, setAuditEntries] = useState<AuditEntry[]>([]);

  const [selectedIds, setSelectedIds] = useState<Set<string>>(new Set());
  const [viewMode, setViewMode] = useState<'grid' | 'table'>('grid');
  const [filter, setFilter] = useState<FilterType>('all');
  const [sortField, setSortField] = useState<SortField>('name');
  const [sortOrder, setSortOrder] = useState<'asc' | 'desc'>('asc');

  const [uploads, setUploads] = useState<UploadProgress[]>([]);
  const [toasts, setToasts] = useState<ToastData[]>([]);

  // Modals
  const [previewFile, setPreviewFile] = useState<FileEntry | null>(null);
  const [editorFile, setEditorFile] = useState<FileEntry | null>(null);
  const [chmodFile, setChmodFile] = useState<FileEntry | null>(null);
  const [renameModalFile, setRenameModalFile] = useState<FileEntry | null>(null);
  const [moveModalFile, setMoveModalFile] = useState<FileEntry | null>(null);
  const [newFolderOpen, setNewFolderOpen] = useState(false);
  const [omnisearchOpen, setOmnisearchOpen] = useState(false);
  const [sidebarMobileOpen, setSidebarMobileOpen] = useState(false);
  const [deleteConfirm, setDeleteConfirm] = useState<DeleteConfirmModalState | null>(null);

  const addToast = useCallback((toast: Omit<ToastData, 'id'>) => {
    const id = `toast_${Date.now()}_${Math.random().toString(36).substring(2, 6)}`;
    setToasts((prev) => [...prev, { ...toast, id }]);
    setTimeout(() => {
      setToasts((prev) => prev.filter((t) => t.id !== id));
    }, 6000);
  }, []);

  const dismissToast = useCallback((id: string) => {
    setToasts((prev) => prev.filter((t) => t.id !== id));
  }, []);

  // Fetch initial session & quota
  useEffect(() => {
    let unsubQuota: () => void = () => {};
    let unsubState: () => void = () => {};

    const init = async () => {
      try {
        const sess = await tealdrive.auth.getSession();
        setSession(sess);

        if (sess) {
          try {
            await tealdrive.handshake();
            const q = await tealdrive.files.quota.get();
            setQuota(q);
          } catch {}

          unsubQuota = tealdrive.files.observeQuotaWarning((nextQuota) => {
            setQuota(nextQuota);
          });

          unsubState = tealdrive.observeState((nextState) => {
            setClientState(nextState);
          });

          await loadDirectory('/');
        }
      } catch (err: any) {
        console.error('Initialization error:', err);
      } finally {
        setLoading(false);
      }
    };

    init();

    return () => {
      unsubQuota();
      unsubState();
    };
  }, []);

  const loadDirectory = async (path: string) => {
    try {
      const res = await tealdrive.files.list({ path });
      setEntries(res.entries);
      setCurrentPath(path);
      setSelectedIds(new Set());
      const q = await tealdrive.files.quota.get();
      setQuota(q);
    } catch (err: any) {
      addToast({ title: 'Navigation error', desc: err.message, type: 'error' });
    }
  };

  const loadRecent = async () => {
    const recent = await tealdrive.files.getRecent(20);
    setRecentEntries(recent);
  };

  const loadTrash = async () => {
    const trash = await tealdrive.files.trash.list();
    setTrashEntries(trash);
  };

  const loadAudit = async () => {
    const audits = await tealdrive.files.undo.list();
    setAuditEntries(audits);
  };

  useEffect(() => {
    if (activeView === 'drive') {
      loadDirectory(currentPath);
    } else if (activeView === 'recent') {
      loadRecent();
    } else if (activeView === 'trash') {
      loadTrash();
    } else if (activeView === 'audit') {
      loadAudit();
    }
  }, [activeView, currentPath]);

  const refreshCurrentDirectory = useCallback(async () => {
    if (activeView === 'drive') {
      await loadDirectory(currentPath);
    } else if (activeView === 'recent') {
      await loadRecent();
    } else if (activeView === 'trash') {
      await loadTrash();
    } else if (activeView === 'audit') {
      await loadAudit();
    }
  }, [activeView, currentPath]);

  const goToPath = useCallback(async (path: string) => {
    setActiveView('drive');
    await loadDirectory(path);
  }, []);

  const login = useCallback(async (username: string, password: string) => {
    setLoading(true);
    try {
      const sess = await tealdrive.auth.login({ username, password });
      setSession(sess);
      try {
        const q = await tealdrive.files.quota.get();
        setQuota(q);
      } catch {}
      await loadDirectory('/');
      addToast({
        title: 'Signed In',
        desc: `Welcome back, ${sess.username}!`,
        type: 'success',
      });
    } catch (err: any) {
      throw err;
    } finally {
      setLoading(false);
    }
  }, [addToast]);

  const logout = useCallback(async () => {
    try {
      await tealdrive.auth.logout();
    } catch {}
    setSession(null);
    setEntries([]);
    setSelectedIds(new Set());
    addToast({
      title: 'Signed Out',
      desc: 'You have been signed out successfully.',
      type: 'info',
    });
  }, [addToast]);

  // Selection
  const toggleSelect = useCallback((id: string, isShift?: boolean) => {
    setSelectedIds((prev) => {
      const next = new Set(prev);
      if (next.has(id)) {
        next.delete(id);
      } else {
        if (!isShift) {
          next.clear();
        }
        next.add(id);
      }
      return next;
    });
  }, []);

  const selectAll = useCallback(() => {
    const all = new Set(entries.map((e) => e.id));
    setSelectedIds(all);
  }, [entries]);

  const clearSelection = useCallback(() => {
    setSelectedIds(new Set());
  }, []);

  const toggleSortOrder = useCallback(() => {
    setSortOrder((prev) => (prev === 'asc' ? 'desc' : 'asc'));
  }, []);

  // CRUD handlers
  const createFolder = useCallback(async (name: string) => {
    try {
      const folderPath = currentPath === '/' ? `/${name}` : `${currentPath}/${name}`;
      await tealdrive.files.createFolder({ path: folderPath });
      await refreshCurrentDirectory();
      const audits = await tealdrive.files.undo.list();
      const latestOp = audits[0]?.id;
      addToast({
        title: 'Folder created',
        desc: `"${name}" added to current directory`,
        type: 'success',
        undoOpId: latestOp,
      });
    } catch (err: any) {
      addToast({ title: 'Failed to create folder', desc: err.message, type: 'error' });
    }
  }, [currentPath, refreshCurrentDirectory, addToast]);

  const renameItem = useCallback(async (path: string, newName: string) => {
    try {
      await tealdrive.files.rename({ path, newName });
      await refreshCurrentDirectory();
      const audits = await tealdrive.files.undo.list();
      const latestOp = audits[0]?.id;
      addToast({
        title: 'Item renamed',
        desc: `Renamed to "${newName}"`,
        type: 'success',
        undoOpId: latestOp,
      });
    } catch (err: any) {
      addToast({ title: 'Failed to rename', desc: err.message, type: 'error' });
    }
  }, [refreshCurrentDirectory, addToast]);

  const moveItem = useCallback(async (sourcePath: string, targetDirPath: string) => {
    try {
      await tealdrive.files.move({ sourcePath, targetPath: targetDirPath });
      await refreshCurrentDirectory();
      const audits = await tealdrive.files.undo.list();
      const latestOp = audits[0]?.id;
      addToast({
        title: 'Item moved',
        desc: `Moved into ${targetDirPath}`,
        type: 'success',
        undoOpId: latestOp,
      });
    } catch (err: any) {
      addToast({ title: 'Failed to move', desc: err.message, type: 'error' });
    }
  }, [refreshCurrentDirectory, addToast]);

  const copyItem = useCallback(async (sourcePath: string, targetDirPath: string) => {
    try {
      await tealdrive.files.copy({ sourcePath, targetPath: targetDirPath });
      await refreshCurrentDirectory();
      addToast({
        title: 'Item duplicated',
        desc: `Created copy in ${targetDirPath}`,
        type: 'success',
      });
    } catch (err: any) {
      addToast({ title: 'Failed to copy', desc: err.message, type: 'error' });
    }
  }, [refreshCurrentDirectory, addToast]);

  const chmodItem = useCallback(async (path: string, mode: string) => {
    try {
      await tealdrive.files.chmod({ path, mode });
      await refreshCurrentDirectory();
      const audits = await tealdrive.files.undo.list();
      const latestOp = audits[0]?.id;
      addToast({
        title: 'Permissions updated',
        desc: `UNIX mode set to ${mode}`,
        type: 'success',
        undoOpId: latestOp,
      });
    } catch (err: any) {
      addToast({ title: 'Failed to set permissions', desc: err.message, type: 'error' });
    }
  }, [refreshCurrentDirectory, addToast]);

  const trashSelectedOrPath = useCallback(async (singlePath?: string) => {
    const pathsToTrash: string[] = [];
    if (singlePath) {
      pathsToTrash.push(singlePath);
    } else {
      for (const e of entries) {
        if (selectedIds.has(e.id)) {
          pathsToTrash.push(e.path);
        }
      }
    }

    if (pathsToTrash.length === 0) return;

    try {
      for (const p of pathsToTrash) {
        await tealdrive.files.trash.move({ path: p });
      }
      setSelectedIds(new Set());
      await refreshCurrentDirectory();
      const audits = await tealdrive.files.undo.list();
      const latestOp = audits[0]?.id;
      addToast({
        title: pathsToTrash.length > 1 ? `${pathsToTrash.length} items moved to Trash` : 'Moved to Trash',
        desc: 'Files can be restored from the Trash view anytime',
        type: 'info',
        undoOpId: latestOp,
      });
    } catch (err: any) {
      const errMsg = err?.message || String(err || '');
      if (err?.code === 'FEATURE_DISABLED' || errMsg.includes('FEATURE_DISABLED')) {
        addToast({
          title: 'Feature Disabled (FEATURE_DISABLED)',
          desc: 'Moving items to trash is disabled by server configuration.',
          type: 'warning',
        });
      } else {
        addToast({ title: 'Error deleting items', desc: errMsg, type: 'error' });
      }
    }
  }, [entries, selectedIds, refreshCurrentDirectory, addToast]);

  const restoreFromTrash = useCallback(async (id: string) => {
    try {
      const restored = await tealdrive.files.trash.restore({ id });
      await loadTrash();
      addToast({
        title: 'Restored from Trash',
        desc: `"${restored.name}" returned to original location`,
        type: 'success',
      });
    } catch (err: any) {
      const errMsg = err?.message || String(err || '');
      if (err?.code === 'FEATURE_DISABLED' || errMsg.includes('FEATURE_DISABLED')) {
        addToast({
          title: 'Feature Disabled (FEATURE_DISABLED)',
          desc: 'Restoring from trash is disabled by server policy.',
          type: 'warning',
        });
      } else {
        addToast({ title: 'Failed to restore', desc: errMsg, type: 'error' });
      }
    }
  }, [addToast]);

  const emptyTrash = useCallback(async () => {
    try {
      const count = await tealdrive.files.trash.empty();
      await loadTrash();
      addToast({
        title: 'Trash emptied',
        desc: `Permanently removed ${count} items`,
        type: 'info',
      });
    } catch (err: any) {
      const errMsg = err?.message || String(err || '');
      if (err?.code === 'FEATURE_DISABLED' || errMsg.includes('FEATURE_DISABLED')) {
        addToast({
          title: 'Feature Disabled (FEATURE_DISABLED)',
          desc: 'Manual permanent deletion is disabled by server security policy. Files in Trash will be automatically purged after the retention period (21 days).',
          type: 'warning',
        });
      } else {
        addToast({ title: 'Failed to empty trash', desc: errMsg, type: 'error' });
      }
    }
  }, [addToast]);

  const deletePermanent = useCallback(async (id: string) => {
    try {
      await tealdrive.files.trash.deletePermanently({ id });
      await loadTrash();
      addToast({
        title: 'Deleted permanently',
        desc: 'Item wiped from disk pool',
        type: 'info',
      });
    } catch (err: any) {
      const errMsg = err?.message || String(err || '');
      if (err?.code === 'FEATURE_DISABLED' || errMsg.includes('FEATURE_DISABLED')) {
        addToast({
          title: 'Feature Disabled (FEATURE_DISABLED)',
          desc: 'Manual permanent deletion is disabled by server security policy. Files in Trash will be automatically purged after the retention period (21 days).',
          type: 'warning',
        });
      } else {
        addToast({ title: 'Failed to delete permanently', desc: errMsg, type: 'error' });
      }
    }
  }, [addToast]);

  const promptTrash = useCallback((singlePath?: string) => {
    let itemsToTrash: FileEntry[] = [];
    if (singlePath) {
      const found = entries.find((e) => e.path === singlePath);
      if (found) {
        itemsToTrash = [found];
      } else {
        const name = singlePath.split('/').pop() || 'item';
        itemsToTrash = [{
          id: singlePath,
          name,
          path: singlePath,
          isDirectory: false,
          size: 0,
          modified: '',
          mode: '0644',
          owner: '',
          group: '',
          mimeType: '',
          starred: false,
          revision: 1,
        }];
      }
    } else {
      itemsToTrash = entries.filter((e) => selectedIds.has(e.id));
    }

    if (itemsToTrash.length === 0) return;

    const count = itemsToTrash.length;
    const title = count === 1 ? `Move "${itemsToTrash[0].name}" to Trash?` : `Move ${count} items to Trash?`;
    const desc = count === 1
      ? 'This item will be moved to the Trash. You can restore it later from the Trash view.'
      : `These ${count} items will be moved to the Trash. You can restore them later from the Trash view.`;

    setDeleteConfirm({
      isOpen: true,
      title,
      description: desc,
      items: itemsToTrash.map((i) => ({
        name: i.name,
        isDirectory: i.isDirectory,
        size: i.size,
        path: i.path,
        mimeType: i.mimeType,
      })),
      isPermanent: false,
      confirmLabel: count === 1 ? 'Move to Trash' : `Move ${count} Items to Trash`,
      onConfirm: async () => {
        await trashSelectedOrPath(singlePath);
      },
    });
  }, [entries, selectedIds, trashSelectedOrPath]);

  const promptPermanentDelete = useCallback((file: FileEntry) => {
    setDeleteConfirm({
      isOpen: true,
      title: `Permanently delete "${file.name}"?`,
      description: 'This item will be immediately and irreversibly removed from storage.',
      items: [{
        name: file.name,
        isDirectory: file.isDirectory,
        size: file.size,
        path: file.originalPath || file.path,
        mimeType: file.mimeType,
      }],
      isPermanent: true,
      confirmLabel: 'Delete Permanently',
      onConfirm: async () => {
        await deletePermanent(file.id);
      },
    });
  }, [deletePermanent]);

  const promptEmptyTrash = useCallback(() => {
    if (trashEntries.length === 0) return;
    setDeleteConfirm({
      isOpen: true,
      title: 'Empty entire Trash bin?',
      description: `All ${trashEntries.length} items in the trash will be permanently wiped from the server storage pool.`,
      items: trashEntries.map((t) => ({
        name: t.name,
        isDirectory: t.isDirectory,
        size: t.size,
        path: t.originalPath || t.path,
        mimeType: t.mimeType,
      })),
      isPermanent: true,
      confirmLabel: `Empty Trash (${trashEntries.length})`,
      onConfirm: async () => {
        await emptyTrash();
      },
    });
  }, [trashEntries, emptyTrash]);

  const promptCustomDelete = useCallback((options: Omit<DeleteConfirmModalState, 'isOpen'>) => {
    setDeleteConfirm({
      ...options,
      isOpen: true,
    });
  }, []);

  const toggleStar = useCallback(async (path: string) => {
    try {
      const updated = await tealdrive.files.star({ path });
      setEntries((prev) => prev.map((e) => (e.path === path ? updated : e)));
    } catch (err: any) {
      addToast({ title: 'Failed to star item', desc: err.message, type: 'error' });
    }
  }, [addToast]);

  const downloadFile = useCallback(async (path: string) => {
    try {
      const { filename, blobUrl } = await tealdrive.files.download({ path });
      const a = document.createElement('a');
      a.href = blobUrl;
      a.download = filename;
      document.body.appendChild(a);
      a.click();
      document.body.removeChild(a);
      if (blobUrl && blobUrl.startsWith('blob:')) {
        URL.revokeObjectURL(blobUrl);
      }
      addToast({ title: 'Download started', desc: filename, type: 'info' });
    } catch (err: any) {
      addToast({ title: 'Download error', desc: err.message, type: 'error' });
    }
  }, [addToast]);

  // Undo
  const undoOperation = useCallback(async (opId: string) => {
    try {
      const res = await tealdrive.files.undo.apply({ operationId: opId });
      if (res.success) {
        addToast({ title: 'Undo successful', desc: res.message, type: 'success' });
        await refreshCurrentDirectory();
        await loadAudit();
      } else {
        addToast({ title: 'Cannot undo operation', desc: res.message, type: 'warning' });
      }
    } catch (err: any) {
      addToast({ title: 'Undo error', desc: err.message, type: 'error' });
    }
  }, [refreshCurrentDirectory, addToast]);

  // Uploads
  const uploadFiles = useCallback((files: FileList | File[], targetDir?: string) => {
    const baseDir = targetDir || currentPath;
    const fileList = Array.from(files);

    fileList.forEach((file: any) => {
      let dir = baseDir;
      let effectiveName = file.name;

      // Handle folder uploads via webkitdirectory
      if (file.webkitRelativePath) {
        const parts = file.webkitRelativePath.split('/');
        effectiveName = parts.pop() || file.name;
        const subDir = parts.join('/');
        if (subDir) {
          dir = baseDir === '/' ? `/${subDir}` : `${baseDir}/${subDir}`;
        }
      }

      const fileId = `upload_${Date.now()}_${Math.random().toString(36).substring(2, 6)}`;
      const controller = new AbortController();

      const newUpload: UploadProgress = {
        fileId,
        fileName: effectiveName,
        directory: dir,
        size: file.size || 1024 * 128,
        uploadedBytes: 0,
        progress: 0,
        speedBytesPerSec: 1024 * 1024 * 3.8,
        status: 'uploading',
        abortController: controller,
      };

      setUploads((prev) => [newUpload, ...prev]);

      // Read content if text
      const reader = new FileReader();
      const isText =
        file.type?.startsWith('text/') ||
        /\.(txt|md|rs|ts|js|json|toml|sh|csv|sql)$/i.test(effectiveName);

      const startUpload = async (contentStr?: string) => {
        try {
          await tealdrive.files.upload({
            directory: dir,
            file: {
              name: effectiveName,
              size: file.size,
              type: file.type,
              content: contentStr,
              rawFile: file,
            },
            signal: controller.signal,
            onProgress: (p) => {
              setUploads((prev) =>
                prev.map((item) =>
                  item.fileId === fileId
                    ? {
                        ...item,
                        progress: p.progress,
                        uploadedBytes: p.uploadedBytes,
                        speedBytesPerSec: p.speedBytesPerSec,
                      }
                    : item
                )
              );
            },
          });

          setUploads((prev) =>
            prev.map((item) =>
              item.fileId === fileId ? { ...item, progress: 100, status: 'completed' } : item
            )
          );

          await refreshCurrentDirectory();
          addToast({
            title: 'Upload complete',
            desc: `"${effectiveName}" uploaded to ${dir}`,
            type: 'success',
          });
        } catch (err: any) {
          if (err.name === 'AbortError') {
            setUploads((prev) =>
              prev.map((item) =>
                item.fileId === fileId ? { ...item, status: 'aborted' } : item
              )
            );
            addToast({ title: 'Upload cancelled', desc: file.name, type: 'info' });
          } else {
            setUploads((prev) =>
              prev.map((item) =>
                item.fileId === fileId ? { ...item, status: 'error', error: err.message } : item
              )
            );
            addToast({ title: 'Upload failed', desc: err.message, type: 'error' });
          }
        }
      };

      if (isText) {
        reader.onload = () => startUpload(reader.result as string);
        reader.onerror = () => startUpload();
        reader.readAsText(file);
      } else {
        startUpload();
      }
    });
  }, [currentPath, refreshCurrentDirectory, addToast]);

  const cancelUpload = useCallback((fileId: string) => {
    setUploads((prev) => {
      const item = prev.find((u) => u.fileId === fileId);
      if (item && item.abortController && item.status === 'uploading') {
        item.abortController.abort();
      }
      return prev.map((u) => (u.fileId === fileId ? { ...u, status: 'aborted' } : u));
    });
  }, []);

  const clearCompletedUploads = useCallback(() => {
    setUploads((prev) => prev.filter((u) => u.status === 'uploading'));
  }, []);

  // Global keyboard shortcuts
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      // Cmd+K or Ctrl+K for omnisearch
      if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === 'k') {
        e.preventDefault();
        setOmnisearchOpen((prev) => !prev);
        return;
      }

      // If active element is an input, textarea or contenteditable, don't trigger explorer shortcuts
      const tag = (document.activeElement?.tagName || '').toLowerCase();
      if (tag === 'input' || tag === 'textarea' || document.activeElement?.getAttribute('contenteditable')) {
        return;
      }

      // Spacebar for QuickLook preview
      if (e.code === 'Space') {
        if (selectedIds.size === 1) {
          e.preventDefault();
          const selectedId = Array.from(selectedIds)[0];
          const file = entries.find((e) => e.id === selectedId);
          if (file) {
            setPreviewFile(file);
          }
        }
      }

      // F2 for rename
      if (e.key === 'F2') {
        if (selectedIds.size === 1) {
          e.preventDefault();
          const selectedId = Array.from(selectedIds)[0];
          const file = entries.find((e) => e.id === selectedId);
          if (file) {
            setRenameModalFile(file);
          }
        }
      }

      // Delete or Backspace
      if (e.key === 'Delete') {
        if (selectedIds.size > 0) {
          e.preventDefault();
          promptTrash();
        }
      }

      // Escape to deselect
      if (e.key === 'Escape') {
        if (selectedIds.size > 0) {
          e.preventDefault();
          clearSelection();
        }
      }

      // Cmd+A or Ctrl+A for Select All (only when in drive view and not in modal)
      if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === 'a') {
        e.preventDefault();
        selectAll();
      }
    };

    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [selectedIds, entries, promptTrash, clearSelection, selectAll]);

  const value = useMemo(
    () => ({
      session,
      clientState,
      quota,
      loading,
      activeView,
      setActiveView,
      login,
      logout,
      currentPath,
      entries,
      recentEntries,
      trashEntries,
      auditEntries,
      goToPath,
      refreshCurrentDirectory,
      selectedIds,
      toggleSelect,
      selectAll,
      clearSelection,
      viewMode,
      setViewMode,
      filter,
      setFilter,
      sortField,
      setSortField,
      sortOrder,
      toggleSortOrder,
      createFolder,
      renameItem,
      moveItem,
      copyItem,
      chmodItem,
      trashSelectedOrPath,
      restoreFromTrash,
      emptyTrash,
      deletePermanent,
      toggleStar,
      downloadFile,
      undoOperation,
      uploads,
      uploadFiles,
      cancelUpload,
      clearCompletedUploads,
      previewFile,
      setPreviewFile,
      editorFile,
      setEditorFile,
      chmodFile,
      setChmodFile,
      renameModalFile,
      setRenameModalFile,
      moveModalFile,
      setMoveModalFile,
      newFolderOpen,
      setNewFolderOpen,
      omnisearchOpen,
      setOmnisearchOpen,
      sidebarMobileOpen,
      setSidebarMobileOpen,
      deleteConfirm,
      setDeleteConfirm,
      promptTrash,
      promptPermanentDelete,
      promptEmptyTrash,
      promptCustomDelete,
      toasts,
      addToast,
      dismissToast,
    }),
    [
      session,
      clientState,
      quota,
      loading,
      activeView,
      login,
      logout,
      currentPath,
      entries,
      recentEntries,
      trashEntries,
      auditEntries,
      goToPath,
      refreshCurrentDirectory,
      selectedIds,
      toggleSelect,
      selectAll,
      clearSelection,
      viewMode,
      filter,
      sortField,
      sortOrder,
      toggleSortOrder,
      createFolder,
      renameItem,
      moveItem,
      copyItem,
      chmodItem,
      trashSelectedOrPath,
      restoreFromTrash,
      emptyTrash,
      deletePermanent,
      toggleStar,
      downloadFile,
      undoOperation,
      uploads,
      uploadFiles,
      cancelUpload,
      clearCompletedUploads,
      previewFile,
      editorFile,
      chmodFile,
      renameModalFile,
      moveModalFile,
      newFolderOpen,
      omnisearchOpen,
      sidebarMobileOpen,
      deleteConfirm,
      promptTrash,
      promptPermanentDelete,
      promptEmptyTrash,
      promptCustomDelete,
      toasts,
      addToast,
      dismissToast,
    ]
  );

  return <TealDriveContext.Provider value={value}>{children}</TealDriveContext.Provider>;
};

export function useTealDrive(): TealDriveContextType {
  const context = useContext(TealDriveContext);
  if (!context) {
    throw new Error('useTealDrive must be used within a TealDriveProvider');
  }
  return context;
}
