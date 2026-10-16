import test from 'node:test';
import assert from 'node:assert/strict';
import { parentOfRelative, recentPath, toAuditEntry, toSession, toVirtualUser } from './mapping.ts';
import { copyName, joinPath, parentPath } from '../utils/paths.ts';

test('search matches resolve to their real parent folder', () => {
  assert.equal(parentOfRelative('docs/notes/a.md'), '/docs/notes');
  assert.equal(parentOfRelative('a.md'), '/');
  assert.equal(parentOfRelative('/x/y/'), '/x');
});

test('recent files follow the location an operation left behind', () => {
  assert.equal(recentPath({ action: 'write', sourcePath: 'a/b.txt' }), 'a/b.txt');
  assert.equal(recentPath({ action: 'upload', sourcePath: 'up.bin' }), 'up.bin');
  assert.equal(recentPath({ action: 'rename', sourcePath: 'old.txt', targetPath: 'new.txt' }), 'new.txt');
  assert.equal(recentPath({ action: 'move', sourcePath: 'a.txt', targetPath: 'dir/a.txt' }), 'dir/a.txt');
  assert.equal(recentPath({ action: 'trash', sourcePath: 'gone.txt', targetPath: '.trash/x' }), null);
  assert.equal(recentPath({ action: 'chmod', sourcePath: 'a.sh' }), null);
  assert.equal(recentPath({ action: 'download', sourcePath: 'a.txt' }), null);
});

test('audit entries carry undo state only when the server offers it', () => {
  const record = {
    id: 'op-1',
    action: 'rename',
    sourcePath: 'old.txt',
    targetPath: 'new.txt',
    createdAt: 1_700_000_000,
    status: 'success',
  };
  const plain = toAuditEntry(record);
  assert.equal(plain.undoable, false);
  assert.equal(plain.description, 'Renamed /old.txt → /new.txt');
  assert.equal(plain.targetPath, '/new.txt');
  assert.equal(plain.timestamp, new Date(1_700_000_000_000).toISOString());

  const offered = toAuditEntry(record, { available: true, expectedState: 'f'.repeat(64) });
  assert.equal(offered.undoable, true);
  assert.equal(offered.expectedState, 'f'.repeat(64));

  const expired = toAuditEntry(record, { available: false, unavailableReason: 'expired', expectedState: 'f'.repeat(64) });
  assert.equal(expired.undoable, false);
  assert.equal(expired.undoUnavailableReason, 'expired');
});

test('chmod audit entries show the octal change', () => {
  const entry = toAuditEntry({ id: 'x', action: 'chmod', sourcePath: 'run.sh', oldMode: 0o644, newMode: 0o755, createdAt: 1, status: 'success' });
  assert.equal(entry.description, 'Changed permissions /run.sh (644 → 755)');
});

test('virtual users keep the id and generation needed for mutations', () => {
  const user = toVirtualUser({
    id: 'acc-1',
    username: 'alex',
    uid: 200001,
    generation: 7,
    quotaBytes: 1024,
    status: 'suspended',
    passwordChangeRequired: true,
    createdAt: 1_700_000_000,
  });
  assert.equal(user.id, 'acc-1');
  assert.equal(user.generation, 7);
  assert.equal(user.enabled, false);
  assert.equal(user.passwordChangeRequired, true);
  assert.equal(user.usedBytes, undefined);
  assert.equal(toVirtualUser({ id: 'a', status: 'active' }).enabled, true);
  assert.equal(toVirtualUser({ id: 'a' }).createdAt, '');
});

test('sessions expose a pending password change', () => {
  assert.equal(toSession({ username: 'a', passwordChangeRequired: true }).passwordChangeRequired, true);
  assert.equal(toSession({ username: 'a', role: 'admin' }).isAdmin, true);
  assert.equal(toSession({ username: 'a' }).passwordChangeRequired, false);
});

test('move and copy targets are full paths with collision-free copy names', () => {
  assert.equal(joinPath('/', 'a.txt'), '/a.txt');
  assert.equal(joinPath('/docs', 'a.txt'), '/docs/a.txt');
  assert.equal(parentPath('/docs/a.txt'), '/docs');
  assert.equal(parentPath('/a.txt'), '/');
  assert.equal(copyName('a.txt', new Set(['a.txt'])), 'a copy.txt');
  assert.equal(copyName('a.txt', new Set(['a.txt', 'a copy.txt'])), 'a copy 2.txt');
  assert.equal(copyName('.env', new Set(['.env'])), '.env copy');
  assert.equal(copyName('folder', new Set()), 'folder copy');
});
