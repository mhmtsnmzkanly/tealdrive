import React, { createContext, useContext, useState, useEffect, useCallback, useMemo, useRef } from 'react';
import { tealdrive } from '../lib/tealdrive.ts';
import { getFileCategory } from '../utils/formatters.ts';
import { joinPath, parentPath, copyName } from '../utils/paths.ts';
import type {
  FileEntry,
  Session,
  ClientState,
  StorageQuota,
  UploadProgress,
  AuditEntry,
} from '../sdk/types.ts';
import { t } from '../i18n/index.ts';

export type ViewType = 'drive' | 'recent' | 'trash' | 'audit' | 'admin';
export type FilterType = 'all' | 'folders' | 'documents' | 'images' | 'media' | 'code' | 'archives';
export type SortField = 'name' | 'size' | 'modified';

export interface UndoTarget {
  operationId: string;
  expectedState: string;
}

interface ToastData {
  id: string;
  title: string;
  desc?: string;
  type: 'info' | 'success' | 'warning' | 'error';
  undo?: UndoTarget;
}

export interface SelectModifiers {
  /** Ctrl/Cmd: add or remove one item. */
  toggle?: boolean;
  /** Shift: select everything between the anchor and this item. */
  range?: boolean;
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
  changePassword: (currentPassword: string, newPassword: string) => Promise<void>;

  // Navigation & directory files
  currentPath: string;
  entries: FileEntry[];
  /** Entries after the active filter and sort; what the explorer shows. */
  visibleEntries: FileEntry[];
  recentEntries: FileEntry[];
  trashEntries: FileEntry[];
  auditEntries: AuditEntry[];
  /** True when older history exists beyond auditEntries. */
  auditHasMore: boolean;
  loadMoreAudit: () => Promise<void>;
  goToPath: (path: string) => Promise<void>;
  refreshCurrentDirectory: () => Promise<void>;

  // Selection & views
  selectedIds: Set<string>;
  toggleSelect: (id: string, additive?: boolean) => void;
  selectItem: (id: string, modifiers?: SelectModifiers) => void;
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
  /** Flips the star, or sets it when `starred` is given. */
  toggleStar: (path: string, starred?: boolean) => Promise<void>;
  downloadFile: (path: string) => Promise<void>;
  compressItems: (paths: string[], archiveName: string, format: 'zip' | 'tar.gz') => Promise<void>;
  extractArchive: (path: string) => Promise<void>;

  // Undo system
  undoOperation: (target: UndoTarget) => Promise<void>;

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

/** Uploads that run at once; the rest wait in the dock as "queued". */
const UPLOAD_CONCURRENCY = 3;

function isAbort(err: any): boolean {
  return err?.name === 'AbortError' || err?.code === 'ABORTED' || err?.code === 'CANCELLED';
}

function errorMessage(err: any): string {
  return err?.message || err?.code || String(err || t('Unknown error'));
}

function isFeatureDisabled(err: any): boolean {
  return err?.code === 'FEATURE_DISABLED' || errorMessage(err).includes('FEATURE_DISABLED');
}

/** Epoch seconds a few minutes back, so modest clock skew still matches our own journal record. */
function journalSince(): number {
  return Math.floor(Date.now() / 1000) - 300;
}

interface QueuedUpload {
  fileId: string;
  file: File;
  name: string;
  dir: string;
  batch: UploadBatch;
}

interface UploadBatch {
  total: number;
  done: number;
  failed: number;
  cancelled: number;
}

export const TealDriveProvider: React.FC<{ children: React.ReactNode }> = ({ children }) => {
  const [session, setSession] = useState<Session | null>(null);
  const [clientState, setClientState] = useState<ClientState>(tealdrive.getState());
  const [quota, setQuota] = useState<StorageQuota | null>(null);
  const [loading, setLoading] = useState(true);
  const [activeView, setActiveViewState] = useState<ViewType>('drive');

  const [currentPath, setCurrentPath] = useState<string>('/');
  const [entries, setEntries] = useState<FileEntry[]>([]);
  const [recentEntries, setRecentEntries] = useState<FileEntry[]>([]);
  const [trashEntries, setTrashEntries] = useState<FileEntry[]>([]);
  const [auditEntries, setAuditEntries] = useState<AuditEntry[]>([]);
  const [auditCursor, setAuditCursor] = useState<string | null>(null);

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

  // Refs hold the latest values for async work that outlives a render.
  const currentPathRef = useRef('/');
  const activeViewRef = useRef<ViewType>('drive');
  const listAbortRef = useRef<AbortController | null>(null);
  const listSeqRef = useRef(0);
  const viewSeqRef = useRef(0);
  const selectionAnchorRef = useRef<string | null>(null);
  const toastTimersRef = useRef(new Map<string, ReturnType<typeof setTimeout>>());
  const uploadQueueRef = useRef<QueuedUpload[]>([]);
  const uploadControllersRef = useRef(new Map<string, AbortController>());
  const signingOutRef = useRef(false);

  const dismissToast = useCallback((id: string) => {
    const timer = toastTimersRef.current.get(id);
    if (timer) clearTimeout(timer);
    toastTimersRef.current.delete(id);
    setToasts((prev) => prev.filter((t) => t.id !== id));
  }, []);

  const addToast = useCallback((toast: Omit<ToastData, 'id'>) => {
    const id = `toast_${Date.now()}_${Math.random().toString(36).substring(2, 6)}`;
    setToasts((prev) => [...prev, { ...toast, id }]);
    // Undo offers stay a little longer so they can be reached.
    const timer = setTimeout(() => dismissToast(id), toast.undo ? 10000 : 6000);
    toastTimersRef.current.set(id, timer);
  }, [dismissToast]);

  useEffect(() => {
    const timers = toastTimersRef.current;
    return () => {
      timers.forEach((timer) => clearTimeout(timer));
      timers.clear();
    };
  }, []);

  const refreshQuota = useCallback(async () => {
    try {
      setQuota(await tealdrive.files.quota.get());
    } catch {
      // Quota is advisory; keep the last known value.
    }
  }, []);

  // Loads a directory. Only the newest request may update state, so a slow
  // older listing never overwrites the folder the user moved on to.
  const loadDirectory = useCallback(async (path: string): Promise<boolean> => {
    listAbortRef.current?.abort();
    const controller = new AbortController();
    listAbortRef.current = controller;
    const seq = ++listSeqRef.current;
    try {
      const res = await tealdrive.files.list({ path, signal: controller.signal });
      if (seq !== listSeqRef.current) return false;
      const changedDir = currentPathRef.current !== path;
      currentPathRef.current = path;
      setCurrentPath(path);
      setEntries(res.entries);
      if (changedDir) {
        setSelectedIds(new Set());
        selectionAnchorRef.current = null;
      } else {
        // Keep the selection on a refresh, minus anything that vanished.
        const ids = new Set(res.entries.map((e) => e.id));
        setSelectedIds((prev) => {
          const next = new Set([...prev].filter((id) => ids.has(id)));
          return next.size === prev.size ? prev : next;
        });
      }
      return true;
    } catch (err: any) {
      if (seq !== listSeqRef.current || isAbort(err)) return false;
      addToast({ title: t('Navigation error'), desc: errorMessage(err), type: 'error' });
      return false;
    } finally {
      if (listAbortRef.current === controller) listAbortRef.current = null;
    }
  }, [addToast]);

  // Each secondary view load is tagged so a late reply cannot land in another view.
  const loadView = useCallback(async (view: ViewType) => {
    const seq = ++viewSeqRef.current;
    const current = () => seq === viewSeqRef.current && activeViewRef.current === view;
    try {
      if (view === 'recent') {
        const recent = await tealdrive.files.getRecent(20);
        if (current()) setRecentEntries(recent);
      } else if (view === 'trash') {
        const trash = await tealdrive.files.trash.list();
        if (current()) setTrashEntries(trash);
      } else if (view === 'audit') {
        const page = await tealdrive.files.undo.list({ limit: 50 });
        if (current()) {
          setAuditEntries(page.entries);
          setAuditCursor(page.nextCursor);
        }
      }
    } catch (err: any) {
      if (!current()) return;
      const titles: Record<string, string> = {
        recent: t('Recent files unavailable'),
        trash: t('Trash unavailable'),
        audit: t('History unavailable'),
      };
      addToast({
        title: titles[view] ?? t('Load error'),
        desc: isFeatureDisabled(err) ? t('This feature is disabled by server configuration.') : errorMessage(err),
        type: isFeatureDisabled(err) ? 'warning' : 'error',
      });
    }
  }, [addToast]);

  const refreshCurrentDirectory = useCallback(async () => {
    const view = activeViewRef.current;
    if (view === 'drive') {
      await loadDirectory(currentPathRef.current);
    } else if (view !== 'admin') {
      await loadView(view);
    }
  }, [loadDirectory, loadView]);

  const setActiveView = useCallback((view: ViewType) => {
    activeViewRef.current = view;
    setActiveViewState(view);
    if (view === 'drive') {
      loadDirectory(currentPathRef.current);
    } else {
      setSelectedIds(new Set());
      loadView(view);
    }
  }, [loadDirectory, loadView]);

  const goToPath = useCallback(async (path: string) => {
    activeViewRef.current = 'drive';
    setActiveViewState('drive');
    await loadDirectory(path);
  }, [loadDirectory]);

  // Forgets everything the previous account could see.
  const resetAccountState = useCallback(() => {
    listAbortRef.current?.abort();
    listSeqRef.current++;
    viewSeqRef.current++;
    uploadQueueRef.current = [];
    uploadControllersRef.current.forEach((c) => c.abort());
    uploadControllersRef.current.clear();
    currentPathRef.current = '/';
    activeViewRef.current = 'drive';
    selectionAnchorRef.current = null;
    setSession(null);
    setQuota(null);
    setActiveViewState('drive');
    setCurrentPath('/');
    setEntries([]);
    setRecentEntries([]);
    setTrashEntries([]);
    setAuditEntries([]);
    setAuditCursor(null);
    setSelectedIds(new Set());
    setUploads([]);
    setPreviewFile(null);
    setEditorFile(null);
    setChmodFile(null);
    setRenameModalFile(null);
    setMoveModalFile(null);
    setNewFolderOpen(false);
    setOmnisearchOpen(false);
    setDeleteConfirm(null);
  }, []);

  // Opens a signed-in session: empty drive at the root, then the listing.
  const enterSession = useCallback(async (sess: Session) => {
    setSession(sess);
    if (sess.passwordChangeRequired) return;
    await Promise.all([loadDirectory('/'), refreshQuota()]);
  }, [loadDirectory, refreshQuota]);

  // Fetch initial session
  useEffect(() => {
    let cancelled = false;
    (async () => {
      try {
        const sess = await tealdrive.auth.getSession();
        if (cancelled) return;
        if (sess) {
          tealdrive.handshake().catch(() => {});
          await enterSession(sess);
        }
      } catch (err: any) {
        console.error('Initialization error:', err);
      } finally {
        if (!cancelled) setLoading(false);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [enterSession]);

  // Live subscriptions exist exactly while someone is signed in, however they signed in.
  const signedIn = session !== null;
  useEffect(() => {
    if (!signedIn) return;
    const offState = tealdrive.observeState((next) => setClientState(next));
    const offQuota = tealdrive.files.observeQuotaWarning((next) => setQuota(next));
    const offRevoked = tealdrive.onSessionRevoked(() => {
      if (signingOutRef.current) return;
      resetAccountState();
      addToast({
        title: t('Session ended'),
        desc: t('Your session expired or was revoked. Please sign in again.'),
        type: 'warning',
      });
    });
    return () => {
      offState();
      offQuota();
      offRevoked();
    };
  }, [signedIn, resetAccountState, addToast]);

  const login = useCallback(async (username: string, password: string) => {
    // Errors propagate to the login form; the form stays mounted to show them.
    const sess = await tealdrive.auth.login({ username, password });
    await enterSession(sess);
    if (sess.passwordChangeRequired) return;
    addToast({
      title: t('Signed In'),
      desc: t('Welcome back, {username}!', { username: sess.username }),
      type: 'success',
    });
  }, [addToast, enterSession]);

  const logout = useCallback(async () => {
    signingOutRef.current = true;
    try {
      await tealdrive.auth.logout();
    } catch {
      // The local state is cleared regardless.
    } finally {
      signingOutRef.current = false;
    }
    resetAccountState();
    addToast({
      title: t('Signed Out'),
      desc: t('You have been signed out successfully.'),
      type: 'info',
    });
  }, [addToast, resetAccountState]);

  const changePassword = useCallback(async (currentPassword: string, newPassword: string) => {
    signingOutRef.current = true;
    try {
      await tealdrive.auth.changePassword({ currentPassword, newPassword });
    } finally {
      signingOutRef.current = false;
    }
    // The server revokes the session after a password change.
    resetAccountState();
    addToast({
      title: t('Password changed'),
      desc: t('Sign in with your new password.'),
      type: 'success',
    });
  }, [addToast, resetAccountState]);

  const visibleEntries = useMemo(() => {
    const filtered = entries.filter((file) => {
      if (filter === 'all') return true;
      if (filter === 'folders') return file.isDirectory;
      const cat = getFileCategory(file.name, file.isDirectory, file.mimeType);
      if (filter === 'documents') return cat === 'document';
      if (filter === 'images') return cat === 'image';
      if (filter === 'media') return cat === 'media';
      if (filter === 'code') return cat === 'code';
      if (filter === 'archives') return cat === 'archive';
      return true;
    });
    return filtered.sort((a, b) => {
      // Directories always come first
      if (a.isDirectory && !b.isDirectory) return -1;
      if (!a.isDirectory && b.isDirectory) return 1;

      let cmp = 0;
      if (sortField === 'name') {
        cmp = a.name.localeCompare(b.name);
      } else if (sortField === 'size') {
        cmp = (a.size || 0) - (b.size || 0);
      } else if (sortField === 'modified') {
        cmp = new Date(a.modified).getTime() - new Date(b.modified).getTime();
      }

      return sortOrder === 'asc' ? cmp : -cmp;
    });
  }, [entries, filter, sortField, sortOrder]);

  // A hidden item must never stay selected: bulk actions would hit files the user cannot see.
  useEffect(() => {
    const visible = new Set(visibleEntries.map((e) => e.id));
    setSelectedIds((prev) => {
      const next = new Set([...prev].filter((id) => visible.has(id)));
      return next.size === prev.size ? prev : next;
    });
  }, [visibleEntries]);

  // Selection
  const selectItem = useCallback((id: string, modifiers: SelectModifiers = {}) => {
    const order = visibleEntries.map((e) => e.id);
    setSelectedIds((prev) => {
      const anchor = selectionAnchorRef.current;
      if (modifiers.range && anchor && order.includes(anchor) && order.includes(id)) {
        const [from, to] = [order.indexOf(anchor), order.indexOf(id)].sort((a, b) => a - b);
        const next = modifiers.toggle ? new Set(prev) : new Set<string>();
        order.slice(from, to + 1).forEach((x) => next.add(x));
        return next;
      }
      selectionAnchorRef.current = id;
      if (modifiers.toggle) {
        const next = new Set(prev);
        if (next.has(id)) next.delete(id);
        else next.add(id);
        return next;
      }
      // A plain click on the only selected item clears it; otherwise it becomes the selection.
      if (prev.size === 1 && prev.has(id)) return new Set();
      return new Set([id]);
    });
  }, [visibleEntries]);

  const toggleSelect = useCallback((id: string, additive?: boolean) => {
    selectItem(id, { toggle: Boolean(additive) });
  }, [selectItem]);

  const selectAll = useCallback(() => {
    setSelectedIds(new Set(visibleEntries.map((e) => e.id)));
  }, [visibleEntries]);

  const clearSelection = useCallback(() => {
    selectionAnchorRef.current = null;
    setSelectedIds(new Set());
  }, []);

  const toggleSortOrder = useCallback(() => {
    setSortOrder((prev) => (prev === 'asc' ? 'desc' : 'asc'));
  }, []);

  // Looks up the undo candidate the server recorded for an operation we just made.
  const findUndo = useCallback(async (action: string, path: string, since: number): Promise<UndoTarget | undefined> => {
    const entry = await tealdrive.files.undo.findLatest({ action, path, since });
    return entry?.expectedState ? { operationId: entry.id, expectedState: entry.expectedState } : undefined;
  }, []);

  // CRUD handlers
  const createFolder = useCallback(async (name: string) => {
    const dir = currentPathRef.current;
    try {
      await tealdrive.files.createFolder({ path: joinPath(dir, name) });
      await refreshCurrentDirectory();
      addToast({
        title: t('Folder created'),
        desc: t('"{name}" added to {dir}', { name, dir }),
        type: 'success',
      });
    } catch (err: any) {
      addToast({ title: t('Failed to create folder'), desc: errorMessage(err), type: 'error' });
    }
  }, [refreshCurrentDirectory, addToast]);

  const renameItem = useCallback(async (path: string, newName: string) => {
    const since = journalSince();
    try {
      await tealdrive.files.rename({ path, newName });
      const [undo] = await Promise.all([findUndo('rename', path, since), refreshCurrentDirectory()]);
      addToast({
        title: t('Item renamed'),
        desc: t('Renamed to "{newName}"', { newName }),
        type: 'success',
        undo,
      });
    } catch (err: any) {
      addToast({ title: t('Failed to rename'), desc: errorMessage(err), type: 'error' });
    }
  }, [refreshCurrentDirectory, addToast, findUndo]);

  const moveItem = useCallback(async (sourcePath: string, targetDirPath: string) => {
    const since = journalSince();
    try {
      await tealdrive.files.move({ sourcePath, targetPath: joinPath(targetDirPath, sourcePath.split('/').pop() || '') });
      const [undo] = await Promise.all([findUndo('move', sourcePath, since), refreshCurrentDirectory()]);
      addToast({
        title: t('Item moved'),
        desc: t('Moved into {targetDirPath}', { targetDirPath }),
        type: 'success',
        undo,
      });
    } catch (err: any) {
      addToast({ title: t('Failed to move'), desc: errorMessage(err), type: 'error' });
    }
  }, [refreshCurrentDirectory, addToast, findUndo]);

  const copyItem = useCallback(async (sourcePath: string, targetDirPath: string) => {
    const name = sourcePath.split('/').pop() || '';
    // The server takes the full destination path; duplicating in place needs a free name.
    let targetName = name;
    if (parentPath(sourcePath) === targetDirPath && targetDirPath === currentPathRef.current) {
      targetName = copyName(name, new Set(entries.map((e) => e.name)));
    }
    try {
      await tealdrive.files.copy({ sourcePath, targetPath: joinPath(targetDirPath, targetName) });
      await Promise.all([refreshCurrentDirectory(), refreshQuota()]);
      addToast({
        title: t('Item duplicated'),
        desc: t('Created "{targetName}" in {targetDirPath}', { targetName, targetDirPath }),
        type: 'success',
      });
    } catch (err: any) {
      addToast({ title: t('Failed to copy'), desc: errorMessage(err), type: 'error' });
    }
  }, [entries, refreshCurrentDirectory, refreshQuota, addToast]);

  const chmodItem = useCallback(async (path: string, mode: string) => {
    const since = journalSince();
    try {
      await tealdrive.files.chmod({ path, mode });
      const [undo] = await Promise.all([findUndo('chmod', path, since), refreshCurrentDirectory()]);
      addToast({
        title: t('Permissions updated'),
        desc: t('UNIX mode set to {mode}', { mode }),
        type: 'success',
        undo,
      });
    } catch (err: any) {
      addToast({ title: t('Failed to set permissions'), desc: errorMessage(err), type: 'error' });
    }
  }, [refreshCurrentDirectory, addToast, findUndo]);

  const trashSelectedOrPath = useCallback(async (singlePath?: string) => {
    const pathsToTrash = singlePath
      ? [singlePath]
      : entries.filter((e) => selectedIds.has(e.id)).map((e) => e.path);
    if (pathsToTrash.length === 0) return;

    const since = journalSince();
    const failures: Array<{ path: string; err: any }> = [];
    let moved = 0;
    for (const p of pathsToTrash) {
      try {
        await tealdrive.files.trash.move({ path: p });
        moved++;
      } catch (err: any) {
        failures.push({ path: p, err });
        // Every remaining item would fail the same way.
        if (isFeatureDisabled(err)) break;
      }
    }
    clearSelection();
    // Refresh in every case: a partial failure still moved some items.
    const [undo] = await Promise.all([
      moved === 1 && pathsToTrash.length === 1 ? findUndo('trash', pathsToTrash[0], since) : Promise.resolve(undefined),
      refreshCurrentDirectory(),
      refreshQuota(),
    ]);

    if (failures.length === 0) {
      addToast({
        title: moved > 1 ? t('{moved} items moved to Trash', { moved }) : t('Moved to Trash'),
        desc: t('Files can be restored from the Trash view anytime'),
        type: 'info',
        undo,
      });
    } else if (moved === 0 && isFeatureDisabled(failures[0].err)) {
      addToast({
        title: t('Feature Disabled (FEATURE_DISABLED)'),
        desc: t('Moving items to trash is disabled by server configuration.'),
        type: 'warning',
      });
    } else {
      const first = failures[0];
      addToast({
        title: moved > 0 ? t('{moved} of {count} items moved to Trash', { moved, count: pathsToTrash.length }) : t('Error deleting items'),
        desc: t('{count} failed. {name}: {error}', {
          count: failures.length,
          name: first.path.split('/').pop() ?? '',
          error: errorMessage(first.err),
        }),
        type: 'error',
      });
    }
  }, [entries, selectedIds, clearSelection, refreshCurrentDirectory, refreshQuota, addToast, findUndo]);

  const restoreFromTrash = useCallback(async (id: string) => {
    try {
      const item = trashEntries.find((t) => t.id === id);
      await tealdrive.files.trash.restore({ id });
      await Promise.all([loadView('trash'), refreshQuota()]);
      addToast({
        title: t('Restored from Trash'),
        desc: item
          ? t('"{name}" returned to {path}', { name: item.name, path: item.originalPath ?? item.path })
          : t('The item returned to its original location.'),
        type: 'success',
      });
    } catch (err: any) {
      if (isFeatureDisabled(err)) {
        addToast({
          title: t('Feature Disabled (FEATURE_DISABLED)'),
          desc: t('Restoring from trash is disabled by server policy.'),
          type: 'warning',
        });
      } else {
        addToast({ title: t('Failed to restore'), desc: errorMessage(err), type: 'error' });
      }
    }
  }, [trashEntries, loadView, refreshQuota, addToast]);

  const permanentDeleteDisabledToast = useCallback(() => {
    addToast({
      title: t('Feature Disabled (FEATURE_DISABLED)'),
      desc: t('Manual permanent deletion is disabled by server security policy. Files in Trash are purged automatically after the retention period.'),
      type: 'warning',
    });
  }, [addToast]);

  const emptyTrash = useCallback(async () => {
    try {
      const count = await tealdrive.files.trash.empty();
      addToast({
        title: t('Trash emptied'),
        desc: t('Permanently removed {count} items', { count }),
        type: 'info',
      });
    } catch (err: any) {
      if (isFeatureDisabled(err) && !err.removed) {
        permanentDeleteDisabledToast();
      } else {
        addToast({
          title: err.removed ? t('Trash partly emptied ({removed} removed)', { removed: err.removed }) : t('Failed to empty trash'),
          desc: errorMessage(err),
          type: 'error',
        });
      }
    } finally {
      await Promise.all([loadView('trash'), refreshQuota()]);
    }
  }, [addToast, loadView, refreshQuota, permanentDeleteDisabledToast]);

  const deletePermanent = useCallback(async (id: string) => {
    try {
      await tealdrive.files.trash.deletePermanently({ id });
      await Promise.all([loadView('trash'), refreshQuota()]);
      addToast({
        title: t('Deleted permanently'),
        desc: t('Item wiped from disk pool'),
        type: 'info',
      });
    } catch (err: any) {
      if (isFeatureDisabled(err)) {
        permanentDeleteDisabledToast();
      } else {
        addToast({ title: t('Failed to delete permanently'), desc: errorMessage(err), type: 'error' });
      }
    }
  }, [addToast, loadView, refreshQuota, permanentDeleteDisabledToast]);

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
          revision: null,
        }];
      }
    } else {
      itemsToTrash = entries.filter((e) => selectedIds.has(e.id));
    }

    if (itemsToTrash.length === 0) return;

    const count = itemsToTrash.length;
    const title = count === 1 ? t('Move "{name}" to Trash?', { name: itemsToTrash[0].name }) : t('Move {count} items to Trash?', { count });
    const desc = count === 1
      ? t('This item will be moved to the Trash. You can restore it later from the Trash view.')
      : t('These {count} items will be moved to the Trash. You can restore them later from the Trash view.', { count });

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
      confirmLabel: count === 1 ? t('Move to Trash') : t('Move {count} Items to Trash', { count }),
      onConfirm: async () => {
        await trashSelectedOrPath(singlePath);
      },
    });
  }, [entries, selectedIds, trashSelectedOrPath]);

  const promptPermanentDelete = useCallback((file: FileEntry) => {
    setDeleteConfirm({
      isOpen: true,
      title: t('Permanently delete "{name}"?', { name: file.name }),
      description: t('This item will be immediately and irreversibly removed from storage.'),
      items: [{
        name: file.name,
        isDirectory: file.isDirectory,
        size: file.size,
        path: file.originalPath || file.path,
        mimeType: file.mimeType,
      }],
      isPermanent: true,
      confirmLabel: t('Delete Permanently'),
      onConfirm: async () => {
        await deletePermanent(file.id);
      },
    });
  }, [deletePermanent]);

  const promptEmptyTrash = useCallback(() => {
    if (trashEntries.length === 0) return;
    setDeleteConfirm({
      isOpen: true,
      title: t('Empty entire Trash bin?'),
      description: t('All {count} items in the trash will be permanently wiped from the server storage pool.', { count: trashEntries.length }),
      items: trashEntries.map((t) => ({
        name: t.name,
        isDirectory: t.isDirectory,
        size: t.size,
        path: t.originalPath || t.path,
        mimeType: t.mimeType,
      })),
      isPermanent: true,
      confirmLabel: t('Empty Trash ({count})', { count: trashEntries.length }),
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

  const toggleStar = useCallback(async (path: string, starred?: boolean) => {
    try {
      const updated = await tealdrive.files.star({ path, starred });
      const apply = (list: FileEntry[]) =>
        list.map((e) => (e.path === path ? { ...e, starred: updated.starred } : e));
      setEntries(apply);
      setRecentEntries(apply);
    } catch (err: any) {
      addToast({ title: t('Failed to star item'), desc: errorMessage(err), type: 'error' });
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
      addToast({ title: t('Download started'), desc: filename, type: 'info' });
    } catch (err: any) {
      addToast({ title: t('Download error'), desc: errorMessage(err), type: 'error' });
    }
  }, [addToast]);

  const loadMoreAudit = useCallback(async () => {
    if (!auditCursor) return;
    const seq = viewSeqRef.current;
    try {
      const page = await tealdrive.files.undo.list({ limit: 50, cursor: auditCursor });
      if (seq !== viewSeqRef.current) return;
      setAuditEntries((prev) => {
        const seen = new Set(prev.map((e) => e.id));
        return [...prev, ...page.entries.filter((e) => !seen.has(e.id))];
      });
      setAuditCursor(page.nextCursor);
    } catch (err: any) {
      addToast({ title: t('History unavailable'), desc: errorMessage(err), type: 'error' });
    }
  }, [auditCursor, addToast]);

  const compressItems = useCallback(async (paths: string[], archiveName: string, format: 'zip' | 'tar.gz') => {
    if (paths.length === 0) return;
    // The archive lands next to its sources, under a name not already taken there.
    const dir = parentPath(paths[0]);
    if (dir === currentPathRef.current) {
      const taken = new Set(entries.map((e) => e.name));
      if (taken.has(archiveName)) {
        const ext = format === 'zip' ? '.zip' : '.tar.gz';
        const stem = archiveName.slice(0, archiveName.length - ext.length);
        for (let i = 2; taken.has(archiveName); i++) archiveName = `${stem} (${i})${ext}`;
      }
    }
    const target = joinPath(dir, archiveName);
    try {
      await tealdrive.files.archives.create({ sources: paths, target, format });
      await Promise.all([refreshCurrentDirectory(), refreshQuota()]);
      addToast({ title: t('Archive created'), desc: `"${archiveName}" in ${dir}`, type: 'success' });
    } catch (err: any) {
      addToast({
        title: t('Failed to create archive'),
        desc: isFeatureDisabled(err) ? t('Archives are disabled by server configuration.') : errorMessage(err),
        type: isFeatureDisabled(err) ? 'warning' : 'error',
      });
    }
  }, [entries, refreshCurrentDirectory, refreshQuota, addToast]);

  const extractArchive = useCallback(async (path: string) => {
    const dir = parentPath(path);
    const name = path.split('/').pop() || 'archive';
    // Unpack into a new folder named after the archive, never over existing files.
    const base = name.replace(/\.(zip|tar\.gz|tgz)$/i, '') || 'archive';
    const taken = new Set(dir === currentPathRef.current ? entries.map((e) => e.name) : []);
    let folder = base;
    for (let i = 2; taken.has(folder); i++) folder = `${base} (${i})`;
    try {
      await tealdrive.files.archives.extract({ path, destination: joinPath(dir, folder) });
      await Promise.all([refreshCurrentDirectory(), refreshQuota()]);
      addToast({ title: t('Archive extracted'), desc: t('Into "{folder}"', { folder }), type: 'success' });
    } catch (err: any) {
      addToast({
        title: t('Failed to extract archive'),
        desc: isFeatureDisabled(err) ? t('Archive extraction is disabled by server configuration.') : errorMessage(err),
        type: isFeatureDisabled(err) ? 'warning' : 'error',
      });
    }
  }, [entries, refreshCurrentDirectory, refreshQuota, addToast]);

  // Undo
  const undoOperation = useCallback(async (target: UndoTarget) => {
    try {
      const res = await tealdrive.files.undo.apply(target);
      addToast({ title: t('Undo successful'), desc: res.message, type: 'success' });
    } catch (err: any) {
      addToast({
        title: t('Cannot undo operation'),
        desc: isFeatureDisabled(err)
          ? t('Undo is disabled by server configuration.')
          : err?.code === 'CONFLICT' || err?.status === 409
          ? t('The file changed after this operation, so it can no longer be reverted safely.')
          : errorMessage(err),
        type: 'warning',
      });
    } finally {
      // Covers the drive listing and, when it is open, the history view.
      await Promise.all([refreshCurrentDirectory(), refreshQuota()]);
    }
  }, [refreshCurrentDirectory, refreshQuota, addToast]);

  // Uploads
  const updateUpload = useCallback((fileId: string, patch: Partial<UploadProgress>) => {
    setUploads((prev) => prev.map((item) => (item.fileId === fileId ? { ...item, ...patch } : item)));
  }, []);

  const finishBatchItem = useCallback((job: QueuedUpload) => {
    const b = job.batch;
    if (b.done + b.failed + b.cancelled < b.total) return;
    if (b.total === 1) {
      if (b.done === 1) {
        addToast({ title: t('Upload complete'), desc: t('"{name}" uploaded to {dir}', { name: job.name, dir: job.dir }), type: 'success' });
      }
      return;
    }
    if (b.failed > 0) {
      addToast({
        title: t('{done} of {total} files uploaded', { done: b.done, total: b.total }),
        desc: t('{failed} failed, {cancelled} cancelled. See the upload panel for details.', {
          failed: b.failed,
          cancelled: b.cancelled,
        }),
        type: 'error',
      });
    } else if (b.done > 0) {
      addToast({
        title: t('Uploads complete'),
        desc: b.cancelled
          ? t('{done} files uploaded, {cancelled} cancelled', { done: b.done, cancelled: b.cancelled })
          : t('{done} files uploaded', { done: b.done }),
        type: 'success',
      });
    }
  }, [addToast]);

  const pumpUploads = useCallback(() => {
    while (uploadControllersRef.current.size < UPLOAD_CONCURRENCY && uploadQueueRef.current.length > 0) {
      const job = uploadQueueRef.current.shift()!;
      const controller = new AbortController();
      uploadControllersRef.current.set(job.fileId, controller);
      updateUpload(job.fileId, { status: 'uploading' });

      (async () => {
        try {
          await tealdrive.files.upload({
            directory: job.dir,
            file: { name: job.name, size: job.file.size, type: job.file.type, rawFile: job.file },
            signal: controller.signal,
            onProgress: (p) => {
              updateUpload(job.fileId, {
                progress: p.progress,
                uploadedBytes: p.uploadedBytes,
                speedBytesPerSec: p.speedBytesPerSec,
              });
            },
          });
          job.batch.done++;
          updateUpload(job.fileId, { progress: 100, uploadedBytes: job.file.size, status: 'completed' });
          // Refresh only when the upload landed in (or below) the folder on screen.
          const here = currentPathRef.current;
          if (
            activeViewRef.current === 'drive' &&
            (job.dir === here || job.dir.startsWith(here === '/' ? '/' : `${here}/`))
          ) {
            await loadDirectory(here);
          }
        } catch (err: any) {
          if (isAbort(err) || controller.signal.aborted) {
            job.batch.cancelled++;
            updateUpload(job.fileId, { status: 'aborted' });
          } else {
            job.batch.failed++;
            updateUpload(job.fileId, { status: 'error', error: errorMessage(err) });
            if (job.batch.total === 1) {
              addToast({ title: t('Upload failed'), desc: `${job.name}: ${errorMessage(err)}`, type: 'error' });
            }
          }
        } finally {
          uploadControllersRef.current.delete(job.fileId);
          finishBatchItem(job);
          if (uploadControllersRef.current.size === 0 && uploadQueueRef.current.length === 0) {
            refreshQuota();
          }
          pumpUploads();
        }
      })();
    }
  }, [updateUpload, loadDirectory, finishBatchItem, refreshQuota, addToast]);

  const uploadFiles = useCallback((files: FileList | File[], targetDir?: string) => {
    // The destination is fixed now; navigating away must not redirect the upload.
    const baseDir = targetDir || currentPathRef.current;
    const fileList = Array.from(files);
    if (fileList.length === 0) return;
    const batch: UploadBatch = { total: fileList.length, done: 0, failed: 0, cancelled: 0 };
    const created: UploadProgress[] = [];

    for (const file of fileList) {
      let dir = baseDir;
      let name = file.name;
      // Handle folder uploads via webkitdirectory
      const relative = (file as any).webkitRelativePath as string | undefined;
      if (relative) {
        const parts = relative.split('/');
        name = parts.pop() || file.name;
        const subDir = parts.join('/');
        if (subDir) dir = joinPath(baseDir, subDir);
      }
      const fileId = `upload_${Date.now()}_${Math.random().toString(36).substring(2, 8)}`;
      created.push({
        fileId,
        fileName: name,
        directory: dir,
        size: file.size,
        uploadedBytes: 0,
        progress: 0,
        speedBytesPerSec: 0,
        status: 'queued',
      });
      uploadQueueRef.current.push({ fileId, file, name, dir, batch });
    }

    setUploads((prev) => [...created.reverse(), ...prev]);
    pumpUploads();
  }, [pumpUploads]);

  const cancelUpload = useCallback((fileId: string) => {
    const queued = uploadQueueRef.current.find((j) => j.fileId === fileId);
    if (queued) {
      uploadQueueRef.current = uploadQueueRef.current.filter((j) => j.fileId !== fileId);
      queued.batch.cancelled++;
      updateUpload(fileId, { status: 'aborted' });
      finishBatchItem(queued);
      return;
    }
    // Running uploads report 'aborted' from their own catch block.
    uploadControllersRef.current.get(fileId)?.abort();
  }, [updateUpload, finishBatchItem]);

  const clearCompletedUploads = useCallback(() => {
    setUploads((prev) => prev.filter((u) => u.status === 'uploading' || u.status === 'queued'));
  }, []);

  // Global keyboard shortcuts
  const modalOpen = Boolean(
    previewFile || editorFile || chmodFile || renameModalFile || moveModalFile || newFolderOpen || omnisearchOpen || deleteConfirm
  );
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      // Cmd+K or Ctrl+K for omnisearch
      if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === 'k') {
        e.preventDefault();
        setOmnisearchOpen((prev) => !prev);
        return;
      }

      // Explorer shortcuts belong to the file list only: never inside a dialog or another view.
      if (modalOpen || activeView !== 'drive') return;
      const active = document.activeElement;
      const tag = (active?.tagName || '').toLowerCase();
      if (tag === 'input' || tag === 'textarea' || tag === 'select' || (active as HTMLElement | null)?.isContentEditable) {
        return;
      }

      const single = () =>
        selectedIds.size === 1 ? entries.find((f) => f.id === Array.from(selectedIds)[0]) : undefined;

      if (e.code === 'Space') {
        const file = single();
        if (file) {
          e.preventDefault();
          setPreviewFile(file);
        }
      } else if (e.key === 'F2') {
        const file = single();
        if (file) {
          e.preventDefault();
          setRenameModalFile(file);
        }
      } else if (e.key === 'Delete') {
        if (selectedIds.size > 0) {
          e.preventDefault();
          promptTrash();
        }
      } else if (e.key === 'Escape') {
        if (selectedIds.size > 0) {
          e.preventDefault();
          clearSelection();
        }
      } else if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === 'a') {
        e.preventDefault();
        selectAll();
      }
    };

    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [modalOpen, activeView, selectedIds, entries, promptTrash, clearSelection, selectAll]);

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
      changePassword,
      currentPath,
      entries,
      visibleEntries,
      recentEntries,
      trashEntries,
      auditEntries,
      auditHasMore: auditCursor !== null,
      loadMoreAudit,
      goToPath,
      refreshCurrentDirectory,
      selectedIds,
      toggleSelect,
      selectItem,
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
      compressItems,
      extractArchive,
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
      setActiveView,
      login,
      logout,
      changePassword,
      currentPath,
      entries,
      visibleEntries,
      recentEntries,
      trashEntries,
      auditEntries,
      auditCursor,
      loadMoreAudit,
      goToPath,
      refreshCurrentDirectory,
      selectedIds,
      toggleSelect,
      selectItem,
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
      compressItems,
      extractArchive,
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
