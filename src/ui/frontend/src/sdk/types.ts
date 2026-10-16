export interface FileEntry {
  id: string;
  name: string;
  path: string;
  isDirectory: boolean;
  size: number;
  modified: string;
  mode: string | number; // e.g. "0755", "0644", 0o755, 493
  owner: string;
  group: string;
  mimeType: string;
  starred?: boolean;
  revision: number;
  content?: string;
  trashedAt?: string;
  originalPath?: string;
}

export interface Session {
  username: string;
  uid?: number;
  role: 'admin' | 'user' | 'auditor';
  isAdmin: boolean;
  email?: string;
  avatarUrl?: string;
}

export interface ClientState {
  connected: boolean;
  latency: number;
  encryption: 'noise' | 'tls' | 'none';
  syncStatus: 'synced' | 'syncing' | 'error';
  activeTransfersCount: number;
}

export interface StorageQuota {
  usedBytes: number;
  limitBytes: number;
  percentage: number;
  warningLevel: 'normal' | 'warning' | 'critical';
}

export interface AuditEntry {
  id: string;
  timestamp: string;
  action: 'create' | 'upload' | 'rename' | 'move' | 'delete' | 'chmod' | 'edit';
  targetPath: string;
  description: string;
  previousState?: Partial<FileEntry>;
  newState?: Partial<FileEntry>;
  undoable: boolean;
}

export interface VirtualUser {
  id: string;
  username: string;
  uid: number;
  quotaBytes: number;
  usedBytes: number;
  enabled: boolean;
  createdAt: string;
  email: string;
}

export interface SystemUser {
  username: string;
  uid: number;
  shell: string;
  homeDir: string;
  approvedForWeb: boolean;
  lastLogin?: string;
}

export interface StorageBudget {
  totalDiskBytes: number;
  virtualUsersAllocatedBytes: number;
  systemReserveBytes: number;
  freePhysicalBytes: number;
}

export interface ServerSettings {
  serverName: string;
  version: string;
  listenAddr: string;
  wsEncryption: string;
  tlsCertExpiry: string;
  loginMaxFailures: number;
  maxUploadSizeMb: number;
  autoTrashPurgeDays: number;
  noiseHandshakePattern: string;
}

export interface UploadProgress {
  fileId: string;
  fileName: string;
  directory: string;
  size: number;
  uploadedBytes: number;
  progress: number; // 0 - 100
  speedBytesPerSec: number;
  status: 'uploading' | 'completed' | 'error' | 'aborted';
  error?: string;
  abortController?: AbortController;
}
