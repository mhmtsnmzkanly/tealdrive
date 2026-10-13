/** TealDrive SDK v1. All paths are relative to the authenticated root; '/musics' is a virtual path. */
export const SDK_VERSION: string;
export class TealDriveError extends Error {
  readonly code: string;
  readonly params: Readonly<Record<string, unknown>>;
  readonly requestId: string | null;
  readonly retryable: boolean;
  readonly status: number | null;
}
export type Unsubscribe = () => void;
export interface RequestOptions {
  signal?: AbortSignal;
}
export interface FileRef {
  rootId?: string;
  path?: string;
}
export interface FileOptions extends FileRef, RequestOptions {}
export interface FileEntry {
  name: string;
  fileType: "File" | "Directory" | "Symlink" | "Other";
  size: number;
  modified: string | null;
  mode: number | null;
  permissions: string | null;
  capabilities: Readonly<Record<string, boolean>>;
}
export interface DirectoryPage {
  entries: FileEntry[];
  currentPath: string;
  nextCursor: string | null;
  hasMore: boolean;
  totalEstimate: number | null;
}
export interface ListOptions extends FileOptions {
  /** Name substring, types/extensions and preset category are applied BEFORE pagination by helper. */
  filter?: {
    name?: string;
    type?: "file" | "directory" | "symlink" | "other";
    extensions?: string[];
    category?:
      | "All"
      | "Folders"
      | "Code"
      | "Images"
      | "Media"
      | "Documents"
      | "Archives"
      | "Hidden"
      | "Writable"
      | "Symlinks";
  };
  sort?: {
    by?: "name" | "modifiedAt" | "size" | "type";
    direction?: "asc" | "desc";
  };
  limit?: number;
  cursor?: string | null;
  includeHidden?: boolean;
}
export interface TextFile {
  content: string;
  /** Opaque content AND metadata revision. Must be used when saving existing text. */
  revision: string;
  rootId: string;
  relativePath: string;
  size: number;
  encoding: "utf-8";
  mode: number | null;
}
export interface OperationResult {
  requestId: string;
  operation: string;
  safeMessage?: string | null;
}
export interface Session {
  accountId?: string;
  accountGeneration?: number;
  accountKind?: "linux" | "virtual";
  role?: "user" | "admin";
  authenticated: true;
  isAdmin: boolean;
  username: string;
  uid: number;
  gid: number;
  /** CSRF only, not the HttpOnly session secret. Keep in RAM. */
  csrfToken: string;
  roots: Array<{
    rootId: string;
    readOnly: boolean;
    isWebRoot: boolean;
    uploadsAllowed: boolean;
    hiddenFilesAllowed?: boolean;
    usedBytes: number | null;
    quotaBytes: number | null;
  }>;
}
export interface Quota {
  limitBytes: number | null;
  usedBytes: number | null;
  usagePercent: number | null;
  warningPercent: number;
  measuredAt: number | string | null;
  stale: boolean;
  measurement: "logical_bytes" | "allocated_bytes";
  source: "helper";
}
export interface QuotaWarning {
  active: boolean;
  status: "known" | "unknown";
  quota: Quota | null;
}
export interface ClientState {
  connection:
    | "disconnected"
    | "connecting"
    | "negotiating"
    | "auth_ready"
    | "authenticated"
    | "draining";
  stale: boolean;
  currentDirectory: string | null;
  rootId: string | null;
  quotaWarning: QuotaWarning;
}
export interface ServerInfo {
  sdkVersion: string;
  protocolVersion: number;
  limits: Readonly<Record<string, number>>;
  features: Readonly<Record<string, boolean>>;
  authentication: Readonly<{
    webPassword: boolean;
    passwordChange: boolean;
    selfRevoke: boolean;
    accountManagement: boolean;
    accountPagination: boolean;
    auditHistory: boolean;
    linuxApproval: boolean;
    policyManagement: boolean;
    virtualAccounts: boolean;
    virtualDelete: boolean;
    enrollment: boolean;
    selfRegistration: boolean;
  }>;
  authenticated: boolean;
}
/** Public account metadata. Password hashes and grant nonces are never returned. */
export interface AuditQuery {
  limit?: number;
  cursor?: string | null;
  action?: string;
  /** Correlates OperationResult.requestId with its protected record. */ requestId?: string;
  status?: "pending" | "success" | "failed" | "unknown";
  rootId?: string;
  /** Case-sensitive substring of source or target path. */ path?: string;
  /** Inclusive Unix epoch seconds. */ from?: number;
  to?: number;
}
export interface AuditRecord {
  version: 1;
  id: string;
  requestId: string;
  accountId: string;
  generation: number;
  /** Unix epoch seconds. */ createdAt: number;
  updatedAt: number;
  action: string;
  rootId: string;
  sourcePath: string;
  targetPath: string | null;
  requestedMode: number | null;
  status: "pending" | "success" | "failed" | "unknown";
  errorCode: string | null;
  bytesRead: number;
  bytesWritten: number;
}
export interface AuditPage {
  items: readonly AuditRecord[];
  nextCursor: string | null;
  hasMore: boolean;
}
export interface AccountMetadata {
  id: string;
  username: string;
  uid: number;
  gid: number;
  home: string;
  kind: "linux" | "virtual";
  role: "user" | "admin";
  ownerAdminId: string | null;
  status: "active" | "suspended" | "revoked" | "deleted";
  loginEnabled: boolean;
  generation: number;
  passwordEnrolled: boolean;
  quotaBytes: number | null;
  undoHours: number | null;
  createdAt: number;
  updatedAt: number;
  deletedAt: number | null;
  denyEnrollment: boolean;
}
/** Transient reactions; use observeState/observeQuotaWarning for persistent conditions. */
export interface ClientEvents {
  "state.changed": ClientState;
  "connection.changed": { status: ClientState["connection"] };
  "server.shutdown": { deadlineSeconds: number };
  "session.revoked": Record<string, never>;
  "files.changed": { operation: string; rootId?: string; path?: string };
  "accounts.changed": {
    operation: "create" | "quota" | "delete";
    accountId: string;
    kind: "virtual";
  };
  "policy.changed": { accountId: string; generation: number };
  "quota.warning.changed": QuotaWarning;
  "quota.warning.entered": Quota;
  "quota.warning.cleared": Quota;
}
export interface AuthAPI {
  /** Same established WebSocket; no HTTP password submission. @throws INVALID_CREDENTIALS, RATE_LIMITED */
  login(options: { username: string; password: string }): Promise<Session>;
  getSession(): Promise<Session | null>;
  logout(): Promise<void>;
  /** Linux accounts only; virtual accounts require their administrator. */
  revokeSelf(): Promise<void>;
  /** PAM verifies only this Linux account. Continue on the same WebSocket within 300s.
   * No proof/token is exposed or persisted in browser storage.
   * @throws FEATURE_DISABLED, INVALID_CREDENTIALS, RATE_LIMITED, ALREADY_AUTHENTICATED
   */
  beginEnrollment(options: {
    username: string;
    linuxPassword: string;
  }): Promise<{ enrollmentRequired: true; expiresAt: number }>;
  /** Set a web password (12+ Unicode characters, <=1024 UTF-8 bytes, no NUL).
   * Uses the pending server-side proof, binds an HttpOnly session and returns your account.
   * @throws ENROLLMENT_REQUIRED, INVALID_ARGUMENT, INVALID_CREDENTIALS, CONNECTION_CLOSED
   */
  completeEnrollment(options: {
    webPassword: string;
  }): Promise<Readonly<Session>>;
  /** Change the web password, revoke every current web session and disconnect this client.
   * Does not change the Linux password. Requires passwordChange capability.
   * @throws INVALID_ARGUMENT, INVALID_CREDENTIALS, RATE_LIMITED, FEATURE_DISABLED
   */
  changePassword(options: {
    currentPassword: string;
    newPassword: string;
  }): Promise<void>;
}
export interface FilesAPI {
  /** @example await api.files.list({filter:{extensions:['pdf']},limit:50}); */
  list(options?: ListOptions): Promise<DirectoryPage>;
  stat(
    options?: FileOptions,
  ): Promise<{ rootId: string; relativePath: string; entry: FileEntry }>;
  readText(options?: FileOptions & { maxBytes?: number }): Promise<TextFile>;
  /** Existing file save requires its readText revision; 'missing' is explicit create-only. */
  writeText(
    options: FileOptions & { content: string; expectedRevision: string },
  ): Promise<OperationResult>;
  /** Create only: an existing name is never silently overwritten. */
  createText(
    options: FileOptions & { content: string },
  ): Promise<OperationResult>;
  createFolder(options: FileOptions): Promise<OperationResult>;
  rename(options: FileOptions & { newName: string }): Promise<OperationResult>;
  move(
    options: RequestOptions & { source: FileRef; destination: FileRef },
  ): Promise<OperationResult>;
  copy(
    options: RequestOptions & { source: FileRef; destination: FileRef },
  ): Promise<OperationResult>;
  chmod(options: FileOptions & { mode: number }): Promise<OperationResult>;
  search(
    options: FileOptions & {
      query: string;
      extension?: string;
      limit?: number;
      includeHidden?: boolean;
    },
  ): Promise<{
    matches: Array<{ relativePath: string; entry: FileEntry }>;
    truncated: boolean;
  }>;
  upload(
    options: RequestOptions & {
      rootId?: string;
      directory?: string;
      file: File;
      onProgress?: (progress: {
        transferredBytes: number;
        totalBytes: number;
      }) => void;
    },
  ): Promise<OperationResult>;
  /** Streaming Response; UI chooses how to save. Promise resolution is not proof of user disk write. */
  download(options: FileOptions): Promise<Response>;
  /** Native browser download/media URL; no secret/token in query. Authorization occurs when requested. */
  getDownloadUrl(options: Omit<FileOptions, "signal">): string;
  /** Same-origin preview URL; backend checks size/type/access policy. */
  getPreviewUrl(options: Omit<FileOptions, "signal">): string;
  validatePath(
    path: string,
    options?: { rootId?: string },
  ): Promise<{ valid: true; path: string }>;
  /** Validates on helper, then updates optional SDK state. */
  goTo(path: string, options?: { rootId?: string }): Promise<ClientState>;
  /** Last verified location; state.stale can be true. Does not perform I/O. */
  getCurrentDirectory(): string;
  getState(): ClientState;
  refreshState(): Promise<ClientState>;
  observeState(callback: (state: ClientState) => void): Unsubscribe;
  /** Immediate current condition + changes. SDK never renders a popup or writes DOM. */
  observeQuotaWarning(callback: (warning: QuotaWarning) => void): Unsubscribe;
  on<K extends keyof ClientEvents>(
    event: K,
    callback: (payload: ClientEvents[K]) => void,
  ): Unsubscribe;
  on(event: string, callback: (payload: unknown) => void): Unsubscribe;
  trash: {
    move(options: FileOptions): Promise<OperationResult>;
    list(
      options?: { rootId?: string } & RequestOptions,
    ): Promise<{ items: unknown[] }>;
    restore(
      options: { trashId: string; destination?: FileRef } & RequestOptions,
    ): Promise<OperationResult>;
    /** Permanent removal requires separately granted feature/policy. */
    deletePermanently(
      options: { rootId?: string; trashId: string } & RequestOptions,
    ): Promise<OperationResult>;
  };
  archives: {
    create(
      options: FileOptions & { sources: string[]; format?: "zip" | "tar.gz" },
    ): Promise<OperationResult>;
    extract(
      options: FileOptions & { destination: string },
    ): Promise<OperationResult>;
  };
  quota: { get(): Promise<Quota> };
  audit: {
    /** Current helper policy filters own history; cursor is not an authority token. */
    list(options?: AuditQuery): Promise<Readonly<AuditPage>>;
  };
  undo: {
    list(options?: {
      limit?: number;
      cursor?: string | null;
    }): Promise<unknown>;
    apply(options: {
      operationId: string;
      expectedState: string;
    }): Promise<OperationResult>;
  };
  policy: PersonalPolicyAPI;
}
/** Discover denial also denies direct access and looks like a missing path. */
export type PolicyAccess =
  | "discover"
  | "read"
  | "write"
  | "chmod"
  | "delete"
  | "move";
export interface PolicyRule {
  path: string;
  rootId?: string | null;
  recursive?: boolean;
  deny: readonly PolicyAccess[];
  /** Revision-bound index token returned by get(); never a standalone stable identity. */
  ruleId?: string;
}
export interface PolicyView {
  account: Readonly<AccountMetadata>;
  revision: string;
  rules: Readonly<{
    rootRules: readonly PolicyRule[];
    adminRules: readonly PolicyRule[];
    personalRules: readonly PolicyRule[];
  }>;
  denySystemComponents: readonly string[];
}
export interface PersonalPolicyAPI {
  get(): Promise<PolicyView>;
  /** Replaces only the personal layer. Success invalidates all old sessions,
   * including this client. Root/admin restrictions remain. */
  set(options: {
    expectedGeneration: number;
    rules: readonly PolicyRule[];
  }): Promise<AccountMetadata>;
  /** Reads the current policy then commits once with generation compare-and-swap.
   * No automatic retry on a concurrent edit. */
  addRule(
    options: PolicyRule & { expectedRevision?: string },
  ): Promise<AccountMetadata>;
  removeRule(options: {
    ruleId: string;
    expectedRevision: string;
  }): Promise<AccountMetadata>;
}
export interface AdminPolicyAPI {
  get(options: {
    accountId: string;
    kind?: "linux" | "virtual";
  }): Promise<PolicyView>;
  set(options: {
    accountId: string;
    kind?: "linux" | "virtual";
    expectedGeneration: number;
    rules: readonly PolicyRule[];
  }): Promise<AccountMetadata>;
  addRule(
    options: PolicyRule & {
      accountId: string;
      kind?: "linux" | "virtual";
      expectedRevision?: string;
    },
  ): Promise<AccountMetadata>;
  removeRule(options: {
    accountId: string;
    kind?: "linux" | "virtual";
    ruleId: string;
    expectedRevision: string;
  }): Promise<AccountMetadata>;
}
export interface UserCandidate {
  username: string;
  uid: number;
  gid: number;
  registered?: boolean;
}
export interface UserCandidates {
  users: readonly UserCandidate[];
  truncated: boolean;
}
export type AccountVersion =
  | { expectedGeneration: number; expectedRevision?: never }
  | { expectedRevision: string; expectedGeneration?: never };
export interface AdminBudget {
  limitBytes: number;
  reservedBytes: number;
  remainingBytes: number;
  /** Available after own helper-measured usage; older backends may omit it. */
  availableBytes?: number;
  usedBytes?: number;
  enforcement: "application" | "user_quota";
}
export interface AccountQuery {
  limit?: number;
  /** Server continuation username; not an authorization token. Scope is checked again. */
  cursor?: string | null;
  filter?: string;
  status?: "active" | "suspended" | "revoked" | "deleted";
  enrolled?: boolean;
}
export interface AccountPage {
  items: readonly AccountMetadata[];
  nextCursor: string | null;
  hasMore: boolean;
}
export interface AdminAPI {
  settings: {
    get(): Promise<unknown>;
    patch(options: {
      changes: Record<string, unknown>;
      persist?: boolean;
    }): Promise<unknown>;
    /** Runtime/legacy configuration mode; root-managed fields remain protected. */
    inspect(): Promise<{
      config: Record<string, unknown>;
      mode: "split" | "legacy";
    }>;
    reload(): Promise<unknown>;
  };
  systemUsers: {
    list(): Promise<UserCandidates>;
    /** Registered system web accounts, distinct from eligible NSS login candidates. */
    accounts(): Promise<readonly AccountMetadata[]>;
    accountsPage(options?: AccountQuery): Promise<AccountPage>;
    getPolicy(options: { accountId: string }): Promise<PolicyView>;
    setPolicy(options: {
      accountId: string;
      expectedGeneration: number;
      rules: readonly PolicyRule[];
    }): Promise<AccountMetadata>;
    /** Approve an existing eligible Linux identity for first PAM enrollment. Does not
     * create an OS user, choose UID/home/role, set a password or modify root config.
     * Existing accounts conflict; use allow/deny instead. Root deny remains superior.
     * @throws FEATURE_DISABLED, INVALID_ARGUMENT, PERMISSION_DENIED, CONFLICT
     */
    approve(options: { username: string }): Promise<AccountMetadata>;
    allow(options: { accountId: string }): Promise<AccountMetadata>;
    deny(options: { accountId: string }): Promise<AccountMetadata>;
  };
  virtualUsers: {
    getPolicy(options: { accountId: string }): Promise<PolicyView>;
    setPolicy(options: {
      accountId: string;
      expectedGeneration: number;
      rules: readonly PolicyRule[];
    }): Promise<AccountMetadata>;
    /** Compatibility array result; supplied filter/limit/cursor uses the bounded page backend. */
    list(options?: AccountQuery): Promise<readonly AccountMetadata[]>;
    listPage(options?: AccountQuery): Promise<AccountPage>;
    /** Creates a locked/nologin OS identity and initial web password atomically in metadata.
     * A repeated idempotency key returns the same account and NEVER resets its password.
     * On a network failure error.params.idempotencyKey can be reused. No credentials are stored by SDK. */
    create(options: {
      name: string;
      quotaBytes: number;
      password: string;
      idempotencyKey?: string;
    }): Promise<AccountMetadata>;
    setQuota(
      options: { accountId: string; quotaBytes: number } & AccountVersion,
    ): Promise<AccountMetadata>;
    disable(options: { accountId: string }): Promise<AccountMetadata>;
    enable(options: { accountId: string }): Promise<AccountMetadata>;
    /** Root must explicitly enable deletion. Access ends immediately; data stays for Root retention (default36h). */
    delete(
      options: { accountId: string } & AccountVersion,
    ): Promise<AccountMetadata>;
  };
  policy: AdminPolicyAPI;
  quota: { getBudget(): Promise<AdminBudget> };
}
export interface ClientOptions {
  stateMode?: "managed" | "client";
  storage?: Storage | null;
  timeoutMs?: number;
  warningPercent?: number;
  /** Poll while a quota observer exists; 0 disables automatic polling. */
  quotaPollMs?: number;
}
export class TealDriveClient {
  constructor(options?: ClientOptions);
  readonly auth: AuthAPI;
  readonly files: FilesAPI;
  readonly admin: AdminAPI;
  handshake(): Promise<ServerInfo>;
  getInfo(): ServerInfo;
  disconnect(): void;
  getState(): ClientState;
  observeState(callback: (state: ClientState) => void): Unsubscribe;
  on<K extends keyof ClientEvents>(
    event: K,
    callback: (payload: ClientEvents[K]) => void,
  ): Unsubscribe;
  on(event: string, callback: (payload: unknown) => void): Unsubscribe;
}
export function createClient(options?: ClientOptions): TealDriveClient;
