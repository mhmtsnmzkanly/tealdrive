import type {
  ClientState,
  FileEntry,
  Session,
  StorageQuota,
  AuditEntry,
  VirtualUser,
  SystemUser,
  StorageBudget,
  ServerSettings,
} from './types.ts';
import {
  recentPath,
  toAuditEntry,
  toSession,
  toVirtualUser,
  parentOfRelative,
  toPolicyView,
  type PolicyRule,
  type PolicyTarget,
  type PolicyView,
  type UndoPreferences,
} from './mapping.ts';

function cleanPath(path: string): string {
  return path.replace(/^\/+/, '').replace(/\/+$/, '');
}

function policyApi(realClient: any, target: PolicyTarget): any {
  return target.kind === 'linux' ? realClient.admin.systemUsers : realClient.admin.virtualUsers;
}

export interface CreateClientOptions {
  baseUrl?: string;
  encryption?: 'auto' | 'noise' | 'tls';
  /** An already created SDK client; tests pass a fake instead of loading /sdk/v1. */
  sdk?: any;
}

export function getMimeType(ext: string): string {
  const map: Record<string, string> = {
    txt: 'text/plain',
    md: 'text/markdown',
    json: 'application/json',
    js: 'application/javascript',
    mjs: 'application/javascript',
    cjs: 'application/javascript',
    ts: 'application/typescript',
    tsx: 'application/typescript',
    jsx: 'application/javascript',
    html: 'text/html',
    htm: 'text/html',
    css: 'text/css',
    svg: 'image/svg+xml',
    png: 'image/png',
    jpg: 'image/jpeg',
    jpeg: 'image/jpeg',
    webp: 'image/webp',
    gif: 'image/gif',
    ico: 'image/x-icon',
    bmp: 'image/bmp',
    pdf: 'application/pdf',
    zip: 'application/zip',
    tar: 'application/x-tar',
    gz: 'application/gzip',
    mp3: 'audio/mpeg',
    wav: 'audio/wav',
    ogg: 'audio/ogg',
    flac: 'audio/flac',
    aac: 'audio/aac',
    mp4: 'video/mp4',
    webm: 'video/webm',
    mov: 'video/quicktime',
    mkv: 'video/x-matroska',
    toml: 'application/toml',
    rs: 'text/x-rust',
    py: 'text/x-python',
    sh: 'application/x-sh',
    bash: 'application/x-sh',
    zsh: 'application/x-sh',
    xml: 'application/xml',
    yaml: 'text/yaml',
    yml: 'text/yaml',
    sql: 'application/sql',
    log: 'text/plain',
    env: 'text/plain',
    ini: 'text/plain',
    conf: 'text/plain',
    csv: 'text/csv',
    diff: 'text/x-diff',
    patch: 'text/x-diff',
    c: 'text/x-c',
    cpp: 'text/x-c',
    h: 'text/x-c',
    hpp: 'text/x-c',
    go: 'text/x-go',
    java: 'text/x-java',
    php: 'application/x-php',
  };
  return map[ext] || 'application/octet-stream';
}

// Stars are a per-browser convenience, so keep them separate for every signed-in account.
let starScope = '';
function starKey(relPath: string): string {
  return `tdrv_starred_${encodeURIComponent(starScope)}:${relPath}`;
}

function toReactEntry(entry: any, dirPath: string): FileEntry {
  const isDir =
    entry.fileType === 'Directory' ||
    entry.file_type === 'Directory' ||
    entry.isDirectory === true ||
    false;
  const entryName =
    entry.name ||
    (typeof entry.path === 'string' ? entry.path.split('/').pop() : '') ||
    'unnamed';
  const cleanDir = dirPath === '/' || !dirPath ? '' : dirPath.replace(/^\/+/, '').replace(/\/+$/, '');
  const relPath = cleanDir ? `${cleanDir}/${entryName}` : entryName;
  const fullPath = relPath.startsWith('/') ? relPath : `/${relPath}`;
  const ext = entryName.includes('.') ? entryName.split('.').pop()?.toLowerCase() || '' : '';

  let modeStr = isDir ? '0755' : '0644';
  if (entry.mode !== null && entry.mode !== undefined) {
    if (typeof entry.mode === 'number') {
      modeStr = '0' + (entry.mode & 0o777).toString(8);
    } else {
      modeStr = String(entry.mode);
    }
  }

  const isStarred =
    Boolean(entry.starred) ||
    (typeof window !== 'undefined' &&
      starScope !== '' &&
      window.localStorage?.getItem(starKey(relPath)) === 'true');

  let modifiedIso = new Date().toISOString();
  if (entry.modified) {
    const sec = Number(entry.modified);
    if (!isNaN(sec) && sec > 100000000) {
      modifiedIso = new Date(sec * 1000).toISOString();
    } else {
      const d = new Date(entry.modified);
      if (!isNaN(d.getTime())) modifiedIso = d.toISOString();
    }
  }

  return {
    id: entry.id || fullPath,
    name: entryName,
    path: fullPath,
    isDirectory: isDir,
    size: Number(entry.size) || 0,
    modified: modifiedIso,
    mode: modeStr,
    owner: entry.owner || 'user',
    group: entry.group || 'user',
    mimeType: isDir ? 'inode/directory' : getMimeType(ext),
    starred: isStarred,
    revision: typeof entry.revision === 'number' ? entry.revision : 1,
    content: entry.content,
  };
}

/**
 * High-performance TealDrive production client adapter.
 * Connects directly to TealDrive's official browser SDK (/sdk/v1/tealdrive.js) and REST APIs.
 */
export function createClient(options: CreateClientOptions = {}) {
  let realClientPromise: Promise<any> | null = null;
  let realClientInstance: any = null;
  let isConnectedToRealBackend = false;

  let clientState: ClientState = {
    connected: false,
    latency: 0,
    encryption: options.encryption === 'noise' ? 'noise' : 'tls',
    syncStatus: 'synced',
    activeTransfersCount: 0,
  };

  const stateListeners = new Set<(state: ClientState) => void>();

  const notifyState = (updates: Partial<ClientState>) => {
    clientState = { ...clientState, ...updates };
    stateListeners.forEach((cb) => cb({ ...clientState }));
  };

  async function getRealSdkClient(): Promise<any | null> {
    if (realClientInstance) return realClientInstance;
    if (options.sdk) {
      realClientInstance = options.sdk;
      return realClientInstance;
    }
    if (typeof window === 'undefined') return null;
    if (realClientPromise) return realClientPromise;

    realClientPromise = (async () => {
      try {
        // @ts-ignore
        const sdkModule = await import(/* @vite-ignore */ '/sdk/v1/tealdrive.js');
        if (typeof sdkModule?.createClient === 'function') {
          const client = sdkModule.createClient({ stateMode: 'managed' });
          client.on('connection.changed', (c: any) => {
            const isAuth = c.status === 'authenticated' || c.status === 'auth_ready';
            isConnectedToRealBackend = isAuth;
            notifyState({ connected: isAuth });
          });
          realClientInstance = client;
          return client;
        }
      } catch (err) {
        console.warn('Could not load /sdk/v1/tealdrive.js:', err);
      }
      return null;
    })();

    return realClientPromise;
  }

  async function ensureConnected(): Promise<any | null> {
    const realClient = await getRealSdkClient();
    if (!realClient) return null;
    if (!isConnectedToRealBackend) {
      try {
        await realClient.handshake();
        isConnectedToRealBackend = true;
      } catch (err) {
        console.warn('RealClient handshake error:', err);
      }
    }
    return realClient;
  }

  return {
    async handshake(): Promise<{ protocol: string; serverVersion: string; latency: number }> {
      const start = Date.now();
      const realClient = await getRealSdkClient();
      if (realClient) {
        try {
          const info = await realClient.handshake();
          const latency = Date.now() - start;
          isConnectedToRealBackend = true;
          // Report the channel actually negotiated, not a configured guess.
          const transport = info?.transport ?? realClient.getInfo?.()?.transport;
          notifyState({ connected: true, latency, encryption: transport?.encryptedWebSocket ? 'noise' : 'tls' });
          return {
            protocol: info?.transport?.protocol || 'TLS',
            serverVersion: info?.protocolVersion ? `TDRV protocol v${info.protocolVersion}` : 'unknown',
            latency,
          };
        } catch (err) {
          console.warn('Handshake failed:', err);
          throw err;
        }
      }
      throw new Error('TealDrive client unavailable');
    },

    getState(): ClientState {
      return { ...clientState };
    },

    /** Fires when the server ends this session (expiry, revocation, password change, logout). */
    onSessionRevoked(callback: () => void): () => void {
      let off: (() => void) | null = null;
      let cancelled = false;
      getRealSdkClient().then((client) => {
        if (client && !cancelled) off = client.on('session.revoked', () => callback());
      });
      return () => {
        cancelled = true;
        off?.();
      };
    },

    /** Fires after mutations made through this client, including undo. */
    onFilesChanged(callback: () => void): () => void {
      let off: (() => void) | null = null;
      let cancelled = false;
      getRealSdkClient().then((client) => {
        if (client && !cancelled) off = client.on('files.changed', () => callback());
      });
      return () => {
        cancelled = true;
        off?.();
      };
    },

    observeState(callback: (state: ClientState) => void): () => void {
      stateListeners.add(callback);
      return () => {
        stateListeners.delete(callback);
      };
    },

    auth: {
      async getSession(): Promise<Session | null> {
        const realClient = await getRealSdkClient();
        if (realClient) {
          try {
            await realClient.handshake();
            const session = await realClient.auth.getSession();
            if (session && session.username) {
              starScope = session.username;
              isConnectedToRealBackend = true;
              notifyState({ connected: true });
              return toSession(session);
            }
          } catch (err) {
            console.warn('getSession error:', err);
          }
        }
        return null;
      },

      async login(credentials: { username: string; password?: string }): Promise<Session> {
        const realClient = await getRealSdkClient();
        if (!realClient) throw new Error('TealDrive client unavailable');

        try {
          // If already in an authenticated session on the socket, disconnect cleanly first
          if (isConnectedToRealBackend) {
            try {
              realClient.disconnect();
            } catch {}
          }

          // Ensure WebSocket handshake is ready
          await realClient.handshake();

          // Official WS-first login: sends AuthLogin frame over WS, receives bind_token, binds cookie
          const session = await realClient.auth.login({
            username: credentials.username,
            password: credentials.password || '',
          });

          isConnectedToRealBackend = true;
          starScope = session.username;
          notifyState({ connected: true });

          return toSession(session);
        } catch (err: any) {
          console.error('Login error:', err);
          const errMsg =
            err?.message ||
            (err?.code === 'INVALID_CREDENTIALS'
              ? 'Invalid username or password.'
              : err?.code === 'PASSWORD_CHANGE_REQUIRED'
              ? 'Password change required.'
              : 'Login failed. Please try again.');
          throw new Error(errMsg);
        }
      },

      /** Replaces the password; the server then revokes this session, so sign in again. */
      async changePassword(params: { currentPassword: string; newPassword: string }): Promise<void> {
        const realClient = await getRealSdkClient();
        if (!realClient) throw new Error('TealDrive client unavailable');
        await realClient.auth.changePassword(params);
        isConnectedToRealBackend = false;
        starScope = '';
        notifyState({ connected: false });
      },

      async logout(): Promise<void> {
        const realClient = await getRealSdkClient();
        try {
          await realClient?.auth?.logout();
        } catch {
          await fetch('/api/logout', { method: 'POST', credentials: 'include' });
          realClient?.disconnect?.();
        }
        isConnectedToRealBackend = false;
        starScope = '';
        notifyState({ connected: false });
      },
    },

    files: {
      async list(params: { path: string; signal?: AbortSignal }): Promise<{ entries: FileEntry[]; currentPath: string }> {
        const realClient = await ensureConnected();
        if (realClient) {
          try {
            const cleanPath = (params.path || '').replace(/^\/+/, '').replace(/\/+$/, '');
            const res = await realClient.files.list({
              path: cleanPath,
              includeHidden: true,
              ...(params.signal ? { signal: params.signal } : {}),
            });
            const rawEntries = res?.entries || (Array.isArray(res) ? res : []);
            const entries = rawEntries.map((e: any) => toReactEntry(e, params.path));
            return { entries, currentPath: params.path };
          } catch (e) {
            console.error('Real files.list failed:', e);
            throw e;
          }
        }
        return { entries: [], currentPath: params.path };
      },

      async get(params: { path: string }): Promise<FileEntry | undefined> {
        const realClient = await ensureConnected();
        if (realClient) {
          try {
            const cleanPath = (params.path || '').replace(/^\/+/, '').replace(/\/+$/, '');
            const stat = await realClient.files.stat({ path: cleanPath });
            if (stat && (stat.entry || stat.name)) {
              const parent = params.path.split('/').slice(0, -1).join('/') || '/';
              return toReactEntry(stat.entry || stat, parent);
            }
          } catch (e) {
            console.warn('Real files.get failed:', e);
          }
        }
        return undefined;
      },

      async createFolder(params: { path: string }): Promise<FileEntry> {
        const realClient = await ensureConnected();
        if (!realClient) throw new Error('Client not connected');
        const cleanPath = params.path.replace(/^\/+/, '').replace(/\/+$/, '');
        await realClient.files.createFolder({ path: cleanPath });
        const name = cleanPath.split('/').pop() || 'folder';
        return toReactEntry({ name, fileType: 'Directory' }, params.path);
      },

      async upload(params: {
        directory: string;
        file: { name: string; size: number; type?: string; content?: string; rawFile?: File };
        onProgress?: (progress: { progress: number; speedBytesPerSec: number; uploadedBytes: number }) => void;
        signal?: AbortSignal;
      }): Promise<FileEntry> {
        const realClient = await ensureConnected();
        if (!realClient) throw new Error('Client not connected');
        const cleanDir = params.directory.replace(/^\/+/, '').replace(/\/+$/, '');
        let uploadBlob: any = params.file.rawFile;
        if (!uploadBlob) {
          uploadBlob = new File([params.file.content || ''], params.file.name, {
            type: params.file.type || 'text/plain',
          });
        }
        await realClient.files.upload({
          file: uploadBlob,
          directory: cleanDir,
          signal: params.signal,
          onProgress: (p: any) => {
            params.onProgress?.({
              progress: Math.round(p.percent ?? p.progress ?? 0),
              speedBytesPerSec: p.speedBytesPerSec ?? 0,
              uploadedBytes: p.uploadedBytes ?? Math.round(((p.percent ?? 0) / 100) * params.file.size),
            });
          },
        });
        return toReactEntry({ name: params.file.name, fileType: 'File', size: params.file.size }, params.directory);
      },

      getDownloadUrl(params: { path: string }): string {
        const cleanPath = params.path.replace(/^\/+/, '').replace(/\/+$/, '');
        if (realClientInstance) {
          return realClientInstance.files.getDownloadUrl({ path: cleanPath });
        }
        return `/api/download?root_id=home&path=${encodeURIComponent(cleanPath)}`;
      },

      getPreviewUrl(params: { path: string }): string {
        const cleanPath = params.path.replace(/^\/+/, '').replace(/\/+$/, '');
        if (realClientInstance) {
          return realClientInstance.files.getPreviewUrl({ path: cleanPath });
        }
        return `/api/preview?root_id=home&path=${encodeURIComponent(cleanPath)}`;
      },

      async getPreviewBlob(params: { path: string; signal?: AbortSignal }): Promise<{ blob: Blob; objectUrl: string; mimeType: string }> {
        const cleanPath = params.path.replace(/^\/+/, '').replace(/\/+$/, '');
        const realClient = await ensureConnected();
        const url = realClient
          ? realClient.files.getPreviewUrl({ path: cleanPath })
          : `/api/preview?root_id=home&path=${encodeURIComponent(cleanPath)}`;

        const res = await fetch(url, {
          credentials: 'include',
          cache: 'no-store',
          signal: params.signal,
        });

        if (!res.ok) {
          throw new Error(`HTTP ${res.status} preview fetch failed`);
        }

        const blob = await res.blob();
        const objectUrl = URL.createObjectURL(blob);
        return {
          blob,
          objectUrl,
          mimeType: res.headers.get('content-type') || blob.type || 'image/png',
        };
      },

      async download(params: { path: string }): Promise<{ filename: string; blobUrl: string; size: number }> {
        const realClient = await ensureConnected();
        const cleanPath = params.path.replace(/^\/+/, '').replace(/\/+$/, '');
        const filename = cleanPath.split('/').pop() || 'file';

        if (realClient) {
          const downloadUrl = realClient.files.getDownloadUrl({ path: cleanPath });
          return {
            filename,
            blobUrl: downloadUrl,
            size: 0,
          };
        }
        throw new Error('Client not connected');
      },

      async rename(params: { path: string; newName: string }): Promise<FileEntry> {
        const realClient = await ensureConnected();
        if (!realClient) throw new Error('Client not connected');
        const cleanPath = params.path.replace(/^\/+/, '').replace(/\/+$/, '');
        await realClient.files.rename({ path: cleanPath, newName: params.newName });
        const parent = params.path.split('/').slice(0, -1).join('/') || '/';
        return toReactEntry({ name: params.newName, fileType: 'File' }, parent);
      },

      async move(params: { sourcePath: string; targetPath: string }): Promise<FileEntry> {
        const realClient = await ensureConnected();
        if (!realClient) throw new Error('Client not connected');
        const cleanSrc = params.sourcePath.replace(/^\/+/, '').replace(/\/+$/, '');
        const cleanDst = params.targetPath.replace(/^\/+/, '').replace(/\/+$/, '');
        await realClient.files.move({ source: { path: cleanSrc }, destination: { path: cleanDst } });
        const name = cleanSrc.split('/').pop() || 'file';
        return toReactEntry({ name, fileType: 'File' }, params.targetPath);
      },

      async copy(params: { sourcePath: string; targetPath: string }): Promise<FileEntry> {
        const realClient = await ensureConnected();
        if (!realClient) throw new Error('Client not connected');
        const cleanSrc = params.sourcePath.replace(/^\/+/, '').replace(/\/+$/, '');
        const cleanDst = params.targetPath.replace(/^\/+/, '').replace(/\/+$/, '');
        await realClient.files.copy({ source: { path: cleanSrc }, destination: { path: cleanDst } });
        const name = cleanSrc.split('/').pop() || 'file';
        return toReactEntry({ name, fileType: 'File' }, params.targetPath);
      },

      async chmod(params: { path: string; mode: string | number }): Promise<FileEntry> {
        const realClient = await ensureConnected();
        if (!realClient) throw new Error('Client not connected');
        const numMode = typeof params.mode === 'number' ? params.mode : parseInt(String(params.mode).replace(/^0+/, ''), 8);
        const clean = params.path.replace(/^\/+/, '').replace(/\/+$/, '');
        await realClient.files.chmod({ path: clean, mode: numMode });
        return toReactEntry({ name: clean.split('/').pop() || 'file', fileType: 'File', mode: numMode }, params.path);
      },

      async readText(params: { path: string }): Promise<{ content: string; revision: any }> {
        const realClient = await ensureConnected();
        if (!realClient) throw new Error('Client not connected');
        const clean = params.path.replace(/^\/+/, '').replace(/\/+$/, '');
        const res = await realClient.files.readText({ path: clean });
        return {
          content: res.content,
          revision: res.revision ?? 1,
        };
      },

      async writeText(params: { path: string; content: string; expectedRevision?: any }): Promise<FileEntry> {
        const realClient = await ensureConnected();
        if (!realClient) throw new Error('Client not connected');
        const clean = params.path.replace(/^\/+/, '').replace(/\/+$/, '');
        const rev = params.expectedRevision;
        // Reading the current revision here would silently overwrite someone else's changes.
        if (typeof rev !== 'string' || !(/^[a-f0-9]{64}$/.test(rev) || rev === 'missing')) {
          const error: any = new Error('The file revision is unknown. Reload the file before saving.');
          error.code = 'REVISION_CONFLICT';
          throw error;
        }
        await realClient.files.writeText({
          path: clean,
          content: params.content,
          expectedRevision: rev,
        });
        // The save reply carries no revision. Adopt the server's revision only if
        // it still holds exactly what we wrote; otherwise someone changed it since.
        let revision: string | null = null;
        try {
          const current = await realClient.files.readText({ path: clean });
          if (current.content === params.content) revision = current.revision;
        } catch {}
        const parent = params.path.split('/').slice(0, -1).join('/') || '/';
        return {
          ...toReactEntry(
            { name: clean.split('/').pop() || 'file', fileType: 'File', size: new TextEncoder().encode(params.content).length },
            parent
          ),
          revision: revision as any,
        };
      },

      trash: {
        async move(params: { path: string }): Promise<FileEntry> {
          const realClient = await ensureConnected();
          if (!realClient) throw new Error('Client not connected');
          const clean = params.path.replace(/^\/+/, '').replace(/\/+$/, '');
          await realClient.files.trash.move({ path: clean });
          return toReactEntry({ name: clean.split('/').pop() || 'file', fileType: 'File' }, params.path);
        },

        async list(): Promise<FileEntry[]> {
          const realClient = await ensureConnected();
          if (realClient) {
            try {
              const res = await realClient.files.trash.list({});
              const items = res.items || res || [];
              return (items || []).map((t: any) => ({
                id: t.trashId || t.id,
                name: t.displayName || t.name || t.path?.split('/').pop() || 'trashed',
                path: t.originalPath || t.path || `/${t.displayName || t.name || 'trashed'}`,
                isDirectory: Boolean(t.isDir || t.isDirectory),
                size: t.size || 0,
                modified: t.deletedAt ? new Date(t.deletedAt * 1000).toISOString() : (t.trashedAt || new Date().toISOString()),
                mode: '0644',
                owner: 'user',
                group: 'user',
                mimeType: 'application/octet-stream',
                trashedAt: t.deletedAt ? new Date(t.deletedAt * 1000).toISOString() : t.trashedAt,
                originalPath: t.originalPath || t.path,
                revision: 1,
              }));
            } catch (e) {
              console.error('Real trash.list failed:', e);
              throw e;
            }
          }
          return [];
        },

        async restore(params: { id: string }): Promise<FileEntry> {
          const realClient = await ensureConnected();
          if (!realClient) throw new Error('Client not connected');
          await realClient.files.trash.restore({ trashId: params.id });
          return toReactEntry({ name: 'restored', fileType: 'File' }, '/');
        },

        async empty(): Promise<number> {
          const realClient = await ensureConnected();
          if (!realClient) throw new Error('Client not connected');
          const res = await realClient.files.trash.list({});
          const queue = [...(res.items || res || [])];
          let removed = 0;
          let failure: any = null;
          // A few deletions in flight at once; stop handing out work after the first failure.
          const worker = async () => {
            while (!failure && queue.length > 0) {
              const item = queue.shift();
              try {
                await realClient.files.trash.deletePermanently({ trashId: item.trashId || item.id });
                removed++;
              } catch (err: any) {
                failure ??= err;
              }
            }
          };
          await Promise.all(Array.from({ length: 4 }, worker));
          if (failure) {
            if (removed > 0) failure.removed = removed;
            throw failure;
          }
          return removed;
        },

        async deletePermanently(params: { id: string }): Promise<boolean> {
          const realClient = await ensureConnected();
          if (!realClient) throw new Error('Client not connected');
          await realClient.files.trash.deletePermanently({ trashId: params.id });
          return true;
        },
      },

      async star(params: { path: string; starred?: boolean }): Promise<FileEntry> {
        const clean = params.path.replace(/^\/+/, '').replace(/\/+$/, '');
        if (!starScope) throw new Error('Not signed in');
        const key = starKey(clean);
        const isStarred = params.starred !== undefined ? params.starred : (typeof window !== 'undefined' && window.localStorage?.getItem(key) !== 'true');
        if (typeof window !== 'undefined') {
          if (isStarred) {
            window.localStorage?.setItem(key, 'true');
          } else {
            window.localStorage?.removeItem(key);
          }
        }
        return toReactEntry({ name: clean.split('/').pop() || 'file', starred: isStarred }, params.path);
      },

      async getRecent(limit = 20): Promise<FileEntry[]> {
        const realClient = await ensureConnected();
        if (!realClient) return [];
        // The journal knows which files this account changed; the directory tree does not.
        const page = await realClient.files.audit.list({ limit: 200, status: 'success' });
        const seen = new Set<string>();
        const paths: string[] = [];
        for (const record of page?.items || []) {
          const path = recentPath(record);
          if (path === null || seen.has(path)) continue;
          seen.add(path);
          paths.push(path);
          if (paths.length >= limit * 2) break;
        }
        // A path that no longer resolves was moved, renamed or trashed later; skip it.
        const stats = await Promise.allSettled(paths.map((p) => realClient.files.stat({ path: p })));
        const entries: FileEntry[] = [];
        stats.forEach((stat, i) => {
          if (stat.status !== 'fulfilled' || entries.length >= limit) return;
          const raw = stat.value?.entry ?? stat.value;
          if (!raw?.name) return;
          const entry = toReactEntry(raw, parentOfRelative(paths[i]));
          if (!entry.isDirectory) entries.push(entry);
        });
        return entries;
      },

      async search(params: { query: string; signal?: AbortSignal }): Promise<FileEntry[]> {
        const realClient = await ensureConnected();
        if (!realClient) throw new Error('Client not connected');
        // An explicit empty path searches the whole root, not the SDK's last directory.
        const results = await realClient.files.search({
          path: '',
          query: params.query,
          limit: 50,
          ...(params.signal ? { signal: params.signal } : {}),
        });
        return (results?.matches || []).map((m: any) => {
          return toReactEntry(m.entry ?? m, parentOfRelative(String(m.relativePath ?? '')));
        });
      },

      quota: {
        async get(): Promise<StorageQuota> {
          const realClient = await ensureConnected();
          if (!realClient) throw new Error('Client not connected');
          const q = await realClient.files.quota.get({ rootId: 'home' });
          const used = Number(q.usedBytes ?? q.used_bytes ?? 0);
          // No limit means the root is unmetered; never invent one.
          const limit = Number(q.limitBytes ?? q.limit_bytes ?? 0);
          const pct = limit > 0 ? (used / limit) * 100 : 0;
          return {
            usedBytes: used,
            limitBytes: limit,
            percentage: Math.round(pct),
            warningLevel: pct > 90 ? 'critical' : pct > 75 ? 'warning' : 'normal',
          };
        },
      },

      archives: {
        /** Packs the given paths into a new archive at `target` (full path, e.g. /docs/a.zip). */
        async create(params: { sources: string[]; target: string; format: 'zip' | 'tar.gz' }): Promise<void> {
          const realClient = await ensureConnected();
          if (!realClient) throw new Error('Client not connected');
          await realClient.files.archives.create({
            sources: params.sources.map(cleanPath),
            path: cleanPath(params.target),
            format: params.format,
          });
        },

        /** Unpacks an archive into `destination`, a folder that must not exist yet. */
        async extract(params: { path: string; destination: string }): Promise<void> {
          const realClient = await ensureConnected();
          if (!realClient) throw new Error('Client not connected');
          await realClient.files.archives.extract({
            path: cleanPath(params.path),
            destination: cleanPath(params.destination),
          });
        },
      },

      /** The SDK polls quota while someone listens; forward each known reading. */
      observeQuotaWarning(callback: (quota: StorageQuota) => void): () => void {
        let off: (() => void) | null = null;
        let cancelled = false;
        getRealSdkClient().then((client) => {
          if (!client || cancelled) return;
          off = client.files.observeQuotaWarning((warning: any) => {
            const q = warning?.quota;
            if (!q || q.stale || warning.status !== 'known') return;
            const used = Number(q.usedBytes ?? 0);
            const limit = Number(q.limitBytes ?? 0);
            const pct = limit > 0 ? (used / limit) * 100 : 0;
            callback({
              usedBytes: used,
              limitBytes: limit,
              percentage: Math.round(pct),
              warningLevel: pct > 90 ? 'critical' : pct > 75 ? 'warning' : 'normal',
            });
          });
        });
        return () => {
          cancelled = true;
          off?.();
        };
      },

      undo: {
        /**
         * One page of own-account history, joined with the undo candidates the server
         * offers. Pass the returned cursor to fetch the next, older page.
         */
        async list(params: { limit?: number; cursor?: string | null } = {}): Promise<{ entries: AuditEntry[]; nextCursor: string | null }> {
          const realClient = await ensureConnected();
          if (!realClient) throw new Error('Client not connected');
          const limit = params.limit ?? 50;
          const [audit, undo] = await Promise.allSettled([
            realClient.files.audit.list({ limit, ...(params.cursor ? { cursor: params.cursor } : {}) }),
            // Undo candidates are few and recent; one wide page covers any history page.
            realClient.files.undo.list({ limit: 200 }),
          ]);
          if (audit.status === 'rejected' && undo.status === 'rejected') throw audit.reason;
          const candidates = new Map<string, any>();
          if (undo.status === 'fulfilled') {
            for (const c of undo.value?.items || []) candidates.set(c.operation.id, c);
          }
          if (audit.status === 'rejected') {
            // Without the audit feature, the undo list is the only history there is.
            const entries = [...candidates.values()].map((c) => toAuditEntry(c.operation, c));
            return { entries: params.cursor ? [] : entries, nextCursor: null };
          }
          return {
            entries: (audit.value?.items || []).map((r: any) => toAuditEntry(r, candidates.get(r.id))),
            nextCursor: audit.value?.nextCursor ?? null,
          };
        },

        async getPreferences(): Promise<UndoPreferences> {
          const realClient = await ensureConnected();
          if (!realClient) throw new Error('Client not connected');
          return realClient.files.undo.getPreferences();
        },

        /** null inherits the server default; otherwise hours, up to maximumHours. */
        async setPreferences(params: { hours: number | null; expectedRevision: number }): Promise<UndoPreferences> {
          const realClient = await ensureConnected();
          if (!realClient) throw new Error('Client not connected');
          return realClient.files.undo.setPreferences(params);
        },

        /** Finds the undo candidate for an operation this tab just performed. */
        async findLatest(params: { action: string; path: string; since: number }): Promise<AuditEntry | null> {
          const realClient = await ensureConnected();
          if (!realClient) return null;
          const clean = params.path.replace(/^\/+/, '').replace(/\/+$/, '');
          try {
            const page = await realClient.files.undo.list({ limit: 10, action: params.action });
            const match = (page?.items || []).find(
              (c: any) =>
                c.available &&
                c.operation.sourcePath === clean &&
                c.operation.createdAt >= params.since
            );
            return match ? toAuditEntry(match.operation, match) : null;
          } catch {
            return null;
          }
        },

        async apply(params: { operationId: string; expectedState: string }): Promise<{ success: boolean; message: string }> {
          const realClient = await ensureConnected();
          if (!realClient) throw new Error('Client not connected');
          await realClient.files.undo.apply({
            operationId: params.operationId,
            expectedState: params.expectedState,
          });
          return { success: true, message: 'The operation was reverted.' };
        },
      },
    },

    policy: {
      /** Reads the policy of `target`, or of the signed-in account when omitted. */
      async get(target?: PolicyTarget): Promise<PolicyView> {
        const realClient = await ensureConnected();
        if (!realClient) throw new Error('Client not connected');
        const view = target
          ? await policyApi(realClient, target).getPolicy({ accountId: target.accountId })
          : await realClient.files.policy.get();
        return toPolicyView(view, target ? 'admin' : 'personal');
      },

      /** Replaces the rules this layer owns. The server then signs the account out everywhere. */
      async set(target: PolicyTarget | undefined, params: { expectedGeneration: number; rules: PolicyRule[] }): Promise<void> {
        const realClient = await ensureConnected();
        if (!realClient) throw new Error('Client not connected');
        const rules = params.rules.map((r) => ({
          path: cleanPath(r.path),
          recursive: r.recursive,
          deny: r.deny,
          ...(r.rootId ? { rootId: r.rootId } : {}),
        }));
        if (target) {
          await policyApi(realClient, target).setPolicy({
            accountId: target.accountId,
            expectedGeneration: params.expectedGeneration,
            rules,
          });
        } else {
          await realClient.files.policy.set({ expectedGeneration: params.expectedGeneration, rules });
        }
      },
    },

    admin: {
      virtualUsers: {
        async list(): Promise<VirtualUser[]> {
          const realClient = await ensureConnected();
          if (!realClient) throw new Error('Client not connected');
          const res = await realClient.admin.virtualUsers.list();
          const list = Array.isArray(res) ? res : res?.accounts || res?.users || res?.items || [];
          return list.map(toVirtualUser);
        },

        async create(params: { username: string; quotaBytes: number; password: string }): Promise<VirtualUser> {
          const realClient = await ensureConnected();
          if (!realClient) throw new Error('Client not connected');
          // The server requires this user to replace the admin-chosen password at first sign-in.
          const u = await realClient.admin.virtualUsers.create({
            name: params.username,
            quotaBytes: params.quotaBytes,
            password: params.password,
          });
          return toVirtualUser(u);
        },

        async setQuota(params: { user: VirtualUser; quotaBytes: number }): Promise<VirtualUser> {
          const realClient = await ensureConnected();
          if (!realClient) throw new Error('Client not connected');
          const u = await realClient.admin.virtualUsers.setQuota({
            accountId: params.user.id,
            expectedGeneration: params.user.generation,
            quotaBytes: params.quotaBytes,
          });
          return toVirtualUser(u);
        },

        async toggleStatus(params: { user: VirtualUser; enabled: boolean }): Promise<void> {
          const realClient = await ensureConnected();
          if (!realClient) throw new Error('Client not connected');
          if (params.enabled) {
            await realClient.admin.virtualUsers.enable({ accountId: params.user.id });
          } else {
            await realClient.admin.virtualUsers.disable({ accountId: params.user.id });
          }
        },

        async delete(params: { user: VirtualUser }): Promise<void> {
          const realClient = await ensureConnected();
          if (!realClient) throw new Error('Client not connected');
          await realClient.admin.virtualUsers.delete({
            accountId: params.user.id,
            expectedGeneration: params.user.generation,
          });
        },
      },

      systemUsers: {
        async list(): Promise<SystemUser[]> {
          const realClient = await ensureConnected();
          if (!realClient) throw new Error('Client not connected');
          const res = await realClient.admin.systemUsers.list();
          const list = Array.isArray(res) ? res : res?.users || res?.items || [];
          return list.map((su: any) => ({
            username: su.username,
            uid: su.uid,
            shell: su.shell || '',
            homeDir: su.homeDir || su.home || '',
            approvedForWeb: Boolean(su.registered ?? su.approvedForWeb ?? su.approved),
            lastLogin: su.lastLogin,
          }));
        },

        /** Registered Linux accounts, keyed by username, with what access changes need. */
        async accounts(): Promise<Map<string, { id: string; generation: number; status: string; loginEnabled: boolean }>> {
          const realClient = await ensureConnected();
          if (!realClient) throw new Error('Client not connected');
          const list = await realClient.admin.systemUsers.accounts();
          const map = new Map<string, { id: string; generation: number; status: string; loginEnabled: boolean }>();
          for (const a of Array.isArray(list) ? list : []) {
            map.set(a.username, {
              id: a.id,
              generation: Number(a.generation) || 0,
              status: a.status,
              loginEnabled: a.loginEnabled !== false,
            });
          }
          return map;
        },

        /** Allows or suspends web access for a registered Linux account. */
        async setAccess(params: { accountId: string; enabled: boolean }): Promise<void> {
          const realClient = await ensureConnected();
          if (!realClient) throw new Error('Client not connected');
          if (params.enabled) await realClient.admin.systemUsers.allow({ accountId: params.accountId });
          else await realClient.admin.systemUsers.deny({ accountId: params.accountId });
        },

        async approve(params: { username: string }): Promise<void> {
          const realClient = await ensureConnected();
          if (!realClient) throw new Error('Client not connected');
          await realClient.admin.systemUsers.approve({ username: params.username });
        },
      },

      quota: {
        async getBudget(): Promise<StorageBudget> {
          const realClient = await ensureConnected();
          if (!realClient) throw new Error('Client not connected');
          // Keys arrive camelCased: limitBytes, reservedBytes, remainingBytes, ownQuotaBytes.
          const b = await realClient.admin.quota.getBudget();
          const total = Number(b?.limitBytes);
          if (!Number.isFinite(total) || total <= 0) throw new Error('Storage budget unavailable');
          const reserved = Number(b.reservedBytes ?? 0);
          const own = Number(b.ownQuotaBytes ?? 0);
          return {
            totalDiskBytes: total,
            virtualUsersAllocatedBytes: reserved,
            systemReserveBytes: own,
            freePhysicalBytes: Math.max(0, Number(b.remainingBytes ?? total - reserved - own)),
          };
        },
      },

      settings: {
        async inspect(): Promise<ServerSettings> {
          const realClient = await ensureConnected();
          if (realClient) {
            try {
              const res = await realClient.admin.settings.inspect();
              const cfg = res?.config ?? res ?? {};
              if (cfg && (cfg.server || cfg.limits)) {
                const srv = cfg.server ?? {};
                const lim = cfg.limits ?? {};
                const rat = cfg.rateLimit ?? {};
                const lif = cfg.lifecycle ?? {};
                // With web accounts the lifecycle task purges trash; otherwise the legacy day setting does.
                const trashDays = cfg.webAccounts?.enabled
                  ? Math.round(Number(lif.trashRetentionHours ?? 504) / 24)
                  : Number(cfg.trashRetentionDays ?? 30);

                const info = realClient.getInfo?.();
                const encrypted = Boolean(info?.transport?.encryptedWebSocket);
                return {
                  serverName: typeof window !== 'undefined' ? window.location.host : '—',
                  version: info?.protocolVersion ? `TDRV protocol v${info.protocolVersion}` : '—',
                  listenAddr: srv.bindAddr || '—',
                  wsEncryption: encrypted ? 'TLS + Noise NK' : 'TLS',
                  tlsCertExpiry: '',
                  loginMaxFailures: Number(rat.maxLoginFailures) || 0,
                  maxUploadSizeMb: lim.maxInFlightUpload ? Math.round(lim.maxInFlightUpload / (1024 * 1024)) : 0,
                  autoTrashPurgeDays: trashDays,
                  noiseHandshakePattern: encrypted ? 'Noise_NK_25519_ChaChaPoly_SHA256' : 'disabled',
                };
              }
            } catch (err) {
              console.warn('Real admin.settings.inspect failed:', err);
              throw err;
            }
          }
          throw new Error('Server settings unavailable');
        },
      },
    },
  };
}
