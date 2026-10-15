import test from 'node:test';
import assert from 'node:assert/strict';
import { octalToMatrix, matrixToOctal, modeToSymbolic, checkPermissionSecurity } from './permissions.ts';

test('octalToMatrix converts 0755 correctly', () => {
  const m = octalToMatrix('0755');
  assert.deepEqual(m.owner, { read: true, write: true, execute: true });
  assert.deepEqual(m.group, { read: true, write: false, execute: true });
  assert.deepEqual(m.others, { read: true, write: false, execute: true });
});

test('octalToMatrix handles numeric POSIX integers (e.g. 493 or 0o755)', () => {
  const m = octalToMatrix(493); // 0o755 in decimal is 493
  assert.deepEqual(m.owner, { read: true, write: true, execute: true });
  assert.deepEqual(m.group, { read: true, write: false, execute: true });
  assert.deepEqual(m.others, { read: true, write: false, execute: true });

  const m644 = octalToMatrix(0o644);
  assert.deepEqual(m644.owner, { read: true, write: true, execute: false });
  assert.equal(modeToSymbolic(0o755, true), 'drwxr-xr-x');
});

test('matrixToOctal converts back to 0755', () => {
  const matrix = {
    owner: { read: true, write: true, execute: true },
    group: { read: true, write: false, execute: true },
    others: { read: true, write: false, execute: true },
  };
  assert.equal(matrixToOctal(matrix), '0755');
});

test('octalToMatrix and matrixToOctal roundtrip 0644', () => {
  const m = octalToMatrix('0644');
  assert.deepEqual(m.owner, { read: true, write: true, execute: false });
  assert.deepEqual(m.group, { read: true, write: false, execute: false });
  assert.deepEqual(m.others, { read: true, write: false, execute: false });
  assert.equal(matrixToOctal(m), '0644');
});

test('modeToSymbolic produces correct strings for file and directory', () => {
  assert.equal(modeToSymbolic('0755', true), 'drwxr-xr-x');
  assert.equal(modeToSymbolic('0644', false), '-rw-r--r--');
  assert.equal(modeToSymbolic('0700', true), 'drwx------');
});

test('checkPermissionSecurity flags world-writable modes', () => {
  const safeMatrix = octalToMatrix('0755');
  assert.equal(checkPermissionSecurity(safeMatrix).isSafe, true);

  const worldWritableMatrix = octalToMatrix('0777');
  const sec = checkPermissionSecurity(worldWritableMatrix);
  assert.equal(sec.isSafe, false);
  assert.match(sec.warning || '', /World-writable/);
});
