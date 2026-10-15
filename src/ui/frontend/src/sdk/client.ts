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

export interface CreateClientOptions {
  baseUrl?: string;
  encryption?: 'auto' | 'noise' | 'tls';
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
      window.localStorage?.getItem(`tdrv_starred_${cleanDir ? cleanDir + '/' : ''}${entryName}`) === 'true');

  return {
    id: entry.id || fullPath,
    name: entryName,
    path: fullPath,
    isDirectory: isDir,
    size: Number(entry.size) || 0,
    modified: entry.modified || new Date().toISOString(),
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
  const quotaListeners = new Set<(quota: StorageQuota) => void>();

  const notifyState = (updates: Partial<ClientState>) => {
    clientState = { ...clientState, ...updates };
    stateListeners.forEach((cb) => cb({ ...clientState }));
  };

  async function getRealSdkClient(): Promise<any | null> {
    if (typeof window === 'undefined') return null;
    if (realClientInstance) return realClientInstance;
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
          notifyState({ connected: true, latency });
          return {
            protocol: info?.transport?.protocol || 'Noise_NK_25519_ChaChaPoly_SHA256',
            serverVersion: `TealDrive Core v${info?.protocolVersion || '1.1'}`,
            latency,
          };
        } catch (err) {
          console.warn('Handshake failed:', err);
        }
      }
      return {
        protocol: 'Noise_NK_25519_ChaChaPoly_SHA256',
        serverVersion: 'TealDrive Core 2.0',
        latency: 0,
      };
    },

    getState(): ClientState {
      return { ...clientState };
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
            const res = await fetch('/api/session', { credentials: 'include' });
            if (res.ok) {
              const data = await res.json();
              if (data && data.authenticated && data.username) {
                try {
                  await realClient.handshake();
                  isConnectedToRealBackend = true;
                  notifyState({ connected: true });
                } catch {}
                return {
                  username: data.username,
                  uid: data.uid || 1000,
                  role: data.is_admin ? 'admin' : 'user',
                  isAdmin: Boolean(data.is_admin),
                  email: `${data.username}@novastrum.dev`,
                };
              }
            }
          } catch (err) {
            console.warn('getSession error:', err);
          }
        }
        return null;
      },

      async login(credentials: { username: string; password?: string }): Promise<Session> {
        const realClient = await getRealSdkClient();
        const res = await fetch('/api/login', {
          method: 'POST',
          credentials: 'include',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify(credentials),
        });

        if (!res.ok) {
          let errMsg = 'Invalid username or password.';
          try {
            const errData = await res.json();
            if (errData.error || errData.message) errMsg = errData.error || errData.message;
          } catch {}
          throw new Error(errMsg);
        }

        const sessionInfo = await res.json();
        if (realClient) {
          try {
            await realClient.handshake();
            isConnectedToRealBackend = true;
            notifyState({ connected: true });
          } catch (e) {
            console.warn('RealClient handshake after login:', e);
          }
        }

        return {
          username: sessionInfo.username || credentials.username,
          uid: sessionInfo.uid || 1000,
          role: sessionInfo.is_admin ? 'admin' : 'user',
          isAdmin: Boolean(sessionInfo.is_admin),
          email: `${sessionInfo.username || credentials.username}@novastrum.dev`,
        };
      },

      async logout(): Promise<void> {
        const realClient = await getRealSdkClient();
        try {
          await fetch('/api/logout', { method: 'POST', credentials: 'include' });
          realClient?.disconnect?.();
        } catch {}
        isConnectedToRealBackend = false;
        notifyState({ connected: false });
      },
    },

    files: {
      async list(params: { path: string }): Promise<{ entries: FileEntry[]; currentPath: string }> {
        const realClient = await ensureConnected();
        if (realClient) {
          try {
            const cleanPath = (params.path || '').replace(/^\/+/, '').replace(/\/+$/, '');
            const res = await realClient.files.list({ path: cleanPath, includeHidden: true });
            const entries = (res.entries || []).map((e: any) => toReactEntry(e, params.path));
            return { entries, currentPath: params.path };
          } catch (e) {
            console.error('Real files.list failed:', e);
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
              return toReactEntry(stat.entry || stat, params.path);
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
              speedBytesPerSec: p.speedBytesPerSec ?? 1024 * 1024 * 3,
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
        let rev = params.expectedRevision;
        if (typeof rev !== 'string' || !(/^[a-f0-9]{64}$/.test(rev) || rev === 'missing')) {
          try {
            const current = await realClient.files.readText({ path: clean });
            rev = current.revision;
          } catch {
            rev = 'missing';
          }
        }
        await realClient.files.writeText({
          path: clean,
          content: params.content,
          expectedRevision: rev,
        });
        return toReactEntry({ name: clean.split('/').pop() || 'file', fileType: 'File', size: params.content.length }, params.path);
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
          const list = res.items || res || [];
          if (list.length === 0) return 0;
          for (const item of list) {
            await realClient.files.trash.deletePermanently({ trashId: item.trashId || item.id });
          }
          return list.length;
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
        const key = `tdrv_starred_${clean}`;
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

      async getRecent(limit = 10): Promise<FileEntry[]> {
        const realClient = await getRealSdkClient();
        if (realClient) {
          try {
            const res = await realClient.files.list({
              path: '',
              sort: { by: 'modifiedAt', direction: 'desc' },
              includeHidden: true,
            });
            const entries = (res.entries || []).map((e: any) => toReactEntry(e, '/'));
            return entries.slice(0, limit);
          } catch {
            return [];
          }
        }
        return [];
      },

      async search(params: { query: string }): Promise<FileEntry[]> {
        const realClient = await getRealSdkClient();
        if (realClient) {
          try {
            const results = await realClient.files.search({ query: params.query, limit: 50 });
            return (results.matches || results || []).map((e: any) => toReactEntry(e, '/'));
          } catch (e) {
            console.error('Real search failed:', e);
          }
        }
        return [];
      },

      quota: {
        async get(): Promise<StorageQuota> {
          const realClient = await getRealSdkClient();
          if (realClient) {
            try {
              const q = await realClient.files.quota.get({ rootId: 'home' });
              const used = q.usedBytes ?? q.used_bytes ?? 0;
              const limit = q.limitBytes ?? q.limit_bytes ?? 10 * 1024 * 1024 * 1024;
              const pct = limit > 0 ? (used / limit) * 100 : 0;
              return {
                usedBytes: used,
                limitBytes: limit,
                percentage: Math.round(pct),
                warningLevel: pct > 90 ? 'critical' : pct > 75 ? 'warning' : 'normal',
              };
            } catch (e) {
              console.error('Real quota.get failed:', e);
            }
          }
          return {
            usedBytes: 0,
            limitBytes: 10 * 1024 * 1024 * 1024,
            percentage: 0,
            warningLevel: 'normal',
          };
        },

        setLimit(_limitBytes: number) {
          // Read-only on live system
        },
      },

      observeQuotaWarning(callback: (quota: StorageQuota) => void): () => void {
        quotaListeners.add(callback);
        return () => {
          quotaListeners.delete(callback);
        };
      },

      undo: {
        async list(): Promise<AuditEntry[]> {
          return [];
        },

        async apply(_params: { operationId: string }): Promise<{ success: boolean; message: string }> {
          return { success: false, message: 'Undo not supported on live server' };
        },
      },
    },

    admin: {
      virtualUsers: {
        async list(): Promise<VirtualUser[]> {
          const realClient = await ensureConnected();
          if (realClient) {
            try {
              const res = await realClient.admin.virtualUsers.list();
              const list = Array.isArray(res) ? res : (res?.users || res?.items || []);
              return list.map((u: any) => ({
                id: u.accountId || u.id || u.username,
                username: u.username || u.name,
                uid: u.uid || 1000,
                quotaBytes: u.quotaBytes ?? u.quota_bytes ?? 1024 * 1024 * 1024,
                usedBytes: u.usedBytes ?? u.used_bytes ?? 0,
                enabled: u.status === 'active' || u.enabled !== false,
                createdAt: u.createdAt
                  ? new Date(u.createdAt * 1000).toISOString()
                  : u.created_at
                  ? new Date(u.created_at * 1000).toISOString()
                  : new Date().toISOString(),
                email: u.email || `${u.username || u.name}@tealdrive.internal`,
              }));
            } catch (err) {
              console.warn('Real admin.virtualUsers.list failed:', err);
            }
          }
          return [];
        },

        async create(params: { username: string; quotaBytes: number; email?: string }): Promise<VirtualUser> {
          const realClient = await ensureConnected();
          if (!realClient) throw new Error('Client not connected');
          const u = await realClient.admin.virtualUsers.create({
            name: params.username,
            quotaBytes: params.quotaBytes,
            password: 'InitialPassword123!',
          });
          return {
            id: u?.accountId || params.username,
            username: params.username,
            uid: u?.uid || 1000,
            quotaBytes: params.quotaBytes,
            usedBytes: 0,
            enabled: true,
            createdAt: new Date().toISOString(),
            email: params.email || `${params.username}@tealdrive.internal`,
          };
        },

        async setQuota(params: { username: string; quotaBytes: number }): Promise<VirtualUser> {
          const realClient = await ensureConnected();
          if (!realClient) throw new Error('Client not connected');
          await realClient.admin.virtualUsers.setQuota({
            accountId: params.username,
            quotaBytes: params.quotaBytes,
          });
          return {
            id: params.username,
            username: params.username,
            uid: 1000,
            quotaBytes: params.quotaBytes,
            usedBytes: 0,
            enabled: true,
            createdAt: new Date().toISOString(),
            email: `${params.username}@tealdrive.internal`,
          };
        },

        async toggleStatus(params: { username: string; enabled: boolean }): Promise<VirtualUser> {
          const realClient = await ensureConnected();
          if (!realClient) throw new Error('Client not connected');
          if (params.enabled) {
            await realClient.admin.virtualUsers.enable({ accountId: params.username });
          } else {
            await realClient.admin.virtualUsers.disable({ accountId: params.username });
          }
          return {
            id: params.username,
            username: params.username,
            uid: 1000,
            quotaBytes: 1024 * 1024 * 1024,
            usedBytes: 0,
            enabled: params.enabled,
            createdAt: new Date().toISOString(),
            email: `${params.username}@tealdrive.internal`,
          };
        },

        async delete(params: { username: string }): Promise<boolean> {
          const realClient = await ensureConnected();
          if (!realClient) return false;
          await realClient.admin.virtualUsers.delete({ accountId: params.username });
          return true;
        },
      },

      systemUsers: {
        async list(): Promise<SystemUser[]> {
          const realClient = await ensureConnected();
          if (realClient) {
            try {
              const res = await realClient.admin.systemUsers.list();
              const list = Array.isArray(res) ? res : (res?.users || res?.items || []);
              return list.map((su: any) => ({
                username: su.username,
                uid: su.uid,
                shell: su.shell || '/bin/bash',
                homeDir: su.homeDir || su.home_directory || `/home/${su.username}`,
                approvedForWeb: Boolean(su.registered ?? su.approvedForWeb ?? su.approved),
                lastLogin: su.lastLogin || su.last_login,
              }));
            } catch (err) {
              console.warn('Real admin.systemUsers.list failed:', err);
            }
          }
          return [];
        },

        async approve(params: { username: string }): Promise<SystemUser> {
          const realClient = await ensureConnected();
          if (!realClient) throw new Error('Client not connected');
          await realClient.admin.systemUsers.approve({ username: params.username });
          return {
            username: params.username,
            uid: 1000,
            shell: '/bin/bash',
            homeDir: `/home/${params.username}`,
            approvedForWeb: true,
          };
        },
      },

      quota: {
        async getBudget(): Promise<StorageBudget> {
          const realClient = await ensureConnected();
          if (realClient) {
            try {
              let b: any = null;
              try {
                b = await realClient.admin.quota.getBudget();
              } catch (e) {}

              let q: any = null;
              try {
                const quotaRes = await realClient.files.quota.get({ rootId: 'home' });
                q = quotaRes;
              } catch (e) {}

              if (b && typeof b === 'object' && (b.limit_bytes || b.totalDiskBytes)) {
                const total = Number(b.limit_bytes ?? b.totalDiskBytes ?? 10737418240);
                const reserved = Number(b.reserved_bytes ?? b.virtualUsersAllocatedBytes ?? 0);
                const used = Number(b.used_bytes ?? q?.usedBytes ?? q?.used_bytes ?? 0);
                const free = Number(b.available_bytes ?? b.remaining_bytes ?? Math.max(0, total - reserved - used));
                return {
                  totalDiskBytes: total,
                  virtualUsersAllocatedBytes: reserved,
                  systemReserveBytes: used,
                  freePhysicalBytes: free,
                };
              }

              if (q && typeof q === 'object' && (q.limitBytes || q.limit_bytes)) {
                const total = Number(q.limitBytes ?? q.limit_bytes ?? 10737418240);
                const used = Number(q.usedBytes ?? q.used_bytes ?? 0);
                const free = Math.max(0, total - used);
                return {
                  totalDiskBytes: total,
                  virtualUsersAllocatedBytes: 0,
                  systemReserveBytes: used,
                  freePhysicalBytes: free,
                };
              }
            } catch (err) {
              console.warn('Real getBudget failed:', err);
            }
          }
          return {
            totalDiskBytes: 10737418240,
            virtualUsersAllocatedBytes: 0,
            systemReserveBytes: 0,
            freePhysicalBytes: 10737418240,
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
              if (cfg && (cfg.server || cfg.limits || cfg.security)) {
                const srv = cfg.server ?? {};
                const lim = cfg.limits ?? {};
                const sec = cfg.security ?? {};
                const rat = cfg.rate_limit ?? {};
                const lif = cfg.lifecycle ?? {};

                return {
                  serverName: srv.server_name || 'TealDrive Production Daemon',
                  version: 'TealDrive Core 2.0 (Rust)',
                  listenAddr: srv.bind_addr || '127.0.0.1:13100',
                  wsEncryption: sec.production_wire_protocol === 'Tdrv1Binary'
                    ? 'Tdrv1Binary (Noise-XX + AES/ChaChaPoly)'
                    : (sec.production_wire_protocol || 'Noise_XX_25519_ChaChaPoly_BLAKE2s'),
                  tlsCertExpiry: '2027-10-13T21:06:35Z',
                  rateLimitPerSec: rat.max_login_failures ? rat.max_login_failures * 100 : 500,
                  maxUploadSizeMb: Math.round((lim.max_in_flight_upload || 1073741824) / (1024 * 1024)),
                  autoTrashPurgeDays: Math.round((lif.trash_retention_hours || 504) / 24),
                  noiseHandshakePattern: srv.channel_key_path
                    ? 'Noise_XX (Static Channel Key Required)'
                    : 'Noise_XX (25519-ChaChaPoly)',
                };
              }
            } catch (err) {
              console.warn('Real admin.settings.inspect failed:', err);
            }
          }
          return {
            serverName: 'TealDrive Production Daemon',
            version: 'TealDrive Core 2.0 (Rust)',
            listenAddr: '127.0.0.1:13100',
            wsEncryption: 'Noise_XX_25519_ChaChaPoly_BLAKE2s',
            tlsCertExpiry: 'N/A',
            rateLimitPerSec: 500,
            maxUploadSizeMb: 1024,
            autoTrashPurgeDays: 21,
            noiseHandshakePattern: 'Noise_XX (25519-ChaChaPoly)',
          };
        },

        async update(updates: Partial<ServerSettings>): Promise<ServerSettings> {
          const realClient = await ensureConnected();
          if (realClient) {
            try {
              const changes: any = {};
              if (updates.maxUploadSizeMb !== undefined) {
                changes.limits = { max_in_flight_upload: updates.maxUploadSizeMb * 1024 * 1024 };
              }
              if (updates.autoTrashPurgeDays !== undefined) {
                changes.lifecycle = { trash_retention_hours: updates.autoTrashPurgeDays * 24 };
              }
              if (Object.keys(changes).length > 0) {
                await realClient.admin.settings.patch({ changes, persist: true });
              }
              return await this.inspect();
            } catch (err) {
              console.warn('Real admin.settings.update failed:', err);
            }
          }
          return await this.inspect();
        },
      },
    },
  };
}
