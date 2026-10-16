import type { AuditEntry, Session, VirtualUser } from './types.ts';
import { t } from '../i18n/index.ts';

// Pure translations from SDK (camelCased server) values to the SPA's view types.

/** Parent directory of a root-relative path, as an absolute SPA path ("a/b/c" -> "/a/b"). */
export function parentOfRelative(rel: string): string {
  const clean = rel.replace(/^\/+/, '').replace(/\/+$/, '');
  return clean.includes('/') ? `/${clean.slice(0, clean.lastIndexOf('/'))}` : '/';
}

const RECENT_TARGET_ACTIONS = new Set(['rename', 'move', 'copy', 'restore', 'archive', 'extract']);
const RECENT_SOURCE_ACTIONS = new Set(['write', 'upload']);

/** The file a journal record left behind, if the record changed file content or location. */
export function recentPath(record: any): string | null {
  const action = String(record?.action ?? '');
  const path = RECENT_TARGET_ACTIONS.has(action)
    ? record.targetPath
    : RECENT_SOURCE_ACTIONS.has(action)
    ? record.sourcePath
    : null;
  return typeof path === 'string' && path !== '' ? path.replace(/^\/+/, '') : null;
}

const AUDIT_LABELS: Record<string, string> = {
  write: 'Edited',
  upload: 'Uploaded',
  createFolder: 'Created folder',
  rename: 'Renamed',
  move: 'Moved',
  copy: 'Copied',
  trash: 'Moved to Trash',
  restore: 'Restored from Trash',
  delete: 'Deleted permanently',
  chmod: 'Changed permissions',
  archive: 'Created archive',
  extract: 'Extracted archive',
};

export function toAuditEntry(record: any, candidate?: any): AuditEntry {
  const source = `/${String(record.sourcePath ?? '').replace(/^\/+/, '')}`;
  const target = record.targetPath ? `/${String(record.targetPath).replace(/^\/+/, '')}` : undefined;
  const label = t(AUDIT_LABELS[record.action] ?? record.action);
  let description = `${label} ${source}`;
  if (target && record.action !== 'trash') description += ` → ${target}`;
  if (record.action === 'chmod' && typeof record.oldMode === 'number') {
    const newMode = record.newMode ?? record.requestedMode;
    description += ` (${record.oldMode.toString(8)} → ${typeof newMode === 'number' ? newMode.toString(8) : '?'})`;
  }
  if (record.status && record.status !== 'success') description += ` · ${record.status}`;
  return {
    id: record.id,
    timestamp: new Date(Number(record.createdAt) * 1000).toISOString(),
    createdAt: Number(record.createdAt),
    action: record.action,
    targetPath: target ?? source,
    description,
    undoable: Boolean(candidate?.available && candidate?.expectedState),
    expectedState: candidate?.expectedState ?? undefined,
    undoUnavailableReason: candidate && !candidate.available ? candidate.unavailableReason ?? undefined : undefined,
    undoStatus: record.undoStatus ?? undefined,
  };
}

export function toVirtualUser(u: any): VirtualUser {
  const created = Number(u?.createdAt);
  return {
    id: String(u?.id ?? u?.accountId ?? ''),
    username: u?.username ?? u?.name ?? '',
    uid: Number(u?.uid) || 0,
    generation: Number(u?.generation) || 0,
    quotaBytes: Number(u?.quotaBytes ?? 0),
    usedBytes: typeof u?.usedBytes === 'number' ? u.usedBytes : undefined,
    enabled: u?.status === 'active',
    status: u?.status ?? 'unknown',
    passwordChangeRequired: Boolean(u?.passwordChangeRequired),
    createdAt: Number.isFinite(created) && created > 0 ? new Date(created * 1000).toISOString() : '',
  };
}

export function toSession(session: any): Session {
  const isAdmin = Boolean(session.isAdmin || session.role === 'admin');
  return {
    username: session.username,
    uid: session.uid,
    role: isAdmin ? 'admin' : 'user',
    isAdmin,
    passwordChangeRequired: Boolean(session.passwordChangeRequired),
  };
}

export type PolicyAccess = 'discover' | 'read' | 'write' | 'chmod' | 'delete' | 'move';
export const POLICY_ACCESS: PolicyAccess[] = ['discover', 'read', 'write', 'chmod', 'delete', 'move'];

export interface PolicyRule {
  path: string;
  recursive: boolean;
  deny: PolicyAccess[];
  rootId?: string;
}

export interface PolicyTarget {
  kind: 'linux' | 'virtual';
  accountId: string;
  username: string;
}

export interface PolicyView {
  username: string;
  generation: number;
  /** Rules this editor owns (admin layer for a target, personal for oneself). */
  editable: PolicyRule[];
  /** Rules from other layers, shown read-only. */
  fixed: Array<{ layer: string; rule: PolicyRule }>;
  denySystemComponents: string[];
}

export interface UndoPreferences {
  hours: number | null;
  effectiveHours: number;
  maximumHours: number;
  revision: number;
}

function toRule(r: any): PolicyRule {
  return {
    path: `/${String(r?.path ?? '').replace(/^\/+/, '')}`,
    recursive: Boolean(r?.recursive),
    deny: (Array.isArray(r?.deny) ? r.deny : []).filter((d: string) => POLICY_ACCESS.includes(d as PolicyAccess)),
    ...(r?.rootId ? { rootId: r.rootId } : {}),
  };
}

/** `layer` is the layer the caller may edit: "admin" for another account, "personal" for oneself. */
export function toPolicyView(view: any, layer: 'admin' | 'personal'): PolicyView {
  const rules = view?.rules ?? {};
  const fixed: PolicyView['fixed'] = [];
  for (const source of ['root', 'admin', 'personal']) {
    if (source === layer) continue;
    for (const r of rules[`${source}Rules`] ?? []) fixed.push({ layer: source, rule: toRule(r) });
  }
  return {
    username: view?.account?.username ?? '',
    generation: Number(view?.account?.generation) || 0,
    editable: (rules[`${layer}Rules`] ?? []).map(toRule),
    fixed,
    denySystemComponents: Array.isArray(view?.denySystemComponents) ? view.denySystemComponents : [],
  };
}
