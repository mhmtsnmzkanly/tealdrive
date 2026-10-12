/**
 * TealDrive browser SDK v1. Transport is independent of DOM or UI framework.
 * Served with the Rust-generated contract and the tested binary codec.
 * @example const api=createClient(); await api.handshake(); await api.auth.login({username,password});
 * @example await api.files.list({filter:{name:'report'},sort:{by:'name',direction:'asc'}});
 */
export const SDK_VERSION = "1.0.0";

/** Stable error: use code/params for i18n, requestId for diagnosis. Never log credentials. */
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
    Object.assign(this, { code, params, requestId, retryable, status });
  }
}
const fail = (code, details) => {
  throw new TealDriveError(code, details);
};
const clone = (value) => structuredClone(value);
const freeze = (value) => {
  if (value && typeof value === "object") {
    Object.values(value).forEach(freeze);
    Object.freeze(value);
  }
  return value;
};
const uuid = () => {
  if (crypto.randomUUID) return crypto.randomUUID();
  const b = crypto.getRandomValues(new Uint8Array(16));
  b[6] = (b[6] & 15) | 64;
  b[8] = (b[8] & 63) | 128;
  const h = [...b].map((v) => v.toString(16).padStart(2, "0")).join("");
  return `${h.slice(0, 8)}-${h.slice(8, 12)}-${h.slice(12, 16)}-${h.slice(16, 20)}-${h.slice(20)}`;
};
const camel = (key) => key.replace(/_([a-z])/g, (_, c) => c.toUpperCase());
const publicValue = (value) =>
  Array.isArray(value)
    ? value.map(publicValue)
    : value && typeof value === "object"
      ? Object.fromEntries(
          Object.entries(value).map(([k, v]) => [camel(k), publicValue(v)]),
        )
      : value;
function object(value, allowed) {
  if (value === undefined) value = {};
  if (!value || typeof value !== "object" || Array.isArray(value))
    fail("INVALID_ARGUMENT");
  for (const key of Object.keys(value))
    if (!allowed.includes(key))
      fail("INVALID_ARGUMENT", { params: { field: key } });
  return value;
}
function integer(value, min, max, field) {
  if (!Number.isSafeInteger(value) || value < min || value > max)
    fail("INVALID_ARGUMENT", { params: { field } });
  return value;
}
function path(value) {
  if (
    typeof value !== "string" ||
    value.length > 4096 ||
    /[\\\x00-\x1f\x7f]/.test(value)
  )
    fail("INVALID_PATH");
  const result = value.startsWith("/") ? value.slice(1) : value;
  if (
    result
      .split("/")
      .some((p) => p === ".." || p === "." || (p === "" && result !== ""))
  )
    fail("INVALID_PATH");
  return result;
}
function name(value) {
  if (
    typeof value !== "string" ||
    !value ||
    value === "." ||
    value === ".." ||
    /[\/\\\x00-\x1f\x7f]/.test(value)
  )
    fail("INVALID_NAME");
  return value;
}
const errorCodes = {
  PERMISSIONDENIED: "PERMISSION_DENIED",
  POLICYDENIED: "POLICY_DENIED",
  NOTFOUND: "NOT_FOUND",
  ALREADYEXISTS: "ALREADY_EXISTS",
  INVALIDPATH: "INVALID_PATH",
  INVALIDTARGET: "INVALID_TARGET",
  FILETOOLARGE: "FILE_TOO_LARGE",
  DISKFULL: "DISK_FULL",
  REVISIONCONFLICT: "REVISION_CONFLICT",
  FEATUREDISABLED: "FEATURE_DISABLED",
  VALIDATIONERROR: "VALIDATION_FAILED",
  INTERNALERROR: "INTERNAL_ERROR",
};
const canonicalCode = (code) => errorCodes[code] ?? code ?? "OPERATION_FAILED";
const sortKeys = {
  name: "Name",
  modifiedAt: "Modified",
  size: "Size",
  type: "Type",
};
const kinds = {
  file: "File",
  directory: "Directory",
  symlink: "Symlink",
  other: "Other",
};

/**
 * @param {object} [options]
 * @param {'managed'|'client'} [options.stateMode='managed'] Optional SDK navigation state.
 * @param {Storage|null} [options.storage] Default localStorage; errors fall back to RAM.
 * @param {number} [options.timeoutMs=60000] Per request/handshake deadline.
 * @param {number} [options.warningPercent=80] Presentation threshold; server limit is authoritative.
 * @returns {TealDriveClient} Three domains: auth, files, admin.
 */
export function createClient(options = {}) {
  return new TealDriveClient(options);
}

export class TealDriveClient {
  constructor(options = {}) {
    object(options, [
      "stateMode",
      "storage",
      "timeoutMs",
      "warningPercent",
      "WebSocket",
      "fetch",
      "location",
      "wire",
      "contract",
      "quotaPollMs",
    ]);
    this._mode = options.stateMode ?? "managed";
    if (!["managed", "client"].includes(this._mode)) fail("INVALID_ARGUMENT");
    this._timeout = integer(
      options.timeoutMs ?? 60000,
      100,
      300000,
      "timeoutMs",
    );
    this._quotaPollMs = integer(
      options.quotaPollMs ?? 60000,
      0,
      3600000,
      "quotaPollMs",
    );
    this._quotaTimer = null;
    this._threshold = integer(
      options.warningPercent ?? 80,
      1,
      100,
      "warningPercent",
    );
    this._fetch = options.fetch ?? globalThis.fetch.bind(globalThis);
    this._WebSocket = options.WebSocket ?? globalThis.WebSocket;
    this._location = options.location ?? globalThis.location;
    this._wire = options.wire ?? globalThis.TealDriveWire;
    this._contract = options.contract ?? TdrvContract;
    this._storage = null;
    try {
      this._storage =
        options.storage === undefined
          ? globalThis.localStorage
          : options.storage;
    } catch {}
    this._listeners = new Map();
    this._pending = new Map();
    this._socket = null;
    this._ready = false;
    this._session = null;
    this._generation = 0;
    this._navigation = 0;
    this._shutdown = false;
    this._handshakePromise = null;
    this._state = {
      connection: "disconnected",
      stale: true,
      currentDirectory: null,
      rootId: null,
      quotaWarning: { active: false, status: "unknown", quota: null },
    };
    this.auth = Object.freeze({
      login: async (o) => this._login(o),
      getSession: () => this._getSession(),
      logout: () => this._logout(),
      revokeSelf: async () =>
        this._featureHttp("self_revoke", "/api/auth/revoke", "POST", {}, true),
      beginEnrollment: async (o) =>
        this._featureHttp("enrollment", "/api/auth/enroll", "POST", o, false),
      completeEnrollment: async (o) =>
        this._featureHttp(
          "enrollment",
          "/api/auth/enroll/complete",
          "POST",
          o,
          false,
        ),
      changePassword: async (o) =>
        this._featureHttp(
          "web_password",
          "/api/auth/password",
          "POST",
          o,
          true,
        ),
    });
    this.files = Object.freeze({
      list: async (o) => this._list(o),
      stat: async (o) => this._stat(o),
      readText: async (o) => this._readText(o),
      writeText: async (o) => this._writeText(o),
      createFolder: async (o) => this._createFolder(o),
      createText: async (o) =>
        this._writeText({ ...o, expectedRevision: "missing" }),
      rename: async (o) => this._rename(o),
      move: async (o) => this._moveCopy("MoveFile", o),
      copy: async (o) => this._moveCopy("CopyFile", o),
      chmod: async (o) => this._chmod(o),
      search: async (o) => this._search(o),
      upload: async (o) => this._upload(o),
      download: async (o) => this._download(o),
      validatePath: (p, o) => this._validatePath(p, o?.rootId),
      goTo: (p, o) => this._goTo(p, o),
      getCurrentDirectory: () => this._current(),
      getState: () => this.getState(),
      refreshState: () => this._refreshState(),
      observeState: (fn) => this.observeState(fn),
      observeQuotaWarning: (fn) => this._observeWarning(fn),
      on: (event, fn) => this.on(event, fn),
      trash: Object.freeze({
        move: async (o) => this._trashMove(o),
        list: async (o) => this._trashList(o),
        restore: async (o) => this._restore(o),
        deletePermanently: async (o) => this._delete(o),
      }),
      archives: Object.freeze({
        create: async (o) => this._archive(o),
        extract: async (o) => this._extract(o),
      }),
      quota: Object.freeze({ get: () => this._quota() }),
      audit: Object.freeze({
        list: async (o) =>
          this._featureHttp("audit_history", "/api/audit", "GET", o, true),
      }),
      undo: Object.freeze({
        list: async (o) =>
          this._featureHttp("undo", "/api/undo", "GET", o, true),
        apply: async (o) =>
          this._featureHttp("undo", "/api/undo", "POST", o, true),
      }),
      policy: Object.freeze({
        get: async () =>
          this._featureHttp("user_policy", "/api/policy", "GET", {}, true),
        addRule: async (o) =>
          this._featureHttp("user_policy", "/api/policy", "POST", o, true),
        removeRule: async (o) =>
          this._featureHttp("user_policy", "/api/policy", "DELETE", o, true),
      }),
    });
    this.admin = Object.freeze({
      settings: Object.freeze({
        get: async () => this._http("/api/admin/config", "GET", {}, true),
        patch: async (o) => this._settings(o),
      }),
      systemUsers: Object.freeze({
        list: async () => this._http("/api/admin/users", "GET", {}, true),
        allow: async (o) =>
          this._featureHttp(
            "account_management",
            "/api/admin/system/allow",
            "POST",
            o,
            true,
          ),
        deny: async (o) =>
          this._featureHttp(
            "account_management",
            "/api/admin/system/deny",
            "POST",
            o,
            true,
          ),
      }),
      virtualUsers: Object.freeze(
        Object.fromEntries(
          ["list", "create", "setQuota", "disable", "enable", "delete"].map(
            (method) => [
              method,
              async (o) =>
                this._featureHttp(
                  "virtual_accounts",
                  `/api/admin/virtual/${method}`,
                  method === "list" ? "GET" : "POST",
                  o,
                  true,
                ),
            ],
          ),
        ),
      ),
      policy: Object.freeze({
        get: async (o) =>
          this._featureHttp("user_policy", "/api/admin/policy", "GET", o, true),
        addRule: async (o) =>
          this._featureHttp(
            "user_policy",
            "/api/admin/policy",
            "POST",
            o,
            true,
          ),
        removeRule: async (o) =>
          this._featureHttp(
            "user_policy",
            "/api/admin/policy",
            "DELETE",
            o,
            true,
          ),
      }),
      quota: Object.freeze({
        getBudget: async () =>
          this._featureHttp(
            "virtual_accounts",
            "/api/admin/budget",
            "GET",
            {},
            true,
          ),
      }),
    });
  }

  /** Register a transient event. Returned function unsubscribes; listeners never own security state. */
  on(event, callback) {
    if (typeof event !== "string" || typeof callback !== "function")
      fail("INVALID_ARGUMENT");
    const listeners = this._listeners.get(event) ?? new Set();
    listeners.add(callback);
    this._listeners.set(event, listeners);
    if (event.startsWith("quota.")) this._ensureQuotaPolling();
    return () => {
      listeners.delete(callback);
      if (!listeners.size) this._listeners.delete(event);
      if (![...this._listeners.keys()].some((e) => e.startsWith("quota.")))
        this._stopQuotaPolling();
    };
  }
  _emit(event, payload) {
    for (const fn of [...(this._listeners.get(event) ?? [])]) {
      try {
        fn(freeze(clone(payload)));
      } catch {
        /* Consumer failure must not corrupt transport. */
      }
    }
  }
  getState() {
    return freeze(clone(this._state));
  }
  observeState(fn) {
    if (typeof fn !== "function") fail("INVALID_ARGUMENT");
    fn(this.getState());
    return this.on("state.changed", fn);
  }
  _observeWarning(fn) {
    if (typeof fn !== "function") fail("INVALID_ARGUMENT");
    fn(freeze(clone(this._state.quotaWarning)));
    return this.on("quota.warning.changed", fn);
  }
  _update(patch) {
    this._state = { ...this._state, ...patch };
    this._emit("state.changed", this._state);
  }
  _require(auth = true) {
    if (!this._ready) fail("HANDSHAKE_REQUIRED");
    if (this._shutdown) fail("SERVER_SHUTTING_DOWN");
    if (auth && !this._session) fail("AUTH_REQUIRED");
  }

  /** WS-first; resolves only after protocol negotiation. TLS is performed by the browser. */
  handshake() {
    if (this._ready) return Promise.resolve(this.getInfo());
    if (this._handshakePromise) return this._handshakePromise;
    const { protocol, host } = this._location ?? {};
    if (!["https:", "http:"].includes(protocol) || !host)
      fail("INVALID_ENDPOINT");
    this._shutdown = false;
    this._update({ connection: "connecting", stale: true });
    const socket = new this._WebSocket(
      `${protocol === "https:" ? "wss:" : "ws:"}//${host}/ws/auth`,
    );
    socket.binaryType = "arraybuffer";
    this._socket = socket;
    this._handshakePromise = new Promise((resolve, reject) => {
      const timer = setTimeout(() => {
        socket.close();
        reject(new TealDriveError("TIMEOUT"));
      }, this._timeout);
      this._handshake = {
        resolve: (value) => {
          clearTimeout(timer);
          resolve(value);
        },
        reject: (error) => {
          clearTimeout(timer);
          reject(error);
        },
      };
      socket.onmessage = (event) => {
        if (this._socket !== socket) return;
        try {
          this._receive(this._wire.decodeFrame(event.data));
        } catch (error) {
          const failure = new TealDriveError("PROTOCOL_ERROR", {
            cause: error,
          });
          this._handshake?.reject(failure);
          this._rejectAll(failure);
          socket.close();
        }
      };
      socket.onclose = () => {
        if (this._socket !== socket) return;
        this._ready = false;
        this._handshakePromise = null;
        this._stopQuotaPolling();
        const error = new TealDriveError(
          this._shutdown ? "SERVER_SHUTTING_DOWN" : "CONNECTION_CLOSED",
        );
        this._handshake?.reject(error);
        this._rejectAll(error);
        this._update({ connection: "disconnected", stale: true });
        this._emit("connection.changed", { status: "disconnected" });
      };
      socket.onerror = () => {
        if (this._socket === socket)
          this._handshake?.reject(new TealDriveError("CONNECTION_FAILED"));
      };
    });
    return this._handshakePromise;
  }
  getInfo() {
    this._require(false);
    return freeze({
      sdkVersion: SDK_VERSION,
      protocolVersion: this._hello.protocol_version,
      limits: publicValue(this._limits),
      features: publicValue(this._hello.feature_flags),
      authenticated: Boolean(this._session),
    });
  }
  disconnect() {
    this._stopQuotaPolling();
    this._generation++;
    this._ready = false;
    this._handshakePromise = null;
    this._rejectAll(new TealDriveError("CONNECTION_CLOSED"));
    this._handshake?.reject(new TealDriveError("CONNECTION_CLOSED"));
    this._socket?.close();
    this._socket = null;
    this._update({ connection: "disconnected", stale: true });
  }
  _receive(frame) {
    const M = this._contract.messages;
    const { messageType: type, payload: p, requestId: id } = frame;
    if (type === M.ServerHello) {
      this._hello = p;
      this._update({ connection: "negotiating" });
      this._socket.send(
        this._wire.encodeFrame(
          M.ClientHello,
          {
            protocol_version: 1,
            supported_encodings: ["MessagePack", "RawBinary"],
            supported_compressions: ["None"],
            max_frame_size: p.max_frame_size,
            max_control_payload_size: p.max_control_payload_size,
            max_chunk_size: p.max_chunk_size,
            feature_flags: p.feature_flags,
          },
          uuid(),
        ),
      );
      return;
    }
    if (type === M.ProtocolReady) {
      this._limits = p.limits;
      this._ready = true;
      this._update({ connection: "auth_ready", stale: false });
      this._getSession()
        .then(() => {
          this._handshake.resolve(this.getInfo());
          this._emit("connection.changed", { status: this._state.connection });
        })
        .catch((error) => this._handshake.reject(error));
      return;
    }
    if (type === M.ConnectionStatus && p.shutting_down) {
      this._shutdown = true;
      this._update({ connection: "draining", stale: true });
      this._emit("server.shutdown", {
        deadlineSeconds: p.grace_period_seconds,
      });
      return;
    }
    if (type === M.AuthResult && !this._pending.has(id)) return; // Existing HttpOnly session resume, already read through /api/session.
    const pending = this._pending.get(id);
    if (!pending) return;
    if (
      [M.OperationStarted, M.OperationProgress].includes(type) &&
      !(
        (pending.method === "UploadBegin" && type === M.OperationStarted) ||
        (pending.method === "UploadChunk" && type === M.OperationProgress)
      )
    ) {
      try {
        pending.progress?.(publicValue(p));
      } catch {}
      return;
    }
    this._pending.delete(id);
    pending.cleanup();
    if (
      [
        M.Error,
        M.OperationFailed,
        M.PermissionDenied,
        M.PolicyDenied,
        M.ValidationError,
      ].includes(type)
    ) {
      const e = p.error ?? p;
      pending.reject(
        new TealDriveError(canonicalCode(e.code), {
          params: { operation: e.operation },
          requestId: id,
          retryable: e.retryable ?? false,
        }),
      );
      return;
    }
    if (type === M.AuthResult && p.authenticated === false) {
      pending.reject(
        new TealDriveError(p.code ?? "INVALID_CREDENTIALS", {
          requestId: id,
          status: p.status,
        }),
      );
      return;
    }
    pending.resolve(p);
  }
  _rejectAll(error) {
    for (const p of this._pending.values()) {
      p.cleanup();
      p.reject(error);
    }
    this._pending.clear();
  }
  _request(
    method,
    payload,
    { signal, onProgress, auth = true, raw = false } = {},
  ) {
    try {
      this._require(auth);
    } catch (error) {
      return Promise.reject(error);
    }
    if (
      signal !== undefined &&
      (!signal ||
        typeof signal.aborted !== "boolean" ||
        typeof signal.addEventListener !== "function" ||
        typeof signal.removeEventListener !== "function")
    )
      return Promise.reject(
        new TealDriveError("INVALID_ARGUMENT", { params: { field: "signal" } }),
      );
    if (onProgress !== undefined && typeof onProgress !== "function")
      return Promise.reject(
        new TealDriveError("INVALID_ARGUMENT", {
          params: { field: "onProgress" },
        }),
      );
    if (signal?.aborted) return Promise.reject(new TealDriveError("ABORTED"));
    const type = this._contract.messages[method];
    if (type === undefined)
      return Promise.reject(
        new TealDriveError("FEATURE_DISABLED", { params: { feature: method } }),
      );
    const id = uuid();
    const key = this._wire.uuidKey(id);
    return new Promise((resolve, reject) => {
      let timer;
      const cleanup = () => {
        clearTimeout(timer);
        signal?.removeEventListener("abort", abort);
      };
      const abort = () => {
        this._pending.delete(key);
        cleanup();
        reject(
          new TealDriveError("ABORTED", {
            requestId: key,
            params: { operationMayHaveCompleted: true },
          }),
        );
      };
      timer = setTimeout(() => {
        this._pending.delete(key);
        cleanup();
        reject(
          new TealDriveError("TIMEOUT", {
            requestId: key,
            params: { operationMayHaveCompleted: true },
          }),
        );
      }, this._timeout);
      this._pending.set(key, {
        resolve,
        reject,
        cleanup,
        progress: onProgress,
        method,
      });
      signal?.addEventListener("abort", abort, { once: true });
      try {
        this._socket.send(this._wire.encodeFrame(type, payload, id, raw));
      } catch (error) {
        this._pending.delete(key);
        cleanup();
        reject(new TealDriveError("CONNECTION_FAILED", { cause: error }));
      }
    });
  }
  async _http(url, method = "GET", body = {}, auth = true) {
    this._require(auth);
    if (method === "GET" && body && Object.keys(body).length) {
      const params = new URLSearchParams(
        Object.entries(body)
          .filter(([, v]) => v !== null && v !== undefined)
          .map(([k, v]) => [k, String(v)]),
      );
      url += `?${params}`;
    }
    const response = await this._fetch(url, {
      method,
      credentials: "same-origin",
      headers: {
        "Content-Type": "application/json",
        ...(method === "GET"
          ? {}
          : { "x-csrf-token": this._session?.csrfToken ?? "" }),
      },
      ...(method === "GET" ? {} : { body: JSON.stringify(body) }),
    });
    const data =
      response.status === 204 ? null : await response.json().catch(() => null);
    if (!response.ok)
      throw new TealDriveError(
        data?.code ??
          (response.status === 401
            ? "AUTH_REQUIRED"
            : response.status === 403
              ? "PERMISSION_DENIED"
              : "HTTP_ERROR"),
        { status: response.status },
      );
    return publicValue(data);
  }
  _featureHttp(feature, url, method, body, auth) {
    this._require(auth);
    if (!this._hello?.feature_flags?.[feature])
      fail("FEATURE_DISABLED", { params: { feature } });
    return this._http(url, method, body, auth);
  }
  async _getSession() {
    this._require(false);
    let data;
    try {
      data = await this._http("/api/session", "GET", {}, false);
    } catch (error) {
      if (error.status !== 401) throw error;
      data = null;
    }
    if (!data?.authenticated) {
      this._session = null;
      this._update({
        connection: "auth_ready",
        rootId: null,
        currentDirectory: null,
      });
      return null;
    }
    this._setSession(data);
    return freeze(clone(this._session));
  }
  _storageKey() {
    const s = this._session;
    return s
      ? `tealdrive:v1:${this._location.origin ?? this._location.host}:${s.accountId ?? `${s.uid}:${s.username}`}:${s.accountGeneration ?? 0}`
      : null;
  }
  _setSession(session) {
    const changed =
      this._session?.uid !== session.uid ||
      this._session?.username !== session.username;
    this._session = session;
    if (changed) {
      this._generation++;
      this._navigation++;
    }
    let location = {
      rootId: session.roots?.[0]?.rootId ?? null,
      currentDirectory: "/",
    };
    if (this._mode === "managed")
      try {
        const restored = JSON.parse(
          this._storage?.getItem(this._storageKey()) ?? "null",
        );
        if (
          restored &&
          session.roots.some((r) => r.rootId === restored.rootId)
        ) {
          location = {
            rootId: restored.rootId,
            currentDirectory: "/" + path(restored.path),
          };
        }
      } catch {}
    this._update({ ...location, connection: "authenticated", stale: changed });
    this._ensureQuotaPolling();
  }
  async _login(options) {
    object(options, ["username", "password"]);
    this._require(false);
    if (
      typeof options.username !== "string" ||
      typeof options.password !== "string"
    )
      fail("INVALID_ARGUMENT");
    if (this._session) fail("ALREADY_AUTHENTICATED");
    const result = await this._request(
      "AuthLogin",
      { username: options.username, password: options.password },
      { auth: false },
    );
    const bind = await this._fetch("/api/session/bind", {
      method: "POST",
      credentials: "same-origin",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ token: result.bind_token }),
    });
    if (!bind.ok) {
      this.disconnect();
      fail("SESSION_BIND_FAILED");
    }
    await this._getSession();
    if (!this._session) fail("SESSION_BIND_FAILED");
    if (this._mode === "managed") await this._refreshState();
    return freeze(clone(this._session));
  }
  async _logout() {
    await this._http("/api/logout", "POST", {}, false);
    this._session = null;
    this._generation++;
    this._update({
      rootId: null,
      currentDirectory: null,
      quotaWarning: { active: false, status: "unknown", quota: null },
    });
    this.disconnect();
    this._emit("session.revoked", {});
  }
  _ref(options = {}) {
    this._require();
    const rootId = options.rootId ?? this._state.rootId;
    if (
      typeof rootId !== "string" ||
      !this._session.roots.some((r) => r.rootId === rootId)
    )
      fail("INVALID_ROOT");
    if (this._mode === "client" && options.path === undefined)
      fail("INVALID_ARGUMENT", { params: { field: "path" } });
    return {
      root_id: rootId,
      relative_path: path(options.path ?? this._state.currentDirectory ?? "/"),
    };
  }
  async _operation(method, payload, options = {}) {
    const result = publicValue(await this._request(method, payload, options));
    if (
      ["MoveToTrash", "DeletePermanently", "MoveFile", "RenameFile"].includes(
        method,
      )
    ) {
      this._emit("files.changed", {
        operation: method,
        rootId: payload.root_id,
        path: payload.relative_path,
      });
      const current = path(this._state.currentDirectory ?? "/");
      if (
        this._mode === "managed" &&
        this._state.rootId === payload.root_id &&
        (current === payload.relative_path ||
          current.startsWith(payload.relative_path + "/"))
      ) {
        try {
          await this._refreshState();
        } catch {
          this._update({ stale: true });
        }
      }
    }
    return result;
  }
  _list(options = {}) {
    object(options, [
      "rootId",
      "path",
      "filter",
      "sort",
      "limit",
      "cursor",
      "includeHidden",
      "signal",
    ]);
    const filter = object(options.filter, [
      "name",
      "type",
      "category",
      "extensions",
    ]);
    const sort = object(options.sort, ["by", "direction"]);
    const by = sortKeys[sort.by ?? "name"];
    if (!by || !["asc", "desc"].includes(sort.direction ?? "asc"))
      fail("INVALID_ARGUMENT");
    if (filter.type && !kinds[filter.type]) fail("INVALID_ARGUMENT");
    if (
      filter.name !== undefined &&
      (typeof filter.name !== "string" || filter.name.length > 1024)
    )
      fail("INVALID_ARGUMENT");
    const extensions = filter.extensions ?? null;
    if (
      extensions !== null &&
      (!Array.isArray(extensions) ||
        extensions.length > 64 ||
        extensions.some(
          (e) =>
            typeof e !== "string" ||
            e.length > 32 ||
            !/^([a-z0-9]+)(\.[a-z0-9]+)*$/i.test(e),
        ))
    )
      fail("INVALID_ARGUMENT");
    const categories = [
      "All",
      "Folders",
      "Code",
      "Images",
      "Media",
      "Documents",
      "Archives",
      "Hidden",
      "Writable",
      "Symlinks",
    ];
    const category = filter.category ?? "All";
    if (!categories.includes(category)) fail("INVALID_ARGUMENT");
    return this._operation(
      "ListDirectory",
      {
        ...this._ref(options),
        cursor: options.cursor ?? null,
        limit: integer(
          options.limit ?? this._limits.default_directory_page_size,
          1,
          this._limits.max_directory_page_size,
          "limit",
        ),
        include_hidden: options.includeHidden ?? false,
        sort: by + (sort.direction === "desc" ? "Desc" : "Asc"),
        filter: {
          query: filter.name ?? null,
          file_kind: kinds[filter.type] ?? null,
          filter: category,
          extensions,
        },
      },
      options,
    );
  }
  _stat(options = {}) {
    object(options, ["rootId", "path", "signal"]);
    return this._operation("FileMetadataRequest", this._ref(options), options);
  }
  _readText(options = {}) {
    object(options, ["rootId", "path", "maxBytes", "signal"]);
    return this._operation(
      "ReadTextFile",
      {
        ...this._ref(options),
        max_bytes: options.maxBytes ?? null,
        encoding: "utf-8",
      },
      options,
    );
  }
  _writeText(options = {}) {
    object(options, [
      "rootId",
      "path",
      "content",
      "expectedRevision",
      "signal",
    ]);
    if (typeof options.content !== "string") fail("INVALID_ARGUMENT");
    if (
      typeof options.expectedRevision !== "string" ||
      !(
        /^[a-f0-9]{64}$/.test(options.expectedRevision) ||
        options.expectedRevision === "missing"
      )
    )
      fail("INVALID_ARGUMENT", { params: { field: "expectedRevision" } });
    return this._operation(
      "SaveTextFile",
      {
        ...this._ref(options),
        content: options.content,
        expected_modified: null,
        expected_revision: options.expectedRevision,
      },
      options,
    );
  }
  _createFolder(options = {}) {
    object(options, ["rootId", "path", "signal"]);
    const ref = this._ref(options);
    const parts = ref.relative_path.split("/");
    const folder = name(parts.pop());
    return this._operation(
      "CreateFolder",
      {
        root_id: ref.root_id,
        parent_relative_path: parts.join("/"),
        folder_name: folder,
      },
      options,
    );
  }
  _rename(options = {}) {
    object(options, ["rootId", "path", "newName", "signal"]);
    return this._operation(
      "RenameFile",
      { ...this._ref(options), new_name: name(options.newName) },
      options,
    );
  }
  _moveCopy(method, options = {}) {
    object(options, ["source", "destination", "signal"]);
    const source = this._ref(options.source);
    const target = this._ref(options.destination);
    return this._operation(
      method,
      {
        ...source,
        target_root_id: target.root_id,
        target_relative_path: target.relative_path,
      },
      options,
    );
  }
  _chmod(options = {}) {
    object(options, ["rootId", "path", "mode", "signal"]);
    return this._operation(
      "ChmodFile",
      { ...this._ref(options), mode: integer(options.mode, 0, 0o777, "mode") },
      options,
    );
  }
  _search(options = {}) {
    object(options, [
      "rootId",
      "path",
      "query",
      "extension",
      "limit",
      "includeHidden",
      "signal",
    ]);
    if (typeof options.query !== "string" || options.query.length > 1024)
      fail("INVALID_ARGUMENT");
    const ref = this._ref(options);
    return this._operation(
      "SearchFiles",
      {
        root_id: ref.root_id,
        base_path: ref.relative_path,
        query: options.query,
        extension: options.extension ?? null,
        max_results: options.limit ?? 100,
        include_hidden: options.includeHidden ?? false,
      },
      options,
    );
  }
  _trashMove(options = {}) {
    object(options, ["rootId", "path", "signal"]);
    return this._operation("MoveToTrash", this._ref(options), options);
  }
  _trashList(options = {}) {
    object(options, ["rootId", "signal"]);
    return this._operation(
      "ListTrash",
      { root_id: this._ref({ ...options, path: "" }).root_id },
      options,
    );
  }
  _restore(options = {}) {
    object(options, ["trashId", "destination", "signal"]);
    if (typeof options.trashId !== "string") fail("INVALID_ARGUMENT");
    const target = options.destination ? this._ref(options.destination) : null;
    return this._operation(
      "RestoreFromTrash",
      {
        trash_id: options.trashId,
        target_root_id: target?.root_id ?? null,
        target_relative_path: target?.relative_path ?? null,
      },
      options,
    );
  }
  _delete(options = {}) {
    object(options, ["rootId", "trashId", "signal"]);
    const ref = this._ref({
      rootId: options.rootId,
      path: ".tealdrive-trash/files/" + name(options.trashId),
    });
    return this._operation("DeletePermanently", ref, options);
  }
  _archive(options = {}) {
    object(options, ["rootId", "sources", "path", "format", "signal"]);
    const ref = this._ref(options);
    if (!Array.isArray(options.sources) || !options.sources.length)
      fail("INVALID_ARGUMENT");
    if (!["zip", "tar.gz"].includes(options.format ?? "zip"))
      fail("INVALID_ARGUMENT");
    return this._operation(
      options.format === "tar.gz" ? "CompressTarGz" : "CompressZip",
      {
        root_id: ref.root_id,
        relative_paths: options.sources.map(path),
        target_relative_path: ref.relative_path,
      },
      options,
    );
  }
  _extract(options = {}) {
    object(options, ["rootId", "path", "destination", "signal"]);
    const ref = this._ref(options);
    return this._operation(
      "ExtractArchive",
      {
        root_id: ref.root_id,
        archive_relative_path: ref.relative_path,
        target_relative_path: path(options.destination),
      },
      options,
    );
  }

  _current() {
    if (!this._session || this._state.currentDirectory === null)
      fail("STATE_NOT_READY");
    return this._state.currentDirectory;
  }
  async _validatePath(value, rootId = this._state.rootId) {
    const normalized = path(value);
    const result = normalized
      ? await this._stat({ rootId, path: normalized })
      : await this._list({ rootId, path: "", limit: 1 });
    if (normalized && result.entry?.fileType !== "Directory")
      fail("NOT_A_DIRECTORY");
    return { valid: true, path: "/" + normalized };
  }
  async _goTo(value, options = {}) {
    object(options, ["rootId"]);
    if (this._mode !== "managed") fail("STATE_DISABLED");
    const navigation = ++this._navigation;
    const generation = this._generation;
    const rootId = options.rootId ?? this._state.rootId;
    const result = await this._validatePath(value, rootId);
    if (navigation !== this._navigation || generation !== this._generation)
      fail("STATE_CONFLICT");
    this._update({ rootId, currentDirectory: result.path, stale: false });
    this._saveLocation();
    return this.getState();
  }
  _saveLocation() {
    if (this._mode !== "managed" || !this._session) return;
    try {
      this._storage?.setItem(
        this._storageKey(),
        JSON.stringify({
          rootId: this._state.rootId,
          path: path(this._state.currentDirectory),
        }),
      );
    } catch {}
  }
  async _refreshState() {
    this._require();
    let current = this._state.currentDirectory ?? "/";
    for (;;) {
      try {
        return await this._goTo(current);
      } catch (error) {
        if (
          ![
            "NOT_FOUND",
            "PERMISSION_DENIED",
            "POLICY_DENIED",
            "INVALID_PATH",
            "NOT_A_DIRECTORY",
          ].includes(error.code)
        )
          throw error;
        if (current === "/") throw error;
        current = current.slice(0, current.lastIndexOf("/")) || "/";
      }
    }
  }
  _stopQuotaPolling() {
    clearTimeout(this._quotaTimer);
    this._quotaTimer = null;
  }
  _ensureQuotaPolling() {
    if (
      this._quotaTimer ||
      !this._quotaPollMs ||
      !this._ready ||
      !this._session ||
      ![...this._listeners.keys()].some((e) => e.startsWith("quota."))
    )
      return;
    this._quotaTimer = setTimeout(async () => {
      this._quotaTimer = null;
      try {
        await this._quota();
      } catch {}
      this._ensureQuotaPolling();
    }, this._quotaPollMs);
  }
  async _quota() {
    let data;
    try {
      data = await this._http(
        "/api/quota",
        "GET",
        { root_id: this._state.rootId },
        true,
      );
    } catch (error) {
      const old = this._state.quotaWarning;
      const warning = {
        ...old,
        status: "unknown",
        quota: old.quota ? { ...old.quota, stale: true } : null,
      };
      this._update({ quotaWarning: warning });
      this._emit("quota.warning.changed", warning);
      throw error;
    }
    const percent =
      data.usagePercent ??
      (data.limitBytes > 0 && data.usedBytes !== null
        ? (100 * data.usedBytes) / data.limitBytes
        : null);
    const quota = {
      ...data,
      usagePercent: percent,
      warningPercent: data.warningPercent ?? this._threshold,
    };
    const active = percent !== null && percent >= quota.warningPercent;
    const old = this._state.quotaWarning;
    const warning = {
      active,
      status: percent === null ? "unknown" : "known",
      quota,
    };
    this._update({ quotaWarning: warning });
    this._emit("quota.warning.changed", warning);
    if (active && !old.active) this._emit("quota.warning.entered", quota);
    if (!active && old.active) this._emit("quota.warning.cleared", quota);
    return freeze(clone(quota));
  }
  _settings(options = {}) {
    object(options, ["changes"]);
    return this._http(
      "/api/admin/config?save=true",
      "PATCH",
      options.changes,
      true,
    );
  }
  async _download(options = {}) {
    object(options, ["rootId", "path", "signal"]);
    const ref = this._ref(options);
    const query = new URLSearchParams({
      root_id: ref.root_id,
      path: ref.relative_path,
    });
    const response = await this._fetch("/api/download?" + query, {
      credentials: "same-origin",
      signal: options.signal,
    });
    if (!response.ok) fail("DOWNLOAD_FAILED", { status: response.status });
    return response;
  }
  async _upload(options = {}) {
    object(options, ["rootId", "directory", "file", "signal", "onProgress"]);
    const file = options.file;
    if (
      !file ||
      typeof file.slice !== "function" ||
      !Number.isSafeInteger(file.size) ||
      file.size < 0
    )
      fail("INVALID_ARGUMENT");
    const ref = this._ref({ rootId: options.rootId, path: options.directory });
    const max = this._limits.max_in_flight_upload;
    if (file.size > max) fail("FILE_TOO_LARGE");
    let transfer = null;
    try {
      const begin = await this._request(
        "UploadBegin",
        {
          ...ref,
          filename: name(file.name),
          total_size: file.size,
          chunk_size: this._limits.max_chunk_size,
          sha256: null,
        },
        options,
      );
      transfer = begin.transfer_id;
      integer(begin.chunk_size, 1, this._limits.max_chunk_size, "chunkSize");
      let offset = 0,
        index = 0;
      while (offset < file.size) {
        if (options.signal?.aborted) fail("ABORTED");
        const chunk = new Uint8Array(
          await file.slice(offset, offset + begin.chunk_size).arrayBuffer(),
        );
        await this._request(
          "UploadChunk",
          this._wire.encodeChunk(transfer, index, offset, chunk),
          { ...options, raw: true },
        );
        offset += chunk.length;
        index++;
        options.onProgress?.({
          transferredBytes: offset,
          totalBytes: file.size,
        });
      }
      const result = await this._request(
        "UploadEnd",
        { transfer_id: transfer, total_size: file.size, sha256: null },
        options,
      );
      return publicValue(result);
    } catch (error) {
      if (transfer && this._ready)
        try {
          await this._request("UploadAbort", {
            transfer_id: transfer,
            reason: "client cancellation or failure",
          });
        } catch {}
      throw error;
    }
  }
}
