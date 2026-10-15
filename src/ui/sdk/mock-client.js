/**
 * TealDrive Standalone Mock SDK v1.1.0
 * 
 * Drop-in replacement for `client.js` allowing UI developers to build, test,
 * and preview the complete TealDrive SPA locally without a running Rust backend
 * or TLS/Noise encryption setup.
 *
 * Fully adheres to the TealDriveClient contract defined in `client.d.ts`.
 * 
 * Features:
 * - Realistic virtual in-memory filesystem with nested folders, text files, images, PDFs, archives.
 * - Live state management (`observeState`, `observeQuotaWarning`, `on('state.changed', ...)`).
 * - Full CRUD: createFolder, createText, readText, writeText with revision check, rename, move, copy, chmod, delete.
 * - Chunked upload simulation with `onProgress` and `onTransfer` callbacks.
 * - Trash bin with move, list, restore, and permanent delete.
 * - Audit log & Undo candidate tracking with `undo.apply()`.
 * - Admin API for Virtual Users, System Users, Storage Budget, and Settings.
 * - Optional browser persistence (localStorage / sessionStorage) and reset capability.
 */

export const SDK_VERSION = "1.1.0-mock";

export class TealDriveError extends Error {
  constructor(
    code,
    {
      params = {},
      requestId = null,
      retryable = false,
      status = null,
      cause,
    } = {},
  ) {
    super(code, { cause });
    this.name = "TealDriveError";
    Object.assign(this, { code, params, requestId: requestId || `mock-req-${Math.random().toString(36).slice(2, 9)}`, retryable, status });
  }
}

const fail = (code, details) => {
  throw new TealDriveError(code, details);
};

const delay = (ms = 40) => new Promise((resolve) => setTimeout(resolve, ms));

const clone = (obj) => JSON.parse(JSON.stringify(obj));

const DEFAULT_MOCK_FILES = {
  "/": {
    type: "Directory",
    mode: 0o755,
    modified: "2026-10-15T10:00:00Z",
    children: ["Documents", "Media", "Projects", "Backups", "welcome.txt", ".system_config"],
  },
  "/Documents": {
    type: "Directory",
    mode: 0o755,
    modified: "2026-10-14T14:20:00Z",
    children: [
      "Q3_Financial_Report.pdf",
      "Product_Roadmap_2026.docx",
      "Meeting_Notes.txt",
      "Security_Policies.md",
    ],
  },
  "/Documents/Q3_Financial_Report.pdf": {
    type: "File",
    size: 2450816,
    mode: 0o644,
    modified: "2026-10-14T12:00:00Z",
    mime: "application/pdf",
  },
  "/Documents/Product_Roadmap_2026.docx": {
    type: "File",
    size: 512400,
    mode: 0o644,
    modified: "2026-10-13T09:15:00Z",
    mime: "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
  },
  "/Documents/Meeting_Notes.txt": {
    type: "File",
    size: 3840,
    mode: 0o644,
    modified: "2026-10-15T08:30:00Z",
    content: "TealDrive Frontend Redesign Notes\n=================================\n\n1. Replace legacy server-rendered table with modern SPA.\n2. Add drag-and-drop file upload with chunked progress.\n3. Integrate Monaco/CodeMirror text editor for config and code files.\n4. Support keyboard navigation (Arrow keys, Space for preview, Cmd+K search).\n5. Responsive mobile drawer & touch friendly context menus.\n",
    revision: "rev-doc-1",
    mime: "text/plain",
  },
  "/Documents/Security_Policies.md": {
    type: "File",
    size: 6144,
    mode: 0o644,
    modified: "2026-10-12T16:45:00Z",
    content: "# TealDrive Security & Access Policy\n\n- Zero-trust encrypted WebSocket channel with Noise protocol.\n- Virtual user sandboxing with root isolation.\n- Automatic quota enforcement and undo audit window.\n- Role-based administration and PAM integration.\n",
    revision: "rev-sec-1",
    mime: "text/markdown",
  },
  "/Media": {
    type: "Directory",
    mode: 0o755,
    modified: "2026-10-15T09:00:00Z",
    children: ["Wallpapers", "Videos", "Audio"],
  },
  "/Media/Wallpapers": {
    type: "Directory",
    mode: 0o755,
    modified: "2026-10-14T11:00:00Z",
    children: ["teal_nordic_aurora.png", "cloud_datacenter_dark.jpg"],
  },
  "/Media/Wallpapers/teal_nordic_aurora.png": {
    type: "File",
    size: 3412580,
    mode: 0o644,
    modified: "2026-10-14T11:00:00Z",
    mime: "image/png",
  },
  "/Media/Wallpapers/cloud_datacenter_dark.jpg": {
    type: "File",
    size: 4210900,
    mode: 0o644,
    modified: "2026-10-13T17:22:00Z",
    mime: "image/jpeg",
  },
  "/Media/Videos": {
    type: "Directory",
    mode: 0o755,
    modified: "2026-10-10T14:00:00Z",
    children: ["tealdrive_architecture_demo.mp4"],
  },
  "/Media/Videos/tealdrive_architecture_demo.mp4": {
    type: "File",
    size: 28410200,
    mode: 0o644,
    modified: "2026-10-10T14:00:00Z",
    mime: "video/mp4",
  },
  "/Media/Audio": {
    type: "Directory",
    mode: 0o755,
    modified: "2026-10-11T16:00:00Z",
    children: ["system_podcast_ep12.mp3"],
  },
  "/Media/Audio/system_podcast_ep12.mp3": {
    type: "File",
    size: 14205000,
    mode: 0o644,
    modified: "2026-10-11T16:00:00Z",
    mime: "audio/mpeg",
  },
  "/Projects": {
    type: "Directory",
    mode: 0o755,
    modified: "2026-10-15T09:30:00Z",
    children: ["tealdrive-web-ui"],
  },
  "/Projects/tealdrive-web-ui": {
    type: "Directory",
    mode: 0o755,
    modified: "2026-10-15T09:35:00Z",
    children: ["package.json", "vite.config.ts", "README.md", "src"],
  },
  "/Projects/tealdrive-web-ui/package.json": {
    type: "File",
    size: 1120,
    mode: 0o644,
    modified: "2026-10-15T09:31:00Z",
    content: '{\n  "name": "tealdrive-web-ui",\n  "version": "2.0.0",\n  "type": "module",\n  "scripts": {\n    "dev": "vite",\n    "build": "tsc && vite build"\n  },\n  "dependencies": {\n    "lucide-react": "^0.450.0"\n  }\n}\n',
    revision: "rev-pkg-1",
    mime: "application/json",
  },
  "/Projects/tealdrive-web-ui/vite.config.ts": {
    type: "File",
    size: 420,
    mode: 0o644,
    modified: "2026-10-15T09:32:00Z",
    content: 'import { defineConfig } from "vite";\n\nexport default defineConfig({\n  server: {\n    port: 3000,\n  },\n});\n',
    revision: "rev-vite-1",
    mime: "text/plain",
  },
  "/Projects/tealdrive-web-ui/README.md": {
    type: "File",
    size: 1850,
    mode: 0o644,
    modified: "2026-10-15T09:33:00Z",
    content: "# TealDrive NextGen Web UI\n\nA modern, sleek, high-performance file management SPA built with modern web technologies.\n",
    revision: "rev-rm-1",
    mime: "text/markdown",
  },
  "/Projects/tealdrive-web-ui/src": {
    type: "Directory",
    mode: 0o755,
    modified: "2026-10-15T09:34:00Z",
    children: ["App.tsx", "main.tsx"],
  },
  "/Projects/tealdrive-web-ui/src/App.tsx": {
    type: "File",
    size: 2450,
    mode: 0o644,
    modified: "2026-10-15T09:35:00Z",
    content: 'export function App() {\n  return <div className="p-8 text-teal-600 font-bold">Hello TealDrive!</div>;\n}\n',
    revision: "rev-app-1",
    mime: "text/plain",
  },
  "/Projects/tealdrive-web-ui/src/main.tsx": {
    type: "File",
    size: 480,
    mode: 0o644,
    modified: "2026-10-15T09:35:00Z",
    content: 'import React from "react";\nimport ReactDOM from "react-dom/client";\nimport { App } from "./App";\n\nReactDOM.createRoot(document.getElementById("root")!).render(<App />);\n',
    revision: "rev-main-1",
    mime: "text/plain",
  },
  "/Backups": {
    type: "Directory",
    mode: 0o700,
    modified: "2026-10-14T03:00:00Z",
    children: ["database_20261014.tar.gz", "assets_archive.zip"],
  },
  "/Backups/database_20261014.tar.gz": {
    type: "File",
    size: 89400000,
    mode: 0o600,
    modified: "2026-10-14T03:00:00Z",
    mime: "application/gzip",
  },
  "/Backups/assets_archive.zip": {
    type: "File",
    size: 45200000,
    mode: 0o600,
    modified: "2026-10-13T03:00:00Z",
    mime: "application/zip",
  },
  "/welcome.txt": {
    type: "File",
    size: 1420,
    mode: 0o644,
    modified: "2026-10-15T10:00:00Z",
    content: "Welcome to TealDrive 2.0!\n\nThis instance is powered by the TealDrive Mock SDK. You can navigate folders, upload files, edit documents, test quota triggers, and manage users.\n",
    revision: "rev-wel-1",
    mime: "text/plain",
  },
  "/.system_config": {
    type: "File",
    size: 320,
    mode: 0o600,
    modified: "2026-10-10T00:00:00Z",
    content: "theme=dark\naccent=#0d9488\nshow_hidden=false\n",
    revision: "rev-sys-1",
    mime: "text/plain",
  },
};

const DEFAULT_VIRTUAL_USERS = [
  {
    id: "vu_dev_01",
    username: "john_doe",
    uid: 200001,
    gid: 200000,
    home: "/home/virtual/john_doe",
    kind: "virtual",
    role: "user",
    status: "active",
    loginEnabled: true,
    generation: 1,
    passwordEnrolled: true,
    quotaBytes: 10737418240, // 10 GB
    undoHours: 24,
    createdAt: 1760400000,
    updatedAt: 1760400000,
    deletedAt: null,
    denyEnrollment: false,
    ownerAdminId: "u_admin_01",
  },
  {
    id: "vu_dev_02",
    username: "sarah_connor",
    uid: 200002,
    gid: 200000,
    home: "/home/virtual/sarah_connor",
    kind: "virtual",
    role: "user",
    status: "active",
    loginEnabled: true,
    generation: 1,
    passwordEnrolled: true,
    quotaBytes: 5368709120, // 5 GB
    undoHours: 48,
    createdAt: 1760410000,
    updatedAt: 1760410000,
    deletedAt: null,
    denyEnrollment: false,
    ownerAdminId: "u_admin_01",
  },
];

const DEFAULT_SYSTEM_USERS = [
  { username: "tealdrive", uid: 200000, gid: 200000, registered: true },
  { username: "caddy", uid: 33, gid: 33, registered: false },
  { username: "ubuntu", uid: 1000, gid: 1000, registered: false },
];

export class MockTealDriveClient {
  constructor(options = {}) {
    this._options = options;
    this._latency = options.latency ?? 40;
    this._persist = Boolean(options.persist);
    this._storageKey = options.storageKey || "tealdrive_mock_db";

    this._subscribers = {
      state: new Set(),
      quota: new Set(),
      events: new Map(),
    };

    this._initData();
  }

  _initData() {
    let saved = null;
    if (this._persist && typeof localStorage !== "undefined") {
      try {
        const raw = localStorage.getItem(this._storageKey);
        if (raw) saved = JSON.parse(raw);
      } catch {}
    }

    this._fs = saved?.fs || clone(DEFAULT_MOCK_FILES);
    this._trash = saved?.trash || [];
    this._audit = saved?.audit || [];
    this._virtualUsers = saved?.virtualUsers || clone(DEFAULT_VIRTUAL_USERS);
    this._systemUsers = saved?.systemUsers || clone(DEFAULT_SYSTEM_USERS);

    this._session = saved?.session || {
      authenticated: true,
      isAdmin: true,
      username: "tealdrive",
      uid: 200000,
      gid: 200000,
      role: "admin",
      accountKind: "virtual",
      csrfToken: "mock-csrf-token-" + Math.random().toString(36).slice(2),
      roots: [
        {
          rootId: "default-root",
          readOnly: false,
          isWebRoot: true,
          uploadsAllowed: true,
          hiddenFilesAllowed: true,
          usedBytes: 198500000,
          quotaBytes: 10737418240, // 10 GB
        },
      ],
    };

    this._state = {
      connection: "authenticated",
      stale: false,
      currentDirectory: "/",
      rootId: "default-root",
      quotaWarning: {
        active: false,
        status: "known",
        quota: {
          limitBytes: 10737418240,
          usedBytes: 198500000,
          usagePercent: 1.85,
          warningPercent: 80,
          measuredAt: Date.now(),
          stale: false,
          measurement: "logical_bytes",
          source: "helper",
        },
      },
    };

    this._serverInfo = {
      transfers: { uploadResumeSeconds: 300 },
      transport: {
        encryptedWebSocket: true,
        protocol: "Noise_NK_25519_ChaChaPoly_SHA256",
        keyTrust: "tls-discovery",
        httpEncryptedChannel: false,
      },
      sdkVersion: SDK_VERSION,
      protocolVersion: 1,
      limits: {
        max_chunk_size: 1048576,
        max_in_flight_upload: 10737418240,
        max_simultaneous_uploads: 4,
      },
      features: {
        resumableUploads: true,
        undo: true,
        auditHistory: true,
        virtualAccounts: true,
        systemAccounts: true,
        policyManagement: true,
      },
      authentication: {
        webPassword: true,
        passwordChange: true,
        selfRevoke: false,
        accountManagement: true,
        accountPagination: true,
        auditHistory: true,
        undo: true,
        linuxApproval: true,
        policyManagement: true,
        virtualAccounts: true,
        virtualDelete: true,
        enrollment: true,
        selfRegistration: false,
      },
      authenticated: true,
    };

    this._initAPIs();
  }

  _save() {
    if (this._persist && typeof localStorage !== "undefined") {
      try {
        localStorage.setItem(
          this._storageKey,
          JSON.stringify({
            fs: this._fs,
            trash: this._trash,
            audit: this._audit,
            virtualUsers: this._virtualUsers,
            systemUsers: this._systemUsers,
            session: this._session,
          }),
        );
      } catch {}
    }
  }

  resetMockData() {
    this._fs = clone(DEFAULT_MOCK_FILES);
    this._trash = [];
    this._audit = [];
    this._virtualUsers = clone(DEFAULT_VIRTUAL_USERS);
    this._systemUsers = clone(DEFAULT_SYSTEM_USERS);
    this._state.currentDirectory = "/";
    this._save();
    this._notifyState();
    this._emit("files.changed", { operation: "reset", path: "/" });
  }

  _notifyState() {
    const s = clone(this._state);
    for (const sub of this._subscribers.state) {
      try {
        sub(s);
      } catch (err) {
        console.error("Mock state subscriber error:", err);
      }
    }
    this._emit("state.changed", s);
  }

  _emit(event, payload) {
    const subs = this._subscribers.events.get(event);
    if (subs) {
      for (const cb of subs) {
        try {
          cb(payload);
        } catch (err) {
          console.error(`Mock event listener [${event}] error:`, err);
        }
      }
    }
  }

  _addAudit(action, sourcePath, targetPath = null, status = "success") {
    const record = {
      version: 1,
      id: `audit-${Date.now()}-${Math.random().toString(36).slice(2, 6)}`,
      requestId: `req-${Date.now()}`,
      accountId: this._session?.username || "anonymous",
      generation: 1,
      createdAt: Math.floor(Date.now() / 1000),
      updatedAt: Math.floor(Date.now() / 1000),
      action,
      rootId: this._state.rootId,
      sourcePath,
      targetPath,
      status,
      bytesRead: 0,
      bytesWritten: 0,
      undoStatus: "pending",
    };
    this._audit.unshift(record);
    if (this._audit.length > 200) this._audit.pop();
    this._save();
    return record;
  }

  // --- Core Lifecycle ---
  async handshake() {
    await delay(this._latency);
    return clone(this._serverInfo);
  }

  getInfo() {
    return clone(this._serverInfo);
  }

  getState() {
    return clone(this._state);
  }

  observeState(callback) {
    this._subscribers.state.add(callback);
    callback(clone(this._state));
    return () => this._subscribers.state.delete(callback);
  }

  observeQuotaWarning(callback) {
    this._subscribers.quota.add(callback);
    callback(clone(this._state.quotaWarning));
    return () => this._subscribers.quota.delete(callback);
  }

  on(event, callback) {
    if (!this._subscribers.events.has(event)) {
      this._subscribers.events.set(event, new Set());
    }
    this._subscribers.events.get(event).add(callback);
    return () => {
      const set = this._subscribers.events.get(event);
      if (set) set.delete(callback);
    };
  }

  disconnect() {
    this._state.connection = "disconnected";
    this._notifyState();
  }

  // --- Domain APIs Initialization ---
  _initAPIs() {
    const client = this;

    // 1. AUTH API
    this.auth = {
      async login({ username, password }) {
        await delay(client._latency * 2);
        if (!username || !password) fail("INVALID_ARGUMENT");
        if (password === "wrong") fail("INVALID_CREDENTIALS");

        const isAdmin = username === "tealdrive" || username === "admin";
        client._session = {
          authenticated: true,
          isAdmin,
          username,
          uid: isAdmin ? 200000 : 200001,
          gid: 200000,
          role: isAdmin ? "admin" : "user",
          accountKind: "virtual",
          csrfToken: "mock-csrf-" + Math.random().toString(36).slice(2),
          roots: [
            {
              rootId: "default-root",
              readOnly: false,
              isWebRoot: true,
              uploadsAllowed: true,
              hiddenFilesAllowed: true,
              usedBytes: 198500000,
              quotaBytes: 10737418240,
            },
          ],
        };
        client._state.connection = "authenticated";
        client._save();
        client._notifyState();
        return clone(client._session);
      },

      async getSession() {
        await delay(client._latency);
        return clone(client._session);
      },

      async logout() {
        await delay(client._latency);
        client._session = null;
        client._state.connection = "auth_ready";
        client._save();
        client._notifyState();
      },

      async revokeSelf() {
        await delay(client._latency);
        await this.logout();
      },

      async beginEnrollment({ username, linuxPassword }) {
        await delay(client._latency);
        if (!username || !linuxPassword) fail("INVALID_ARGUMENT");
        return {
          enrollmentRequired: true,
          expiresAt: Math.floor(Date.now() / 1000) + 300,
        };
      },

      async completeEnrollment({ webPassword }) {
        await delay(client._latency);
        if (!webPassword || webPassword.length < 12)
          fail("INVALID_ARGUMENT", { params: { reason: "password_too_short" } });
        return await this.login({ username: "enrolled_user", password: webPassword });
      },

      async changePassword({ currentPassword, newPassword }) {
        await delay(client._latency);
        if (!newPassword || newPassword.length < 12) fail("INVALID_ARGUMENT");
        client._addAudit("changePassword", "/auth/password");
      },
    };

    // 2. FILES API
    this.files = {
      getCurrentDirectory() {
        return client._state.currentDirectory;
      },

      getState() {
        return clone(client._state);
      },

      async refreshState() {
        await delay(client._latency);
        return clone(client._state);
      },

      observeState(cb) {
        return client.observeState(cb);
      },

      observeQuotaWarning(cb) {
        return client.observeQuotaWarning(cb);
      },

      on(evt, cb) {
        return client.on(evt, cb);
      },

      async validatePath(path) {
        await delay(client._latency / 2);
        const norm = client._norm(path);
        return { valid: true, path: norm };
      },

      async goTo(path) {
        await delay(client._latency);
        const norm = client._norm(path);
        if (!client._fs[norm]) fail("NOT_FOUND", { params: { path: norm } });
        if (client._fs[norm].type !== "Directory") fail("NOT_A_DIRECTORY");
        client._state.currentDirectory = norm;
        client._notifyState();
        return clone(client._state);
      },

      async list(options = {}) {
        await delay(client._latency);
        const targetPath = client._norm(options.path || client._state.currentDirectory);
        const dir = client._fs[targetPath];
        if (!dir) fail("NOT_FOUND", { params: { path: targetPath } });
        if (dir.type !== "Directory") fail("NOT_A_DIRECTORY", { params: { path: targetPath } });

        const childrenNames = dir.children || [];
        let entries = childrenNames.map((name) => {
          const childPath = targetPath === "/" ? `/${name}` : `${targetPath}/${name}`;
          const item = client._fs[childPath] || {
            type: "File",
            size: 1024,
            mode: 0o644,
            modified: new Date().toISOString(),
          };
          const isDir = item.type === "Directory";
          return {
            name,
            fileType: item.type,
            size: isDir ? 0 : (item.size || 0),
            modified: item.modified || null,
            mode: item.mode || (isDir ? 0o755 : 0o644),
            permissions: isDir ? "drwxr-xr-x" : "-rw-r--r--",
            capabilities: {
              canRead: true,
              canWrite: true,
              canDelete: true,
              canRename: true,
              canMove: true,
              canChmod: true,
            },
          };
        });

        // Filter: Hidden
        if (!options.includeHidden) {
          entries = entries.filter((e) => !e.name.startsWith("."));
        }

        // Filter: Options filter
        if (options.filter) {
          const f = options.filter;
          if (f.name) {
            const query = f.name.toLowerCase();
            entries = entries.filter((e) => e.name.toLowerCase().includes(query));
          }
          if (f.type) {
            entries = entries.filter((e) => e.fileType.toLowerCase() === f.type.toLowerCase());
          }
          if (f.extensions && f.extensions.length > 0) {
            const exts = f.extensions.map((x) => (x.startsWith(".") ? x.slice(1) : x).toLowerCase());
            entries = entries.filter((e) => {
              if (e.fileType === "Directory") return true;
              const ext = e.name.split(".").pop().toLowerCase();
              return exts.includes(ext);
            });
          }
          if (f.category && f.category !== "All") {
            const cat = f.category;
            entries = entries.filter((e) => {
              if (cat === "Folders") return e.fileType === "Directory";
              if (cat === "Images") return /\.(png|jpe?g|gif|webp|svg)$/i.test(e.name);
              if (cat === "Media") return /\.(mp4|webm|mp3|wav|ogg)$/i.test(e.name);
              if (cat === "Documents") return /\.(pdf|docx?|xlsx?|pptx?|txt|md)$/i.test(e.name);
              if (cat === "Code") return /\.(js|jsx|ts|tsx|rs|py|json|html|css)$/i.test(e.name);
              if (cat === "Archives") return /\.(zip|tar(\.gz)?|rar|7z)$/i.test(e.name);
              if (cat === "Hidden") return e.name.startsWith(".");
              return true;
            });
          }
        }

        // Sorting
        const sort = options.sort || { by: "name", direction: "asc" };
        const factor = sort.direction === "desc" ? -1 : 1;
        entries.sort((a, b) => {
          // Directories always first
          if (a.fileType === "Directory" && b.fileType !== "Directory") return -1;
          if (a.fileType !== "Directory" && b.fileType === "Directory") return 1;

          if (sort.by === "size") return (a.size - b.size) * factor;
          if (sort.by === "modifiedAt") {
            return (new Date(a.modified || 0) - new Date(b.modified || 0)) * factor;
          }
          return a.name.localeCompare(b.name) * factor;
        });

        // Pagination
        const limit = options.limit || entries.length;
        const pageEntries = entries.slice(0, limit);
        const hasMore = entries.length > limit;

        return {
          entries: pageEntries,
          currentPath: targetPath,
          nextCursor: hasMore ? "cursor-page-2" : null,
          hasMore,
          totalEstimate: entries.length,
        };
      },

      async stat(options = {}) {
        await delay(client._latency);
        const path = client._norm(options.path || client._state.currentDirectory);
        const item = client._fs[path];
        if (!item) fail("NOT_FOUND", { params: { path } });
        const name = path.split("/").filter(Boolean).pop() || "/";
        const isDir = item.type === "Directory";

        return {
          rootId: client._state.rootId,
          relativePath: path,
          entry: {
            name,
            fileType: item.type,
            size: isDir ? 0 : item.size || 0,
            modified: item.modified || null,
            mode: item.mode || (isDir ? 0o755 : 0o644),
            permissions: isDir ? "drwxr-xr-x" : "-rw-r--r--",
            capabilities: {
              canRead: true,
              canWrite: true,
              canDelete: true,
              canRename: true,
              canMove: true,
              canChmod: true,
            },
          },
        };
      },

      async readText(options = {}) {
        await delay(client._latency);
        const path = client._norm(options.path);
        const item = client._fs[path];
        if (!item) fail("NOT_FOUND", { params: { path } });
        if (item.type === "Directory") fail("IS_A_DIRECTORY", { params: { path } });

        const content = item.content !== undefined ? item.content : `[Mock file content for ${path}]`;
        return {
          content,
          revision: item.revision || "rev-init",
          rootId: client._state.rootId,
          relativePath: path,
          size: content.length,
          encoding: "utf-8",
          mode: item.mode || 0o644,
        };
      },

      async writeText(options = {}) {
        await delay(client._latency);
        const path = client._norm(options.path);
        const item = client._fs[path];
        if (!item) fail("NOT_FOUND", { params: { path } });
        if (options.expectedRevision && item.revision && options.expectedRevision !== item.revision) {
          fail("REVISION_CONFLICT", { params: { expected: options.expectedRevision, current: item.revision } });
        }

        item.content = options.content;
        item.size = (options.content || "").length;
        item.modified = new Date().toISOString();
        item.revision = `rev-${Date.now()}`;
        client._save();
        client._addAudit("writeText", path);
        client._emit("files.changed", { operation: "writeText", path });

        return {
          requestId: `mock-req-${Date.now()}`,
          operation: "writeText",
          safeMessage: "File saved successfully",
        };
      },

      async createText(options = {}) {
        await delay(client._latency);
        const path = client._norm(options.path);
        if (client._fs[path]) fail("ALREADY_EXISTS", { params: { path } });

        const parentPath = client._parent(path);
        const fileName = path.split("/").pop();
        if (!client._fs[parentPath] || client._fs[parentPath].type !== "Directory") {
          fail("NOT_FOUND", { params: { path: parentPath } });
        }

        client._fs[path] = {
          type: "File",
          size: (options.content || "").length,
          mode: 0o644,
          modified: new Date().toISOString(),
          content: options.content || "",
          revision: `rev-${Date.now()}`,
          mime: "text/plain",
        };

        if (!client._fs[parentPath].children.includes(fileName)) {
          client._fs[parentPath].children.push(fileName);
        }

        client._save();
        client._addAudit("createText", path);
        client._emit("files.changed", { operation: "createText", path });

        return {
          requestId: `mock-req-${Date.now()}`,
          operation: "createText",
          safeMessage: "Text file created",
        };
      },

      async createFolder(options = {}) {
        await delay(client._latency);
        const path = client._norm(options.path);
        if (client._fs[path]) fail("ALREADY_EXISTS", { params: { path } });

        const parentPath = client._parent(path);
        const folderName = path.split("/").pop();
        if (!client._fs[parentPath] || client._fs[parentPath].type !== "Directory") {
          fail("NOT_FOUND", { params: { path: parentPath } });
        }

        client._fs[path] = {
          type: "Directory",
          mode: 0o755,
          modified: new Date().toISOString(),
          children: [],
        };

        if (!client._fs[parentPath].children.includes(folderName)) {
          client._fs[parentPath].children.push(folderName);
        }

        client._save();
        client._addAudit("createFolder", path);
        client._emit("files.changed", { operation: "createFolder", path });

        return {
          requestId: `mock-req-${Date.now()}`,
          operation: "createFolder",
          safeMessage: "Directory created",
        };
      },

      async rename(options = {}) {
        await delay(client._latency);
        const path = client._norm(options.path);
        const newName = options.newName;
        if (!client._fs[path]) fail("NOT_FOUND", { params: { path } });
        if (!newName || newName.includes("/")) fail("INVALID_ARGUMENT", { params: { field: "newName" } });

        const parentPath = client._parent(path);
        const oldName = path.split("/").pop();
        const newPath = parentPath === "/" ? `/${newName}` : `${parentPath}/${newName}`;

        if (client._fs[newPath]) fail("ALREADY_EXISTS", { params: { path: newPath } });

        // Rename in tree
        client._fs[newPath] = client._fs[path];
        delete client._fs[path];

        // Update parent list
        const pChildren = client._fs[parentPath]?.children;
        if (pChildren) {
          const idx = pChildren.indexOf(oldName);
          if (idx !== -1) pChildren[idx] = newName;
        }

        client._save();
        client._addAudit("rename", path, newPath);
        client._emit("files.changed", { operation: "rename", path: newPath });

        return {
          requestId: `mock-req-${Date.now()}`,
          operation: "rename",
          safeMessage: "Renamed successfully",
        };
      },

      async move(options = {}) {
        await delay(client._latency);
        const src = client._norm(options.source?.path);
        const destDir = client._norm(options.destination?.path || "/");
        if (!client._fs[src]) fail("NOT_FOUND", { params: { path: src } });
        if (!client._fs[destDir] || client._fs[destDir].type !== "Directory") fail("NOT_A_DIRECTORY");

        const itemName = src.split("/").pop();
        const targetPath = destDir === "/" ? `/${itemName}` : `${destDir}/${itemName}`;
        if (client._fs[targetPath]) fail("ALREADY_EXISTS", { params: { path: targetPath } });

        // Remove from old parent
        const oldParent = client._parent(src);
        if (client._fs[oldParent]?.children) {
          client._fs[oldParent].children = client._fs[oldParent].children.filter((c) => c !== itemName);
        }

        // Add to new parent
        client._fs[destDir].children.push(itemName);
        client._fs[targetPath] = client._fs[src];
        delete client._fs[src];

        client._save();
        client._addAudit("move", src, targetPath);
        client._emit("files.changed", { operation: "move", path: targetPath });

        return {
          requestId: `mock-req-${Date.now()}`,
          operation: "move",
        };
      },

      async copy(options = {}) {
        await delay(client._latency);
        const src = client._norm(options.source?.path);
        const destDir = client._norm(options.destination?.path || "/");
        if (!client._fs[src]) fail("NOT_FOUND", { params: { path: src } });
        if (!client._fs[destDir] || client._fs[destDir].type !== "Directory") fail("NOT_A_DIRECTORY");

        const itemName = src.split("/").pop();
        const targetPath = destDir === "/" ? `/${itemName}` : `${destDir}/${itemName}`;
        if (client._fs[targetPath]) fail("ALREADY_EXISTS", { params: { path: targetPath } });

        client._fs[targetPath] = clone(client._fs[src]);
        client._fs[targetPath].modified = new Date().toISOString();
        client._fs[destDir].children.push(itemName);

        client._save();
        client._addAudit("copy", src, targetPath);
        client._emit("files.changed", { operation: "copy", path: targetPath });

        return {
          requestId: `mock-req-${Date.now()}`,
          operation: "copy",
        };
      },

      async chmod(options = {}) {
        await delay(client._latency);
        const path = client._norm(options.path);
        if (!client._fs[path]) fail("NOT_FOUND", { params: { path } });
        client._fs[path].mode = options.mode;

        client._save();
        client._addAudit("chmod", path);
        client._emit("files.changed", { operation: "chmod", path });

        return {
          requestId: `mock-req-${Date.now()}`,
          operation: "chmod",
        };
      },

      async deletePermanently(options = {}) {
        await delay(client._latency);
        const path = client._norm(options.path);
        if (!client._fs[path]) fail("NOT_FOUND", { params: { path } });
        if (path === "/") fail("INVALID_ARGUMENT", { params: { reason: "cannot_delete_root" } });

        const parentPath = client._parent(path);
        const name = path.split("/").pop();

        if (client._fs[parentPath]?.children) {
          client._fs[parentPath].children = client._fs[parentPath].children.filter((c) => c !== name);
        }
        delete client._fs[path];

        client._save();
        client._addAudit("deletePermanently", path);
        client._emit("files.changed", { operation: "deletePermanently", path });

        return {
          requestId: `mock-req-${Date.now()}`,
          operation: "deletePermanently",
        };
      },

      async search(options = {}) {
        await delay(client._latency * 1.5);
        const query = (options.query || "").toLowerCase();
        const matches = [];

        for (const [path, item] of Object.entries(client._fs)) {
          if (path === "/") continue;
          const name = path.split("/").pop();
          if (!options.includeHidden && name.startsWith(".")) continue;
          if (options.extension && !name.toLowerCase().endsWith(options.extension.toLowerCase())) continue;

          if (name.toLowerCase().includes(query)) {
            const isDir = item.type === "Directory";
            matches.push({
              relativePath: path,
              entry: {
                name,
                fileType: item.type,
                size: isDir ? 0 : item.size || 0,
                modified: item.modified || null,
                mode: item.mode || (isDir ? 0o755 : 0o644),
                permissions: isDir ? "drwxr-xr-x" : "-rw-r--r--",
                capabilities: { canRead: true, canWrite: true, canDelete: true },
              },
            });
          }
        }

        return {
          matches: matches.slice(0, options.limit || 50),
          truncated: matches.length > (options.limit || 50),
        };
      },

      async upload(options = {}) {
        const file = options.file;
        if (!file) fail("INVALID_ARGUMENT", { params: { field: "file" } });

        const dir = client._norm(options.directory || client._state.currentDirectory);
        if (!client._fs[dir] || client._fs[dir].type !== "Directory") fail("NOT_A_DIRECTORY");

        const transferId = `transfer-${Date.now()}-${Math.random().toString(36).slice(2, 6)}`;
        if (options.onTransfer) {
          options.onTransfer({
            transferId,
            resumedBytes: 0,
            expiresInSeconds: 300,
          });
        }

        // Simulate chunked upload progress
        const chunks = 5;
        const total = file.size || 1024;
        for (let i = 1; i <= chunks; i++) {
          if (options.signal?.aborted) fail("ABORTED");
          await delay(client._latency);
          if (options.onProgress) {
            options.onProgress({
              transferredBytes: Math.min(Math.round((total / chunks) * i), total),
              totalBytes: total,
            });
          }
        }

        const filePath = dir === "/" ? `/${file.name}` : `${dir}/${file.name}`;
        client._fs[filePath] = {
          type: "File",
          size: total,
          mode: 0o644,
          modified: new Date().toISOString(),
          mime: file.type || "application/octet-stream",
          revision: `rev-${Date.now()}`,
        };

        if (!client._fs[dir].children.includes(file.name)) {
          client._fs[dir].children.push(file.name);
        }

        // Update quota usage
        if (client._state.quotaWarning.quota) {
          client._state.quotaWarning.quota.usedBytes += total;
          client._state.quotaWarning.quota.usagePercent = Number(
            ((client._state.quotaWarning.quota.usedBytes / client._state.quotaWarning.quota.limitBytes) * 100).toFixed(2)
          );
        }

        client._save();
        client._addAudit("upload", filePath);
        client._emit("files.changed", { operation: "upload", path: filePath });

        return {
          requestId: `mock-req-${Date.now()}`,
          operation: "upload",
          transferId,
          bytesWritten: total,
          sha256Verified: true,
        };
      },

      async abortUpload() {
        return { requestId: `mock-req-${Date.now()}`, operation: "abortUpload" };
      },

      async download(options = {}) {
        await delay(client._latency);
        const path = client._norm(options.path);
        const item = client._fs[path];
        if (!item) fail("NOT_FOUND", { params: { path } });
        const content = item.content || `Simulated file binary content for ${path}`;
        const blob = new Blob([content], { type: item.mime || "application/octet-stream" });
        return new Response(blob);
      },

      getDownloadUrl(options = {}) {
        const path = client._norm(options.path);
        return `/api/download?path=${encodeURIComponent(path)}`;
      },

      getPreviewUrl(options = {}) {
        const path = client._norm(options.path);
        return `/api/preview?path=${encodeURIComponent(path)}`;
      },

      trash: {
        async move(options = {}) {
          await delay(client._latency);
          const path = client._norm(options.path);
          if (!client._fs[path]) fail("NOT_FOUND", { params: { path } });
          const name = path.split("/").pop();
          const trashId = `trash-${Date.now()}-${Math.random().toString(36).slice(2, 6)}`;

          client._trash.push({
            trashId,
            originalPath: path,
            name,
            item: clone(client._fs[path]),
            deletedAt: new Date().toISOString(),
          });

          const parent = client._parent(path);
          if (client._fs[parent]?.children) {
            client._fs[parent].children = client._fs[parent].children.filter((c) => c !== name);
          }
          delete client._fs[path];

          client._save();
          client._addAudit("trash.move", path);
          client._emit("files.changed", { operation: "trash.move", path });
          return { requestId: `req-${Date.now()}`, operation: "trash.move" };
        },

        async list() {
          await delay(client._latency);
          return { items: clone(client._trash) };
        },

        async restore(options = {}) {
          await delay(client._latency);
          const idx = client._trash.findIndex((t) => t.trashId === options.trashId);
          if (idx === -1) fail("NOT_FOUND");
          const trashItem = client._trash.splice(idx, 1)[0];

          const destPath = options.destination?.path || trashItem.originalPath;
          const parent = client._parent(destPath);
          const name = destPath.split("/").pop();

          if (client._fs[parent]?.children) {
            client._fs[parent].children.push(name);
          }
          client._fs[destPath] = trashItem.item;

          client._save();
          client._addAudit("trash.restore", destPath);
          client._emit("files.changed", { operation: "trash.restore", path: destPath });
          return { requestId: `req-${Date.now()}`, operation: "trash.restore" };
        },

        async deletePermanently(options = {}) {
          await delay(client._latency);
          const idx = client._trash.findIndex((t) => t.trashId === options.trashId);
          if (idx === -1) fail("NOT_FOUND");
          client._trash.splice(idx, 1);
          client._save();
          return { requestId: `req-${Date.now()}`, operation: "trash.deletePermanently" };
        },
      },

      archives: {
        async create(options = {}) {
          await delay(client._latency * 2);
          const format = options.format || "zip";
          const archiveName = `archive_${Date.now()}.${format}`;
          const currentDir = client._state.currentDirectory;
          const archivePath = currentDir === "/" ? `/${archiveName}` : `${currentDir}/${archiveName}`;

          client._fs[archivePath] = {
            type: "File",
            size: 204800,
            mode: 0o644,
            modified: new Date().toISOString(),
            mime: format === "zip" ? "application/zip" : "application/gzip",
          };
          client._fs[currentDir].children.push(archiveName);

          client._save();
          client._emit("files.changed", { operation: "archive.create", path: archivePath });
          return { requestId: `req-${Date.now()}`, operation: "archive.create" };
        },

        async extract(options = {}) {
          await delay(client._latency * 2);
          return { requestId: `req-${Date.now()}`, operation: "archive.extract" };
        },
      },

      quota: {
        async get() {
          await delay(client._latency);
          return clone(client._state.quotaWarning.quota);
        },
      },

      audit: {
        async list(options = {}) {
          await delay(client._latency);
          return {
            items: clone(client._audit),
            nextCursor: null,
            hasMore: false,
          };
        },
      },

      undo: {
        async getPreferences() {
          return { hours: 24, effectiveHours: 24, maximumHours: 168, revision: 1 };
        },
        async setPreferences(options) {
          return { hours: options.hours, effectiveHours: options.hours || 24, maximumHours: 168, revision: 2 };
        },
        async list(options = {}) {
          await delay(client._latency);
          const candidates = client._audit
            .filter((a) => ["writeText", "rename", "move", "trash.move"].includes(a.action))
            .map((op) => ({
              operation: op,
              available: true,
              unavailableReason: null,
              expiresAt: op.createdAt + 86400,
              expectedState: "valid",
            }));
          return { items: candidates, nextCursor: null, hasMore: false };
        },
        async apply(options = {}) {
          await delay(client._latency);
          return { requestId: `req-${Date.now()}`, operation: "undo.apply" };
        },
      },

      policy: {
        async get() {
          return {
            account: { username: "mock_user", id: "u_1" },
            revision: "rev-pol-1",
            rules: { rootRules: [], adminRules: [], personalRules: [] },
            denySystemComponents: [],
          };
        },
        async set() {
          return { id: "u_1", username: "mock_user" };
        },
        async addRule() {
          return { id: "u_1", username: "mock_user" };
        },
        async removeRule() {
          return { id: "u_1", username: "mock_user" };
        },
      },
    };

    // 3. ADMIN API
    this.admin = {
      quota: {
        async getBudget() {
          await delay(client._latency);
          return {
            limitBytes: 53687091200, // 50 GB
            reservedBytes: 16106127360, // 15 GB
            remainingBytes: 37580963840,
            usedBytes: 198500000,
            enforcement: "application",
          };
        },
      },

      systemUsers: {
        async list() {
          await delay(client._latency);
          return { users: clone(client._systemUsers), truncated: false };
        },
        async accounts() {
          await delay(client._latency);
          return clone(client._systemUsers.map((u, i) => ({
            id: `sys_${u.username}`,
            username: u.username,
            uid: u.uid,
            gid: u.gid,
            home: `/home/${u.username}`,
            kind: "linux",
            role: u.username === "tealdrive" ? "admin" : "user",
            status: "active",
            loginEnabled: true,
            generation: 1,
            passwordEnrolled: true,
            quotaBytes: null,
            createdAt: 1760000000,
            updatedAt: 1760000000,
          })));
        },
        async accountsPage() {
          const accs = await this.accounts();
          return { items: accs, nextCursor: null, hasMore: false };
        },
        async approve(options = {}) {
          await delay(client._latency);
          return { id: `sys_${options.username}`, username: options.username };
        },
        async allow(options = {}) {
          return { id: options.accountId };
        },
        async deny(options = {}) {
          return { id: options.accountId };
        },
        async getPolicy() {
          return {
            account: { id: "sys_1", username: "sys" },
            revision: "1",
            rules: { rootRules: [], adminRules: [], personalRules: [] },
            denySystemComponents: [],
          };
        },
        async setPolicy() {
          return { id: "sys_1" };
        },
      },

      virtualUsers: {
        async list() {
          await delay(client._latency);
          return clone(client._virtualUsers);
        },
        async listPage() {
          await delay(client._latency);
          return { items: clone(client._virtualUsers), nextCursor: null, hasMore: false };
        },
        async create(options = {}) {
          await delay(client._latency);
          if (!options.name || !options.password) fail("INVALID_ARGUMENT");
          const newUser = {
            id: `vu_${Date.now()}`,
            username: options.name,
            uid: 200000 + client._virtualUsers.length + 1,
            gid: 200000,
            home: `/home/virtual/${options.name}`,
            kind: "virtual",
            role: "user",
            status: "active",
            loginEnabled: true,
            generation: 1,
            passwordEnrolled: true,
            quotaBytes: options.quotaBytes || 5368709120,
            undoHours: 24,
            createdAt: Math.floor(Date.now() / 1000),
            updatedAt: Math.floor(Date.now() / 1000),
            deletedAt: null,
            denyEnrollment: false,
            ownerAdminId: client._session?.username || "admin",
          };
          client._virtualUsers.push(newUser);
          client._save();
          client._emit("accounts.changed", { operation: "create", accountId: newUser.id, kind: "virtual" });
          return clone(newUser);
        },
        async setQuota(options = {}) {
          await delay(client._latency);
          const u = client._virtualUsers.find((x) => x.id === options.accountId);
          if (!u) fail("NOT_FOUND");
          u.quotaBytes = options.quotaBytes;
          client._save();
          return clone(u);
        },
        async disable(options = {}) {
          await delay(client._latency);
          const u = client._virtualUsers.find((x) => x.id === options.accountId);
          if (!u) fail("NOT_FOUND");
          u.loginEnabled = false;
          u.status = "suspended";
          client._save();
          return clone(u);
        },
        async enable(options = {}) {
          await delay(client._latency);
          const u = client._virtualUsers.find((x) => x.id === options.accountId);
          if (!u) fail("NOT_FOUND");
          u.loginEnabled = true;
          u.status = "active";
          client._save();
          return clone(u);
        },
        async delete(options = {}) {
          await delay(client._latency);
          const idx = client._virtualUsers.findIndex((x) => x.id === options.accountId);
          if (idx === -1) fail("NOT_FOUND");
          const removed = client._virtualUsers.splice(idx, 1)[0];
          client._save();
          client._emit("accounts.changed", { operation: "delete", accountId: removed.id, kind: "virtual" });
          return clone(removed);
        },
        async getPolicy() {
          return {
            account: { id: "vu_1", username: "v_user" },
            revision: "1",
            rules: { rootRules: [], adminRules: [], personalRules: [] },
            denySystemComponents: [],
          };
        },
        async setPolicy() {
          return { id: "vu_1" };
        },
      },

      settings: {
        async get() {
          return {
            server_name: "TealDrive Mock Node",
            upload_max_size_bytes: 10737418240,
            trash_retention_days: 30,
            allow_virtual_registration: false,
          };
        },
        async patch(options = {}) {
          return options.changes;
        },
        async inspect() {
          return {
            mode: "split",
            config: {
              server_port: 13100,
              tls_enabled: true,
              virtual_users_enabled: true,
            },
          };
        },
        async reload() {
          return { status: "reloaded" };
        },
      },

      policy: {
        async get() {
          return {
            account: { id: "adm_pol", username: "admin" },
            revision: "1",
            rules: { rootRules: [], adminRules: [], personalRules: [] },
            denySystemComponents: [],
          };
        },
        async set() {
          return { id: "adm_pol" };
        },
        async addRule() {
          return { id: "adm_pol" };
        },
        async removeRule() {
          return { id: "adm_pol" };
        },
      },
    };
  }

  // --- Path Helpers ---
  _norm(p = "/") {
    if (!p || p === ".") return "/";
    let norm = p.replace(/\\+/g, "/").replace(/\/+/g, "/");
    if (!norm.startsWith("/")) norm = "/" + norm;
    if (norm.length > 1 && norm.endsWith("/")) norm = norm.slice(0, -1);
    return norm;
  }

  _parent(p) {
    const norm = this._norm(p);
    if (norm === "/") return "/";
    const lastSlash = norm.lastIndexOf("/");
    return lastSlash === 0 ? "/" : norm.slice(0, lastSlash);
  }
}

/**
 * Creates a standalone mockup client instance.
 * Accepts options for latency, storage persistence, etc.
 */
export function createMockClient(options = {}) {
  return new MockTealDriveClient(options);
}

/**
 * Default factory compatible with `client.js` createClient.
 */
export function createClient(options = {}) {
  return new MockTealDriveClient(options);
}

export default createMockClient;
